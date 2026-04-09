// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use wgpu::util::DeviceExt;
use crate::volumetric::VolumetricData;

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
    pub camera_forward: [f32; 4],
    pub volume_clip_threshold: f32,
    pub volume_density_cutoff: f32,
    pub _pad1: [f32; 2],
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
    scalar_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl VolumeRaycastPipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        volumetric_data: &VolumetricData,
        depth_view: &wgpu::TextureView,
    ) -> Self {
        let mat = glam::DMat3::from_cols_array(&volumetric_data.lattice).as_mat3();
        let inv_mat_t = mat.inverse().transpose();
        
        let grid = volumetric_data.grid_dims;
        let t_min = volumetric_data.data_min;
        let t_max = volumetric_data.data_max;

        // Nyquist-compliant step size: half the minimum voxel spacing
        // $\Delta t = 0.5 \cdot \min(|\mathbf{a}|/N_x,\, |\mathbf{b}|/N_y,\, |\mathbf{c}|/N_z)$
        let h_a = mat.x_axis.length() / grid[0].max(1) as f32;
        let h_b = mat.y_axis.length() / grid[1].max(1) as f32;
        let h_c = mat.z_axis.length() / grid[2].max(1) as f32;
        let step_size = (h_a.min(h_b).min(h_c) * 0.5).max(1e-4);

        // max_steps covers the full body diagonal with headroom
        let diagonal = mat.x_axis.length() + mat.y_axis.length() + mat.z_axis.length();
        let max_steps = ((diagonal / step_size) * 1.5) as u32;
        let max_steps = max_steps.clamp(256, 2048);

        log::info!(
            "Volume raycast: voxel h=({:.4}, {:.4}, {:.4}) Å, step_size={:.4} Å, max_steps={}",
            h_a, h_b, h_c, step_size, max_steps
        );

        let uniforms = VolumeRaycastUniforms {
            lattice_a: [mat.x_axis.x, mat.x_axis.y, mat.x_axis.z, 0.0],
            lattice_b: [mat.y_axis.x, mat.y_axis.y, mat.y_axis.z, 0.0],
            lattice_c: [mat.z_axis.x, mat.z_axis.y, mat.z_axis.z, 0.0],
            inv_lattice_a: [inv_mat_t.x_axis.x, inv_mat_t.x_axis.y, inv_mat_t.x_axis.z, 0.0],
            inv_lattice_b: [inv_mat_t.y_axis.x, inv_mat_t.y_axis.y, inv_mat_t.y_axis.z, 0.0],
            inv_lattice_c: [inv_mat_t.z_axis.x, inv_mat_t.z_axis.y, inv_mat_t.z_axis.z, 0.0],
            eye_pos: [0.0, 0.0, 0.0, 1.0],
            origin: [volumetric_data.origin[0] as f32, volumetric_data.origin[1] as f32, volumetric_data.origin[2] as f32, 0.0],
            grid_dims: [grid[0] as u32, grid[1] as u32, grid[2] as u32, 0],
            transfer_range: [t_min, t_max],
            opacity_scale: 1.0,
            step_size,
            max_steps,
            colormap_mode: 0,
            is_orthographic: 1,
            use_signed_mapping: 0,
            camera_forward: [0.0, 0.0, -1.0, 0.0],
            volume_clip_threshold: 0.0,
            volume_density_cutoff: 0.0,
            _pad1: [0.0; 2],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Volume Raycast Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let scalar_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Volume Raycast Scalar Buffer"),
            contents: bytemuck::cast_slice(&volumetric_data.data),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let vertices = [
            VolumeVertex { position: [0.0, 0.0, 0.0] },
            VolumeVertex { position: [1.0, 0.0, 0.0] },
            VolumeVertex { position: [1.0, 1.0, 0.0] },
            VolumeVertex { position: [0.0, 1.0, 0.0] },
            VolumeVertex { position: [0.0, 0.0, 1.0] },
            VolumeVertex { position: [1.0, 0.0, 1.0] },
            VolumeVertex { position: [1.0, 1.0, 1.0] },
            VolumeVertex { position: [0.0, 1.0, 1.0] },
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
            device, &bind_group_layout, &uniform_buffer, &scalar_buffer, depth_view,
        );

        let render_pipeline = super::pipeline::create_volume_raycast_pipeline(
            device,
            surface_format,
            camera_bind_group_layout,
            &bind_group_layout,
        );

        Self {
            render_pipeline,
            render_bind_group,
            bind_group_layout,
            uniform_buffer,
            scalar_buffer,
            vertex_buffer,
            index_buffer,
            index_count,
        }
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

    /// Rebuild bind group when depth texture changes (e.g., window resize).
    pub fn update_depth_view(&mut self, device: &wgpu::Device, depth_view: &wgpu::TextureView) {
        self.render_bind_group = Self::build_bind_group(
            device, &self.bind_group_layout, &self.uniform_buffer, &self.scalar_buffer, depth_view,
        );
    }

    pub fn update_transfer_function(
        &mut self,
        queue: &wgpu::Queue,
        transfer_range: [f32; 2],
        opacity_scale: f32,
    ) {
        // offset 144: transfer_range[0..1] + opacity_scale
        queue.write_buffer(
            &self.uniform_buffer,
            144,
            bytemuck::cast_slice(&[transfer_range[0], transfer_range[1], opacity_scale]),
        );
    }
    
    pub fn update_camera(
        &mut self,
        queue: &wgpu::Queue,
        eye_pos: glam::Vec3,
        is_perspective: bool,
        forward: glam::Vec3,
    ) {
        // eye_pos at offset 96
        queue.write_buffer(
            &self.uniform_buffer,
            96,
            bytemuck::cast_slice(&[eye_pos.x, eye_pos.y, eye_pos.z, 1.0f32]),
        );
        // is_orthographic at offset 168
        let is_ortho: u32 = if is_perspective { 0 } else { 1 };
        queue.write_buffer(
            &self.uniform_buffer,
            168,
            bytemuck::cast_slice(&[is_ortho]),
        );
        // camera_forward at offset 176
        queue.write_buffer(
            &self.uniform_buffer,
            176,
            bytemuck::cast_slice(&[forward.x, forward.y, forward.z, 0.0f32]),
        );
    }

    /// Set colormap mode index.
    pub fn set_colormap(&self, queue: &wgpu::Queue, mode: u32) {
        // offset 164
        queue.write_buffer(
            &self.uniform_buffer,
            164,
            bytemuck::cast_slice(&[mode]),
        );
    }

    pub fn set_signed_mapping(&self, queue: &wgpu::Queue, enabled: bool) {
        // offset 172
        let val: u32 = if enabled { 1 } else { 0 };
        queue.write_buffer(
            &self.uniform_buffer,
            172,
            bytemuck::cast_slice(&[val]),
        );
    }

    /// Set the volume clip threshold for soft-fade in Both mode.
    /// offset = camera_forward(176) + 16 = 192
    pub fn set_clip_threshold(&self, queue: &wgpu::Queue, threshold: f32) {
        queue.write_buffer(
            &self.uniform_buffer,
            192,
            bytemuck::cast_slice(&[threshold]),
        );
    }

    /// Set the volume density cutoff: voxels with |value| below this are transparent.
    /// offset = volume_clip_threshold(192) + 4 = 196
    pub fn set_density_cutoff(&self, queue: &wgpu::Queue, cutoff: f32) {
        queue.write_buffer(
            &self.uniform_buffer,
            196,
            bytemuck::cast_slice(&[cutoff]),
        );
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_bind_group(1, &self.render_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}
