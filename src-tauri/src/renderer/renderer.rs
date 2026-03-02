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
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
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
        }

        // Submit command buffer
        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
