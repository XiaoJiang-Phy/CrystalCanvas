//! Top-level Renderer — owns GPU context, camera, pipeline, and buffers; provides render() + resize()
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::sync::Arc;
use wgpu::util::DeviceExt;

use super::camera::{Camera, CameraUniform};
use super::gpu_context::GpuContext;
use super::instance::AtomInstance;
use super::pipeline;

/// Main rendering engine for CrystalCanvas.
/// Manages the full render pipeline lifecycle: initialization, buffer updates, frame rendering.
pub struct Renderer {
    pub gpu: GpuContext,
    pub camera: Camera,

    // GPU resources
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,

    // Instance data
    instance_buffer: wgpu::Buffer,
    instance_count: u32,

    // Depth buffer
    _depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,

    // Lines rendering (Unit cell box)
    line_pipeline: wgpu::RenderPipeline,
    cell_line_buffer: wgpu::Buffer,
    cell_line_count: u32,

    // Thick Cylinder Bonding
    bond_pipeline: wgpu::RenderPipeline,
    bond_instance_buffer: wgpu::Buffer,
    bond_instance_count: u32,

    pub show_cell: bool,
    pub show_bonds: bool,

    // Background clear color (for dark/light mode toggles)
    pub clear_color: wgpu::Color,
}

impl Renderer {
    /// Create a new Renderer attached to the given window.
    /// Initializes GPU context, camera, pipeline, and an empty instance buffer.
    pub fn new<W>(window: Arc<W>, width: u32, height: u32) -> Self
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        let gpu = GpuContext::new(window, width, height);

        // Camera
        let mut camera = Camera::default_for_crystal();
        camera.set_aspect(gpu.config.width as f32, gpu.config.height as f32);

        // Camera uniform buffer
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_from_camera(&camera);

        let camera_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Camera Uniform Buffer"),
                contents: bytemuck::cast_slice(&[camera_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // Pipeline
        let (render_pipeline, camera_bind_group_layout) =
            pipeline::create_render_pipeline(&gpu.device, gpu.surface_format());

        let line_pipeline = pipeline::create_line_pipeline(
            &gpu.device,
            gpu.surface_format(),
            &camera_bind_group_layout,
        );

        let bond_pipeline = pipeline::create_bond_pipeline(
            &gpu.device,
            gpu.surface_format(),
            &camera_bind_group_layout,
        );

        // Camera bind group
        let camera_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Create an instance buffer with 1 dummy element to avoid 0-sized buffer panics
        let dummy_instance = [AtomInstance {
            position: [0.0, 0.0, 0.0],
            radius: 0.0,
            color: [0.0, 0.0, 0.0, 0.0],
        }];
        let instance_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&dummy_instance),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

        // Depth texture
        let (_depth_texture, depth_view) =
            pipeline::create_depth_texture(&gpu.device, gpu.config.width, gpu.config.height);

        // default dark mode color: #0f172a
        let default_clear = wgpu::Color {
            r: 15.0 / 255.0,
            g: 23.0 / 255.0,
            b: 42.0 / 255.0,
            a: 1.0,
        };

        let dummy_line = [crate::renderer::instance::LineVertex {
            position: [0.0, 0.0, 0.0],
            color: [0.0, 0.0, 0.0, 0.0],
        }];
        let cell_line_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cell Line Buffer"),
                contents: bytemuck::cast_slice(&dummy_line),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        let dummy_bond = [crate::renderer::instance::BondInstance {
            start: [0.0, 0.0, 0.0],
            radius: 0.0,
            end: [0.0, 0.0, 0.0],
            _pad: 0.0,
            color: [0.0, 0.0, 0.0, 0.0],
        }];
        let bond_instance_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Bond Instance Buffer"),
                contents: bytemuck::cast_slice(&dummy_bond),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

        Self {
            gpu,
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            render_pipeline,
            instance_buffer,
            instance_count: 0,
            _depth_texture,
            depth_view,
            line_pipeline,
            cell_line_buffer,
            cell_line_count: 0,
            bond_pipeline,
            bond_instance_buffer,
            bond_instance_count: 0,
            show_cell: true,
            show_bonds: true,
            clear_color: default_clear,
        }
    }

    /// Handle window resize: reconfigure surface and rebuild depth texture.
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.gpu.resize(new_size);
            self.camera
                .set_aspect(new_size.width as f32, new_size.height as f32);

            // Rebuild depth texture to match new size
            let (depth_texture, depth_view) =
                pipeline::create_depth_texture(&self.gpu.device, new_size.width, new_size.height);
            self._depth_texture = depth_texture;
            self.depth_view = depth_view;
        }
    }

    /// Upload new atom instance data to the GPU (Phase A: full rebuild).
    /// Per TDD §2.3: for ≤1K atoms (~32 KB), full rebuild is <0.1ms.
    pub fn update_atoms(&mut self, instances: &[AtomInstance]) {
        self.instance_count = instances.len() as u32;

        if instances.is_empty() {
            return;
        }

        // Recreate the instance buffer with new data
        self.instance_buffer =
            self.gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(instances),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        log::debug!(
            "Instance buffer updated: {} atoms, {} bytes",
            self.instance_count,
            std::mem::size_of_val(instances)
        );
    }

    /// Update cell boundaries and bond lines from the CrystalState and settings.
    pub fn update_lines(
        &mut self,
        state: &crate::crystal_state::CrystalState,
        settings: &crate::settings::AppSettings,
    ) {
        let cell_lines = crate::renderer::instance::build_cell_lines(state);
        self.cell_line_count = cell_lines.len() as u32;
        if self.cell_line_count > 0 {
            self.cell_line_buffer =
                self.gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Cell Line Buffer"),
                        contents: bytemuck::cast_slice(&cell_lines),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    });
        }

        let bond_instances = crate::renderer::instance::build_bond_instances(state, settings, &state.selected_atoms);
        self.update_bonds(&bond_instances);
    }

    /// Update actual bond cylinder instances.
    pub fn update_bonds(&mut self, instances: &[crate::renderer::instance::BondInstance]) {
        self.bond_instance_count = instances.len() as u32;
        if instances.is_empty() {
            return;
        }

        self.bond_instance_buffer =
            self.gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Bond Instance Buffer"),
                    contents: bytemuck::cast_slice(instances),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });
    }

    /// Update camera uniform and upload to GPU. Call once per frame (or on camera change).
    pub fn update_camera(&mut self) {
        self.camera_uniform.update_from_camera(&self.camera);
        self.gpu.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
    }

    /// Render one frame. Returns Err if the surface texture cannot be acquired.
    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Update camera uniform before rendering
        self.update_camera();

        // Acquire surface texture
        let output = self.gpu.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Build command buffer
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Impostor Sphere Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            // Only set vertex buffers and draw if we have instances
            // This prevents panics on .slice(..) or drawing out of bounds
            if self.instance_count > 0 {
                render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
                // 6 vertices per impostor quad (two triangles), instance_count instances
                render_pass.draw(0..6, 0..self.instance_count);
            }

            // Draw lines
            // Reuse the camera bind group but switch to the Line pipeline
            if self.show_cell && self.cell_line_count > 0 {
                render_pass.set_pipeline(&self.line_pipeline);
                // Bind group 0 is already camera_bind_group, but let's be explicit
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.cell_line_buffer.slice(..));
                render_pass.draw(0..self.cell_line_count, 0..1);
            }
            if self.show_bonds && self.bond_instance_count > 0 {
                render_pass.set_pipeline(&self.bond_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.bond_instance_buffer.slice(..));
                // 12 segments * 6 vertices = 72 vertices per impostor cylinder quad equivalent
                render_pass.draw(0..72, 0..self.bond_instance_count);
            }
        }

        // Submit command buffer
        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// Render the current scene to an off-screen texture and return raw RGBA pixel bytes.
    ///
    /// # Arguments
    /// * `width` - Target image width in pixels.
    /// * `height` - Target image height in pixels.
    /// * `bg_mode` - Background mode: "transparent", "white", or "default" (current theme).
    pub fn render_offscreen(
        &mut self,
        width: u32,
        height: u32,
        bg_mode: &str,
    ) -> Result<Vec<u8>, String> {
        let width = width.max(1);
        let height = height.max(1);

        // Choose background clear color
        let clear_color = match bg_mode {
            "transparent" => wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
            "white" => wgpu::Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            "black" => wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            _ => self.clear_color, // "default" — use current theme color
        };

        // Temporarily adjust camera aspect ratio for the off-screen dimensions
        let original_aspect_w = self.gpu.config.width;
        let original_aspect_h = self.gpu.config.height;
        self.camera.set_aspect(width as f32, height as f32);
        self.camera_uniform.update_from_camera(&self.camera);
        self.gpu.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        // Use the same format as the surface so existing pipelines are compatible
        let tex_format = self.gpu.surface_format();

        // Create off-screen color texture
        let color_texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Offscreen Color Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: tex_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create off-screen depth texture
        let (_, depth_view) =
            pipeline::create_depth_texture(&self.gpu.device, width, height);

        // Encode the render pass (identical to on-screen render())
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Offscreen Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Offscreen Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Atoms
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            if self.instance_count > 0 {
                render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
                render_pass.draw(0..6, 0..self.instance_count);
            }

            // Cell lines
            if self.show_cell && self.cell_line_count > 0 {
                render_pass.set_pipeline(&self.line_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.cell_line_buffer.slice(..));
                render_pass.draw(0..self.cell_line_count, 0..1);
            }

            // Bonds
            if self.show_bonds && self.bond_instance_count > 0 {
                render_pass.set_pipeline(&self.bond_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.bond_instance_buffer.slice(..));
                render_pass.draw(0..72, 0..self.bond_instance_count);
            }
        }

        // Copy texture to a CPU-readable buffer
        let bytes_per_pixel: u32 = 4;
        let unpadded_bytes_per_row = bytes_per_pixel * width;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let staging_size = (padded_bytes_per_row * height) as u64;

        let staging_buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Offscreen Staging Buffer"),
            size: staging_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &color_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.gpu.queue.submit(std::iter::once(encoder.finish()));

        // Map the staging buffer and read the data
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.gpu.device.poll(wgpu::Maintain::Wait);

        rx.recv()
            .map_err(|e| format!("Failed to receive map result: {}", e))?
            .map_err(|e| format!("Buffer map failed: {:?}", e))?;

        // Strip row padding and convert BGRA -> RGBA
        let data = buffer_slice.get_mapped_range();
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
        let is_bgra = matches!(
            tex_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );

        for row in 0..height {
            let offset = (row * padded_bytes_per_row) as usize;
            let row_end = offset + (unpadded_bytes_per_row as usize);
            let row_data = &data[offset..row_end];
            if is_bgra {
                // Swap B and R channels: BGRA -> RGBA
                for pixel in row_data.chunks_exact(4) {
                    rgba_data.push(pixel[2]); // R
                    rgba_data.push(pixel[1]); // G
                    rgba_data.push(pixel[0]); // B
                    rgba_data.push(pixel[3]); // A
                }
            } else {
                rgba_data.extend_from_slice(row_data);
            }
        }
        drop(data);
        staging_buffer.unmap();

        // Restore camera aspect ratio
        self.camera
            .set_aspect(original_aspect_w as f32, original_aspect_h as f32);
        self.update_camera();

        log::info!(
            "Offscreen render complete: {}x{}, {} bytes (bg={})",
            width,
            height,
            rgba_data.len(),
            bg_mode
        );

        Ok(rgba_data)
    }
}
