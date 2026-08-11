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
    AtomInstance, LineVertex, PreparedAtomScene, RenderLineScene, apply_phonon_frame,
    validate_phonon_display_envelope,
};
use super::pipeline;
use super::publication_look::{
    PublicationCellLineBackground, PublicationCellLineStyle, PublicationLookProfile,
    PublicationLookUniform,
};
use crate::renderer::field_scene::{
    FieldRenderSnapshot, FieldRepresentation, MAX_VISIBLE_FIELD_LAYERS_FIGURE_2,
};
// FIELD-1's legacy one-visible-layer cap remains documented for migration; the
// FIGURE-2 renderer enforces its own bounded multi-layer capacity.
use crate::volumetric::MAX_VISIBLE_FIELD_LAYERS_FIELD_1;

const _FIELD_1_LEGACY_VISIBLE_CAP: usize = MAX_VISIBLE_FIELD_LAYERS_FIELD_1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererVolumeMode {
    Isosurface,
    Volume,
    Both,
}

fn field_representations(
    settings: crate::volumetric::FieldRenderSettings,
    slices: &[crate::renderer::field_scene::FieldSliceRequest],
) -> Vec<FieldRepresentation> {
    let mut representations = Vec::with_capacity(5);
    if matches!(
        settings.render_mode,
        crate::volumetric::FieldRenderMode::Isosurface | crate::volumetric::FieldRenderMode::Both
    ) {
        if !matches!(
            settings.sign_mode,
            crate::volumetric::FieldSignMode::Negative
        ) {
            representations.push(FieldRepresentation::PositiveIsosurface);
        }
        if !matches!(
            settings.sign_mode,
            crate::volumetric::FieldSignMode::Positive
        ) {
            representations.push(FieldRepresentation::NegativeIsosurface);
        }
    }
    if matches!(
        settings.render_mode,
        crate::volumetric::FieldRenderMode::Volume | crate::volumetric::FieldRenderMode::Both
    ) {
        representations.push(FieldRepresentation::VolumeRaycast);
    }
    if !slices.is_empty() {
        representations.push(FieldRepresentation::Slice);
        if slices.iter().any(|slice| !slice.contour_levels.is_empty()) {
            representations.push(FieldRepresentation::Contour);
        }
    }
    representations
}

fn field_scene_hash(snapshots: &[FieldRenderSnapshot]) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    for snapshot in snapshots {
        digest.update(snapshot.layer_id.to_le_bytes());
        digest.update(snapshot.source_layer_revision.to_le_bytes());
        digest.update(serde_json::to_vec(snapshot).unwrap_or_default());
    }
    format!("{:x}", digest.finalize())
}

fn add_field_resource_footprint(
    total: FieldResourceFootprint,
    next: FieldResourceFootprint,
) -> Option<FieldResourceFootprint> {
    Some(FieldResourceFootprint {
        resident_field_bytes: total
            .resident_field_bytes
            .checked_add(next.resident_field_bytes)?,
        isosurface_vertex_bytes: total
            .isosurface_vertex_bytes
            .checked_add(next.isosurface_vertex_bytes)?,
        volume_storage_bytes: total
            .volume_storage_bytes
            .checked_add(next.volume_storage_bytes)?,
        slice_vertex_bytes: total
            .slice_vertex_bytes
            .checked_add(next.slice_vertex_bytes)?,
        contour_vertex_bytes: total
            .contour_vertex_bytes
            .checked_add(next.contour_vertex_bytes)?,
    })
}

fn draw_frozen_field_resources<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    camera_bind_group: &'a wgpu::BindGroup,
    settings: crate::volumetric::FieldRenderSettings,
    volume: &'a crate::renderer::volume_raycast::VolumeRaycastPipeline,
    positive_isosurface: Option<&'a crate::renderer::isosurface::IsosurfacePipeline>,
    negative_isosurface: Option<&'a crate::renderer::isosurface::IsosurfacePipeline>,
    slices: &'a [crate::renderer::field_slice::FieldSliceGpuResource],
    slice_pipeline: &'a wgpu::RenderPipeline,
    line_pipeline: &'a wgpu::RenderPipeline,
) {
    if !settings.visible {
        return;
    }
    if matches!(
        settings.render_mode,
        crate::volumetric::FieldRenderMode::Volume | crate::volumetric::FieldRenderMode::Both
    ) {
        volume.render(pass, camera_bind_group);
    }
    if matches!(
        settings.render_mode,
        crate::volumetric::FieldRenderMode::Isosurface | crate::volumetric::FieldRenderMode::Both
    ) {
        if !matches!(
            settings.sign_mode,
            crate::volumetric::FieldSignMode::Negative
        ) && let Some(iso) = positive_isosurface
        {
            iso.draw(pass, camera_bind_group);
        }
        if !matches!(
            settings.sign_mode,
            crate::volumetric::FieldSignMode::Positive
        ) && let Some(iso) = negative_isosurface
        {
            iso.draw(pass, camera_bind_group);
        }
    }
    for slice in slices {
        if let Some(buffer) = &slice.slice_buffer {
            pass.set_pipeline(slice_pipeline);
            pass.set_bind_group(0, camera_bind_group, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..slice.slice_vertex_count, 0..1);
        }
        if let Some(buffer) = &slice.contour_buffer {
            pass.set_pipeline(line_pipeline);
            pass.set_bind_group(0, camera_bind_group, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..slice.contour_vertex_count, 0..1);
        }
    }
}

struct PublicationFieldLayerPipelines {
    layer_id: crate::volumetric::FieldLayerId,
    positive: Option<wgpu::RenderPipeline>,
    negative: Option<wgpu::RenderPipeline>,
    volume: Option<wgpu::RenderPipeline>,
    slice: wgpu::RenderPipeline,
}

struct PublicationVolumeBinding {
    layer_id: crate::volumetric::FieldLayerId,
    bind_group: wgpu::BindGroup,
}

fn draw_publication_field_layer<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    camera_bind_group: &'a wgpu::BindGroup,
    field_pipelines: &'a [PublicationFieldLayerPipelines],
    volume_bindings: &'a [PublicationVolumeBinding],
    line_pipeline: &'a wgpu::RenderPipeline,
    layer_id: crate::volumetric::FieldLayerId,
    settings: crate::volumetric::FieldRenderSettings,
    volume: Option<&'a crate::renderer::volume_raycast::VolumeRaycastPipeline>,
    positive_isosurface: Option<&'a crate::renderer::isosurface::IsosurfacePipeline>,
    negative_isosurface: Option<&'a crate::renderer::isosurface::IsosurfacePipeline>,
    slices: &'a [crate::renderer::field_slice::FieldSliceGpuResource],
) {
    if !settings.visible {
        return;
    }
    let Some(pipelines) = field_pipelines
        .iter()
        .find(|pipelines| pipelines.layer_id == layer_id)
    else {
        return;
    };
    if matches!(
        settings.render_mode,
        crate::volumetric::FieldRenderMode::Volume | crate::volumetric::FieldRenderMode::Both
    ) {
        if let (Some(volume), Some(binding), Some(render_pipeline)) = (
            volume,
            volume_bindings
                .iter()
                .find(|binding| binding.layer_id == layer_id),
            pipelines.volume.as_ref(),
        ) {
            volume.render_with_bind_group(
                render_pipeline,
                pass,
                camera_bind_group,
                &binding.bind_group,
            );
        }
    }
    if matches!(
        settings.render_mode,
        crate::volumetric::FieldRenderMode::Isosurface | crate::volumetric::FieldRenderMode::Both
    ) {
        if !matches!(
            settings.sign_mode,
            crate::volumetric::FieldSignMode::Negative
        ) && let (Some(iso), Some(render_pipeline)) = (positive_isosurface, &pipelines.positive)
        {
            iso.draw_with_pipeline(render_pipeline, pass, camera_bind_group);
        }
        if !matches!(
            settings.sign_mode,
            crate::volumetric::FieldSignMode::Positive
        ) && let (Some(iso), Some(render_pipeline)) = (negative_isosurface, &pipelines.negative)
        {
            iso.draw_with_pipeline(render_pipeline, pass, camera_bind_group);
        }
    }
    for slice in slices {
        if let Some(buffer) = &slice.slice_buffer {
            pass.set_pipeline(&pipelines.slice);
            pass.set_bind_group(0, camera_bind_group, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..slice.slice_vertex_count, 0..1);
        }
        if let Some(buffer) = &slice.contour_buffer {
            pass.set_pipeline(line_pipeline);
            pass.set_bind_group(0, camera_bind_group, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..slice.contour_vertex_count, 0..1);
        }
    }
}

fn draw_publication_retained_field_layer<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    camera_bind_group: &'a wgpu::BindGroup,
    field_pipelines: &'a [PublicationFieldLayerPipelines],
    volume_bindings: &'a [PublicationVolumeBinding],
    line_pipeline: &'a wgpu::RenderPipeline,
    layer: &'a RetainedFieldLayer,
) {
    draw_publication_field_layer(
        pass,
        camera_bind_group,
        field_pipelines,
        volume_bindings,
        line_pipeline,
        layer.layer_id,
        layer.render_settings,
        Some(&layer.volume_raycast_pipeline),
        layer.isosurface_pipeline.as_ref(),
        layer.negative_isosurface_pipeline.as_ref(),
        &layer.slices,
    );
}

fn draw_publication_fields<'a>(
    renderer: &'a Renderer,
    pass: &mut wgpu::RenderPass<'a>,
    camera_bind_group: &'a wgpu::BindGroup,
    field_pipelines: &'a [PublicationFieldLayerPipelines],
    volume_bindings: &'a [PublicationVolumeBinding],
    line_pipeline: &'a wgpu::RenderPipeline,
    draw_list: &'a FrozenFieldDrawList,
) {
    for layer_id in draw_list.iter() {
        if renderer
            .active_field_layer
            .is_some_and(|(active_id, _)| active_id == *layer_id)
        {
            if let Some(settings) = renderer.active_field_render_settings {
                draw_publication_field_layer(
                    pass,
                    camera_bind_group,
                    field_pipelines,
                    volume_bindings,
                    line_pipeline,
                    *layer_id,
                    settings,
                    renderer.volume_raycast_pipeline.as_ref(),
                    renderer.active_field_layer_pipeline.as_ref(),
                    renderer.active_negative_field_layer_pipeline.as_ref(),
                    &renderer.active_field_slices,
                );
            }
        } else if let Some(layer) = renderer
            .retained_field_layers
            .iter()
            .find(|layer| layer.layer_id == *layer_id)
        {
            draw_publication_retained_field_layer(
                pass,
                camera_bind_group,
                field_pipelines,
                volume_bindings,
                line_pipeline,
                layer,
            );
        }
    }
}

pub struct PreparedFieldLayer {
    layer_id: crate::volumetric::FieldLayerId,
    layer_revision: crate::volumetric::FieldSceneRevision,
    renderer_field_epoch: u64,
    render_settings: crate::volumetric::FieldRenderSettings,
    field_snapshot: FieldRenderSnapshot,
    grid_dims: [usize; 3],
    gpu_bytes: u64,
    resource_footprint: FieldResourceFootprint,
    isosurface_pipeline: Option<crate::renderer::isosurface::IsosurfacePipeline>,
    negative_isosurface_pipeline: Option<crate::renderer::isosurface::IsosurfacePipeline>,
    volume_raycast_pipeline: crate::renderer::volume_raycast::VolumeRaycastPipeline,
    slices: Vec<crate::renderer::field_slice::FieldSliceGpuResource>,
}

struct RetainedFieldLayer {
    layer_id: crate::volumetric::FieldLayerId,
    render_settings: crate::volumetric::FieldRenderSettings,
    field_snapshot: FieldRenderSnapshot,
    gpu_bytes: u64,
    resource_footprint: FieldResourceFootprint,
    isosurface_pipeline: Option<crate::renderer::isosurface::IsosurfacePipeline>,
    negative_isosurface_pipeline: Option<crate::renderer::isosurface::IsosurfacePipeline>,
    volume_raycast_pipeline: crate::renderer::volume_raycast::VolumeRaycastPipeline,
    slices: Vec<crate::renderer::field_slice::FieldSliceGpuResource>,
}

/// Immutable ordered field resources shared by interactive and publication draws.
#[derive(Clone, Debug)]
pub struct FrozenFieldDrawList {
    layer_ids: [crate::volumetric::FieldLayerId; MAX_VISIBLE_FIELD_LAYERS_FIGURE_2],
    len: usize,
}

impl FrozenFieldDrawList {
    fn empty() -> Self {
        Self {
            layer_ids: [0; MAX_VISIBLE_FIELD_LAYERS_FIGURE_2],
            len: 0,
        }
    }

    fn push(&mut self, layer_id: crate::volumetric::FieldLayerId) -> bool {
        if self.len >= self.layer_ids.len() {
            return false;
        }
        self.layer_ids[self.len] = layer_id;
        self.len += 1;
        true
    }

    fn sort(&mut self) {
        self.layer_ids[..self.len].sort_unstable();
    }

    fn iter(&self) -> impl Iterator<Item = &crate::volumetric::FieldLayerId> {
        self.layer_ids[..self.len].iter()
    }
}

/// Immutable FIGURE-2 field state captured before publication rasterization.
/// It deliberately contains no interactive renderer handles or mutable structure state.
#[derive(Clone, Debug)]
pub struct FieldPublicationSnapshot {
    pub field_snapshots: Vec<FieldRenderSnapshot>,
    pub draw_list: FrozenFieldDrawList,
    pub representation: Vec<FieldRepresentation>,
    pub transfer_function: crate::renderer::field_scene::FieldTransferFunction,
    pub clip_planes: Vec<crate::renderer::field_scene::FieldClipPlane>,
    pub composition_method: crate::renderer::field_scene::FieldTransparencyMethod,
    pub requested_field_capabilities: u32,
    pub selected_field_capabilities: u32,
    pub field_scene_hash: String,
    pub resource_footprint: FieldResourceFootprint,
}

/// The actual resident GPU allocation categorized by field representation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FieldResourceFootprint {
    pub resident_field_bytes: u64,
    pub isosurface_vertex_bytes: u64,
    pub volume_storage_bytes: u64,
    pub slice_vertex_bytes: u64,
    pub contour_vertex_bytes: u64,
}

pub type PreparedVolumetric = PreparedFieldLayer;

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
    field_slice_pipeline: wgpu::RenderPipeline,
    cell_line_buffer: wgpu::Buffer,
    cell_line_count: u32,
    cell_line_vertices: Vec<LineVertex>,

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
    pub active_field_layer_pipeline: Option<crate::renderer::isosurface::IsosurfacePipeline>,
    active_negative_field_layer_pipeline: Option<crate::renderer::isosurface::IsosurfacePipeline>,
    pub active_field_layer: Option<(
        crate::volumetric::FieldLayerId,
        crate::volumetric::FieldSceneRevision,
    )>,
    active_field_snapshot: Option<FieldRenderSnapshot>,
    active_field_render_settings: Option<crate::volumetric::FieldRenderSettings>,
    active_field_slices: Vec<crate::renderer::field_slice::FieldSliceGpuResource>,
    retained_field_layers: Vec<RetainedFieldLayer>,
    /// Changes whenever a prepared field resource can no longer be committed.
    field_resource_epoch: u64,
    active_field_gpu_bytes: u64,
    active_field_resource_footprint: FieldResourceFootprint,
    resident_field_gpu_bytes: u64,
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

const MAX_PUBLICATION_READBACK_BYTES: u64 = 192 * 1024 * 1024;
const MAX_PUBLICATION_TOTAL_GPU_BYTES: u64 = 384 * 1024 * 1024;
const MAX_PUBLICATION_PEAK_CPU_BYTES: u64 = 384 * 1024 * 1024;
const PUBLICATION_EXPORT_CAMERA_UNIFORM_BYTES: u64 = 256;
const PUBLICATION_LOOK_UNIFORM_BYTES: u64 = 256;
const PUBLICATION_EXPORT_GPU_DRIVER_RESERVE_BYTES: u64 = 16 * 1024 * 1024;
const PUBLICATION_EXPORT_CPU_ENCODER_RESERVE_BYTES: u64 = 16 * 1024 * 1024;
const PUBLICATION_EXPORT_ENCODED_OVERHEAD_BYTES: u64 = 1024 * 1024;
const MAX_ACTIVE_FIELD_GPU_BYTES: u64 = 128 * 1024 * 1024;
const MAX_FIELD_ISOSURFACE_VERTEX_BYTES: u64 = 96 * 1024 * 1024;
pub(crate) const MAX_PUBLICATION_RECIPE_BYTES: u64 = 1024 * 1024;
pub const PUBLICATION_EXPORT_POLICY_VERSION: u32 = 7;
const PUBLICATION_FRAMING_MARGIN: f32 = 0.08;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OffscreenReadbackLayout {
    unpadded_bytes_per_row: u32,
    padded_bytes_per_row: u32,
    staging_size: u64,
    rgba_len: usize,
}

/// Background policy is resolved at the command boundary before GPU allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationBackground {
    Transparent,
    White,
    Black,
    Current,
}

pub(crate) fn cell_line_style_for_background(
    background: PublicationBackground,
    current_background: wgpu::Color,
) -> Result<PublicationCellLineStyle, String> {
    let background = match background {
        PublicationBackground::White => PublicationCellLineBackground::White,
        PublicationBackground::Black => PublicationCellLineBackground::Black,
        PublicationBackground::Transparent => PublicationCellLineBackground::Transparent,
        PublicationBackground::Current => {
            let channels = [
                current_background.r,
                current_background.g,
                current_background.b,
                current_background.a,
            ];
            if !channels
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            {
                return Err("publication current background is invalid".to_owned());
            }
            let luminance = 0.2126 * current_background.r
                + 0.7152 * current_background.g
                + 0.0722 * current_background.b;
            if luminance >= 0.5 {
                PublicationCellLineBackground::White
            } else {
                PublicationCellLineBackground::Black
            }
        }
    };
    Ok(PublicationCellLineStyle::for_background(background))
}

fn publication_srgb_rgba_to_linear(color: [f32; 4]) -> [f32; 4] {
    let convert = |value: f32| {
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    [
        convert(color[0]),
        convert(color[1]),
        convert(color[2]),
        color[3],
    ]
}

fn fit_visible_structure_to_export(
    mut camera: Camera,
    width: u32,
    height: u32,
    opaque_atoms: &[AtomInstance],
    transparent_atoms: &[AtomInstance],
    cell_lines: &[LineVertex],
    bonds: &[crate::renderer::instance::BondInstance],
) -> Result<Camera, String> {
    if width == 0 || height == 0 {
        return Err("publication framing dimensions must be non-zero".to_owned());
    }
    let forward = (camera.target - camera.eye).normalize_or_zero();
    let right = forward.cross(camera.up).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    if forward.length_squared() <= f32::EPSILON
        || right.length_squared() <= f32::EPSILON
        || up.length_squared() <= f32::EPSILON
    {
        return Err("publication camera basis is degenerate".to_owned());
    }
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    let mut include_sphere = |position: [f32; 3], radius: f32| -> Result<(), String> {
        let position = glam::Vec3::from_array(position);
        if !position.is_finite() || !radius.is_finite() || radius < 0.0 {
            return Err("publication framing structure bounds are invalid".to_owned());
        }
        let relative = position - camera.target;
        let projected =
            glam::Vec3::new(relative.dot(right), relative.dot(up), relative.dot(forward));
        let radius = glam::Vec3::splat(radius);
        min = min.min(projected - radius);
        max = max.max(projected + radius);
        Ok(())
    };
    for atom in opaque_atoms.iter().chain(transparent_atoms) {
        if atom.radius <= 0.0 {
            return Err("publication framing atom bounds are invalid".to_owned());
        }
        include_sphere(atom.position, atom.radius)?;
    }
    for line in cell_lines {
        include_sphere(line.position, 0.0)?;
    }
    for bond in bonds {
        include_sphere(bond.start, bond.radius)?;
        include_sphere(bond.end, bond.radius)?;
    }
    if !min.is_finite() || !max.is_finite() || min.x > max.x || min.y > max.y || min.z > max.z {
        return Err("publication framing has no visible atom bounds".to_owned());
    }
    let center = (min + max) * 0.5;
    let structure_translation = right * center.x + up * center.y + forward * center.z;
    camera.eye += structure_translation;
    camera.target += structure_translation;
    let span = max - min;
    let aspect = width as f32 / height as f32;
    let inner = 1.0 - 2.0 * PUBLICATION_FRAMING_MARGIN;
    if !aspect.is_finite() || aspect <= 0.0 || inner <= 0.0 {
        return Err("publication framing aspect is invalid".to_owned());
    }
    camera.set_aspect(width as f32, height as f32);
    let half_depth = span.z * 0.5;
    camera.znear = 0.01;
    if camera.is_perspective {
        let half_vertical = (camera.fovy_deg.to_radians() * 0.5).tan() * inner;
        let half_horizontal = half_vertical * aspect;
        if !half_vertical.is_finite()
            || !half_horizontal.is_finite()
            || half_vertical <= 0.0
            || half_horizontal <= 0.0
        {
            return Err("publication perspective framing is invalid".to_owned());
        }
        let required_distance = (span.x * 0.5 / half_horizontal + half_depth)
            .max(span.y * 0.5 / half_vertical + half_depth)
            .max(camera.znear + half_depth + 1.0e-3);
        camera.eye = camera.target - forward * required_distance;
        camera.zfar = (required_distance + half_depth + 1.0).max(10.0);
    } else {
        camera.orthographic_scale = (span.y / inner).max(span.x / (aspect * inner)).max(1.0e-3);
        let current_distance = (camera.target - camera.eye).length();
        let required_distance = current_distance.max(camera.znear + half_depth + 1.0e-3);
        camera.eye = camera.target - forward * required_distance;
        camera.zfar = (required_distance + half_depth + 1.0).max(10.0);
    }
    Ok(camera)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PublicationAlphaMode {
    Premultiplied,
}

/// Immutable export-only state. It holds no physical scene data and never
/// aliases mutable interactive camera or presentation state.
pub(crate) struct PublicationRenderConfig {
    width: u32,
    height: u32,
    camera: Camera,
    background: wgpu::Color,
    alpha_mode: PublicationAlphaMode,
    target_format: wgpu::TextureFormat,
    requested_samples: u32,
    selected_samples: u32,
    tile_dimensions: [u32; 2],
    tile_layout: [u32; 2],
    look_profile: PublicationLookProfile,
    cell_line_style: PublicationCellLineStyle,
    publication_bond_instances: Vec<crate::renderer::instance::BondInstance>,
    readback_layout: OffscreenReadbackLayout,
    admission: PublicationExportAdmissionReceipt,
    field_snapshot: Option<FieldPublicationSnapshot>,
}

/// Per-export pipelines. They are deliberately local to one export and reused
/// by every tile; this is not a cross-export pipeline cache.
struct PublicationPipelines {
    render: wgpu::RenderPipeline,
    transparent: wgpu::RenderPipeline,
    line: wgpu::RenderPipeline,
    bond: wgpu::RenderPipeline,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    look_bind_group_layout: wgpu::BindGroupLayout,
}

impl PublicationPipelines {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat, sample_count: u32) -> Self {
        let pipelines = pipeline::create_publication_pipelines(device, target_format, sample_count);
        Self {
            render: pipelines.render,
            transparent: pipelines.transparent,
            line: pipelines.line,
            bond: pipelines.bond,
            camera_bind_group_layout: pipelines.camera_bind_group_layout,
            look_bind_group_layout: pipelines.look_bind_group_layout,
        }
    }
}

/// Readback pixels with the semantics required by the publication encoder.
pub(crate) struct PublicationRenderResult {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    alpha_mode: PublicationAlphaMode,
}

impl PublicationRenderResult {
    pub(crate) fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }

    pub(crate) fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub(crate) fn is_premultiplied_alpha(&self) -> bool {
        self.alpha_mode == PublicationAlphaMode::Premultiplied
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PublicationExportRequest {
    pub width: u32,
    pub height: u32,
    pub publication_bond_instance_count: u32,
    pub needs_transparent_depth: bool,
    pub has_measurement_overlays: bool,
    pub has_hopping_overlays: bool,
    pub has_isosurface: bool,
    pub has_volume: bool,
    pub has_phonon_presentation: bool,
    pub has_atom_drag: bool,
    pub show_bz: bool,
    pub has_measurement_state: bool,
    pub has_selection_highlights: bool,
    pub has_wannier_overlay: bool,
    pub has_active_phonon_state: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PublicationExportSourceState {
    pub has_measurement_state: bool,
    pub has_selection_highlights: bool,
    pub has_wannier_overlay: bool,
    pub has_active_phonon_state: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PublicationExportLimits {
    pub max_texture_dimension_2d: u32,
    pub max_buffer_size: u64,
    pub publication_msaa_x4: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PublicationRenderPlan {
    pub requested_samples: u32,
    pub selected_samples: u32,
    pub tile_dimensions: [u32; 2],
    pub tile_layout: [u32; 2],
    pub tile_overlap_pixels: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PublicationExportResourceEstimate {
    pub staging_bytes: u64,
    pub rgba_bytes: u64,
    pub tile_rgba_bytes: u64,
    pub resolve_color_bytes: u64,
    pub msaa_color_bytes: u64,
    pub opaque_depth_bytes: u64,
    pub transparent_depth_bytes: u64,
    pub depth_replay_color_bytes: u64,
    pub volume_depth_replay_color_bytes: u64,
    pub volume_depth_replay_depth_bytes: u64,
    pub look_uniform_bytes: u64,
    pub publication_bond_bytes: u64,
    pub jpeg_rgb_bytes: u64,
    pub max_encoded_bytes: u64,
    pub export_camera_uniform_bytes: u64,
    pub gpu_driver_reserve_bytes: u64,
    pub cpu_encoder_reserve_bytes: u64,
    pub transient_gpu_bytes: u64,
    pub readback_peak_cpu_bytes: u64,
    pub png_encode_peak_cpu_bytes: u64,
    pub jpeg_encode_peak_cpu_bytes: u64,
    pub peak_cpu_bytes: u64,
    pub field_accumulation_bytes: u64,
    pub field_revealage_bytes: u64,
    pub field_slice_bytes: u64,
    pub field_contour_bytes: u64,
    pub field_staging_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PublicationExportBudgets {
    pub max_readback_bytes: u64,
    pub max_transient_gpu_bytes: u64,
    pub max_peak_cpu_bytes: u64,
    pub gpu_driver_reserve_bytes: u64,
    pub cpu_encoder_reserve_bytes: u64,
    pub encoded_overhead_bytes: u64,
    pub max_recipe_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PublicationExportAdmissionReceipt {
    pub(crate) policy_version: u32,
    pub(crate) request: PublicationExportRequest,
    pub(crate) limits: PublicationExportLimits,
    pub(crate) budgets: PublicationExportBudgets,
    pub(crate) render_plan: PublicationRenderPlan,
    pub(crate) estimate: PublicationExportResourceEstimate,
    pub(crate) field_layer_count: u8,
    pub(crate) field_has_slice: bool,
    pub(crate) field_has_contour: bool,
    pub(crate) field_admitted: bool,
    pub(crate) field_scene_hash: Option<String>,
    pub(crate) field_resource_footprint: Option<FieldResourceFootprint>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationExportRejection {
    ZeroDimensions,
    MeasurementOverlays,
    HoppingOverlays,
    Isosurface,
    Volume,
    PhononPresentation,
    AtomDrag,
    BrillouinZone,
    MeasurementState,
    SelectionHighlights,
    WannierOverlay,
    ActivePhononState,
    TextureDimensionLimit,
    RowByteOverflow,
    PaddedRowByteOverflow,
    StagingByteOverflow,
    StagingAddressSpace,
    CpuByteOverflow,
    RowLayoutLimit,
    PaddedRowLayoutLimit,
    DeviceBufferLimit,
    ReadbackBudget,
    TransientGpuBudget,
    PeakCpuBudget,
    PolicyVersion,
    ReceiptMismatch,
}

impl std::fmt::Display for PublicationExportRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ZeroDimensions => "publication export dimensions must be non-zero",
            Self::MeasurementOverlays => {
                "publication export currently rejects measurement overlays"
            }
            Self::HoppingOverlays => "publication export currently rejects hopping overlays",
            Self::Isosurface => "publication export currently rejects isosurface rendering",
            Self::Volume => "publication export currently rejects volume rendering",
            Self::PhononPresentation => "publication export currently rejects phonon presentation",
            Self::AtomDrag => "publication export currently rejects atom drag preview",
            Self::BrillouinZone => "publication export currently rejects Brillouin-zone view",
            Self::MeasurementState => "publication export currently rejects measurement state",
            Self::SelectionHighlights => {
                "publication export currently rejects selection highlights"
            }
            Self::WannierOverlay => "publication export currently rejects Wannier overlays",
            Self::ActivePhononState => "publication export currently rejects phonon state",
            Self::TextureDimensionLimit => {
                "publication export dimensions exceed the active device limit"
            }
            Self::RowByteOverflow => "offscreen row byte count overflow",
            Self::PaddedRowByteOverflow => "offscreen padded row byte count overflow",
            Self::StagingByteOverflow => "offscreen staging byte count overflow",
            Self::StagingAddressSpace => "offscreen staging buffer exceeds addressable memory",
            Self::CpuByteOverflow => "offscreen CPU byte count overflow",
            Self::RowLayoutLimit => "offscreen row exceeds wgpu copy layout limits",
            Self::PaddedRowLayoutLimit => "offscreen padded row exceeds wgpu copy layout limits",
            Self::DeviceBufferLimit => {
                "publication export readback exceeds the active device buffer limit"
            }
            Self::ReadbackBudget => "publication export readback exceeds the policy budget",
            Self::TransientGpuBudget => {
                "publication export transient GPU allocation exceeds the policy budget"
            }
            Self::PeakCpuBudget => {
                "publication export peak CPU allocation exceeds the policy budget"
            }
            Self::PolicyVersion => "publication export receipt has an unsupported policy version",
            Self::ReceiptMismatch => {
                "publication export receipt does not match its request and limits"
            }
        })
    }
}

impl std::error::Error for PublicationExportRejection {}

const fn publication_export_budgets() -> PublicationExportBudgets {
    PublicationExportBudgets {
        max_readback_bytes: MAX_PUBLICATION_READBACK_BYTES,
        max_transient_gpu_bytes: MAX_PUBLICATION_TOTAL_GPU_BYTES,
        max_peak_cpu_bytes: MAX_PUBLICATION_PEAK_CPU_BYTES,
        gpu_driver_reserve_bytes: PUBLICATION_EXPORT_GPU_DRIVER_RESERVE_BYTES,
        cpu_encoder_reserve_bytes: PUBLICATION_EXPORT_CPU_ENCODER_RESERVE_BYTES,
        encoded_overhead_bytes: PUBLICATION_EXPORT_ENCODED_OVERHEAD_BYTES,
        max_recipe_bytes: MAX_PUBLICATION_RECIPE_BYTES,
    }
}

fn offscreen_readback_layout(
    width: u32,
    height: u32,
) -> Result<OffscreenReadbackLayout, PublicationExportRejection> {
    let unpadded_bytes_per_row = u64::from(width)
        .checked_mul(4)
        .ok_or(PublicationExportRejection::RowByteOverflow)?;
    let alignment = u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let padded_bytes_per_row = unpadded_bytes_per_row
        .checked_add(alignment - 1)
        .ok_or(PublicationExportRejection::PaddedRowByteOverflow)?
        / alignment
        * alignment;
    let staging_size = padded_bytes_per_row
        .checked_mul(u64::from(height))
        .ok_or(PublicationExportRejection::StagingByteOverflow)?;
    usize::try_from(staging_size).map_err(|_| PublicationExportRejection::StagingAddressSpace)?;
    let rgba_len = unpadded_bytes_per_row
        .checked_mul(u64::from(height))
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(PublicationExportRejection::CpuByteOverflow)?;

    Ok(OffscreenReadbackLayout {
        unpadded_bytes_per_row: u32::try_from(unpadded_bytes_per_row)
            .map_err(|_| PublicationExportRejection::RowLayoutLimit)?,
        padded_bytes_per_row: u32::try_from(padded_bytes_per_row)
            .map_err(|_| PublicationExportRejection::PaddedRowLayoutLimit)?,
        staging_size,
        rgba_len,
    })
}

fn publication_render_plan(
    request: PublicationExportRequest,
    limits: PublicationExportLimits,
    field_resources: Option<&FieldPublicationResources>,
) -> Result<PublicationRenderPlan, PublicationExportRejection> {
    if limits.max_texture_dimension_2d == 0 {
        return Err(PublicationExportRejection::TextureDimensionLimit);
    }
    let selected_samples = if limits.publication_msaa_x4 { 4 } else { 1 };
    let mut tile_dimensions = [
        request.width.min(limits.max_texture_dimension_2d),
        request.height.min(limits.max_texture_dimension_2d),
    ];
    loop {
        let plan = PublicationRenderPlan {
            requested_samples: 4,
            selected_samples,
            tile_layout: [
                request.width.div_ceil(tile_dimensions[0]),
                request.height.div_ceil(tile_dimensions[1]),
            ],
            tile_dimensions,
            tile_overlap_pixels: 0,
        };
        let estimate = publication_export_resource_estimate(request, plan)?;
        let estimate = if let Some(resources) = field_resources {
            add_field_publication_resource_estimate(estimate, resources)?
        } else {
            estimate
        };
        if estimate.transient_gpu_bytes <= publication_export_budgets().max_transient_gpu_bytes {
            return Ok(plan);
        }
        if tile_dimensions == [1, 1] {
            return Err(PublicationExportRejection::TransientGpuBudget);
        }
        let axis = usize::from(tile_dimensions[1] > tile_dimensions[0]);
        tile_dimensions[axis] = tile_dimensions[axis].div_ceil(2).max(1);
    }
}

fn add_field_publication_resource_estimate(
    mut estimate: PublicationExportResourceEstimate,
    resources: &FieldPublicationResources,
) -> Result<PublicationExportResourceEstimate, PublicationExportRejection> {
    let tile_pixels = estimate
        .tile_rgba_bytes
        .checked_div(4)
        .ok_or(PublicationExportRejection::TransientGpuBudget)?;
    estimate.field_slice_bytes = if resources.has_slice {
        tile_pixels
            .checked_mul(8)
            .ok_or(PublicationExportRejection::TransientGpuBudget)?
    } else {
        0
    };
    estimate.field_contour_bytes = if resources.has_contour {
        tile_pixels
            .checked_mul(4)
            .ok_or(PublicationExportRejection::TransientGpuBudget)?
    } else {
        0
    };
    estimate.field_staging_bytes = estimate
        .field_slice_bytes
        .checked_add(estimate.field_contour_bytes)
        .ok_or(PublicationExportRejection::TransientGpuBudget)?;
    estimate.transient_gpu_bytes = estimate
        .transient_gpu_bytes
        .checked_add(resources.footprint.resident_field_bytes)
        .and_then(|value| value.checked_add(estimate.field_slice_bytes))
        .and_then(|value| value.checked_add(estimate.field_contour_bytes))
        .and_then(|value| value.checked_add(estimate.field_staging_bytes))
        .ok_or(PublicationExportRejection::TransientGpuBudget)?;
    Ok(estimate)
}

fn publication_export_resource_estimate(
    request: PublicationExportRequest,
    plan: PublicationRenderPlan,
) -> Result<PublicationExportResourceEstimate, PublicationExportRejection> {
    let layout = offscreen_readback_layout(plan.tile_dimensions[0], plan.tile_dimensions[1])?;
    let full_layout = offscreen_readback_layout(request.width, request.height)?;
    let rgba_bytes = u64::try_from(full_layout.rgba_len)
        .map_err(|_| PublicationExportRejection::CpuByteOverflow)?;
    let tile_rgba_bytes =
        u64::try_from(layout.rgba_len).map_err(|_| PublicationExportRejection::CpuByteOverflow)?;
    let msaa_color_bytes = if plan.selected_samples > 1 {
        tile_rgba_bytes
            .checked_mul(u64::from(plan.selected_samples))
            .ok_or(PublicationExportRejection::TransientGpuBudget)?
    } else {
        0
    };
    let resolve_color_bytes = tile_rgba_bytes;
    let opaque_depth_bytes = tile_rgba_bytes
        .checked_mul(u64::from(plan.selected_samples))
        .ok_or(PublicationExportRejection::TransientGpuBudget)?;
    let transparent_depth_bytes = request
        .needs_transparent_depth
        .then_some(opaque_depth_bytes)
        .unwrap_or(0);
    let depth_replay_color_bytes = (request.needs_transparent_depth && plan.selected_samples > 1)
        .then_some(msaa_color_bytes)
        .unwrap_or(0);
    let needs_volume_depth_replay = request.has_volume && plan.selected_samples > 1;
    let volume_depth_replay_color_bytes = needs_volume_depth_replay
        .then_some(tile_rgba_bytes)
        .unwrap_or(0);
    let volume_depth_replay_depth_bytes = needs_volume_depth_replay
        .then_some(tile_rgba_bytes)
        .unwrap_or(0);
    let publication_bond_bytes = u64::from(request.publication_bond_instance_count)
        .checked_mul(std::mem::size_of::<crate::renderer::instance::BondInstance>() as u64)
        .ok_or(PublicationExportRejection::TransientGpuBudget)?;
    let budgets = publication_export_budgets();
    let max_encoded_bytes = rgba_bytes
        .checked_add(budgets.encoded_overhead_bytes)
        .ok_or(PublicationExportRejection::PeakCpuBudget)?;
    // Field targets are separate from the structure targets. The default legacy request
    // contains no field snapshot, so these are zero until FIGURE-2 admission supplies one.
    let field_accumulation_bytes = 0_u64;
    let field_revealage_bytes = 0_u64;
    let field_slice_bytes = 0_u64;
    let field_contour_bytes = 0_u64;
    let field_staging_bytes = 0_u64;
    let total_gpu_bytes = resolve_color_bytes
        .checked_add(msaa_color_bytes)
        .and_then(|value| value.checked_add(opaque_depth_bytes))
        .and_then(|value| value.checked_add(transparent_depth_bytes))
        .and_then(|value| value.checked_add(depth_replay_color_bytes))
        .and_then(|value| value.checked_add(volume_depth_replay_color_bytes))
        .and_then(|value| value.checked_add(volume_depth_replay_depth_bytes))
        .and_then(|value| value.checked_add(layout.staging_size))
        .and_then(|value| value.checked_add(PUBLICATION_EXPORT_CAMERA_UNIFORM_BYTES))
        .and_then(|value| value.checked_add(PUBLICATION_LOOK_UNIFORM_BYTES))
        .and_then(|value| value.checked_add(publication_bond_bytes))
        .and_then(|value| value.checked_add(field_accumulation_bytes))
        .and_then(|value| value.checked_add(field_revealage_bytes))
        .and_then(|value| value.checked_add(field_slice_bytes))
        .and_then(|value| value.checked_add(field_contour_bytes))
        .and_then(|value| value.checked_add(field_staging_bytes))
        .and_then(|value| value.checked_add(budgets.gpu_driver_reserve_bytes))
        .ok_or(PublicationExportRejection::TransientGpuBudget)?;
    let readback_peak_cpu_bytes = rgba_bytes
        .checked_add(tile_rgba_bytes)
        .and_then(|value| value.checked_add(layout.staging_size))
        .and_then(|value| value.checked_add(publication_bond_bytes))
        .ok_or(PublicationExportRejection::PeakCpuBudget)?;
    let jpeg_rgb_bytes = rgba_bytes
        .checked_div(4)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or(PublicationExportRejection::PeakCpuBudget)?;
    let png_encode_peak_cpu_bytes = rgba_bytes
        .checked_add(publication_bond_bytes)
        .and_then(|value| value.checked_add(budgets.cpu_encoder_reserve_bytes))
        .ok_or(PublicationExportRejection::PeakCpuBudget)?;
    let jpeg_encode_peak_cpu_bytes = rgba_bytes
        .checked_add(jpeg_rgb_bytes)
        .and_then(|value| value.checked_add(publication_bond_bytes))
        .and_then(|value| value.checked_add(budgets.cpu_encoder_reserve_bytes))
        .ok_or(PublicationExportRejection::PeakCpuBudget)?;
    let peak_cpu_bytes = readback_peak_cpu_bytes
        .max(png_encode_peak_cpu_bytes)
        .max(jpeg_encode_peak_cpu_bytes);

    Ok(PublicationExportResourceEstimate {
        staging_bytes: layout.staging_size,
        rgba_bytes,
        tile_rgba_bytes,
        resolve_color_bytes,
        msaa_color_bytes,
        opaque_depth_bytes,
        transparent_depth_bytes,
        depth_replay_color_bytes,
        volume_depth_replay_color_bytes,
        volume_depth_replay_depth_bytes,
        look_uniform_bytes: PUBLICATION_LOOK_UNIFORM_BYTES,
        publication_bond_bytes,
        jpeg_rgb_bytes,
        max_encoded_bytes,
        export_camera_uniform_bytes: PUBLICATION_EXPORT_CAMERA_UNIFORM_BYTES,
        gpu_driver_reserve_bytes: budgets.gpu_driver_reserve_bytes,
        cpu_encoder_reserve_bytes: budgets.cpu_encoder_reserve_bytes,
        transient_gpu_bytes: total_gpu_bytes,
        readback_peak_cpu_bytes,
        png_encode_peak_cpu_bytes,
        jpeg_encode_peak_cpu_bytes,
        peak_cpu_bytes,
        field_accumulation_bytes,
        field_revealage_bytes,
        field_slice_bytes,
        field_contour_bytes,
        field_staging_bytes,
    })
}

fn finish_publication_export_error_scopes(device: &wgpu::Device) -> Result<(), String> {
    device.poll(wgpu::Maintain::Wait);
    let validation_error = pollster::block_on(device.pop_error_scope());
    let out_of_memory_error = pollster::block_on(device.pop_error_scope());
    let internal_error = pollster::block_on(device.pop_error_scope());
    if let Some(error) = validation_error {
        return Err(format!("publication export GPU validation error: {error}"));
    }
    if let Some(error) = out_of_memory_error {
        return Err(format!("publication export GPU allocation error: {error}"));
    }
    if let Some(error) = internal_error {
        return Err(format!("publication export GPU internal error: {error}"));
    }
    Ok(())
}

fn unpack_publication_readback(
    data: &[u8],
    layout: OffscreenReadbackLayout,
    height: u32,
    target_format: wgpu::TextureFormat,
) -> Result<Vec<u8>, String> {
    let expected_staging_bytes = usize::try_from(layout.staging_size)
        .map_err(|_| "offscreen staging buffer exceeds addressable memory".to_owned())?;
    if data.len() != expected_staging_bytes {
        return Err("offscreen readback size does not match its declared layout".to_owned());
    }

    let mut rgba = Vec::new();
    rgba.try_reserve_exact(layout.rgba_len)
        .map_err(|_| "unable to allocate offscreen RGBA output".to_owned())?;
    let is_bgra = matches!(
        target_format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    );

    for row in 0..height {
        let offset = usize::try_from(
            u64::from(row)
                .checked_mul(u64::from(layout.padded_bytes_per_row))
                .ok_or_else(|| "offscreen row offset overflow".to_owned())?,
        )
        .map_err(|_| "offscreen row offset exceeds addressable memory".to_owned())?;
        let row_end = offset
            .checked_add(
                usize::try_from(layout.unpadded_bytes_per_row)
                    .map_err(|_| "offscreen row length exceeds addressable memory".to_owned())?,
            )
            .ok_or_else(|| "offscreen row end overflow".to_owned())?;
        let row_data = data
            .get(offset..row_end)
            .ok_or_else(|| "offscreen row exceeds mapped staging data".to_owned())?;
        if is_bgra {
            // BGRA -> RGBA. The copy layout is already top-to-bottom.
            for pixel in row_data.chunks_exact(4) {
                rgba.push(pixel[2]);
                rgba.push(pixel[1]);
                rgba.push(pixel[0]);
                rgba.push(pixel[3]);
            }
        } else {
            rgba.extend_from_slice(row_data);
        }
    }
    if rgba.len() != layout.rgba_len {
        return Err("offscreen readback produced an unexpected RGBA length".to_owned());
    }
    Ok(rgba)
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
    pub fn with_active_volume_pipeline(
        &mut self,
        update: impl FnOnce(&mut crate::renderer::volume_raycast::VolumeRaycastPipeline, &wgpu::Queue),
    ) {
        let Renderer {
            gpu,
            volume_raycast_pipeline,
            ..
        } = self;
        if let Some(volume) = volume_raycast_pipeline.as_mut() {
            update(volume, &gpu.queue);
        }
    }

    fn publication_field_pipelines(
        &self,
        sample_count: u32,
    ) -> Result<Vec<PublicationFieldLayerPipelines>, String> {
        let count = self
            .retained_field_layers
            .len()
            .checked_add(usize::from(self.active_field_layer.is_some()))
            .ok_or_else(|| "publication field-layer count overflow".to_owned())?;
        let mut pipelines = Vec::new();
        pipelines
            .try_reserve_exact(count)
            .map_err(|_| "unable to allocate publication field pipelines".to_owned())?;
        let create_layer =
            |layer_id,
             positive: Option<&crate::renderer::isosurface::IsosurfacePipeline>,
             negative: Option<&crate::renderer::isosurface::IsosurfacePipeline>,
             volume: Option<&crate::renderer::volume_raycast::VolumeRaycastPipeline>| {
                PublicationFieldLayerPipelines {
                    layer_id,
                    positive: positive.map(|iso| {
                        iso.publication_render_pipeline(
                            &self.gpu.device,
                            self.gpu.surface_format(),
                            &self.camera_bind_group_layout,
                            sample_count,
                        )
                    }),
                    negative: negative.map(|iso| {
                        iso.publication_render_pipeline(
                            &self.gpu.device,
                            self.gpu.surface_format(),
                            &self.camera_bind_group_layout,
                            sample_count,
                        )
                    }),
                    volume: volume.map(|volume| {
                        volume.publication_render_pipeline(
                            &self.gpu.device,
                            self.gpu.surface_format(),
                            &self.camera_bind_group_layout,
                            sample_count,
                        )
                    }),
                    slice: pipeline::create_field_slice_pipeline(
                        &self.gpu.device,
                        self.gpu.surface_format(),
                        &self.camera_bind_group_layout,
                        sample_count,
                    ),
                }
            };
        for layer in &self.retained_field_layers {
            pipelines.push(create_layer(
                layer.layer_id,
                layer.isosurface_pipeline.as_ref(),
                layer.negative_isosurface_pipeline.as_ref(),
                Some(&layer.volume_raycast_pipeline),
            ));
        }
        if let Some((layer_id, _)) = self.active_field_layer {
            pipelines.push(create_layer(
                layer_id,
                self.active_field_layer_pipeline.as_ref(),
                self.active_negative_field_layer_pipeline.as_ref(),
                self.volume_raycast_pipeline.as_ref(),
            ));
        }
        Ok(pipelines)
    }

    fn publication_volume_bindings(
        &self,
        depth_view: &wgpu::TextureView,
        camera: &Camera,
    ) -> Result<Vec<PublicationVolumeBinding>, String> {
        let count = self
            .retained_field_layers
            .len()
            .checked_add(usize::from(self.active_field_layer.is_some()))
            .ok_or_else(|| "publication field-layer count overflow".to_owned())?;
        let mut bindings = Vec::new();
        bindings
            .try_reserve_exact(count)
            .map_err(|_| "unable to allocate publication volume bindings".to_owned())?;
        for layer in &self.retained_field_layers {
            if layer.render_settings.visible
                && matches!(
                    layer.render_settings.render_mode,
                    crate::volumetric::FieldRenderMode::Volume
                        | crate::volumetric::FieldRenderMode::Both
                )
            {
                bindings.push(PublicationVolumeBinding {
                    layer_id: layer.layer_id,
                    bind_group: layer.volume_raycast_pipeline.publication_bind_group(
                        &self.gpu.device,
                        depth_view,
                        camera.eye,
                        camera.is_perspective,
                        (camera.target - camera.eye).normalize(),
                    ),
                });
            }
        }
        if let (Some((layer_id, _)), Some(settings), Some(volume)) = (
            self.active_field_layer,
            self.active_field_render_settings,
            self.volume_raycast_pipeline.as_ref(),
        ) {
            if settings.visible
                && matches!(
                    settings.render_mode,
                    crate::volumetric::FieldRenderMode::Volume
                        | crate::volumetric::FieldRenderMode::Both
                )
            {
                bindings.push(PublicationVolumeBinding {
                    layer_id,
                    bind_group: volume.publication_bind_group(
                        &self.gpu.device,
                        depth_view,
                        camera.eye,
                        camera.is_perspective,
                        (camera.target - camera.eye).normalize(),
                    ),
                });
            }
        }
        Ok(bindings)
    }

    pub(crate) fn publication_export_camera(
        &self,
        width: u32,
        height: u32,
        publication_bond_instances: &[crate::renderer::instance::BondInstance],
    ) -> Result<Camera, String> {
        fit_visible_structure_to_export(
            self.camera,
            width,
            height,
            &self.opaque_atom_instances,
            &self.transparent_atom_instances,
            if self.show_cell {
                &self.cell_line_vertices
            } else {
                &[]
            },
            if self.show_bonds {
                publication_bond_instances
            } else {
                &[]
            },
        )
    }

    pub(crate) fn publication_export_request(
        &self,
        width: u32,
        height: u32,
        source_state: PublicationExportSourceState,
        publication_bond_instance_count: u32,
    ) -> PublicationExportRequest {
        let active_settings = self.active_field_render_settings;
        let retained_has_isosurface = self.retained_field_layers.iter().any(|layer| {
            layer.render_settings.visible
                && (layer.isosurface_pipeline.is_some()
                    || layer.negative_isosurface_pipeline.is_some())
                && matches!(
                    layer.render_settings.render_mode,
                    crate::volumetric::FieldRenderMode::Isosurface
                        | crate::volumetric::FieldRenderMode::Both
                )
        });
        let retained_has_volume = self.retained_field_layers.iter().any(|layer| {
            layer.render_settings.visible
                && matches!(
                    layer.render_settings.render_mode,
                    crate::volumetric::FieldRenderMode::Volume
                        | crate::volumetric::FieldRenderMode::Both
                )
        });
        PublicationExportRequest {
            width,
            height,
            publication_bond_instance_count,
            needs_transparent_depth: self.transparent_instance_count > 0
                || self
                    .active_field_render_settings
                    .is_some_and(|settings| settings.visible)
                || self
                    .retained_field_layers
                    .iter()
                    .any(|layer| layer.render_settings.visible),
            has_measurement_overlays: self.measurement_line_count > 0,
            has_hopping_overlays: self.hopping_instance_count > 0,
            has_isosurface: retained_has_isosurface
                || ((self.active_field_layer_pipeline.is_some()
                    || self.active_negative_field_layer_pipeline.is_some())
                    && active_settings.is_some_and(|settings| {
                        settings.visible
                            && matches!(
                                settings.render_mode,
                                crate::volumetric::FieldRenderMode::Isosurface
                                    | crate::volumetric::FieldRenderMode::Both
                            )
                    })),
            has_volume: retained_has_volume
                || (self.volume_raycast_pipeline.is_some()
                    && active_settings.is_some_and(|settings| {
                        settings.visible
                            && matches!(
                                settings.render_mode,
                                crate::volumetric::FieldRenderMode::Volume
                                    | crate::volumetric::FieldRenderMode::Both
                            )
                    })),
            has_phonon_presentation: self.phonon_presentation.is_some(),
            has_atom_drag: self.atom_drag.is_some(),
            show_bz: self.show_bz,
            has_measurement_state: source_state.has_measurement_state,
            has_selection_highlights: source_state.has_selection_highlights,
            has_wannier_overlay: source_state.has_wannier_overlay,
            has_active_phonon_state: source_state.has_active_phonon_state,
        }
    }

    pub(crate) fn publication_export_limits(&self) -> PublicationExportLimits {
        let limits = &self.gpu.render_config;
        PublicationExportLimits {
            max_texture_dimension_2d: limits.max_texture_dimension_2d,
            max_buffer_size: limits.max_buffer_size,
            publication_msaa_x4: limits.publication_msaa_x4,
        }
    }

    pub(crate) fn publication_render_config_with_profile(
        &self,
        admission: &PublicationExportAdmissionReceipt,
        background: PublicationBackground,
        look_profile: PublicationLookProfile,
        publication_bond_instances: Vec<crate::renderer::instance::BondInstance>,
    ) -> Result<PublicationRenderConfig, String> {
        let publication_bond_instance_count = u32::try_from(publication_bond_instances.len())
            .map_err(|_| "publication bond instance count overflow".to_owned())?;
        if publication_bond_instance_count != admission.request.publication_bond_instance_count {
            return Err("publication bond scene changed after admission".to_owned());
        }
        self.validate_publication_export_receipt(admission)?;
        let field_snapshot = if admission.field_admitted {
            let snapshot = self
                .field_publication_snapshot()
                .ok_or_else(|| "publication field scene changed after admission".to_owned())?;
            let expected = evaluate_field_publication_export_admission(
                admission.request,
                admission.limits,
                &snapshot,
            )
            .map_err(|error| error.to_string())?;
            if expected != admission.clone() {
                return Err("publication field scene changed after admission".to_owned());
            }
            Some(snapshot)
        } else {
            None
        };
        let width = admission.request.width;
        let height = admission.request.height;
        let target_format = self.gpu.surface_format();
        if !target_format.is_srgb() {
            return Err(format!(
                "publication render requires an sRGB target format, got {target_format:?}"
            ));
        }

        let cell_line_style = cell_line_style_for_background(background, self.clear_color)?;
        let background = match background {
            PublicationBackground::Transparent => wgpu::Color::TRANSPARENT,
            PublicationBackground::White => wgpu::Color::WHITE,
            PublicationBackground::Black => wgpu::Color::BLACK,
            PublicationBackground::Current => self.clear_color,
        };
        let camera = self.publication_export_camera(width, height, &publication_bond_instances)?;

        let plan = admission.render_plan;
        look_profile.validate_fixed()?;
        Ok(PublicationRenderConfig {
            width,
            height,
            camera,
            background,
            alpha_mode: PublicationAlphaMode::Premultiplied,
            target_format,
            requested_samples: plan.requested_samples,
            selected_samples: plan.selected_samples,
            tile_dimensions: plan.tile_dimensions,
            tile_layout: plan.tile_layout,
            look_profile,
            cell_line_style,
            publication_bond_instances,
            readback_layout: offscreen_readback_layout(
                plan.tile_dimensions[0],
                plan.tile_dimensions[1],
            )
            .map_err(|error| error.to_string())?,
            admission: admission.clone(),
            field_snapshot,
        })
    }
}

#[must_use]
fn reject_legacy_unadmitted_field(
    request: PublicationExportRequest,
) -> Result<(), PublicationExportRejection> {
    if request.has_isosurface {
        return Err(PublicationExportRejection::Isosurface);
    }
    if request.has_volume {
        return Err(PublicationExportRejection::Volume);
    }
    Ok(())
}

#[derive(Clone)]
struct FieldPublicationResources {
    layer_count: u8,
    has_slice: bool,
    has_contour: bool,
    footprint: FieldResourceFootprint,
    field_scene_hash: String,
}

fn field_publication_resources(
    field_snapshot: &FieldPublicationSnapshot,
) -> Result<FieldPublicationResources, PublicationExportRejection> {
    if field_snapshot.representation.is_empty()
        || field_snapshot.field_snapshots.is_empty()
        || field_snapshot.field_snapshots.len() > MAX_VISIBLE_FIELD_LAYERS_FIGURE_2
        || field_snapshot.clip_planes.len() > crate::renderer::field_scene::MAX_FIELD_CLIP_PLANES
        || field_snapshot.transfer_function.validate().is_err()
    {
        return Err(PublicationExportRejection::ReceiptMismatch);
    }
    let mut has_slice = false;
    let mut has_contour = false;
    for snapshot in &field_snapshot.field_snapshots {
        if snapshot.validate().is_err() {
            return Err(PublicationExportRejection::ReceiptMismatch);
        }
        has_slice |= snapshot
            .representations
            .contains(&FieldRepresentation::Slice);
        has_contour |= snapshot
            .representations
            .contains(&FieldRepresentation::Contour);
    }
    u8::try_from(field_snapshot.field_snapshots.len())
        .map(|layer_count| FieldPublicationResources {
            layer_count,
            has_slice,
            has_contour,
            footprint: field_snapshot.resource_footprint,
            field_scene_hash: field_snapshot.field_scene_hash.clone(),
        })
        .map_err(|_| PublicationExportRejection::ReceiptMismatch)
}

/// Admit a frozen FIGURE-2 field scene without mutating the interactive renderer.
pub fn evaluate_field_publication_export_admission(
    request: PublicationExportRequest,
    limits: PublicationExportLimits,
    field_snapshot: &FieldPublicationSnapshot,
) -> Result<PublicationExportAdmissionReceipt, PublicationExportRejection> {
    let resources = field_publication_resources(field_snapshot)?;
    if field_snapshot.selected_field_capabilities & field_snapshot.requested_field_capabilities == 0
    {
        return Err(PublicationExportRejection::ReceiptMismatch);
    }
    evaluate_publication_export_admission_inner(request, limits, Some(resources))
}

pub fn evaluate_publication_export_admission(
    request: PublicationExportRequest,
    limits: PublicationExportLimits,
) -> Result<PublicationExportAdmissionReceipt, PublicationExportRejection> {
    evaluate_publication_export_admission_inner(request, limits, None)
}

fn evaluate_publication_export_admission_inner(
    request: PublicationExportRequest,
    limits: PublicationExportLimits,
    field_resources: Option<FieldPublicationResources>,
) -> Result<PublicationExportAdmissionReceipt, PublicationExportRejection> {
    if request.width == 0 || request.height == 0 {
        return Err(PublicationExportRejection::ZeroDimensions);
    }
    if request.has_measurement_overlays {
        return Err(PublicationExportRejection::MeasurementOverlays);
    }
    if request.has_hopping_overlays {
        return Err(PublicationExportRejection::HoppingOverlays);
    }
    if field_resources.is_none() {
        reject_legacy_unadmitted_field(request)?;
    }
    if request.has_phonon_presentation {
        return Err(PublicationExportRejection::PhononPresentation);
    }
    if request.has_atom_drag {
        return Err(PublicationExportRejection::AtomDrag);
    }
    if request.show_bz {
        return Err(PublicationExportRejection::BrillouinZone);
    }
    if request.has_measurement_state {
        return Err(PublicationExportRejection::MeasurementState);
    }
    if request.has_selection_highlights {
        return Err(PublicationExportRejection::SelectionHighlights);
    }
    if request.has_wannier_overlay {
        return Err(PublicationExportRejection::WannierOverlay);
    }
    if request.has_active_phonon_state {
        return Err(PublicationExportRejection::ActivePhononState);
    }
    let budgets = publication_export_budgets();
    let render_plan = publication_render_plan(request, limits, field_resources.as_ref())?;
    let estimate = publication_export_resource_estimate(request, render_plan)?;
    let estimate = if let Some(resources) = field_resources.as_ref() {
        add_field_publication_resource_estimate(estimate, resources)?
    } else {
        estimate
    };
    let (
        field_layer_count,
        field_has_slice,
        field_has_contour,
        field_admitted,
        field_scene_hash,
        field_resource_footprint,
    ) = if let Some(resources) = field_resources {
        (
            resources.layer_count,
            resources.has_slice,
            resources.has_contour,
            true,
            Some(resources.field_scene_hash),
            Some(resources.footprint),
        )
    } else {
        (0, false, false, false, None, None)
    };
    if estimate.staging_bytes > limits.max_buffer_size
        || estimate.publication_bond_bytes > limits.max_buffer_size
    {
        return Err(PublicationExportRejection::DeviceBufferLimit);
    }
    if estimate.rgba_bytes > budgets.max_readback_bytes {
        return Err(PublicationExportRejection::ReadbackBudget);
    }
    if estimate.transient_gpu_bytes > budgets.max_transient_gpu_bytes {
        return Err(PublicationExportRejection::TransientGpuBudget);
    }
    if estimate.peak_cpu_bytes > budgets.max_peak_cpu_bytes {
        return Err(PublicationExportRejection::PeakCpuBudget);
    }

    Ok(PublicationExportAdmissionReceipt {
        policy_version: PUBLICATION_EXPORT_POLICY_VERSION,
        request,
        limits,
        budgets,
        render_plan,
        estimate,
        field_layer_count,
        field_has_slice,
        field_has_contour,
        field_admitted,
        field_scene_hash,
        field_resource_footprint,
    })
}

pub(crate) fn validate_publication_export_receipt_fields(
    receipt: &PublicationExportAdmissionReceipt,
) -> Result<(), PublicationExportRejection> {
    if receipt.policy_version != PUBLICATION_EXPORT_POLICY_VERSION {
        return Err(PublicationExportRejection::PolicyVersion);
    }
    let field_resources = if receipt.field_admitted {
        Some(FieldPublicationResources {
            layer_count: receipt.field_layer_count,
            has_slice: receipt.field_has_slice,
            has_contour: receipt.field_has_contour,
            footprint: receipt
                .field_resource_footprint
                .ok_or(PublicationExportRejection::ReceiptMismatch)?,
            field_scene_hash: receipt
                .field_scene_hash
                .clone()
                .ok_or(PublicationExportRejection::ReceiptMismatch)?,
        })
    } else {
        None
    };
    let expected = evaluate_publication_export_admission_inner(
        receipt.request,
        receipt.limits,
        field_resources,
    )?;
    if expected != receipt.clone() {
        return Err(PublicationExportRejection::ReceiptMismatch);
    }
    Ok(())
}

impl Renderer {
    fn validate_publication_export_receipt(
        &self,
        receipt: &PublicationExportAdmissionReceipt,
    ) -> Result<(), String> {
        self.validate_publication_export_receipt_with_bond_count(
            receipt,
            receipt.request.publication_bond_instance_count,
        )
    }

    fn validate_publication_export_receipt_with_bond_count(
        &self,
        receipt: &PublicationExportAdmissionReceipt,
        publication_bond_instance_count: u32,
    ) -> Result<(), String> {
        validate_publication_export_receipt_fields(receipt).map_err(|error| error.to_string())?;
        if receipt.limits != self.publication_export_limits() {
            return Err(
                "publication export receipt does not match the active device limits".to_owned(),
            );
        }
        let current_request = self.publication_export_request(
            receipt.request.width,
            receipt.request.height,
            PublicationExportSourceState::default(),
            publication_bond_instance_count,
        );
        if current_request != receipt.request {
            return Err(
                "publication export receipt does not match the active renderer scene".to_owned(),
            );
        }
        Ok(())
    }

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
            pipeline::create_render_pipeline(&gpu.device, gpu.surface_format(), 1);

        let transparent_pipeline = pipeline::create_transparent_atom_pipeline(
            &gpu.device,
            gpu.surface_format(),
            &camera_bind_group_layout,
            1,
        );

        let line_pipeline = pipeline::create_line_pipeline(
            &gpu.device,
            gpu.surface_format(),
            &camera_bind_group_layout,
            1,
        );
        let field_slice_pipeline = pipeline::create_field_slice_pipeline(
            &gpu.device,
            gpu.surface_format(),
            &camera_bind_group_layout,
            1,
        );

        let bond_pipeline = pipeline::create_bond_pipeline(
            &gpu.device,
            gpu.surface_format(),
            &camera_bind_group_layout,
            1,
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
            pipeline::create_depth_texture(&gpu.device, gpu.config.width, gpu.config.height, 1);
        let (transparent_depth_texture, transparent_depth_view) =
            pipeline::create_transparent_depth_texture(
                &gpu.device,
                gpu.config.width,
                gpu.config.height,
                1,
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
            field_slice_pipeline,
            cell_line_buffer,
            cell_line_count: 0,
            cell_line_vertices: Vec::new(),
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
            active_field_layer_pipeline: None,
            active_negative_field_layer_pipeline: None,
            active_field_layer: None,
            active_field_snapshot: None,
            active_field_render_settings: None,
            active_field_slices: Vec::new(),
            retained_field_layers: Vec::with_capacity(MAX_VISIBLE_FIELD_LAYERS_FIGURE_2 - 1),
            field_resource_epoch: 0,
            active_field_gpu_bytes: 0,
            active_field_resource_footprint: FieldResourceFootprint::default(),
            resident_field_gpu_bytes: 0,
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
            let (opaque_depth_texture, opaque_depth_view) = pipeline::create_depth_texture(
                &self.gpu.device,
                new_size.width,
                new_size.height,
                1,
            );
            let (transparent_depth_texture, transparent_depth_view) =
                pipeline::create_transparent_depth_texture(
                    &self.gpu.device,
                    new_size.width,
                    new_size.height,
                    1,
                );
            self.opaque_depth_texture = opaque_depth_texture;
            self.opaque_depth_view = opaque_depth_view;
            self.transparent_depth_texture = transparent_depth_texture;
            self.transparent_depth_view = transparent_depth_view;

            // Notify volume pipeline to rebind depth texture
            if let Some(vol_pipe) = &mut self.volume_raycast_pipeline {
                vol_pipe.update_depth_view(&self.gpu.device, &self.opaque_depth_view);
            }
            for layer in &mut self.retained_field_layers {
                layer
                    .volume_raycast_pipeline
                    .update_depth_view(&self.gpu.device, &self.opaque_depth_view);
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

    pub fn set_phonon_display_scale(&mut self, display_scale: f64) -> crate::ipc::IpcResult<()> {
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
        self.cell_line_vertices.clear();
        self.cell_line_vertices.extend_from_slice(&scene.cell_lines);
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
        let forward = (self.camera.target - self.camera.eye).normalize();
        for layer in &mut self.retained_field_layers {
            layer.volume_raycast_pipeline.update_camera(
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
        let has_retained_field = self
            .retained_field_layers
            .iter()
            .any(|layer| layer.render_settings.visible);
        let needs_transparent_pass = has_retained_field
            || (self.show_volume
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

                let frozen_draw_list = self.frozen_field_draw_list();
                for layer_id in frozen_draw_list.iter() {
                    if self
                        .active_field_layer
                        .is_some_and(|(active_id, _)| active_id == *layer_id)
                    {
                        if let (Some(settings), Some(volume)) = (
                            self.active_field_render_settings,
                            self.volume_raycast_pipeline.as_ref(),
                        ) {
                            draw_frozen_field_resources(
                                &mut pass,
                                &self.camera_bind_group,
                                settings,
                                volume,
                                self.active_field_layer_pipeline.as_ref(),
                                self.active_negative_field_layer_pipeline.as_ref(),
                                &self.active_field_slices,
                                &self.field_slice_pipeline,
                                &self.line_pipeline,
                            );
                        }
                    } else if let Some(layer) = self
                        .retained_field_layers
                        .iter()
                        .find(|layer| layer.layer_id == *layer_id)
                    {
                        draw_frozen_field_resources(
                            &mut pass,
                            &self.camera_bind_group,
                            layer.render_settings,
                            &layer.volume_raycast_pipeline,
                            layer.isosurface_pipeline.as_ref(),
                            layer.negative_isosurface_pipeline.as_ref(),
                            &layer.slices,
                            &self.field_slice_pipeline,
                            &self.line_pipeline,
                        );
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

    /// Render the admitted structure scene through an export-owned configuration.
    pub(crate) fn render_offscreen(
        &self,
        config: &PublicationRenderConfig,
    ) -> Result<PublicationRenderResult, String> {
        let publication_bond_instance_count =
            u32::try_from(config.publication_bond_instances.len())
                .map_err(|_| "publication bond instance count overflow".to_owned())?;
        if publication_bond_instance_count
            != config.admission.request.publication_bond_instance_count
        {
            return Err("publication bond scene changed after admission".to_owned());
        }
        self.validate_publication_export_receipt(&config.admission)?;
        if config.width != config.admission.request.width
            || config.height != config.admission.request.height
        {
            return Err("publication render dimensions do not match admission".to_owned());
        }
        let full_output_len = usize::try_from(
            u64::from(config.width)
                .checked_mul(u64::from(config.height))
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or_else(|| "publication output size overflow".to_owned())?,
        )
        .map_err(|_| "publication output exceeds addressable memory".to_owned())?;
        let mut full_output = Vec::new();
        full_output
            .try_reserve_exact(full_output_len)
            .map_err(|_| "publication output allocation failed".to_owned())?;
        full_output.resize(full_output_len, 0);
        let publication_pipelines = PublicationPipelines::new(
            &self.gpu.device,
            config.target_format,
            config.selected_samples,
        );
        let publication_volume_replay_pipelines = (config.selected_samples > 1
            && config.admission.request.has_volume)
            .then(|| PublicationPipelines::new(&self.gpu.device, config.target_format, 1));
        let publication_field_pipelines = config
            .field_snapshot
            .as_ref()
            .map(|_| self.publication_field_pipelines(config.selected_samples))
            .transpose()?;
        let publication_look_uniform =
            PublicationLookUniform::from_profile(config.look_profile, config.camera.is_perspective);
        let publication_look_buffer =
            self.gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Publication Look Uniform Buffer"),
                    contents: bytemuck::bytes_of(&publication_look_uniform),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
        let publication_look_bind_group =
            self.gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Publication Look Bind Group"),
                    layout: &publication_pipelines.look_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: publication_look_buffer.as_entire_binding(),
                    }],
                });
        let publication_bond_buffer = (!config.publication_bond_instances.is_empty()).then(|| {
            self.gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Publication Element Bond Instance Buffer"),
                    contents: bytemuck::cast_slice(&config.publication_bond_instances),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        let publication_bond_buffer = publication_bond_buffer.as_ref();
        let publication_cell_line_color =
            publication_srgb_rgba_to_linear(config.cell_line_style.cell_line_color_rgba);
        let publication_cell_lines: Vec<LineVertex> = self
            .cell_line_vertices
            .iter()
            .map(|line| LineVertex {
                position: line.position,
                color: publication_cell_line_color,
            })
            .collect();
        let publication_cell_line_buffer = (!publication_cell_lines.is_empty()).then(|| {
            self.gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Publication Cell Line Buffer"),
                    contents: bytemuck::cast_slice(&publication_cell_lines),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        let publication_cell_line_buffer = publication_cell_line_buffer.as_ref();

        for tile_row in 0..config.tile_layout[1] {
            let tile_y = tile_row
                .checked_mul(config.tile_dimensions[1])
                .ok_or_else(|| "publication tile y offset overflow".to_owned())?;
            let tile_height = config
                .height
                .checked_sub(tile_y)
                .ok_or_else(|| "publication tile y offset exceeds output height".to_owned())?
                .min(config.tile_dimensions[1]);
            for tile_column in 0..config.tile_layout[0] {
                let tile_x = tile_column
                    .checked_mul(config.tile_dimensions[0])
                    .ok_or_else(|| "publication tile x offset overflow".to_owned())?;
                let tile_width = config
                    .width
                    .checked_sub(tile_x)
                    .ok_or_else(|| "publication tile x offset exceeds output width".to_owned())?
                    .min(config.tile_dimensions[0]);
                let tile_camera = config.camera;
                let current_tile = PublicationRenderConfig {
                    width: tile_width,
                    height: tile_height,
                    camera: tile_camera,
                    background: config.background,
                    alpha_mode: config.alpha_mode,
                    target_format: config.target_format,
                    requested_samples: config.requested_samples,
                    selected_samples: config.selected_samples,
                    tile_dimensions: [tile_width, tile_height],
                    tile_layout: [1, 1],
                    look_profile: config.look_profile,
                    cell_line_style: config.cell_line_style,
                    publication_bond_instances: Vec::new(),
                    readback_layout: offscreen_readback_layout(tile_width, tile_height)
                        .map_err(|error| error.to_string())?,
                    admission: config.admission.clone(),
                    field_snapshot: config.field_snapshot.clone(),
                };
                let rgba = self
                    .render_offscreen_tile(
                        &current_tile,
                        &publication_pipelines,
                        &publication_look_bind_group,
                        &publication_look_buffer,
                        publication_bond_buffer,
                        publication_cell_line_buffer,
                        publication_bond_instance_count,
                        publication_field_pipelines.as_deref(),
                        publication_volume_replay_pipelines.as_ref(),
                        tile_x,
                        tile_y,
                        config.width,
                        config.height,
                    )?
                    .into_rgba();
                let tile_row_bytes = usize::try_from(
                    u64::from(tile_width)
                        .checked_mul(4)
                        .ok_or_else(|| "publication tile row overflow".to_owned())?,
                )
                .map_err(|_| "publication tile row overflow".to_owned())?;
                let full_row_bytes = usize::try_from(
                    u64::from(config.width)
                        .checked_mul(4)
                        .ok_or_else(|| "publication output row overflow".to_owned())?,
                )
                .map_err(|_| "publication output row overflow".to_owned())?;
                for row in 0..usize::try_from(tile_height)
                    .map_err(|_| "tile height overflow".to_owned())?
                {
                    let source_start = row
                        .checked_mul(tile_row_bytes)
                        .ok_or_else(|| "publication tile source offset overflow".to_owned())?;
                    let source_end = source_start
                        .checked_add(tile_row_bytes)
                        .ok_or_else(|| "publication tile source end overflow".to_owned())?;
                    let destination_row = usize::try_from(tile_y)
                        .map_err(|_| "publication tile origin overflow".to_owned())?
                        .checked_add(row)
                        .ok_or_else(|| "publication destination row overflow".to_owned())?;
                    let destination_column = usize::try_from(tile_x)
                        .map_err(|_| "publication tile origin overflow".to_owned())?
                        .checked_mul(4)
                        .ok_or_else(|| "publication destination column overflow".to_owned())?;
                    let destination_start = destination_row
                        .checked_mul(full_row_bytes)
                        .and_then(|offset| offset.checked_add(destination_column))
                        .ok_or_else(|| "publication destination offset overflow".to_owned())?;
                    let destination_end = destination_start
                        .checked_add(tile_row_bytes)
                        .ok_or_else(|| "publication destination end overflow".to_owned())?;
                    let source = rgba
                        .get(source_start..source_end)
                        .ok_or_else(|| "publication tile source range is invalid".to_owned())?;
                    let destination = full_output
                        .get_mut(destination_start..destination_end)
                        .ok_or_else(|| "publication destination range is invalid".to_owned())?;
                    destination.copy_from_slice(source);
                }
            }
        }
        Ok(PublicationRenderResult {
            rgba: full_output,
            width: config.width,
            height: config.height,
            alpha_mode: config.alpha_mode,
        })
    }

    fn render_offscreen_tile(
        &self,
        config: &PublicationRenderConfig,
        publication_pipelines: &PublicationPipelines,
        publication_look_bind_group: &wgpu::BindGroup,
        publication_look_buffer: &wgpu::Buffer,
        publication_bond_buffer: Option<&wgpu::Buffer>,
        publication_cell_line_buffer: Option<&wgpu::Buffer>,
        publication_bond_instance_count: u32,
        publication_field_pipelines: Option<&[PublicationFieldLayerPipelines]>,
        publication_volume_replay_pipelines: Option<&PublicationPipelines>,
        tile_x: u32,
        tile_y: u32,
        full_width: u32,
        full_height: u32,
    ) -> Result<PublicationRenderResult, String> {
        self.validate_publication_export_receipt(&config.admission)?;
        if config.target_format != self.gpu.surface_format() {
            return Err("publication render target format changed after configuration".to_owned());
        }
        if config.requested_samples != 4
            || !matches!(config.selected_samples, 1 | 4)
            || (config.selected_samples == 4 && !self.gpu.render_config.publication_msaa_x4)
        {
            return Err(
                "publication sampling selection is incompatible with active GPU capabilities"
                    .to_owned(),
            );
        }
        if config.alpha_mode != PublicationAlphaMode::Premultiplied {
            return Err("publication render alpha policy is unsupported".to_owned());
        }
        let width = config.width;
        let height = config.height;
        self.gpu
            .device
            .push_error_scope(wgpu::ErrorFilter::Internal);
        self.gpu
            .device
            .push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        self.gpu
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);

        let mut export_camera_uniform = CameraUniform::new();
        export_camera_uniform.update_from_camera_tile(
            &config.camera,
            tile_x,
            tile_y,
            width,
            height,
            full_width,
            full_height,
        );
        let export_camera_buffer =
            self.gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Publication Export Camera Uniform Buffer"),
                    contents: bytemuck::cast_slice(&[export_camera_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
        let export_camera_bind_group =
            self.gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Publication Export Camera Bind Group"),
                    layout: &publication_pipelines.camera_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: export_camera_buffer.as_entire_binding(),
                    }],
                });
        let field_export_camera_bind_group = publication_field_pipelines.map(|_| {
            self.gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Publication Field Camera Bind Group"),
                    layout: &self.camera_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: export_camera_buffer.as_entire_binding(),
                    }],
                })
        });
        let volume_replay_camera_bind_group =
            publication_volume_replay_pipelines.map(|pipelines| {
                self.gpu
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Publication Volume Replay Camera Bind Group"),
                        layout: &pipelines.camera_bind_group_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: export_camera_buffer.as_entire_binding(),
                        }],
                    })
            });
        let volume_replay_look_bind_group = publication_volume_replay_pipelines.map(|pipelines| {
            self.gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Publication Volume Replay Look Bind Group"),
                    layout: &pipelines.look_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: publication_look_buffer.as_entire_binding(),
                    }],
                })
        });

        let tex_format = config.target_format;
        let needs_transparent = config.admission.request.needs_transparent_depth;

        // The resolve texture is always single-sample and is the only texture copied to host.
        let color_texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Publication Resolve Color Texture"),
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
        let multisample_color = (config.selected_samples > 1).then(|| {
            self.gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Publication MSAA Color Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: config.selected_samples,
                dimension: wgpu::TextureDimension::D2,
                format: tex_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        });
        let multisample_color_view = multisample_color
            .as_ref()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));
        let render_color_view = multisample_color_view.as_ref().unwrap_or(&color_view);
        let depth_replay_color = (needs_transparent && config.selected_samples > 1).then(|| {
            self.gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Publication Depth Replay Color Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: config.selected_samples,
                dimension: wgpu::TextureDimension::D2,
                format: tex_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        });
        let depth_replay_color_view = depth_replay_color
            .as_ref()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));
        let needs_volume_depth_replay =
            config.admission.request.has_volume && config.selected_samples > 1;
        let volume_depth_replay_color = needs_volume_depth_replay.then(|| {
            self.gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Publication Volume Depth Replay Color Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: tex_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        });
        let volume_depth_replay_color_view = volume_depth_replay_color
            .as_ref()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));

        let (offscreen_opaque_depth, offscreen_opaque_depth_view) = pipeline::create_depth_texture(
            &self.gpu.device,
            width,
            height,
            config.selected_samples,
        );
        let offscreen_transparent_depth = needs_transparent.then(|| {
            pipeline::create_transparent_depth_texture(
                &self.gpu.device,
                width,
                height,
                config.selected_samples,
            )
        });
        let offscreen_volume_depth = needs_volume_depth_replay
            .then(|| pipeline::create_depth_texture(&self.gpu.device, width, height, 1));
        let offscreen_volume_depth_view = offscreen_volume_depth.as_ref().map(|(_, view)| view);

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
                    view: render_color_view,
                    resolve_target: if config.selected_samples > 1 && !needs_transparent {
                        Some(&color_view)
                    } else {
                        None
                    },
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(config.background),
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

            pass.set_pipeline(&publication_pipelines.render);
            pass.set_bind_group(0, &export_camera_bind_group, &[]);
            pass.set_bind_group(1, publication_look_bind_group, &[]);
            if self.instance_count > 0 {
                pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
                pass.draw(0..6, 0..self.instance_count);
            }

            if self.show_cell && self.cell_line_count > 0 {
                let publication_cell_line_buffer = publication_cell_line_buffer
                    .ok_or_else(|| "publication cell line buffer is missing".to_owned())?;
                pass.set_pipeline(&publication_pipelines.line);
                pass.set_bind_group(0, &export_camera_bind_group, &[]);
                pass.set_vertex_buffer(0, publication_cell_line_buffer.slice(..));
                pass.draw(0..self.cell_line_count, 0..1);
            }

            if self.show_bonds && publication_bond_instance_count > 0 {
                let publication_bond_buffer = publication_bond_buffer
                    .ok_or_else(|| "publication bond buffer is missing".to_owned())?;
                pass.set_pipeline(&publication_pipelines.bond);
                pass.set_bind_group(0, &export_camera_bind_group, &[]);
                pass.set_bind_group(1, publication_look_bind_group, &[]);
                pass.set_vertex_buffer(0, publication_bond_buffer.slice(..));
                pass.draw(0..72, 0..publication_bond_instance_count);
            }
        }

        // ═══ Offscreen Pass 2: Transparent structure atoms ═══
        if let Some((offscreen_transparent_depth, offscreen_transparent_depth_view)) =
            &offscreen_transparent_depth
        {
            let volume_depth_view =
                offscreen_volume_depth_view.unwrap_or(offscreen_transparent_depth_view);
            let publication_volume_bindings = publication_field_pipelines
                .map(|_| self.publication_volume_bindings(volume_depth_view, &config.camera))
                .transpose()?;
            if config.selected_samples == 1 {
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
            } else {
                let replay_color_view = depth_replay_color_view.as_ref().ok_or_else(|| {
                    "publication depth replay color attachment is missing".to_owned()
                })?;
                let mut replay = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Publication Opaque Depth Replay Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: replay_color_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Discard,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: offscreen_transparent_depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                replay.set_pipeline(&publication_pipelines.render);
                replay.set_bind_group(0, &export_camera_bind_group, &[]);
                replay.set_bind_group(1, publication_look_bind_group, &[]);
                if self.instance_count > 0 {
                    replay.set_vertex_buffer(0, self.instance_buffer.slice(..));
                    replay.draw(0..6, 0..self.instance_count);
                }
                if self.show_cell && self.cell_line_count > 0 {
                    let publication_cell_line_buffer = publication_cell_line_buffer
                        .ok_or_else(|| "publication cell line buffer is missing".to_owned())?;
                    replay.set_pipeline(&publication_pipelines.line);
                    replay.set_bind_group(0, &export_camera_bind_group, &[]);
                    replay.set_vertex_buffer(0, publication_cell_line_buffer.slice(..));
                    replay.draw(0..self.cell_line_count, 0..1);
                }
                if self.show_bonds && publication_bond_instance_count > 0 {
                    let publication_bond_buffer = publication_bond_buffer
                        .ok_or_else(|| "publication bond buffer is missing".to_owned())?;
                    replay.set_pipeline(&publication_pipelines.bond);
                    replay.set_bind_group(0, &export_camera_bind_group, &[]);
                    replay.set_bind_group(1, publication_look_bind_group, &[]);
                    replay.set_vertex_buffer(0, publication_bond_buffer.slice(..));
                    replay.draw(0..72, 0..publication_bond_instance_count);
                }
                drop(replay);

                if let (
                    Some(replay_pipelines),
                    Some(replay_camera_bind_group),
                    Some(replay_look_bind_group),
                    Some(volume_replay_color_view),
                    Some(volume_depth_view),
                ) = (
                    publication_volume_replay_pipelines,
                    volume_replay_camera_bind_group.as_ref(),
                    volume_replay_look_bind_group.as_ref(),
                    volume_depth_replay_color_view.as_ref(),
                    offscreen_volume_depth_view,
                ) {
                    let mut volume_replay =
                        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Publication Single-Sample Volume Depth Replay Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: volume_replay_color_view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                    store: wgpu::StoreOp::Discard,
                                },
                            })],
                            depth_stencil_attachment: Some(
                                wgpu::RenderPassDepthStencilAttachment {
                                    view: volume_depth_view,
                                    depth_ops: Some(wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(1.0),
                                        store: wgpu::StoreOp::Store,
                                    }),
                                    stencil_ops: None,
                                },
                            ),
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });
                    volume_replay.set_pipeline(&replay_pipelines.render);
                    volume_replay.set_bind_group(0, replay_camera_bind_group, &[]);
                    volume_replay.set_bind_group(1, replay_look_bind_group, &[]);
                    if self.instance_count > 0 {
                        volume_replay.set_vertex_buffer(0, self.instance_buffer.slice(..));
                        volume_replay.draw(0..6, 0..self.instance_count);
                    }
                    if self.show_cell && self.cell_line_count > 0 {
                        let publication_cell_line_buffer = publication_cell_line_buffer
                            .ok_or_else(|| "publication cell line buffer is missing".to_owned())?;
                        volume_replay.set_pipeline(&replay_pipelines.line);
                        volume_replay.set_bind_group(0, replay_camera_bind_group, &[]);
                        volume_replay.set_vertex_buffer(0, publication_cell_line_buffer.slice(..));
                        volume_replay.draw(0..self.cell_line_count, 0..1);
                    }
                    if self.show_bonds && publication_bond_instance_count > 0 {
                        let publication_bond_buffer = publication_bond_buffer
                            .ok_or_else(|| "publication bond buffer is missing".to_owned())?;
                        volume_replay.set_pipeline(&replay_pipelines.bond);
                        volume_replay.set_bind_group(0, replay_camera_bind_group, &[]);
                        volume_replay.set_bind_group(1, replay_look_bind_group, &[]);
                        volume_replay.set_vertex_buffer(0, publication_bond_buffer.slice(..));
                        volume_replay.draw(0..72, 0..publication_bond_instance_count);
                    }
                }
            }

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Offscreen Transparent Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: render_color_view,
                        resolve_target: if config.selected_samples > 1 {
                            Some(&color_view)
                        } else {
                            None
                        },
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

                if let (Some(field_pipelines), Some(field_camera_bind_group)) = (
                    publication_field_pipelines,
                    field_export_camera_bind_group.as_ref(),
                ) {
                    let volume_bindings = publication_volume_bindings.as_deref().unwrap_or(&[]);
                    draw_publication_fields(
                        self,
                        &mut pass,
                        field_camera_bind_group,
                        field_pipelines,
                        volume_bindings,
                        &publication_pipelines.line,
                        config
                            .field_snapshot
                            .as_ref()
                            .map(|snapshot| &snapshot.draw_list)
                            .ok_or_else(|| "publication field draw list is missing".to_owned())?,
                    );
                }
                // Match the interactive fallback: fields precede translucent structure.
                if self.transparent_instance_count > 0 {
                    pass.set_pipeline(&publication_pipelines.transparent);
                    pass.set_bind_group(0, &export_camera_bind_group, &[]);
                    pass.set_bind_group(1, publication_look_bind_group, &[]);
                    pass.set_vertex_buffer(0, self.transparent_instance_buffer.slice(..));
                    pass.draw(0..6, 0..self.transparent_instance_count);
                }
            }
        }

        let staging_buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Offscreen Staging Buffer"),
            size: config.readback_layout.staging_size,
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
                    bytes_per_row: Some(config.readback_layout.padded_bytes_per_row),
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
        finish_publication_export_error_scopes(&self.gpu.device)?;

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

        let data = buffer_slice.get_mapped_range();
        let unpacked = unpack_publication_readback(
            &data,
            config.readback_layout,
            height,
            config.target_format,
        );
        drop(data);
        staging_buffer.unmap();
        let rgba_data = unpacked?;

        log::info!(
            "Offscreen render complete: {}x{}, {} bytes",
            width,
            height,
            rgba_data.len()
        );

        Ok(PublicationRenderResult {
            rgba: rgba_data,
            width,
            height,
            alpha_mode: config.alpha_mode,
        })
    }

    /// Clear volumetric pipelines when switching to a non-volumetric file.
    pub fn clear_volumetric(&mut self) {
        self.field_resource_epoch = self.field_resource_epoch.wrapping_add(1);
        self.drop_all_field_resources();
    }

    fn drop_all_field_resources(&mut self) {
        self.active_field_layer_pipeline = None;
        self.active_negative_field_layer_pipeline = None;
        self.active_field_layer = None;
        self.active_field_snapshot = None;
        self.active_field_render_settings = None;
        self.active_field_slices.clear();
        self.retained_field_layers.clear();
        self.active_field_gpu_bytes = 0;
        self.active_field_resource_footprint = FieldResourceFootprint::default();
        self.resident_field_gpu_bytes = 0;
        self.volume_raycast_pipeline = None;
        self.show_isosurface = false;
        self.show_volume = false;
        self.volume_render_mode = RendererVolumeMode::Isosurface;
    }

    pub fn clear_structure_bound_overlays(&mut self) {
        self.clear_volumetric();
        self.clear_non_field_structure_bound_overlays();
    }

    pub fn set_active_field_visibility(&mut self, visible: bool) {
        self.show_isosurface = visible && self.active_field_layer_pipeline.is_some();
        self.show_volume = visible && self.volume_raycast_pipeline.is_some();
    }

    /// Clear overlays derived from the crystal structure while preserving a
    /// prepared field resource until its replacement can be committed.
    pub fn clear_non_field_structure_bound_overlays(&mut self) {
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

    fn prepare_scalar_field(
        &self,
        layer_id: crate::volumetric::FieldLayerId,
        layer_revision: crate::volumetric::FieldSceneRevision,
        vol: &impl crate::volumetric::ScalarFieldView,
        scalar_unit: crate::volumetric::ScalarUnit,
        render_settings: crate::volumetric::FieldRenderSettings,
        presentation_settings: &crate::renderer::field_scene::FieldPresentationSettings,
        precomputed_vertex_counts: Option<(u32, u32)>,
    ) -> Result<PreparedFieldLayer, ()> {
        let clip_planes = presentation_settings
            .normalized_clip_planes()
            .map_err(|_| ())?;
        if !clip_planes
            .iter()
            .flat_map(|plane| {
                plane
                    .normal
                    .into_iter()
                    .chain(std::iter::once(plane.signed_offset_angstrom))
            })
            .all(|value| (value as f32).is_finite())
        {
            return Err(());
        }
        let scalar_bytes = (vol.scalar_data().len() as u64)
            .checked_mul(std::mem::size_of::<f32>() as u64)
            .ok_or(())?;
        let slices = crate::renderer::field_slice::prepare_field_slices(
            &self.gpu.device,
            vol,
            layer_revision,
            &presentation_settings.slices,
            &clip_planes,
            &presentation_settings.transfer_function,
            presentation_settings.display_range,
            presentation_settings.opacity_scale,
        )
        .map_err(|_| ())?;
        let slice_gpu_bytes = slices
            .iter()
            .try_fold(0_u64, |total, slice| total.checked_add(slice.gpu_bytes))
            .ok_or(())?;
        let slice_vertex_bytes = slices
            .iter()
            .try_fold(0_u64, |total, slice| {
                total.checked_add(slice.slice_vertex_bytes)
            })
            .ok_or(())?;
        let contour_vertex_bytes = slices
            .iter()
            .try_fold(0_u64, |total, slice| {
                total.checked_add(slice.contour_vertex_bytes)
            })
            .ok_or(())?;
        let positive_isovalue = render_settings.positive_isovalue;
        let negative_isovalue = render_settings.negative_isovalue;
        let (exact_positive_vertices, exact_negative_vertices) =
            if self.gpu.render_config.supports_compute_shaders {
                match precomputed_vertex_counts {
                    Some(counts) => counts,
                    None => crate::renderer::isosurface::marching_cubes_signed_vertex_counts(
                        vol,
                        (!matches!(
                            render_settings.sign_mode,
                            crate::volumetric::FieldSignMode::Negative
                        ))
                        .then_some(positive_isovalue),
                        (!matches!(
                            render_settings.sign_mode,
                            crate::volumetric::FieldSignMode::Positive
                        ))
                        .then_some(negative_isovalue),
                    )?,
                }
            } else {
                (0, 0)
            };
        let mut positive_vertex_capacity = exact_positive_vertices;
        let mut negative_vertex_capacity = exact_negative_vertices;
        let proposed_positive_capacity = exact_positive_vertices
            .checked_add(exact_positive_vertices / 2)
            .unwrap_or(exact_positive_vertices);
        let proposed_negative_capacity = exact_negative_vertices
            .checked_add(exact_negative_vertices / 2)
            .unwrap_or(exact_negative_vertices);
        let proposed_isosurface_bytes = u64::from(proposed_positive_capacity.max(3))
            .checked_add(u64::from(proposed_negative_capacity.max(3)))
            .and_then(|vertices| {
                vertices.checked_mul(
                    std::mem::size_of::<crate::renderer::isosurface::IsoVertex>() as u64,
                )
            })
            .ok_or(())?;
        let proposed_gpu_bytes = scalar_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(proposed_isosurface_bytes))
            .and_then(|bytes| bytes.checked_add(slice_gpu_bytes))
            .ok_or(())?;
        if proposed_isosurface_bytes <= MAX_FIELD_ISOSURFACE_VERTEX_BYTES
            && proposed_gpu_bytes <= MAX_ACTIVE_FIELD_GPU_BYTES
        {
            positive_vertex_capacity = proposed_positive_capacity;
            negative_vertex_capacity = proposed_negative_capacity;
        }
        let maximum_isosurface_bytes = u64::from(positive_vertex_capacity.max(3))
            .checked_add(u64::from(negative_vertex_capacity.max(3)))
            .and_then(|vertices| {
                vertices.checked_mul(
                    std::mem::size_of::<crate::renderer::isosurface::IsoVertex>() as u64,
                )
            })
            .ok_or(())?;
        let maximum_gpu_bytes = scalar_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(maximum_isosurface_bytes))
            .and_then(|bytes| bytes.checked_add(slice_gpu_bytes))
            .ok_or(())?;
        let limits = self.gpu.device.limits();
        if scalar_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || maximum_isosurface_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || maximum_isosurface_bytes > MAX_FIELD_ISOSURFACE_VERTEX_BYTES
            || maximum_gpu_bytes > MAX_ACTIVE_FIELD_GPU_BYTES
            || self
                .resident_field_gpu_bytes
                .checked_sub(
                    self.active_field_layer
                        .is_some_and(|(active_layer_id, active_layer_revision)| {
                            active_layer_id == layer_id && active_layer_revision == layer_revision
                        })
                        .then_some(self.active_field_gpu_bytes)
                        .unwrap_or(0),
                )
                .ok_or(())?
                .checked_add(maximum_gpu_bytes)
                .ok_or(())?
                > MAX_ACTIVE_FIELD_GPU_BYTES
            || !vol
                .lattice_angstrom()
                .iter()
                .chain(vol.origin_angstrom().iter())
                .all(|value| value.is_finite() && (*value as f32).is_finite())
        {
            return Err(());
        }
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let shared_scalar_buffer = Arc::new(self.gpu.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Shared Field Scalar Buffer"),
                    contents: bytemuck::cast_slice(vol.scalar_data()),
                    usage: wgpu::BufferUsages::STORAGE,
                },
            ));
            let mut isosurface_pipeline = if self.gpu.render_config.supports_compute_shaders
                && positive_vertex_capacity > 0
            {
                Some(
                    crate::renderer::isosurface::IsosurfacePipeline::new_with_vertex_capacity(
                        &self.gpu.device,
                        &self.gpu.queue,
                        self.gpu.surface_format(),
                        &self.camera_bind_group_layout,
                        vol,
                        positive_vertex_capacity,
                        &clip_planes,
                        Some(shared_scalar_buffer.clone()),
                    )?,
                )
            } else {
                log::warn!(
                    "Compute shaders not supported! GPU Marching Cubes cannot run on this device."
                );
                None
            };
            let mut negative_isosurface_pipeline = if self
                .gpu
                .render_config
                .supports_compute_shaders
                && negative_vertex_capacity > 0
            {
                Some(
                    crate::renderer::isosurface::IsosurfacePipeline::new_with_vertex_capacity(
                        &self.gpu.device,
                        &self.gpu.queue,
                        self.gpu.surface_format(),
                        &self.camera_bind_group_layout,
                        vol,
                        negative_vertex_capacity,
                        &clip_planes,
                        Some(shared_scalar_buffer.clone()),
                    )?,
                )
            } else {
                None
            };
            let mut volume_raycast_pipeline =
                crate::renderer::volume_raycast::VolumeRaycastPipeline::new(
                    &self.gpu.device,
                    self.gpu.surface_format(),
                    &self.camera_bind_group_layout,
                    vol,
                    &self.opaque_depth_view,
                    &clip_planes,
                    Some(shared_scalar_buffer.clone()),
                )?;
            let display_range = presentation_settings
                .display_range
                .unwrap_or([vol.scalar_range().0, vol.scalar_range().1]);
            volume_raycast_pipeline.update_transfer_function(
                &self.gpu.queue,
                display_range,
                (render_settings.opacity * presentation_settings.opacity_scale).clamp(0.0, 1.0),
            );
            volume_raycast_pipeline
                .set_density_cutoff(&self.gpu.queue, presentation_settings.density_cutoff);
            if presentation_settings.use_explicit_transfer_function {
                volume_raycast_pipeline.update_explicit_transfer_function(
                    &self.gpu.queue,
                    &presentation_settings.transfer_function,
                )?;
            }
            volume_raycast_pipeline
                .set_material_mode(&self.gpu.queue, presentation_settings.field_material_mode);
            if let Some(pipeline) = &mut isosurface_pipeline {
                pipeline
                    .set_material_mode(&self.gpu.queue, presentation_settings.field_material_mode);
            }
            if let Some(pipeline) = &mut negative_isosurface_pipeline {
                pipeline
                    .set_material_mode(&self.gpu.queue, presentation_settings.field_material_mode);
            }
            let isosurface_vertex_bytes = isosurface_pipeline
                .iter()
                .chain(negative_isosurface_pipeline.iter())
                .try_fold(0_u64, |total, pipeline| {
                    total.checked_add(pipeline.vertex_buffer_bytes())
                })
                .ok_or(())?;
            let isosurface_resident_bytes = isosurface_pipeline
                .iter()
                .chain(negative_isosurface_pipeline.iter())
                .try_fold(0_u64, |total, pipeline| {
                    total.checked_add(pipeline.resident_bytes())
                })
                .ok_or(())?;
            let resident_field_bytes = volume_raycast_pipeline
                .resident_bytes()
                .checked_add(isosurface_resident_bytes)
                .and_then(|bytes| bytes.checked_add(scalar_bytes))
                .and_then(|bytes| bytes.checked_add(slice_gpu_bytes))
                .ok_or(())?;
            let resource_footprint = FieldResourceFootprint {
                resident_field_bytes,
                isosurface_vertex_bytes,
                volume_storage_bytes: volume_raycast_pipeline.scalar_storage_bytes(),
                slice_vertex_bytes,
                contour_vertex_bytes,
            };
            let gpu_bytes = resource_footprint.resident_field_bytes;
            if gpu_bytes > MAX_ACTIVE_FIELD_GPU_BYTES
                || self
                    .resident_field_gpu_bytes
                    .checked_sub(
                        self.active_field_layer
                            .is_some_and(|(active_layer_id, active_layer_revision)| {
                                active_layer_id == layer_id
                                    && active_layer_revision == layer_revision
                            })
                            .then_some(self.active_field_gpu_bytes)
                            .unwrap_or(0),
                    )
                    .ok_or(())?
                    .checked_add(gpu_bytes)
                    .ok_or(())?
                    > MAX_ACTIVE_FIELD_GPU_BYTES
            {
                return Err(());
            }
            let field_snapshot = FieldRenderSnapshot {
                layer_id,
                source_layer_revision: layer_revision,
                source_origin_angstrom: vol.source_origin_angstrom(),
                scalar_unit: match scalar_unit {
                    crate::volumetric::ScalarUnit::ElectronPerCubicAngstrom => {
                        "electron_per_cubic_angstrom"
                    }
                    crate::volumetric::ScalarUnit::ElectronPerBohrCubed => {
                        "electron_per_bohr_cubed"
                    }
                    crate::volumetric::ScalarUnit::Arbitrary => "arbitrary",
                }
                .to_owned(),
                scalar_range: [vol.scalar_range().0, vol.scalar_range().1],
                representations: field_representations(
                    render_settings,
                    &presentation_settings.slices,
                ),
                positive_isovalue: (!matches!(
                    render_settings.sign_mode,
                    crate::volumetric::FieldSignMode::Negative
                ) && matches!(
                    render_settings.render_mode,
                    crate::volumetric::FieldRenderMode::Isosurface
                        | crate::volumetric::FieldRenderMode::Both
                ))
                .then_some(positive_isovalue),
                negative_isovalue: (!matches!(
                    render_settings.sign_mode,
                    crate::volumetric::FieldSignMode::Positive
                ) && matches!(
                    render_settings.render_mode,
                    crate::volumetric::FieldRenderMode::Isosurface
                        | crate::volumetric::FieldRenderMode::Both
                ))
                .then_some(negative_isovalue),
                positive_color: render_settings.color,
                negative_color: render_settings.color_negative,
                clip_planes,
                slices: presentation_settings.slices.clone(),
                transfer_function: presentation_settings.transfer_function.clone(),
                use_explicit_transfer_function: presentation_settings
                    .use_explicit_transfer_function,
                transparency_method: presentation_settings.transparency_method,
                field_material_mode: presentation_settings.field_material_mode,
                display_range: presentation_settings.display_range,
                opacity_scale: presentation_settings.opacity_scale,
                density_cutoff: presentation_settings.density_cutoff,
                colormap_mode: render_settings.colormap_mode,
            };
            field_snapshot.validate().map_err(|_| ())?;
            Ok(PreparedFieldLayer {
                layer_id,
                layer_revision,
                renderer_field_epoch: self.field_resource_epoch,
                render_settings,
                field_snapshot,
                grid_dims: vol.grid_dims(),
                gpu_bytes,
                resource_footprint,
                isosurface_pipeline,
                negative_isosurface_pipeline,
                volume_raycast_pipeline,
                slices,
            })
        }))
        .map_err(|_| ())?
    }

    pub fn prepare_volumetric(
        &self,
        vol: &crate::volumetric::VolumetricData,
    ) -> Result<PreparedVolumetric, ()> {
        self.prepare_scalar_field(
            0,
            0,
            vol,
            crate::volumetric::ScalarUnit::Arbitrary,
            crate::volumetric::FieldRenderSettings::default(),
            &crate::renderer::field_scene::FieldPresentationSettings::default(),
            None,
        )
    }

    pub fn prepare_field_layer(
        &self,
        layer: &crate::volumetric::FieldLayer,
    ) -> Result<PreparedFieldLayer, ()> {
        self.prepare_scalar_field(
            layer.id,
            layer.revision,
            layer,
            layer.scalar_unit,
            layer.render_settings,
            &layer.presentation_settings,
            None,
        )
    }

    pub fn prepare_field_layer_with_vertex_counts(
        &self,
        layer: &crate::volumetric::FieldLayer,
        vertex_counts: (u32, u32),
    ) -> Result<PreparedFieldLayer, ()> {
        self.prepare_scalar_field(
            layer.id,
            layer.revision,
            layer,
            layer.scalar_unit,
            layer.render_settings,
            &layer.presentation_settings,
            Some(vertex_counts),
        )
    }

    pub fn prepare_field_layer_with_render_settings(
        &self,
        layer: &crate::volumetric::FieldLayer,
        render_settings: crate::volumetric::FieldRenderSettings,
    ) -> Result<PreparedFieldLayer, ()> {
        self.prepare_scalar_field(
            layer.id,
            layer.revision,
            layer,
            layer.scalar_unit,
            render_settings,
            &layer.presentation_settings,
            None,
        )
    }

    pub fn update_field_render_settings(
        &mut self,
        layer: &crate::volumetric::FieldLayer,
        render_settings: crate::volumetric::FieldRenderSettings,
    ) -> Result<(), ()> {
        let prepared = self.prepare_field_layer_with_render_settings(layer, render_settings)?;
        self.commit_field_layer(prepared, layer.id, layer.revision)
    }

    pub fn commit_volumetric(&mut self, prepared: PreparedVolumetric) {
        let _ = self.commit_field_layer(prepared, 0, 0);
    }

    pub fn commit_field_layer(
        &mut self,
        prepared: PreparedFieldLayer,
        layer_id: crate::volumetric::FieldLayerId,
        layer_revision: crate::volumetric::FieldSceneRevision,
    ) -> Result<(), ()> {
        if MAX_VISIBLE_FIELD_LAYERS_FIGURE_2 == 0 {
            log::warn!("FIGURE-2 requires a positive visible field-layer capacity");
            return Err(());
        }
        if prepared.layer_id != layer_id || prepared.layer_revision != layer_revision {
            log::warn!("stale prepared field layer was not committed");
            return Err(());
        }
        if prepared.renderer_field_epoch != self.field_resource_epoch {
            log::warn!("stale prepared field layer resource epoch was not committed");
            return Err(());
        }
        let replaces_active = self
            .active_field_layer
            .is_some_and(|(active_layer_id, _)| active_layer_id == layer_id);
        let retained_without_replacement = self
            .retained_field_layers
            .iter()
            .filter(|layer| layer.layer_id != layer_id)
            .count();
        let pending_previous_active =
            usize::from(self.active_field_layer.is_some() && !replaces_active);
        if retained_without_replacement
            .checked_add(pending_previous_active)
            .ok_or(())?
            >= MAX_VISIBLE_FIELD_LAYERS_FIGURE_2
        {
            log::warn!("FIGURE-2 visible field-layer capacity exceeded");
            return Err(());
        }
        if let Some(index) = self
            .retained_field_layers
            .iter()
            .position(|layer| layer.layer_id == layer_id)
        {
            self.retained_field_layers.remove(index);
        }
        let previous = match (
            self.active_field_layer.take(),
            self.active_field_render_settings.take(),
            self.active_field_snapshot.take(),
            self.active_field_layer_pipeline.take(),
            self.active_negative_field_layer_pipeline.take(),
            self.volume_raycast_pipeline.take(),
            std::mem::take(&mut self.active_field_slices),
        ) {
            (
                Some((previous_id, _previous_revision)),
                Some(render_settings),
                Some(field_snapshot),
                isosurface_pipeline,
                negative_isosurface_pipeline,
                Some(volume_raycast_pipeline),
                slices,
            ) if previous_id != layer_id => Some(RetainedFieldLayer {
                layer_id: previous_id,
                render_settings,
                field_snapshot,
                gpu_bytes: self.active_field_gpu_bytes,
                resource_footprint: self.active_field_resource_footprint,
                isosurface_pipeline,
                negative_isosurface_pipeline,
                volume_raycast_pipeline,
                slices,
            }),
            _ => None,
        };
        if let Some(previous) = previous {
            let insertion = self
                .retained_field_layers
                .binary_search_by_key(&previous.layer_id, |layer| layer.layer_id)
                .unwrap_or_else(|index| index);
            self.retained_field_layers.insert(insertion, previous);
        }
        let render_settings = prepared.render_settings;
        let grid_dims = prepared.grid_dims;
        let gpu_bytes = prepared.gpu_bytes;
        let resource_footprint = prepared.resource_footprint;
        self.show_isosurface = prepared.isosurface_pipeline.is_some()
            || prepared.negative_isosurface_pipeline.is_some();
        self.active_field_layer_pipeline = prepared.isosurface_pipeline;
        self.active_negative_field_layer_pipeline = prepared.negative_isosurface_pipeline;
        self.active_field_layer = Some((layer_id, layer_revision));
        self.active_field_snapshot = Some(prepared.field_snapshot);
        self.active_field_render_settings = Some(render_settings);
        self.volume_raycast_pipeline = Some(prepared.volume_raycast_pipeline);
        self.active_field_slices = prepared.slices;
        self.active_field_gpu_bytes = gpu_bytes;
        self.active_field_resource_footprint = resource_footprint;
        self.resident_field_gpu_bytes = self
            .retained_field_layers
            .iter()
            .try_fold(gpu_bytes, |total, layer| total.checked_add(layer.gpu_bytes))
            .ok_or(())?;
        self.field_resource_epoch = self.field_resource_epoch.wrapping_add(1);
        self.apply_field_render_settings(render_settings);
        self.update_signed_isovalues(
            grid_dims,
            render_settings.positive_isovalue,
            render_settings.negative_isovalue,
        );
        Ok(())
    }

    pub fn commit_replacement_field_layer(
        &mut self,
        prepared: PreparedFieldLayer,
        layer_id: crate::volumetric::FieldLayerId,
        layer_revision: crate::volumetric::FieldSceneRevision,
    ) -> Result<(), ()> {
        if prepared.layer_id != layer_id
            || prepared.layer_revision != layer_revision
            || prepared.renderer_field_epoch != self.field_resource_epoch
        {
            log::warn!("stale replacement field layer was not committed");
            return Err(());
        }
        self.drop_all_field_resources();
        self.commit_field_layer(prepared, layer_id, layer_revision)
    }

    /// Drop every renderer-owned resource for one logical field layer.
    /// Call only after the replacement resource has prepared successfully.
    pub fn remove_field_layer_resources(&mut self, layer_id: crate::volumetric::FieldLayerId) {
        if self
            .active_field_layer
            .is_some_and(|(active_layer_id, _)| active_layer_id == layer_id)
        {
            self.active_field_layer_pipeline = None;
            self.active_negative_field_layer_pipeline = None;
            self.volume_raycast_pipeline = None;
            self.active_field_slices.clear();
            self.active_field_layer = None;
            self.active_field_snapshot = None;
            self.active_field_render_settings = None;
            self.active_field_gpu_bytes = 0;
            self.active_field_resource_footprint = FieldResourceFootprint::default();
            self.show_isosurface = false;
            self.show_volume = false;
        }
        if let Some(index) = self
            .retained_field_layers
            .iter()
            .position(|layer| layer.layer_id == layer_id)
        {
            self.retained_field_layers.remove(index);
        }
        self.resident_field_gpu_bytes = self
            .retained_field_layers
            .iter()
            .try_fold(self.active_field_gpu_bytes, |total, layer| {
                total.checked_add(layer.gpu_bytes)
            })
            .unwrap_or(0);
    }

    pub fn set_field_layer_visibility(
        &mut self,
        layer_id: crate::volumetric::FieldLayerId,
        visible: bool,
    ) -> bool {
        if self
            .active_field_layer
            .is_some_and(|(active_layer_id, _)| active_layer_id == layer_id)
        {
            self.set_active_field_visibility(visible);
            if let Some(settings) = &mut self.active_field_render_settings {
                settings.visible = visible;
            }
            return true;
        }
        if let Some(layer) = self
            .retained_field_layers
            .iter_mut()
            .find(|layer| layer.layer_id == layer_id)
        {
            layer.render_settings.visible = visible;
            return true;
        }
        false
    }

    /// Capture field-only publication inputs without retaining mutable interactive state.
    fn frozen_field_draw_list(&self) -> FrozenFieldDrawList {
        let mut draw_list = FrozenFieldDrawList::empty();
        for layer in &self.retained_field_layers {
            if layer.render_settings.visible {
                let pushed = draw_list.push(layer.layer_id);
                debug_assert!(pushed);
            }
        }
        if self
            .active_field_render_settings
            .is_some_and(|settings| settings.visible)
        {
            if let Some((layer_id, _)) = self.active_field_layer {
                let pushed = draw_list.push(layer_id);
                debug_assert!(pushed);
            }
        }
        draw_list.sort();
        draw_list
    }

    pub fn field_publication_snapshot(&self) -> Option<FieldPublicationSnapshot> {
        let mut field_snapshots = Vec::new();
        let mut resource_footprint = FieldResourceFootprint::default();
        field_snapshots
            .try_reserve_exact(self.retained_field_layers.len().checked_add(1)?)
            .ok()?;
        for layer in &self.retained_field_layers {
            if layer.render_settings.visible {
                field_snapshots.push(layer.field_snapshot.clone());
                resource_footprint =
                    add_field_resource_footprint(resource_footprint, layer.resource_footprint)?;
            }
        }
        if self
            .active_field_render_settings
            .is_some_and(|settings| settings.visible)
        {
            field_snapshots.push(self.active_field_snapshot.as_ref()?.clone());
            resource_footprint = add_field_resource_footprint(
                resource_footprint,
                self.active_field_resource_footprint,
            )?;
        }
        field_snapshots.sort_unstable_by_key(|snapshot| snapshot.layer_id);
        let mut draw_list = FrozenFieldDrawList::empty();
        for snapshot in &field_snapshots {
            if !draw_list.push(snapshot.layer_id) {
                return None;
            }
        }
        let primary = field_snapshots.first()?.clone();
        if field_snapshots
            .iter()
            .any(|snapshot| snapshot.transparency_method != primary.transparency_method)
        {
            return None;
        }
        let field_scene_hash = field_scene_hash(&field_snapshots);
        let frozen = FieldPublicationSnapshot {
            field_snapshots,
            draw_list,
            representation: primary.representations.clone(),
            transfer_function: primary.transfer_function.clone(),
            clip_planes: primary.clip_planes.clone(),
            composition_method: primary.transparency_method,
            requested_field_capabilities: 1,
            selected_field_capabilities: 1,
            field_scene_hash,
            resource_footprint,
        };
        Some(frozen)
    }

    fn apply_field_render_settings(&mut self, settings: crate::volumetric::FieldRenderSettings) {
        self.active_colormap_mode = settings.colormap_mode;
        self.show_isosurface = settings.visible
            && (self.active_field_layer_pipeline.is_some()
                || self.active_negative_field_layer_pipeline.is_some());
        self.show_volume = settings.visible;
        self.volume_render_mode = match settings.render_mode {
            crate::volumetric::FieldRenderMode::Isosurface => RendererVolumeMode::Isosurface,
            crate::volumetric::FieldRenderMode::Volume => RendererVolumeMode::Volume,
            crate::volumetric::FieldRenderMode::Both => RendererVolumeMode::Both,
        };
        if let Some(iso) = &mut self.active_field_layer_pipeline {
            iso.set_color(&self.gpu.queue, settings.color);
            iso.set_color_negative(&self.gpu.queue, settings.color_negative);
            iso.set_opacity(&self.gpu.queue, settings.opacity);
            iso.set_sign_mode(&self.gpu.queue, 0);
        }
        if let Some(iso) = &mut self.active_negative_field_layer_pipeline {
            iso.set_color(&self.gpu.queue, settings.color);
            iso.set_color_negative(&self.gpu.queue, settings.color_negative);
            iso.set_opacity(&self.gpu.queue, settings.opacity);
            iso.set_sign_mode(&self.gpu.queue, 1);
        }
        if let Some(volume) = &mut self.volume_raycast_pipeline {
            volume.set_colormap(&self.gpu.queue, settings.colormap_mode);
            volume.set_signed_mapping(
                &self.gpu.queue,
                matches!(settings.sign_mode, crate::volumetric::FieldSignMode::Both),
            );
        }
    }

    pub fn set_active_field_render_mode(
        &mut self,
        render_mode: crate::volumetric::FieldRenderMode,
    ) -> bool {
        let Some(settings) = &mut self.active_field_render_settings else {
            return false;
        };
        settings.render_mode = render_mode;
        self.volume_render_mode = match render_mode {
            crate::volumetric::FieldRenderMode::Isosurface => RendererVolumeMode::Isosurface,
            crate::volumetric::FieldRenderMode::Volume => RendererVolumeMode::Volume,
            crate::volumetric::FieldRenderMode::Both => RendererVolumeMode::Both,
        };
        if let Some(snapshot) = &mut self.active_field_snapshot {
            snapshot.representations = field_representations(*settings, &snapshot.slices);
            let includes_isosurface = matches!(
                render_mode,
                crate::volumetric::FieldRenderMode::Isosurface
                    | crate::volumetric::FieldRenderMode::Both
            );
            snapshot.positive_isovalue = (includes_isosurface
                && !matches!(
                    settings.sign_mode,
                    crate::volumetric::FieldSignMode::Negative
                ))
            .then_some(settings.positive_isovalue);
            snapshot.negative_isovalue = (includes_isosurface
                && !matches!(
                    settings.sign_mode,
                    crate::volumetric::FieldSignMode::Positive
                ))
            .then_some(settings.negative_isovalue);
        }
        true
    }

    pub fn set_active_field_sign_mode(
        &mut self,
        sign_mode: crate::volumetric::FieldSignMode,
    ) -> bool {
        let resources_available = match sign_mode {
            crate::volumetric::FieldSignMode::Positive => {
                self.active_field_layer_pipeline.is_some()
            }
            crate::volumetric::FieldSignMode::Negative => {
                self.active_negative_field_layer_pipeline.is_some()
            }
            crate::volumetric::FieldSignMode::Both => {
                self.active_field_layer_pipeline.is_some()
                    && self.active_negative_field_layer_pipeline.is_some()
            }
        };
        if !resources_available {
            return false;
        }
        let Some(settings) = &mut self.active_field_render_settings else {
            return false;
        };
        settings.sign_mode = sign_mode;
        if let Some(snapshot) = &mut self.active_field_snapshot {
            snapshot.representations = field_representations(*settings, &snapshot.slices);
            let includes_isosurface = matches!(
                settings.render_mode,
                crate::volumetric::FieldRenderMode::Isosurface
                    | crate::volumetric::FieldRenderMode::Both
            );
            snapshot.positive_isovalue = (includes_isosurface
                && !matches!(sign_mode, crate::volumetric::FieldSignMode::Negative))
            .then_some(settings.positive_isovalue);
            snapshot.negative_isovalue = (includes_isosurface
                && !matches!(sign_mode, crate::volumetric::FieldSignMode::Positive))
            .then_some(settings.negative_isovalue);
        }
        if let Some(volume) = &mut self.volume_raycast_pipeline {
            volume.set_signed_mapping(
                &self.gpu.queue,
                matches!(sign_mode, crate::volumetric::FieldSignMode::Both),
            );
        }
        true
    }

    pub fn update_active_volume_transfer(
        &mut self,
        display_range: [f32; 2],
        opacity_scale: f32,
    ) -> bool {
        let Some(settings) = self.active_field_render_settings else {
            return false;
        };
        let Some(volume) = &mut self.volume_raycast_pipeline else {
            return false;
        };
        volume.update_transfer_function(
            &self.gpu.queue,
            display_range,
            (settings.opacity * opacity_scale).clamp(0.0, 1.0),
        );
        if let Some(snapshot) = &mut self.active_field_snapshot {
            snapshot.display_range = Some(display_range);
            snapshot.opacity_scale = opacity_scale;
        }
        true
    }

    pub fn update_active_volume_density_cutoff(&mut self, density_cutoff: f32) -> bool {
        let Some(volume) = &mut self.volume_raycast_pipeline else {
            return false;
        };
        volume.set_density_cutoff(&self.gpu.queue, density_cutoff);
        if let Some(snapshot) = &mut self.active_field_snapshot {
            snapshot.density_cutoff = density_cutoff;
        }
        true
    }

    pub fn set_active_isosurface_colors(&mut self, positive: [f32; 4], negative: [f32; 4]) -> bool {
        let Some(settings) = &mut self.active_field_render_settings else {
            return false;
        };
        settings.color = positive;
        settings.color_negative = negative;
        if let Some(snapshot) = &mut self.active_field_snapshot {
            snapshot.positive_color = positive;
            snapshot.negative_color = negative;
        }
        if let Some(iso) = &mut self.active_field_layer_pipeline {
            iso.set_color(&self.gpu.queue, positive);
            iso.set_color_negative(&self.gpu.queue, negative);
        }
        if let Some(iso) = &mut self.active_negative_field_layer_pipeline {
            iso.set_color(&self.gpu.queue, positive);
            iso.set_color_negative(&self.gpu.queue, negative);
        }
        true
    }

    /// Update isovalue threshold and trigger compute pass.
    pub fn update_isovalue(&mut self, grid_dims: [usize; 3], threshold: f32) {
        self.update_signed_isovalues(grid_dims, threshold, threshold);
    }

    pub fn update_signed_isovalues(
        &mut self,
        grid_dims: [usize; 3],
        positive_threshold: f32,
        negative_threshold: f32,
    ) {
        if self.active_field_layer_pipeline.is_none()
            && self.active_negative_field_layer_pipeline.is_none()
        {
            return;
        }
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Signed Isosurface Compute Encoder"),
            });
        if let Some(iso_pipe) = &mut self.active_field_layer_pipeline {
            self.isosurface_dispatch_size =
                iso_pipe.update_threshold(&self.gpu.queue, grid_dims, positive_threshold);
            iso_pipe.dispatch_compute(&mut encoder, self.isosurface_dispatch_size);
        }
        if let Some(iso_pipe) = &mut self.active_negative_field_layer_pipeline {
            let dispatch_size =
                iso_pipe.update_threshold(&self.gpu.queue, grid_dims, negative_threshold);
            iso_pipe.dispatch_compute(&mut encoder, dispatch_size);
        }
        self.gpu.queue.submit(std::iter::once(encoder.finish()));
    }

    pub fn update_active_isovalues_if_capacity(
        &mut self,
        grid_dims: [usize; 3],
        positive_threshold: f32,
        negative_threshold: f32,
        positive_vertices: u32,
        negative_vertices: u32,
    ) -> bool {
        let Some(settings) = self.active_field_render_settings else {
            return false;
        };
        let positive_fits = matches!(
            settings.sign_mode,
            crate::volumetric::FieldSignMode::Negative
        ) || self
            .active_field_layer_pipeline
            .as_ref()
            .is_some_and(|pipeline| positive_vertices <= pipeline.vertex_capacity());
        let negative_fits = matches!(
            settings.sign_mode,
            crate::volumetric::FieldSignMode::Positive
        ) || self
            .active_negative_field_layer_pipeline
            .as_ref()
            .is_some_and(|pipeline| negative_vertices <= pipeline.vertex_capacity());
        if !positive_fits || !negative_fits {
            return false;
        }
        self.update_signed_isovalues(grid_dims, positive_threshold, negative_threshold);
        if let Some(settings) = &mut self.active_field_render_settings {
            settings.isovalue = positive_threshold;
            settings.positive_isovalue = positive_threshold;
            settings.negative_isovalue = negative_threshold;
        }
        if let Some(snapshot) = &mut self.active_field_snapshot {
            if snapshot.positive_isovalue.is_some() {
                snapshot.positive_isovalue = Some(positive_threshold);
            }
            if snapshot.negative_isovalue.is_some() {
                snapshot.negative_isovalue = Some(negative_threshold);
            }
        }
        true
    }

    /// Update isosurface solid color.
    pub fn set_isosurface_color(&mut self, color: [f32; 4]) {
        if let Some(iso_pipe) = &mut self.active_field_layer_pipeline {
            iso_pipe.set_color(&self.gpu.queue, color);
        }
        if let Some(iso_pipe) = &mut self.active_negative_field_layer_pipeline {
            iso_pipe.set_color_negative(&self.gpu.queue, color);
        }
    }

    /// Update isosurface opacity.
    pub fn set_isosurface_opacity(&mut self, opacity: f32) -> bool {
        if self.active_field_render_settings.is_none() {
            return false;
        }
        if let Some(iso_pipe) = &mut self.active_field_layer_pipeline {
            iso_pipe.set_opacity(&self.gpu.queue, opacity);
        }
        if let Some(iso_pipe) = &mut self.active_negative_field_layer_pipeline {
            iso_pipe.set_opacity(&self.gpu.queue, opacity);
        }
        if let Some(settings) = &mut self.active_field_render_settings {
            settings.opacity = opacity;
            settings.color[3] = opacity;
            settings.color_negative[3] = opacity;
        }
        let volume_mapping = self.active_field_snapshot.as_mut().map(|snapshot| {
            snapshot.positive_color[3] = opacity;
            snapshot.negative_color[3] = opacity;
            (
                snapshot.display_range.unwrap_or(snapshot.scalar_range),
                snapshot.opacity_scale,
            )
        });
        if let (Some(volume), Some((display_range, opacity_scale))) =
            (&mut self.volume_raycast_pipeline, volume_mapping)
        {
            volume.update_transfer_function(
                &self.gpu.queue,
                display_range,
                (opacity * opacity_scale).clamp(0.0, 1.0),
            );
        }
        true
    }
}

#[cfg(test)]
mod publication_export_tests {
    use super::*;
    use crate::renderer::instance::BondInstance;

    fn test_camera(is_perspective: bool) -> Camera {
        Camera {
            eye: glam::Vec3::new(0.0, 0.0, 30.0),
            target: glam::Vec3::ZERO,
            up: glam::Vec3::Y,
            fovy_deg: 45.0,
            aspect: 1.0,
            znear: 0.1,
            zfar: 200.0,
            is_perspective,
            orthographic_scale: 30.0,
        }
    }

    fn assert_inside_framing_margin(camera: &Camera, point: glam::Vec3) {
        let clip =
            (camera.build_projection_matrix() * camera.build_view_matrix()) * point.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        let inner = 1.0 - 2.0 * PUBLICATION_FRAMING_MARGIN;
        assert!(ndc.x.abs() <= inner + 1.0e-5, "x={}", ndc.x);
        assert!(ndc.y.abs() <= inner + 1.0e-5, "y={}", ndc.y);
        assert!((0.0..=1.0).contains(&ndc.z), "z={}", ndc.z);
    }

    #[test]
    fn field_publication_plan_retiles_after_counting_resident_and_tile_local_resources() {
        let request = PublicationExportRequest {
            width: 2560,
            height: 1600,
            publication_bond_instance_count: 10,
            needs_transparent_depth: true,
            has_measurement_overlays: false,
            has_hopping_overlays: false,
            has_isosurface: true,
            has_volume: true,
            has_phonon_presentation: false,
            has_atom_drag: false,
            show_bz: false,
            has_measurement_state: false,
            has_selection_highlights: false,
            has_wannier_overlay: false,
            has_active_phonon_state: false,
        };
        let limits = PublicationExportLimits {
            max_texture_dimension_2d: 8192,
            max_buffer_size: 256 * 1024 * 1024,
            publication_msaa_x4: true,
        };
        let resources = FieldPublicationResources {
            layer_count: 1,
            has_slice: true,
            has_contour: true,
            footprint: FieldResourceFootprint {
                resident_field_bytes: MAX_ACTIVE_FIELD_GPU_BYTES,
                ..FieldResourceFootprint::default()
            },
            field_scene_hash: "field-publication-plan-test".to_owned(),
        };
        let full_frame_plan = PublicationRenderPlan {
            requested_samples: 4,
            selected_samples: 4,
            tile_dimensions: [request.width, request.height],
            tile_layout: [1, 1],
            tile_overlap_pixels: 0,
        };
        let full_frame_estimate = add_field_publication_resource_estimate(
            publication_export_resource_estimate(request, full_frame_plan).unwrap(),
            &resources,
        )
        .unwrap();
        assert!(
            full_frame_estimate.transient_gpu_bytes > MAX_PUBLICATION_TOTAL_GPU_BYTES,
            "the regression fixture must require field-aware tiling"
        );

        let plan = publication_render_plan(request, limits, Some(&resources)).unwrap();
        assert!(plan.tile_layout[0] > 1 || plan.tile_layout[1] > 1);
        let estimate = add_field_publication_resource_estimate(
            publication_export_resource_estimate(request, plan).unwrap(),
            &resources,
        )
        .unwrap();
        assert!(estimate.transient_gpu_bytes <= MAX_PUBLICATION_TOTAL_GPU_BYTES);
    }

    #[test]
    fn current_background_uses_actual_luminance() {
        let light =
            cell_line_style_for_background(PublicationBackground::Current, wgpu::Color::WHITE)
                .unwrap();
        let dark =
            cell_line_style_for_background(PublicationBackground::Current, wgpu::Color::BLACK)
                .unwrap();
        assert_eq!(light.cell_line_color_rgba, [0.18, 0.22, 0.28, 1.0]);
        assert_eq!(dark.cell_line_color_rgba, [0.82, 0.86, 0.92, 1.0]);
        assert!(
            cell_line_style_for_background(
                PublicationBackground::Current,
                wgpu::Color {
                    r: f64::NAN,
                    ..wgpu::Color::BLACK
                },
            )
            .is_err()
        );
        let converted = publication_srgb_rgba_to_linear([0.0, 0.04045, 1.0, 0.5]);
        assert_eq!(converted[0], 0.0);
        assert!((converted[1] - 0.003130805).abs() <= 1.0e-8);
        assert_eq!(converted[2], 1.0);
        assert_eq!(converted[3], 0.5);
    }

    #[test]
    fn orthographic_framing_includes_atoms_cell_lines_and_bonds() {
        let atoms = [AtomInstance {
            position: [0.0, 0.0, 0.0],
            radius: 1.0,
            color: [1.0; 4],
        }];
        let cell_lines = [LineVertex {
            position: [10.0, 0.0, 0.0],
            color: [1.0; 4],
        }];
        let bonds = [BondInstance {
            start: [0.0, -8.0, 0.0],
            radius: 0.5,
            end: [0.0, 8.0, 0.0],
            _pad: 0.0,
            color: [1.0; 4],
        }];
        let camera = fit_visible_structure_to_export(
            test_camera(false),
            1000,
            1000,
            &atoms,
            &[],
            &cell_lines,
            &bonds,
        )
        .unwrap();

        for point in [
            glam::Vec3::new(-1.0, 0.0, 0.0),
            glam::Vec3::new(10.0, 0.0, 0.0),
            glam::Vec3::new(0.0, -8.5, 0.0),
            glam::Vec3::new(0.0, 8.5, 0.0),
        ] {
            assert_inside_framing_margin(&camera, point);
        }
    }

    #[test]
    fn perspective_framing_uses_the_nearest_depth_extent() {
        let atoms = [
            AtomInstance {
                position: [-5.0, -3.0, 8.0],
                radius: 1.0,
                color: [1.0; 4],
            },
            AtomInstance {
                position: [5.0, 3.0, -8.0],
                radius: 1.0,
                color: [1.0; 4],
            },
        ];
        let camera =
            fit_visible_structure_to_export(test_camera(true), 1600, 900, &atoms, &[], &[], &[])
                .unwrap();

        for x in [-6.0, 6.0] {
            for y in [-4.0, 4.0] {
                for z in [-9.0, 9.0] {
                    assert_inside_framing_margin(&camera, glam::Vec3::new(x, y, z));
                }
            }
        }
    }
}
