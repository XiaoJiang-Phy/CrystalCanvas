// [Overview: Brillouin Zone Sub-Viewport Rendering Engine]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use wgpu::util::DeviceExt;
use crate::brillouin_zone::BrillouinZone;
use crate::kpath::KPath;
use crate::renderer::camera::{Camera, CameraUniform};
use crate::renderer::gpu_context::GpuContext;
use crate::renderer::instance::{AtomInstance, LineVertex};
use crate::renderer::pipeline;

pub struct BzSubViewport {
    pub camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    
    pub color_texture: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,

    line_pipeline: wgpu::RenderPipeline,
    edge_buffer: wgpu::Buffer,
    edge_count: u32,
    
    point_pipeline: wgpu::RenderPipeline,
    point_buffer: wgpu::Buffer,
    point_count: u32,
    
    pub width: u32,
    pub height: u32,
    
    // Blit pass
    pub blit_pipeline: wgpu::RenderPipeline,
    pub blit_bind_group: wgpu::BindGroup,
    pub blit_bind_group_layout: wgpu::BindGroupLayout,
    pub bz_sampler: wgpu::Sampler,
}

impl BzSubViewport {
    pub fn new(gpu: &GpuContext, width: u32, height: u32) -> Self {
        let format = gpu.surface_format();

        let color_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("BZ Offscreen Color Texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("BZ Offscreen Depth Texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut camera = Camera::default_for_crystal();
        camera.set_aspect(width as f32, height as f32);

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_from_camera(&camera);

        let camera_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("BZ Camera Uniform Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let (point_pipeline, layout) = pipeline::create_render_pipeline(&gpu.device, format);
        
        let camera_bind_group_layout = layout;
        let camera_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BZ Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let line_pipeline = pipeline::create_line_pipeline(&gpu.device, format, &camera_bind_group_layout);

        let edge_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BZ Empty Edge Buffer"),
            size: 16,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let point_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BZ Empty Point Buffer"),
            size: 32,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Blit sampler + pipeline
        let bz_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("BZ Blit Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let shader = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BZ Blit Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/bz_blit.wgsl").into()),
        });

        let blit_bind_group_layout = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("BZ Blit Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let blit_pipeline_layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("BZ Blit Pipeline Layout"),
            bind_group_layouts: &[&blit_bind_group_layout],
            push_constant_ranges: &[],
        });

        let blit_pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BZ Blit Pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let blit_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BZ Blit Bind Group"),
            layout: &blit_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&bz_sampler),
                },
            ],
        });

        Self {
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            color_texture,
            color_view,
            depth_texture,
            depth_view,
            line_pipeline,
            edge_buffer,
            edge_count: 0,
            point_pipeline,
            point_buffer,
            point_count: 0,
            width,
            height,
            blit_pipeline,
            blit_bind_group,
            blit_bind_group_layout,
            bz_sampler,
        }
    }

    pub fn resize(&mut self, gpu: &GpuContext, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        self.width = width;
        self.height = height;

        let format = gpu.surface_format();
        self.color_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("BZ Offscreen Color Texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.color_view = self.color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.depth_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("BZ Offscreen Depth Texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.depth_view = self.depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.camera.set_aspect(width as f32, height as f32);

        self.blit_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BZ Blit Bind Group"),
            layout: &self.blit_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.bz_sampler),
                },
            ],
        });
    }

    pub fn update_bz(&mut self, gpu: &GpuContext, bz: &BrillouinZone, kpath: &KPath) {
        let mut vertices = Vec::with_capacity(bz.edges.len() * 2 + 32);
        for edge in &bz.edges {
            let v1 = bz.vertices[edge[0]];
            let v2 = bz.vertices[edge[1]];
            vertices.push(LineVertex { position: [v1[0] as f32, v1[1] as f32, v1[2] as f32], color: [0.8, 0.8, 0.8, 1.0] });
            vertices.push(LineVertex { position: [v2[0] as f32, v2[1] as f32, v2[2] as f32], color: [0.8, 0.8, 0.8, 1.0] });
        }
        
        let path_color = [1.0, 0.4, 0.0, 1.0];
        for segment in &kpath.path_segments {
            for i in 0..segment.len() {
                if i < segment.len() - 1 {
                    let p1 = kpath.points.iter().find(|p| p.label == segment[i]);
                    let p2 = kpath.points.iter().find(|p| p.label == segment[i+1]);
                    if let (Some(a), Some(b)) = (p1, p2) {
                        let mut ca = [0.0; 3];
                        let mut cb = [0.0; 3];
                        for j in 0..3 {
                            ca[j] = a.coord_frac[0]*bz.recip_lattice[0][j] + a.coord_frac[1]*bz.recip_lattice[1][j] + a.coord_frac[2]*bz.recip_lattice[2][j];
                            cb[j] = b.coord_frac[0]*bz.recip_lattice[0][j] + b.coord_frac[1]*bz.recip_lattice[1][j] + b.coord_frac[2]*bz.recip_lattice[2][j];
                        }
                        vertices.push(LineVertex { position: [ca[0] as f32, ca[1] as f32, ca[2] as f32], color: path_color });
                        vertices.push(LineVertex { position: [cb[0] as f32, cb[1] as f32, cb[2] as f32], color: path_color });
                    }
                }
            }
        }

        self.edge_count = vertices.len() as u32;
        if self.edge_count > 0 {
            self.edge_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("BZ Edge Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        }

        let mut points = Vec::with_capacity(kpath.points.len());
        let mut max_b = 0.0_f64;
        for r in &bz.recip_lattice {
            let m = (r[0]*r[0] + r[1]*r[1] + r[2]*r[2]).sqrt();
            if m > max_b { max_b = m; }
        }
        let radius = (max_b * 0.02) as f32;
        
        for kp in &kpath.points {
            let mut c = [0.0; 3];
            for j in 0..3 {
                c[j] = kp.coord_frac[0]*bz.recip_lattice[0][j] + kp.coord_frac[1]*bz.recip_lattice[1][j] + kp.coord_frac[2]*bz.recip_lattice[2][j];
            }
            points.push(AtomInstance {
                position: [c[0] as f32, c[1] as f32, c[2] as f32],
                radius,
                color: [1.0, 0.4, 0.0, 1.0],
            });
        }

        self.point_count = points.len() as u32;
        if self.point_count > 0 {
            self.point_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("BZ Point Buffer"),
                contents: bytemuck::cast_slice(&points),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        }

        self.camera.target = [0.0, 0.0, 0.0].into();       
        self.camera.eye = self.camera.target + glam::Vec3::new(0.0, 0.0, (max_b * 2.5) as f32);
        self.camera_uniform.update_from_camera(&self.camera);
        gpu.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));
    }

    pub fn render_to_texture(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("BZ Offscreen Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }),
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

        if self.edge_count > 0 {
            render_pass.set_pipeline(&self.line_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.edge_buffer.slice(..));
            render_pass.draw(0..self.edge_count, 0..1);
        }

        if self.point_count > 0 {
            render_pass.set_pipeline(&self.point_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.point_buffer.slice(..));
            render_pass.draw(0..6, 0..self.point_count);
        }
    }

    pub fn blit_to_main(&self, render_pass: &mut wgpu::RenderPass, x_offset: f32, y_offset: f32, w: f32, h: f32) {
        render_pass.set_viewport(x_offset, y_offset, w, h, 0.0, 1.0);
        render_pass.set_pipeline(&self.blit_pipeline);
        render_pass.set_bind_group(0, &self.blit_bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
    
    pub fn update_camera(&mut self, queue: &wgpu::Queue) {
        self.camera_uniform.update_from_camera(&self.camera);
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));
    }
}
