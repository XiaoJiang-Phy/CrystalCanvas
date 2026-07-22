//! Top-level Renderer — owns GPU context, camera, pipeline, and buffers; provides render() + resize()
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;

use super::camera::{Camera, CameraUniform};
use super::gpu_context::GpuContext;
use super::instance::{
    apply_phonon_frame, validate_phonon_display_envelope, AtomInstance, PreparedAtomScene,
    RenderLineScene,
};
use super::pipeline;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererVolumeMode {
    Isosurface,
    Volume,
    Both,
}

pub struct PreparedVolumetric {
    isosurface_pipeline: Option<crate::renderer::isosurface::IsosurfacePipeline>,
    volume_raycast_pipeline: crate::renderer::volume_raycast::VolumeRaycastPipeline,
}

#[derive(Clone, Copy)]
struct AtomDragInstance {
    base_position: [f32; 3],
    base_radius: f32,
    base_color: [f32; 4],
}

struct AtomDragInstances {
    selected: Vec<AtomDragInstance>,
    stationary: Vec<AtomInstance>,
}

/// Monotonic presentation clock. Its rate has no physical-time interpretation.
pub struct PhononPlayback {
    anchor_phase: f64,
    anchor_time: f64,
    display_angular_velocity: f64,
    playing: bool,
}

impl PhononPlayback {
    pub fn new(display_angular_velocity: f64) -> crate::ipc::IpcResult<Self> {
        if !display_angular_velocity.is_finite() {
            return Err(crate::ipc::IpcError::invalid_argument(
                "phonon display rate must be finite",
            ));
        }
        Ok(Self {
            anchor_phase: 0.0,
            anchor_time: 0.0,
            display_angular_velocity,
            playing: false,
        })
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn phase_at(&self, now: f64) -> crate::ipc::IpcResult<f64> {
        if !now.is_finite() {
            return Err(crate::ipc::IpcError::invalid_argument(
                "phonon playback time must be finite",
            ));
        }
        if !self.playing {
            return Ok(self.anchor_phase);
        }
        if now < self.anchor_time {
            return Err(crate::ipc::IpcError::invalid_argument(
                "phonon playback time cannot move backwards",
            ));
        }
        let phase = self
            .display_angular_velocity
            .mul_add(now - self.anchor_time, self.anchor_phase);
        if !phase.is_finite() {
            return Err(crate::ipc::IpcError::invalid_argument(
                "phonon playback phase is not finite",
            ));
        }
        Ok(phase.rem_euclid(std::f64::consts::TAU))
    }

    pub fn start(&mut self, now: f64) -> crate::ipc::IpcResult<()> {
        if !now.is_finite() {
            return Err(crate::ipc::IpcError::invalid_argument(
                "phonon playback time must be finite",
            ));
        }
        if self.playing {
            self.phase_at(now)?;
            return Ok(());
        }
        self.anchor_time = now;
        self.playing = true;
        Ok(())
    }

    pub fn stop(&mut self, now: f64) -> crate::ipc::IpcResult<()> {
        if !now.is_finite() {
            return Err(crate::ipc::IpcError::invalid_argument(
                "phonon playback time must be finite",
            ));
        }
        if !self.playing {
            return Ok(());
        }
        let phase = self.phase_at(now)?;
        self.anchor_phase = phase;
        self.anchor_time = now;
        self.playing = false;
        Ok(())
    }

    pub fn seek(&mut self, phase: f64, now: f64) -> crate::ipc::IpcResult<()> {
        if !phase.is_finite() || !now.is_finite() {
            return Err(crate::ipc::IpcError::invalid_argument(
                "phonon phase and playback time must be finite",
            ));
        }
        if self.playing && now < self.anchor_time {
            return Err(crate::ipc::IpcError::invalid_argument(
                "phonon playback time cannot move backwards",
            ));
        }
        self.anchor_phase = phase.rem_euclid(std::f64::consts::TAU);
        self.anchor_time = now;
        Ok(())
    }

    fn halt(&mut self) {
        self.playing = false;
    }
}

/// Renderer-owned presentation state for one selected phonon mode.
struct PhononPresentation {
    display_scale: f64,
    dirty: bool,
    mode_displacements: Vec<[f32; 3]>,
    opaque_display_instances: Vec<AtomInstance>,
    transparent_display_instances: Vec<AtomInstance>,
    playback: PhononPlayback,
    time_origin: Instant,
}

impl PhononPresentation {
    fn new(
        opaque_base_instances: &[AtomInstance],
        transparent_base_instances: &[AtomInstance],
        opaque_source_atom_indices: &[usize],
        transparent_source_atom_indices: &[usize],
        mode_displacements: &[[f64; 3]],
    ) -> crate::ipc::IpcResult<Self> {
        if opaque_base_instances.len() != opaque_source_atom_indices.len()
            || transparent_base_instances.len() != transparent_source_atom_indices.len()
        {
            return Err(crate::ipc::IpcError::render(
                "phonon source map does not match the render buffers",
            ));
        }
        if opaque_source_atom_indices
            .iter()
            .chain(transparent_source_atom_indices)
            .any(|&source_atom_index| source_atom_index >= mode_displacements.len())
        {
            return Err(crate::ipc::IpcError::invalid_argument(
                "phonon source index has no mode displacement",
            ));
        }

        let mut prepared_displacements = Vec::new();
        prepared_displacements
            .try_reserve_exact(mode_displacements.len())
            .map_err(|_| crate::ipc::IpcError::render("unable to allocate phonon displacements"))?;
        for displacement in mode_displacements {
            let prepared = [
                displacement[0] as f32,
                displacement[1] as f32,
                displacement[2] as f32,
            ];
            if !prepared.iter().all(|component| component.is_finite()) {
                return Err(crate::ipc::IpcError::invalid_argument(
                    "phonon displacement must be finite",
                ));
            }
            prepared_displacements.push(prepared);
        }

        validate_phonon_display_envelope(
            opaque_base_instances,
            opaque_source_atom_indices,
            &prepared_displacements,
            1.0,
        )?;
        validate_phonon_display_envelope(
            transparent_base_instances,
            transparent_source_atom_indices,
            &prepared_displacements,
            1.0,
        )?;

        let mut opaque_display_instances = Vec::new();
        opaque_display_instances
            .try_reserve_exact(opaque_base_instances.len())
            .map_err(|_| {
                crate::ipc::IpcError::render("unable to allocate opaque phonon instances")
            })?;
        opaque_display_instances.extend_from_slice(opaque_base_instances);

        let mut transparent_display_instances = Vec::new();
        transparent_display_instances
            .try_reserve_exact(transparent_base_instances.len())
            .map_err(|_| {
                crate::ipc::IpcError::render("unable to allocate transparent phonon instances")
            })?;
        transparent_display_instances.extend_from_slice(transparent_base_instances);

        Ok(Self {
            display_scale: 1.0,
            dirty: true,
            mode_displacements: prepared_displacements,
            opaque_display_instances,
            transparent_display_instances,
            playback: PhononPlayback::new(std::f64::consts::TAU)?,
            time_origin: Instant::now(),
        })
    }
}

pub(crate) struct AtomDragSession {
    session_id: String,
    pub(crate) source_version: u32,
    pub(crate) source_indices: Vec<usize>,
    pub(crate) translation: glam::Vec3,
    opaque_instances: Vec<AtomDragInstance>,
    transparent_instances: Vec<AtomDragInstance>,
    opaque_preview_instances: Vec<AtomInstance>,
    transparent_preview_instances: Vec<AtomInstance>,
    opaque_stationary_buffer: Option<wgpu::Buffer>,
    transparent_stationary_buffer: Option<wgpu::Buffer>,
    opaque_preview_buffer: Option<wgpu::Buffer>,
    transparent_preview_buffer: Option<wgpu::Buffer>,
    opaque_stationary_count: u32,
    transparent_stationary_count: u32,
    opaque_preview_count: u32,
    transparent_preview_count: u32,
}

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

    transparent_pipeline: wgpu::RenderPipeline,
    transparent_instance_buffer: wgpu::Buffer,
    transparent_instance_count: u32,
    atom_pick_data: Arc<Vec<crate::renderer::ray_picking::PickAtom>>,
    opaque_atom_instances: Vec<AtomInstance>,
    transparent_atom_instances: Vec<AtomInstance>,
    opaque_source_atom_indices: Vec<usize>,
    transparent_source_atom_indices: Vec<usize>,
    phonon_presentation: Option<PhononPresentation>,
    atom_drag: Option<AtomDragSession>,
    next_atom_drag_session: u64,

    // Depth buffers (dual-pass architecture)
    opaque_depth_texture: wgpu::Texture,
    opaque_depth_view: wgpu::TextureView,
    transparent_depth_texture: wgpu::Texture,
    transparent_depth_view: wgpu::TextureView,

    // Lines rendering (Unit cell box)
    line_pipeline: wgpu::RenderPipeline,
    cell_line_buffer: wgpu::Buffer,
    cell_line_count: u32,

    // Measurement lines
    measurement_line_buffer: wgpu::Buffer,
    measurement_line_count: u32,

    // Thick Cylinder Bonding
    bond_pipeline: wgpu::RenderPipeline,
    bond_instance_buffer: wgpu::Buffer,
    bond_instance_count: u32,

    pub hopping_instance_buffer: wgpu::Buffer,
    pub hopping_instance_count: u32,
    pub show_hoppings: bool,

    pub show_cell: bool,
    pub show_bonds: bool,

    // Volumetric rendering
    pub isosurface_pipeline: Option<crate::renderer::isosurface::IsosurfacePipeline>,
    pub show_isosurface: bool,
    pub volume_raycast_pipeline: Option<crate::renderer::volume_raycast::VolumeRaycastPipeline>,
    pub show_volume: bool,
    pub volume_render_mode: RendererVolumeMode,
    pub active_colormap_mode: u32,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    pub isosurface_dispatch_size: [u32; 3],

    // Background clear color (for dark/light mode toggles)
    pub clear_color: wgpu::Color,

    // Reciprocal Space
    pub bz_viewport: Option<crate::renderer::bz_renderer::BzSubViewport>,
    pub show_bz: bool,
    pub bz_scale: f32,
}

fn drag_instances(
    atoms: &[AtomInstance],
    source_atom_indices: &[usize],
    selected_source_indices: &[usize],
) -> crate::ipc::IpcResult<AtomDragInstances> {
    if atoms.len() != source_atom_indices.len() {
        return Err(crate::ipc::IpcError::render(
            "atom drag source map does not match the render buffer",
        ));
    }
    let selected_count = source_atom_indices
        .iter()
        .filter(|&&source_atom_index| {
            selected_source_indices
                .binary_search(&source_atom_index)
                .is_ok()
        })
        .count();
    let stationary_count = atoms
        .len()
        .checked_sub(selected_count)
        .ok_or_else(|| crate::ipc::IpcError::render("atom drag selection exceeds render buffer"))?;
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(selected_count)
        .map_err(|_| crate::ipc::IpcError::render("unable to allocate atom drag metadata"))?;
    let mut stationary = Vec::new();
    stationary
        .try_reserve_exact(stationary_count)
        .map_err(|_| {
            crate::ipc::IpcError::render("unable to allocate atom drag stationary data")
        })?;
    for (&atom, &source_atom_index) in atoms.iter().zip(source_atom_indices) {
        if selected_source_indices
            .binary_search(&source_atom_index)
            .is_ok()
        {
            selected.push(AtomDragInstance {
                base_position: atom.position,
                base_radius: atom.radius,
                base_color: atom.color,
            });
        } else {
            stationary.push(atom);
        }
    }
    Ok(AtomDragInstances {
        selected,
        stationary,
    })
}

fn drag_preview_instances(
    instances: &[AtomDragInstance],
) -> crate::ipc::IpcResult<Vec<AtomInstance>> {
    let mut preview = Vec::new();
    preview
        .try_reserve_exact(instances.len())
        .map_err(|_| crate::ipc::IpcError::render("unable to allocate atom drag preview"))?;
    for instance in instances {
        preview.push(AtomInstance {
            position: instance.base_position,
            radius: instance.base_radius,
            color: instance.base_color,
        });
    }
    Ok(preview)
}

fn drag_instance_buffer(
    device: &wgpu::Device,
    label: &'static str,
    instances: &[AtomInstance],
) -> Option<wgpu::Buffer> {
    (!instances.is_empty()).then(|| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        })
    })
}

fn active_atom_drag_mut<'a>(
    atom_drag: &'a mut Option<AtomDragSession>,
    session_id: &str,
) -> crate::ipc::IpcResult<&'a mut AtomDragSession> {
    let session = atom_drag
        .as_mut()
        .ok_or_else(|| crate::ipc::IpcError::invalid_argument("no active atom drag session"))?;
    if session.session_id != session_id {
        return Err(crate::ipc::IpcError::invalid_argument(
            "atom drag session does not match the active session",
        ));
    }
    Ok(session)
}

fn upload_atom_instances(queue: &wgpu::Queue, buffer: &wgpu::Buffer, instances: &[AtomInstance]) {
    if !instances.is_empty() {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(instances));
    }
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

        let transparent_pipeline = pipeline::create_transparent_atom_pipeline(
            &gpu.device,
            gpu.surface_format(),
            &camera_bind_group_layout,
        );

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

        let transparent_instance_buffer =
            gpu.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Transparent Instance Buffer"),
                contents: bytemuck::cast_slice(&dummy_instance),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

        // Depth textures (dual-pass architecture)
        let (opaque_depth_texture, opaque_depth_view) =
            pipeline::create_depth_texture(&gpu.device, gpu.config.width, gpu.config.height);
        let (transparent_depth_texture, transparent_depth_view) =
            pipeline::create_transparent_depth_texture(
                &gpu.device,
                gpu.config.width,
                gpu.config.height,
            );

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
        let measurement_line_buffer =
            gpu.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Measurement Line Buffer"),
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
        let bond_instance_buffer =
            gpu.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Bond Instance Buffer"),
                contents: bytemuck::cast_slice(&dummy_bond),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

        let dummy_hopping = [crate::renderer::instance::BondInstance {
            start: [0.0, 0.0, 0.0],
            radius: 0.0,
            end: [0.0, 0.0, 0.0],
            _pad: 0.0,
            color: [0.0, 0.0, 0.0, 0.0],
        }];
        let hopping_instance_buffer =
            gpu.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Hopping Instance Buffer"),
                contents: bytemuck::cast_slice(&dummy_hopping),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

        let bz_viewport = Some(crate::renderer::bz_renderer::BzSubViewport::new(
            &gpu, 400, 400,
        ));

        Self {
            gpu,
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            render_pipeline,
            instance_buffer,
            instance_count: 0,
            transparent_pipeline,
            transparent_instance_buffer,
            transparent_instance_count: 0,
            atom_pick_data: Arc::new(Vec::new()),
            opaque_atom_instances: Vec::new(),
            transparent_atom_instances: Vec::new(),
            opaque_source_atom_indices: Vec::new(),
            transparent_source_atom_indices: Vec::new(),
            phonon_presentation: None,
            atom_drag: None,
            next_atom_drag_session: 0,
            opaque_depth_texture,
            opaque_depth_view,
            transparent_depth_texture,
            transparent_depth_view,
            line_pipeline,
            cell_line_buffer,
            cell_line_count: 0,
            measurement_line_buffer,
            measurement_line_count: 0,
            bond_pipeline,
            bond_instance_buffer,
            bond_instance_count: 0,
            hopping_instance_buffer,
            hopping_instance_count: 0,
            show_hoppings: true,
            show_cell: true,
            show_bonds: true,
            isosurface_pipeline: None,
            show_isosurface: false,
            volume_raycast_pipeline: None,
            show_volume: false,
            volume_render_mode: RendererVolumeMode::Isosurface,
            active_colormap_mode: 0,
            camera_bind_group_layout,
            isosurface_dispatch_size: [0; 3],
            clear_color: default_clear,
            bz_viewport,
            show_bz: false,
            bz_scale: 0.35,
        }
    }

    /// Handle window resize: reconfigure surface and rebuild depth textures.
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.gpu.resize(new_size);
            self.camera
                .set_aspect(new_size.width as f32, new_size.height as f32);

            // Rebuild both depth textures
            let (opaque_depth_texture, opaque_depth_view) =
                pipeline::create_depth_texture(&self.gpu.device, new_size.width, new_size.height);
            let (transparent_depth_texture, transparent_depth_view) =
                pipeline::create_transparent_depth_texture(
                    &self.gpu.device,
                    new_size.width,
                    new_size.height,
                );
            self.opaque_depth_texture = opaque_depth_texture;
            self.opaque_depth_view = opaque_depth_view;
            self.transparent_depth_texture = transparent_depth_texture;
            self.transparent_depth_view = transparent_depth_view;

            // Notify volume pipeline to rebind depth texture
            if let Some(vol_pipe) = &mut self.volume_raycast_pipeline {
                vol_pipe.update_depth_view(&self.gpu.device, &self.opaque_depth_view);
            }
        }
    }

    /// Upload a CPU-prepared atom scene to the GPU.
    pub fn commit_atoms(&mut self, scene: PreparedAtomScene) {
        let instance_count = scene.opaque.len() as u32;
        let transparent_instance_count = scene.transparent.len() as u32;
        let opaque_buffer = (instance_count > 0).then(|| {
            self.gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(&scene.opaque),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                })
        });
        let transparent_buffer = (transparent_instance_count > 0).then(|| {
            self.gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Transparent Instance Buffer"),
                    contents: bytemuck::cast_slice(&scene.transparent),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                })
        });

        self.instance_count = instance_count;
        self.transparent_instance_count = transparent_instance_count;
        self.atom_pick_data = scene.pick_data;
        self.opaque_atom_instances = scene.opaque;
        self.transparent_atom_instances = scene.transparent;
        self.opaque_source_atom_indices = scene.opaque_source_atom_indices;
        self.transparent_source_atom_indices = scene.transparent_source_atom_indices;
        self.phonon_presentation = None;
        self.atom_drag = None;
        if let Some(buffer) = opaque_buffer {
            self.instance_buffer = buffer;
        }
        if let Some(buffer) = transparent_buffer {
            self.transparent_instance_buffer = buffer;
        }

        log::debug!(
            "Instance buffers updated: {} opaque, {} transparent",
            self.instance_count,
            self.transparent_instance_count
        );
    }

    pub fn clear_atoms(&mut self) {
        self.instance_count = 0;
        self.transparent_instance_count = 0;
        self.atom_pick_data = Arc::new(Vec::new());
        self.opaque_atom_instances.clear();
        self.transparent_atom_instances.clear();
        self.opaque_source_atom_indices.clear();
        self.transparent_source_atom_indices.clear();
        self.phonon_presentation = None;
        self.atom_drag = None;
    }

    pub fn pick_scene_snapshot(&self) -> Arc<Vec<crate::renderer::ray_picking::PickAtom>> {
        Arc::clone(&self.atom_pick_data)
    }

    pub fn is_pick_scene_current(
        &self,
        snapshot: &Arc<Vec<crate::renderer::ray_picking::PickAtom>>,
    ) -> bool {
        Arc::ptr_eq(&self.atom_pick_data, snapshot)
    }

    pub fn set_phonon_mode(
        &mut self,
        mode: Option<&crate::phonon::PhononMode>,
    ) -> crate::ipc::IpcResult<()> {
        if self.atom_drag.is_some() {
            return Err(crate::ipc::IpcError::busy(
                "cannot change phonon mode during an atom drag",
            ));
        }

        self.phonon_presentation = match mode {
            Some(mode) => Some(PhononPresentation::new(
                &self.opaque_atom_instances,
                &self.transparent_atom_instances,
                &self.opaque_source_atom_indices,
                &self.transparent_source_atom_indices,
                &mode.eigenvectors,
            )?),
            None => None,
        };
        self.restore_phonon_base_instances();
        Ok(())
    }

    pub fn set_phonon_phase(
        &mut self,
        phase: f64,
        display_scale: f64,
    ) -> crate::ipc::IpcResult<()> {
        if !phase.is_finite() || !display_scale.is_finite() {
            return Err(crate::ipc::IpcError::invalid_argument(
                "phonon phase and display scale must be finite",
            ));
        }

        if let Some(presentation) = &mut self.phonon_presentation {
            validate_phonon_display_envelope(
                &self.opaque_atom_instances,
                &self.opaque_source_atom_indices,
                &presentation.mode_displacements,
                display_scale,
            )?;
            validate_phonon_display_envelope(
                &self.transparent_atom_instances,
                &self.transparent_source_atom_indices,
                &presentation.mode_displacements,
                display_scale,
            )?;
            let now = presentation.time_origin.elapsed().as_secs_f64();
            presentation.playback.seek(phase, now)?;
            presentation.display_scale = display_scale;
            presentation.dirty = true;
        }
        Ok(())
    }

    pub fn set_phonon_display_scale(
        &mut self,
        display_scale: f64,
    ) -> crate::ipc::IpcResult<()> {
        if !display_scale.is_finite() {
            return Err(crate::ipc::IpcError::invalid_argument(
                "phonon display scale must be finite",
            ));
        }

        if let Some(presentation) = &mut self.phonon_presentation {
            validate_phonon_display_envelope(
                &self.opaque_atom_instances,
                &self.opaque_source_atom_indices,
                &presentation.mode_displacements,
                display_scale,
            )?;
            validate_phonon_display_envelope(
                &self.transparent_atom_instances,
                &self.transparent_source_atom_indices,
                &presentation.mode_displacements,
                display_scale,
            )?;
            presentation.display_scale = display_scale;
            presentation.dirty = true;
        }
        Ok(())
    }

    pub fn set_phonon_playing(&mut self, playing: bool) -> crate::ipc::IpcResult<()> {
        let Some(presentation) = &mut self.phonon_presentation else {
            if playing {
                return Err(crate::ipc::IpcError::invalid_argument(
                    "select a phonon mode before starting playback",
                ));
            }
            return Ok(());
        };
        let now = presentation.time_origin.elapsed().as_secs_f64();
        if playing {
            presentation.playback.start(now)?;
        } else {
            presentation.playback.stop(now)?;
        }
        presentation.dirty = true;
        Ok(())
    }

    pub fn phonon_is_playing(&self) -> bool {
        self.phonon_presentation
            .as_ref()
            .is_some_and(|presentation| presentation.playback.is_playing())
    }

    fn restore_phonon_base_instances(&mut self) {
        if self.instance_count > 0 {
            upload_atom_instances(
                &self.gpu.queue,
                &self.instance_buffer,
                &self.opaque_atom_instances,
            );
        }
        if self.transparent_instance_count > 0 {
            upload_atom_instances(
                &self.gpu.queue,
                &self.transparent_instance_buffer,
                &self.transparent_atom_instances,
            );
        }
    }

    pub(crate) fn begin_atom_drag(
        &mut self,
        source_indices: Vec<usize>,
        source_version: u32,
    ) -> crate::ipc::IpcResult<String> {
        if self.atom_drag.is_some() {
            return Err(crate::ipc::IpcError::busy(
                "an atom drag session is already active",
            ));
        }
        self.next_atom_drag_session = self
            .next_atom_drag_session
            .checked_add(1)
            .ok_or_else(|| crate::ipc::IpcError::busy("atom drag session id exhausted"))?;
        let session_id = self.next_atom_drag_session.to_string();
        let opaque_instances = drag_instances(
            &self.opaque_atom_instances,
            &self.opaque_source_atom_indices,
            &source_indices,
        )?;
        let transparent_instances = drag_instances(
            &self.transparent_atom_instances,
            &self.transparent_source_atom_indices,
            &source_indices,
        )?;
        let opaque_preview_instances = drag_preview_instances(&opaque_instances.selected)?;
        let transparent_preview_instances =
            drag_preview_instances(&transparent_instances.selected)?;
        let opaque_stationary_count =
            u32::try_from(opaque_instances.stationary.len()).map_err(|_| {
                crate::ipc::IpcError::render("opaque atom drag stationary buffer exceeds u32 range")
            })?;
        let transparent_stationary_count = u32::try_from(transparent_instances.stationary.len())
            .map_err(|_| {
                crate::ipc::IpcError::render(
                    "transparent atom drag stationary buffer exceeds u32 range",
                )
            })?;
        let opaque_preview_count = u32::try_from(opaque_preview_instances.len()).map_err(|_| {
            crate::ipc::IpcError::render("opaque atom drag preview exceeds u32 range")
        })?;
        let transparent_preview_count = u32::try_from(transparent_preview_instances.len())
            .map_err(|_| {
                crate::ipc::IpcError::render("transparent atom drag preview exceeds u32 range")
            })?;
        let opaque_stationary_buffer = drag_instance_buffer(
            &self.gpu.device,
            "Opaque Atom Drag Stationary Buffer",
            &opaque_instances.stationary,
        );
        let transparent_stationary_buffer = drag_instance_buffer(
            &self.gpu.device,
            "Transparent Atom Drag Stationary Buffer",
            &transparent_instances.stationary,
        );
        let opaque_preview_buffer = drag_instance_buffer(
            &self.gpu.device,
            "Opaque Atom Drag Preview Buffer",
            &opaque_preview_instances,
        );
        let transparent_preview_buffer = drag_instance_buffer(
            &self.gpu.device,
            "Transparent Atom Drag Preview Buffer",
            &transparent_preview_instances,
        );
        self.atom_drag = Some(AtomDragSession {
            session_id: session_id.clone(),
            source_version,
            source_indices,
            translation: glam::Vec3::ZERO,
            opaque_instances: opaque_instances.selected,
            transparent_instances: transparent_instances.selected,
            opaque_preview_instances,
            transparent_preview_instances,
            opaque_stationary_buffer,
            transparent_stationary_buffer,
            opaque_preview_buffer,
            transparent_preview_buffer,
            opaque_stationary_count,
            transparent_stationary_count,
            opaque_preview_count,
            transparent_preview_count,
        });
        Ok(session_id)
    }

    pub(crate) fn update_atom_drag(
        &mut self,
        session_id: &str,
        dx: f32,
        dy: f32,
    ) -> crate::ipc::IpcResult<()> {
        if !dx.is_finite() || !dy.is_finite() {
            return Err(crate::ipc::IpcError::invalid_argument(
                "atom drag screen delta must be finite",
            ));
        }
        let translation = self.screen_drag_translation(dx, dy)?;
        let session = active_atom_drag_mut(&mut self.atom_drag, session_id)?;
        let candidate_translation = session.translation + translation;
        if !candidate_translation.is_finite() {
            return Err(crate::ipc::IpcError::invalid_argument(
                "atom drag translation is not finite",
            ));
        }

        session.translation = candidate_translation;
        for (preview, instance) in session
            .opaque_preview_instances
            .iter_mut()
            .zip(&session.opaque_instances)
        {
            preview.position =
                (glam::Vec3::from_array(instance.base_position) + candidate_translation).to_array();
        }
        for (preview, instance) in session
            .transparent_preview_instances
            .iter_mut()
            .zip(&session.transparent_instances)
        {
            preview.position =
                (glam::Vec3::from_array(instance.base_position) + candidate_translation).to_array();
        }
        if let Some(buffer) = &session.opaque_preview_buffer {
            upload_atom_instances(&self.gpu.queue, buffer, &session.opaque_preview_instances);
        }
        if let Some(buffer) = &session.transparent_preview_buffer {
            upload_atom_instances(
                &self.gpu.queue,
                buffer,
                &session.transparent_preview_instances,
            );
        }
        Ok(())
    }

    pub(crate) fn take_atom_drag(
        &mut self,
        session_id: &str,
    ) -> crate::ipc::IpcResult<AtomDragSession> {
        let session = self
            .atom_drag
            .take()
            .ok_or_else(|| crate::ipc::IpcError::invalid_argument("no active atom drag session"))?;
        if session.session_id == session_id {
            return Ok(session);
        }
        self.atom_drag = Some(session);
        Err(crate::ipc::IpcError::invalid_argument(
            "atom drag session does not match the active session",
        ))
    }

    pub(crate) fn cancel_atom_drag(&mut self, session_id: &str) -> crate::ipc::IpcResult<()> {
        let session = self
            .atom_drag
            .as_ref()
            .ok_or_else(|| crate::ipc::IpcError::invalid_argument("no active atom drag session"))?;
        if session.session_id != session_id {
            return Err(crate::ipc::IpcError::invalid_argument(
                "atom drag session does not match the active session",
            ));
        }
        self.atom_drag = None;
        Ok(())
    }

    fn screen_drag_translation(&self, dx: f32, dy: f32) -> crate::ipc::IpcResult<glam::Vec3> {
        let pan_speed = 0.001 * (self.camera.eye - self.camera.target).length();
        let forward = (self.camera.target - self.camera.eye).normalize();
        let right = forward.cross(self.camera.up).normalize();
        let up = right.cross(forward).normalize();
        let translation = right * dx * pan_speed - up * dy * pan_speed;
        if !translation.is_finite() {
            return Err(crate::ipc::IpcError::invalid_argument(
                "atom drag translation is not finite",
            ));
        }
        Ok(translation)
    }

    /// Upload prepared cell boundaries, bonds, and measurement lines.
    pub fn update_lines(&mut self, scene: &RenderLineScene) {
        self.cell_line_count = scene.cell_lines.len() as u32;
        if self.cell_line_count > 0 {
            self.cell_line_buffer =
                self.gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Cell Line Buffer"),
                        contents: bytemuck::cast_slice(&scene.cell_lines),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    });
        }

        self.update_bonds(&scene.bond_instances);

        self.measurement_line_count = scene.measurement_lines.len() as u32;
        if self.measurement_line_count > 0 {
            self.measurement_line_buffer =
                self.gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Measurement Line Buffer"),
                        contents: bytemuck::cast_slice(&scene.measurement_lines),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    });
        }
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

    /// Update actual hopping cylinder instances.
    pub fn update_hoppings(&mut self, instances: &[crate::renderer::instance::BondInstance]) {
        self.hopping_instance_count = instances.len() as u32;
        if instances.is_empty() {
            return;
        }

        self.hopping_instance_buffer =
            self.gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Hopping Instance Buffer"),
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
        if let Some(vol_pipe) = &mut self.volume_raycast_pipeline {
            let forward = (self.camera.target - self.camera.eye).normalize();
            vol_pipe.update_camera(
                &self.gpu.queue,
                self.camera.eye,
                self.camera.is_perspective,
                forward,
            );
        }
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

        // ═══ Full-screen BZ mode — takes over entire viewport ════════════
        if self.show_bz {
            if let Some(bz) = &mut self.bz_viewport {
                let w = self.gpu.config.width as f32;
                let h = self.gpu.config.height as f32;
                let cc = self.clear_color;
                bz.render_fullscreen(
                    &mut encoder,
                    &view,
                    &self.opaque_depth_view,
                    cc,
                    w,
                    h,
                    &self.gpu.queue,
                );
            }
            self.gpu.queue.submit(std::iter::once(encoder.finish()));
            output.present();
            return Ok(());
        }

        if let Some(presentation) = &mut self.phonon_presentation {
            if presentation.dirty || presentation.playback.is_playing() {
                let now = presentation.time_origin.elapsed().as_secs_f64();
                let frame_result = presentation.playback.phase_at(now).and_then(|phase| {
                    apply_phonon_frame(
                        &self.opaque_atom_instances,
                        &self.opaque_source_atom_indices,
                        &presentation.mode_displacements,
                        phase,
                        presentation.display_scale,
                        &mut presentation.opaque_display_instances,
                    )?;
                    apply_phonon_frame(
                        &self.transparent_atom_instances,
                        &self.transparent_source_atom_indices,
                        &presentation.mode_displacements,
                        phase,
                        presentation.display_scale,
                        &mut presentation.transparent_display_instances,
                    )
                });

                match frame_result {
                    Ok(()) => {
                        upload_atom_instances(
                            &self.gpu.queue,
                            &self.instance_buffer,
                            &presentation.opaque_display_instances,
                        );
                        upload_atom_instances(
                            &self.gpu.queue,
                            &self.transparent_instance_buffer,
                            &presentation.transparent_display_instances,
                        );
                    }
                    Err(error) => {
                        log::warn!("phonon presentation frame rejected: {error:?}");
                        presentation.playback.halt();
                    }
                }
                presentation.dirty = false;
            }
        }

        // ═══ Normal crystal rendering path ═══════════════════════════════

        // ═══ Pass 1: Opaque objects — write depth ═════════════════════════
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Opaque Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.opaque_depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Atoms (impostor spheres — opaque, write depth via frag_depth)
            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            if let Some(drag) = &self.atom_drag {
                if let Some(buffer) = &drag.opaque_stationary_buffer {
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..6, 0..drag.opaque_stationary_count);
                }
                if let Some(buffer) = &drag.opaque_preview_buffer {
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..6, 0..drag.opaque_preview_count);
                }
            } else if self.instance_count > 0 {
                pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
                pass.draw(0..6, 0..self.instance_count);
            }

            // Cell box lines
            if self.show_cell && self.cell_line_count > 0 {
                pass.set_pipeline(&self.line_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.cell_line_buffer.slice(..));
                pass.draw(0..self.cell_line_count, 0..1);
            }

            if self.measurement_line_count > 0 {
                pass.set_pipeline(&self.line_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.measurement_line_buffer.slice(..));
                pass.draw(0..self.measurement_line_count, 0..1);
            }

            // Bond cylinders
            if self.show_bonds && self.bond_instance_count > 0 {
                pass.set_pipeline(&self.bond_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.bond_instance_buffer.slice(..));
                pass.draw(0..72, 0..self.bond_instance_count);
            }

            // Hopping cylinders
            if self.show_hoppings && self.hopping_instance_count > 0 {
                pass.set_pipeline(&self.bond_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.hopping_instance_buffer.slice(..));
                pass.draw(0..72, 0..self.hopping_instance_count);
            }
        }

        // ═══ Depth copy: opaque → transparent (for Pass 2 depth test) ════
        let needs_transparent_pass = (self.show_volume
            && (self.volume_render_mode == RendererVolumeMode::Volume
                || self.volume_render_mode == RendererVolumeMode::Both))
            || (self.show_isosurface
                && (self.volume_render_mode == RendererVolumeMode::Isosurface
                    || self.volume_render_mode == RendererVolumeMode::Both))
            || (self.atom_drag.is_none() && self.transparent_instance_count > 0)
            || self.atom_drag.as_ref().is_some_and(|drag| {
                drag.transparent_stationary_count > 0 || drag.transparent_preview_count > 0
            });

        if needs_transparent_pass {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.opaque_depth_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &self.transparent_depth_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: self.gpu.config.width,
                    height: self.gpu.config.height,
                    depth_or_array_layers: 1,
                },
            );

            // ═══ Pass 2: Transparent objects — depth read-only ═══════════
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Transparent Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load, // preserve opaque colors
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.transparent_depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load, // preserve opaque depth
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                // Volume raycast FIRST (inner fill), then Isosurface (outer skin)
                if self.show_volume
                    && (self.volume_render_mode == RendererVolumeMode::Volume
                        || self.volume_render_mode == RendererVolumeMode::Both)
                {
                    if let Some(vol_pipe) = &self.volume_raycast_pipeline {
                        vol_pipe.render(&mut pass, &self.camera_bind_group);
                    }
                }

                // Isosurface (semi-transparent outer envelope)
                if self.show_isosurface
                    && (self.volume_render_mode == RendererVolumeMode::Isosurface
                        || self.volume_render_mode == RendererVolumeMode::Both)
                {
                    if let Some(iso_pipe) = &self.isosurface_pipeline {
                        iso_pipe.draw(&mut pass, &self.camera_bind_group);
                    }
                }

                // Translucent atoms last
                if let Some(drag) = &self.atom_drag {
                    if let Some(buffer) = &drag.transparent_stationary_buffer {
                        pass.set_pipeline(&self.transparent_pipeline);
                        pass.set_bind_group(0, &self.camera_bind_group, &[]);
                        pass.set_vertex_buffer(0, buffer.slice(..));
                        pass.draw(0..6, 0..drag.transparent_stationary_count);
                    }
                    if let Some(buffer) = &drag.transparent_preview_buffer {
                        pass.set_pipeline(&self.transparent_pipeline);
                        pass.set_bind_group(0, &self.camera_bind_group, &[]);
                        pass.set_vertex_buffer(0, buffer.slice(..));
                        pass.draw(0..6, 0..drag.transparent_preview_count);
                    }
                } else if self.transparent_instance_count > 0 {
                    pass.set_pipeline(&self.transparent_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_vertex_buffer(0, self.transparent_instance_buffer.slice(..));
                    pass.draw(0..6, 0..self.transparent_instance_count);
                }
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

        // Create off-screen depth textures (dual-pass)
        let (offscreen_opaque_depth, offscreen_opaque_depth_view) =
            pipeline::create_depth_texture(&self.gpu.device, width, height);
        let (offscreen_transparent_depth, offscreen_transparent_depth_view) =
            pipeline::create_transparent_depth_texture(&self.gpu.device, width, height);

        // Temporarily rebind volume pipeline depth for offscreen size
        if let Some(vol_pipe) = &mut self.volume_raycast_pipeline {
            vol_pipe.update_depth_view(&self.gpu.device, &offscreen_opaque_depth_view);
        }

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Offscreen Render Encoder"),
            });

        // ═══ Offscreen Pass 1: Opaque ═══
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Offscreen Opaque Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &offscreen_opaque_depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            if self.instance_count > 0 {
                pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
                pass.draw(0..6, 0..self.instance_count);
            }

            if self.show_cell && self.cell_line_count > 0 {
                pass.set_pipeline(&self.line_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.cell_line_buffer.slice(..));
                pass.draw(0..self.cell_line_count, 0..1);
            }

            if self.measurement_line_count > 0 {
                pass.set_pipeline(&self.line_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.measurement_line_buffer.slice(..));
                pass.draw(0..self.measurement_line_count, 0..1);
            }

            if self.show_bonds && self.bond_instance_count > 0 {
                pass.set_pipeline(&self.bond_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.bond_instance_buffer.slice(..));
                pass.draw(0..72, 0..self.bond_instance_count);
            }

            if self.show_hoppings && self.hopping_instance_count > 0 {
                pass.set_pipeline(&self.bond_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.hopping_instance_buffer.slice(..));
                pass.draw(0..72, 0..self.hopping_instance_count);
            }
        }

        // ═══ Offscreen Pass 2: Transparent ═══
        let needs_transparent = (self.show_volume
            && (self.volume_render_mode == RendererVolumeMode::Volume
                || self.volume_render_mode == RendererVolumeMode::Both))
            || (self.show_isosurface
                && (self.volume_render_mode == RendererVolumeMode::Isosurface
                    || self.volume_render_mode == RendererVolumeMode::Both))
            || self.transparent_instance_count > 0;

        if needs_transparent {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &offscreen_opaque_depth,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &offscreen_transparent_depth,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Offscreen Transparent Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &color_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &offscreen_transparent_depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                if self.show_volume
                    && (self.volume_render_mode == RendererVolumeMode::Volume
                        || self.volume_render_mode == RendererVolumeMode::Both)
                {
                    if let Some(vol_pipe) = &self.volume_raycast_pipeline {
                        vol_pipe.render(&mut pass, &self.camera_bind_group);
                    }
                }

                if self.show_isosurface
                    && (self.volume_render_mode == RendererVolumeMode::Isosurface
                        || self.volume_render_mode == RendererVolumeMode::Both)
                {
                    if let Some(iso_pipe) = &self.isosurface_pipeline {
                        iso_pipe.draw(&mut pass, &self.camera_bind_group);
                    }
                }

                if self.transparent_instance_count > 0 {
                    pass.set_pipeline(&self.transparent_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_vertex_buffer(0, self.transparent_instance_buffer.slice(..));
                    pass.draw(0..6, 0..self.transparent_instance_count);
                }
            }
        }

        // Restore volume pipeline depth binding to on-screen size
        if let Some(vol_pipe) = &mut self.volume_raycast_pipeline {
            vol_pipe.update_depth_view(&self.gpu.device, &self.opaque_depth_view);
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

    /// Clear volumetric pipelines when switching to a non-volumetric file.
    pub fn clear_volumetric(&mut self) {
        self.isosurface_pipeline = None;
        self.volume_raycast_pipeline = None;
        self.show_isosurface = false;
        self.show_volume = false;
        self.volume_render_mode = RendererVolumeMode::Isosurface;
    }

    pub fn clear_structure_bound_overlays(&mut self) {
        self.clear_volumetric();
        self.update_hoppings(&[]);
        self.show_hoppings = false;
        self.bz_viewport = None;
        self.show_bz = false;
    }

    /// Toggle bond display.
    pub fn toggle_bonds(&mut self, show: bool) {
        self.show_bonds = show;
    }

    /// Update Brillouin Zone data and trigger refresh of the PiP viewport buffers.
    pub fn update_bz_data(
        &mut self,
        bz_opt: Option<(&crate::brillouin_zone::BrillouinZone, &crate::kpath::KPath)>,
    ) {
        if let Some((bz, kpath)) = bz_opt {
            if self.bz_viewport.is_none() {
                self.bz_viewport = Some(crate::renderer::bz_renderer::BzSubViewport::new(
                    &self.gpu, 400, 400,
                ));
            }
            if let Some(viewport) = &mut self.bz_viewport {
                viewport.update_bz(&self.gpu, bz, kpath);
                self.show_bz = true;
            }
        } else {
            self.show_bz = false;
        }
    }

    pub fn prepare_volumetric(
        &self,
        vol: &crate::volumetric::VolumetricData,
    ) -> Result<PreparedVolumetric, ()> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let isosurface_pipeline = if self.gpu.render_config.supports_compute_shaders {
                Some(crate::renderer::isosurface::IsosurfacePipeline::new(
                    &self.gpu.device,
                    &self.gpu.queue,
                    self.gpu.surface_format(),
                    &self.camera_bind_group_layout,
                    vol,
                ))
            } else {
                log::warn!(
                    "Compute shaders not supported! GPU Marching Cubes cannot run on this device."
                );
                None
            };
            let volume_raycast_pipeline =
                crate::renderer::volume_raycast::VolumeRaycastPipeline::new(
                &self.gpu.device,
                self.gpu.surface_format(),
                &self.camera_bind_group_layout,
                vol,
                &self.opaque_depth_view,
            );
            PreparedVolumetric {
                isosurface_pipeline,
                volume_raycast_pipeline,
            }
        }))
        .map_err(|_| ())
    }

    pub fn commit_volumetric(&mut self, prepared: PreparedVolumetric) {
        self.show_isosurface = prepared.isosurface_pipeline.is_some();
        self.isosurface_pipeline = prepared.isosurface_pipeline;
        self.volume_raycast_pipeline = Some(prepared.volume_raycast_pipeline);
        self.show_volume = true;
        self.volume_render_mode = RendererVolumeMode::Both;
    }

    /// Update isovalue threshold and trigger compute pass.
    pub fn update_isovalue(&mut self, grid_dims: [usize; 3], threshold: f32) {
        if let Some(iso_pipe) = &mut self.isosurface_pipeline {
            self.isosurface_dispatch_size =
                iso_pipe.update_threshold(&self.gpu.queue, grid_dims, threshold);

            // Dispatch compute pass immediately to update the mesh buffers
            let mut encoder =
                self.gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Isosurface Compute Encoder"),
            });
            iso_pipe.dispatch_compute(&mut encoder, self.isosurface_dispatch_size);
            self.gpu.queue.submit(std::iter::once(encoder.finish()));
        }
    }

    /// Update isosurface solid color.
    pub fn set_isosurface_color(&mut self, color: [f32; 4]) {
        if let Some(iso_pipe) = &mut self.isosurface_pipeline {
            iso_pipe.set_color(&self.gpu.queue, color);
        }
    }

    /// Update isosurface opacity.
    pub fn set_isosurface_opacity(&mut self, opacity: f32) {
        if let Some(iso_pipe) = &mut self.isosurface_pipeline {
            iso_pipe.set_opacity(&self.gpu.queue, opacity);
        }
    }
}
