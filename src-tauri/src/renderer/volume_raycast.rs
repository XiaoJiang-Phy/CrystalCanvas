// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::volumetric::ScalarFieldView;
use wgpu::util::DeviceExt;

/// Above this proxy condition number, converting Cartesian rays into fractional
/// coordinates loses too much precision for a reliable volume intersection.
const MAX_FIELD_LATTICE_CONDITION: f64 = 1.0e12;

fn finite_f32(value: f64) -> Result<f32, ()> {
    let converted = value as f32;
    converted.is_finite().then_some(converted).ok_or(())
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VolumeRaycastUniforms {
    pub lattice_a: [f32; 4],
    pub lattice_b: [f32; 4],
    pub lattice_c: [f32; 4],
    pub inv_lattice_a: [f32; 4],
    pub inv_lattice_b: [f32; 4],
    pub inv_lattice_c: [f32; 4],
    pub eye_pos: [f32; 4],
    pub origin: [f32; 4],
    pub grid_dims: [u32; 4], // x, y, z, pad
    pub transfer_range: [f32; 2],
    pub opacity_scale: f32,
    pub step_size: f32,
    pub max_steps: u32,
    pub colormap_mode: u32,
    pub is_orthographic: u32,
    pub use_signed_mapping: u32,
    pub unlit: u32,
    pub _pad_after_unlit: [u32; 3],
    pub camera_forward: [f32; 4],
    pub volume_clip_threshold: f32,
    pub volume_density_cutoff: f32,
    pub _pad1: [f32; 2],
    pub clip_planes: [[f32; 4]; crate::renderer::field_scene::MAX_FIELD_CLIP_PLANES],
    pub clip_keep_positive: [[u32; 4]; 2],
    pub transfer_negative_positions:
        [[f32; 4]; crate::renderer::field_scene::MAX_FIELD_TRANSFER_POINTS],
    pub transfer_positive_positions:
        [[f32; 4]; crate::renderer::field_scene::MAX_FIELD_TRANSFER_POINTS],
    pub transfer_negative_colors:
        [[f32; 4]; crate::renderer::field_scene::MAX_FIELD_TRANSFER_POINTS],
    pub transfer_positive_colors:
        [[f32; 4]; crate::renderer::field_scene::MAX_FIELD_TRANSFER_POINTS],
    pub transfer_point_counts: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VolumeVertex {
    pub position: [f32; 3],
}

impl VolumeVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

pub struct VolumeRaycastPipeline {
    render_pipeline: wgpu::RenderPipeline,
    render_bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    uniforms: VolumeRaycastUniforms,
    scalar_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl VolumeRaycastPipeline {
    // VolumeRaycastPipeline::new rejects required_steps above the representable budget.
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        volumetric_data: &impl ScalarFieldView,
        depth_view: &wgpu::TextureView,
        clip_planes: &[crate::renderer::field_scene::FieldClipPlane],
    ) -> Result<Self, ()> {
        if clip_planes.len() > crate::renderer::field_scene::MAX_FIELD_CLIP_PLANES {
            return Err(());
        }
        let mapping = volumetric_data.grid_mapping();
        let lattice = mapping.domain_lattice_col_major();
        let origin = mapping.origin_angstrom;
        let data = volumetric_data.scalar_data();
        let mat = glam::DMat3::from_cols_array(&lattice);
        let cross_bc = mat.y_axis.cross(mat.z_axis);
        let cross_ca = mat.z_axis.cross(mat.x_axis);
        let cross_ab = mat.x_axis.cross(mat.y_axis);
        let determinant = mat.x_axis.dot(cross_bc);
        let matrix_norm = (mat.x_axis.length_squared()
            + mat.y_axis.length_squared()
            + mat.z_axis.length_squared())
        .sqrt();
        let inverse_norm =
            (cross_bc.length_squared() + cross_ca.length_squared() + cross_ab.length_squared())
                .sqrt()
                / determinant.abs();
        let condition = matrix_norm * inverse_norm;
        if !determinant.is_finite()
            || determinant.abs() <= f64::MIN_POSITIVE
            || !condition.is_finite()
            || condition > MAX_FIELD_LATTICE_CONDITION
        {
            log::warn!(
                "Rejected volume raycast lattice: determinant={determinant:e}, condition={condition:e}"
            );
            return Err(());
        }
        // Columns of M^{-T}; the shader consumes this form for Cartesian-to-
        // fractional coordinates.  The cofactor form avoids an unchecked inverse.
        let inv_mat_t = glam::DMat3::from_cols(
            cross_bc / determinant,
            cross_ca / determinant,
            cross_ab / determinant,
        );

        let grid = mapping.dimensions;
        let periodic_mask =
            mapping
                .axis_sampling
                .iter()
                .enumerate()
                .fold(0_u32, |mask, (axis, sampling)| {
                    mask | (u32::from(matches!(
                        sampling,
                        crate::volumetric::AxisSampling::PeriodicExclusive
                    )) << axis)
                });
        let (t_min, t_max) = volumetric_data.scalar_range();

        // Nyquist-compliant step size: half the minimum voxel spacing
        // $\Delta t = 0.5 \cdot \min(|\mathbf{a}|/N_x,\, |\mathbf{b}|/N_y,\, |\mathbf{c}|/N_z)$
        let h_a = mat.x_axis.length() / grid[0].max(1) as f64;
        let h_b = mat.y_axis.length() / grid[1].max(1) as f64;
        let h_c = mat.z_axis.length() / grid[2].max(1) as f64;
        let step_size = finite_f32((h_a.min(h_b).min(h_c) * 0.5).max(1e-4))?;

        // max_steps covers the full body diagonal with headroom
        let diagonal = mat.x_axis.length() + mat.y_axis.length() + mat.z_axis.length();
        let required_steps = ((diagonal / f64::from(step_size)) * 1.5).ceil();
        if !required_steps.is_finite() || required_steps > 2048.0 {
            log::warn!("Rejected volume raycast requiring {required_steps} steps; maximum is 2048");
            return Err(());
        }
        let max_steps = (required_steps as u32).max(256);

        log::info!(
            "Volume raycast: voxel h=({:.4}, {:.4}, {:.4}) Å, step_size={:.4} Å, max_steps={}",
            h_a,
            h_b,
            h_c,
            step_size,
            max_steps
        );

        let lattice_a = [
            finite_f32(mat.x_axis.x)?,
            finite_f32(mat.x_axis.y)?,
            finite_f32(mat.x_axis.z)?,
            0.0,
        ];
        let lattice_b = [
            finite_f32(mat.y_axis.x)?,
            finite_f32(mat.y_axis.y)?,
            finite_f32(mat.y_axis.z)?,
            0.0,
        ];
        let lattice_c = [
            finite_f32(mat.z_axis.x)?,
            finite_f32(mat.z_axis.y)?,
            finite_f32(mat.z_axis.z)?,
            0.0,
        ];
        let inv_lattice_a = [
            finite_f32(inv_mat_t.x_axis.x)?,
            finite_f32(inv_mat_t.x_axis.y)?,
            finite_f32(inv_mat_t.x_axis.z)?,
            0.0,
        ];
        let inv_lattice_b = [
            finite_f32(inv_mat_t.y_axis.x)?,
            finite_f32(inv_mat_t.y_axis.y)?,
            finite_f32(inv_mat_t.y_axis.z)?,
            0.0,
        ];
        let inv_lattice_c = [
            finite_f32(inv_mat_t.z_axis.x)?,
            finite_f32(inv_mat_t.z_axis.y)?,
            finite_f32(inv_mat_t.z_axis.z)?,
            0.0,
        ];
        let uniform_origin = [
            finite_f32(origin[0])?,
            finite_f32(origin[1])?,
            finite_f32(origin[2])?,
            0.0,
        ];
        let mut uniform_clip_planes =
            [[0.0_f32; 4]; crate::renderer::field_scene::MAX_FIELD_CLIP_PLANES];
        let mut clip_keep_positive = [[0_u32; 4]; 2];
        for (index, plane) in clip_planes.iter().enumerate() {
            uniform_clip_planes[index] = [
                finite_f32(plane.normal[0])?,
                finite_f32(plane.normal[1])?,
                finite_f32(plane.normal[2])?,
                finite_f32(plane.signed_offset_angstrom)?,
            ];
            clip_keep_positive[index / 4][index % 4] = u32::from(plane.keep_positive);
        }
        clip_keep_positive[1][2] = u32::try_from(clip_planes.len()).map_err(|_| ())?;
        let uniforms = VolumeRaycastUniforms {
            lattice_a,
            lattice_b,
            lattice_c,
            inv_lattice_a,
            inv_lattice_b,
            inv_lattice_c,
            eye_pos: [0.0, 0.0, 0.0, 1.0],
            origin: uniform_origin,
            grid_dims: [
                grid[0] as u32,
                grid[1] as u32,
                grid[2] as u32,
                periodic_mask,
            ],
            transfer_range: [t_min, t_max],
            opacity_scale: 1.0,
            step_size,
            max_steps,
            colormap_mode: 0,
            is_orthographic: 1,
            use_signed_mapping: 0,
            unlit: 0,
            _pad_after_unlit: [0; 3],
            camera_forward: [0.0, 0.0, -1.0, 0.0],
            volume_clip_threshold: 0.0,
            volume_density_cutoff: 0.0,
            _pad1: [0.0; 2],
            clip_planes: uniform_clip_planes,
            clip_keep_positive,
            transfer_negative_positions: [[0.0; 4];
                crate::renderer::field_scene::MAX_FIELD_TRANSFER_POINTS],
            transfer_positive_positions: [[0.0; 4];
                crate::renderer::field_scene::MAX_FIELD_TRANSFER_POINTS],
            transfer_negative_colors: [[0.0; 4];
                crate::renderer::field_scene::MAX_FIELD_TRANSFER_POINTS],
            transfer_positive_colors: [[0.0; 4];
                crate::renderer::field_scene::MAX_FIELD_TRANSFER_POINTS],
            transfer_point_counts: [0; 4],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Volume Raycast Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let scalar_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Volume Raycast Scalar Buffer"),
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let vertices = [
            VolumeVertex {
                position: [0.0, 0.0, 0.0],
            },
            VolumeVertex {
                position: [1.0, 0.0, 0.0],
            },
            VolumeVertex {
                position: [1.0, 1.0, 0.0],
            },
            VolumeVertex {
                position: [0.0, 1.0, 0.0],
            },
            VolumeVertex {
                position: [0.0, 0.0, 1.0],
            },
            VolumeVertex {
                position: [1.0, 0.0, 1.0],
            },
            VolumeVertex {
                position: [1.0, 1.0, 1.0],
            },
            VolumeVertex {
                position: [0.0, 1.0, 1.0],
            },
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Volume Proxy Vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices: &[u16] = &[
            0, 1, 2, 2, 3, 0, // front
            1, 5, 6, 6, 2, 1, // right
            5, 4, 7, 7, 6, 5, // back
            4, 0, 3, 3, 7, 4, // left
            3, 2, 6, 6, 7, 3, // top
            4, 5, 1, 1, 0, 4, // bottom
        ];
        let index_count = indices.len() as u32;

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Volume Proxy Indices"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Volume Raycast Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let render_bind_group = Self::build_bind_group(
            device,
            &bind_group_layout,
            &uniform_buffer,
            &scalar_buffer,
            depth_view,
        );

        let render_pipeline = super::pipeline::create_volume_raycast_pipeline(
            device,
            surface_format,
            camera_bind_group_layout,
            &bind_group_layout,
            1,
        );

        Ok(Self {
            render_pipeline,
            render_bind_group,
            bind_group_layout,
            uniform_buffer,
            uniforms,
            scalar_buffer,
            vertex_buffer,
            index_buffer,
            index_count,
        })
    }

    fn build_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
        scalar_buffer: &wgpu::Buffer,
        depth_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Volume Raycast Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: scalar_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
            ],
        })
    }

    fn write_uniforms(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );
    }

    #[must_use]
    pub fn resident_bytes(&self) -> u64 {
        self.uniform_buffer
            .size()
            .saturating_add(self.scalar_buffer.size())
            .saturating_add(self.vertex_buffer.size())
            .saturating_add(self.index_buffer.size())
    }

    #[must_use]
    pub fn scalar_storage_bytes(&self) -> u64 {
        self.scalar_buffer.size()
    }

    /// Rebuild bind group when depth texture changes (e.g., window resize).
    pub fn update_depth_view(&mut self, device: &wgpu::Device, depth_view: &wgpu::TextureView) {
        self.render_bind_group = Self::build_bind_group(
            device,
            &self.bind_group_layout,
            &self.uniform_buffer,
            &self.scalar_buffer,
            depth_view,
        );
    }

    pub fn update_transfer_function(
        &mut self,
        queue: &wgpu::Queue,
        transfer_range: [f32; 2],
        opacity_scale: f32,
    ) {
        self.uniforms.transfer_range = transfer_range;
        self.uniforms.opacity_scale = opacity_scale;
        self.write_uniforms(queue);
    }

    pub fn update_camera(
        &mut self,
        queue: &wgpu::Queue,
        eye_pos: glam::Vec3,
        is_perspective: bool,
        forward: glam::Vec3,
    ) {
        self.uniforms.eye_pos = [eye_pos.x, eye_pos.y, eye_pos.z, 1.0];
        let is_ortho: u32 = if is_perspective { 0 } else { 1 };
        self.uniforms.is_orthographic = is_ortho;
        self.uniforms.camera_forward = [forward.x, forward.y, forward.z, 0.0];
        self.write_uniforms(queue);
    }

    pub fn update_explicit_transfer_function(
        &mut self,
        queue: &wgpu::Queue,
        transfer_function: &crate::renderer::field_scene::FieldTransferFunction,
    ) -> Result<(), ()> {
        transfer_function.validate().map_err(|_| ())?;
        let negative_count = transfer_function.negative_control_points.len();
        let positive_count = transfer_function.positive_control_points.len();
        for (index, point) in transfer_function.negative_control_points.iter().enumerate() {
            self.uniforms.transfer_negative_positions[index][0] = point.position;
            self.uniforms.transfer_negative_colors[index] = point.color_linear_rgba;
        }
        for (index, point) in transfer_function.positive_control_points.iter().enumerate() {
            self.uniforms.transfer_positive_positions[index][0] = point.position;
            self.uniforms.transfer_positive_colors[index] = point.color_linear_rgba;
        }
        self.uniforms.transfer_point_counts = [
            u32::try_from(negative_count).map_err(|_| ())?,
            u32::try_from(positive_count).map_err(|_| ())?,
            1,
            0,
        ];
        self.write_uniforms(queue);
        Ok(())
    }

    /// Set colormap mode index.
    pub fn set_colormap(&mut self, queue: &wgpu::Queue, mode: u32) {
        self.uniforms.colormap_mode = mode;
        self.write_uniforms(queue);
    }

    pub fn set_signed_mapping(&mut self, queue: &wgpu::Queue, enabled: bool) {
        let val: u32 = if enabled { 1 } else { 0 };
        self.uniforms.use_signed_mapping = val;
        self.write_uniforms(queue);
    }

    pub fn set_material_mode(
        &mut self,
        queue: &wgpu::Queue,
        mode: crate::renderer::field_scene::FieldMaterialMode,
    ) {
        self.uniforms.unlit = u32::from(matches!(
            mode,
            crate::renderer::field_scene::FieldMaterialMode::Unlit
        ));
        self.write_uniforms(queue);
    }

    /// Set the volume clip threshold for soft-fade in Both mode.
    pub fn set_clip_threshold(&mut self, queue: &wgpu::Queue, threshold: f32) {
        self.uniforms.volume_clip_threshold = threshold;
        self.write_uniforms(queue);
    }

    /// Set the volume density cutoff: voxels with |value| below this are transparent.
    pub fn set_density_cutoff(&mut self, queue: &wgpu::Queue, cutoff: f32) {
        self.uniforms.volume_density_cutoff = cutoff;
        self.write_uniforms(queue);
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        self.render_with_bind_group(
            &self.render_pipeline,
            render_pass,
            camera_bind_group,
            &self.render_bind_group,
        );
    }

    /// Create an export-owned depth binding. The caller keeps this binding
    /// alive for the complete render pass.
    pub fn bind_group_for_depth(
        &self,
        device: &wgpu::Device,
        depth_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        Self::build_bind_group(
            device,
            &self.bind_group_layout,
            &self.uniform_buffer,
            &self.scalar_buffer,
            depth_view,
        )
    }

    /// Bind immutable export uniforms so publication rendering cannot observe
    /// an interactive camera update after its camera snapshot was captured.
    pub fn publication_bind_group(
        &self,
        device: &wgpu::Device,
        depth_view: &wgpu::TextureView,
        eye_pos: glam::Vec3,
        is_perspective: bool,
        forward: glam::Vec3,
    ) -> wgpu::BindGroup {
        let mut uniforms = self.uniforms;
        uniforms.eye_pos = [eye_pos.x, eye_pos.y, eye_pos.z, 1.0];
        uniforms.is_orthographic = u32::from(!is_perspective);
        uniforms.camera_forward = [forward.x, forward.y, forward.z, 0.0];
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Publication Volume Raycast Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        Self::build_bind_group(
            device,
            &self.bind_group_layout,
            &uniform_buffer,
            &self.scalar_buffer,
            depth_view,
        )
    }

    pub fn render_with_bind_group<'a>(
        &'a self,
        render_pipeline: &'a wgpu::RenderPipeline,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
        render_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_pipeline(render_pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_bind_group(1, render_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }

    pub fn publication_render_pipeline(
        &self,
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        sample_count: u32,
    ) -> wgpu::RenderPipeline {
        super::pipeline::create_volume_raycast_pipeline(
            device,
            surface_format,
            camera_bind_group_layout,
            &self.bind_group_layout,
            sample_count,
        )
    }
}
