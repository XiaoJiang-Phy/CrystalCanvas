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

const MAX_PUBLICATION_READBACK_BYTES: u64 = 192 * 1024 * 1024;
const MAX_PUBLICATION_TOTAL_GPU_BYTES: u64 = 384 * 1024 * 1024;
const MAX_PUBLICATION_PEAK_CPU_BYTES: u64 = 384 * 1024 * 1024;
const PUBLICATION_EXPORT_CAMERA_UNIFORM_BYTES: u64 = 256;
const PUBLICATION_EXPORT_GPU_DRIVER_RESERVE_BYTES: u64 = 16 * 1024 * 1024;
const PUBLICATION_EXPORT_CPU_ENCODER_RESERVE_BYTES: u64 = 16 * 1024 * 1024;
const PUBLICATION_EXPORT_ENCODED_OVERHEAD_BYTES: u64 = 1024 * 1024;
pub(crate) const MAX_PUBLICATION_RECIPE_BYTES: u64 = 1024 * 1024;
pub const PUBLICATION_EXPORT_POLICY_VERSION: u32 = 4;

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
    sample_count: u32,
    readback_layout: OffscreenReadbackLayout,
    admission: PublicationExportAdmissionReceipt,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PublicationExportResourceEstimate {
    pub staging_bytes: u64,
    pub rgba_bytes: u64,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PublicationExportAdmissionReceipt {
    pub(crate) policy_version: u32,
    pub(crate) request: PublicationExportRequest,
    pub(crate) limits: PublicationExportLimits,
    pub(crate) budgets: PublicationExportBudgets,
    pub(crate) estimate: PublicationExportResourceEstimate,
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

fn publication_export_resource_estimate(
    width: u32,
    height: u32,
    needs_transparent_depth: bool,
) -> Result<PublicationExportResourceEstimate, PublicationExportRejection> {
    let layout = offscreen_readback_layout(width, height)?;
    let rgba_bytes =
        u64::try_from(layout.rgba_len).map_err(|_| PublicationExportRejection::CpuByteOverflow)?;
    let color_bytes = rgba_bytes;
    let opaque_depth_bytes = rgba_bytes;
    let transparent_depth_bytes = needs_transparent_depth.then_some(rgba_bytes).unwrap_or(0);
    let budgets = publication_export_budgets();
    let max_encoded_bytes = rgba_bytes
        .checked_add(budgets.encoded_overhead_bytes)
        .ok_or(PublicationExportRejection::PeakCpuBudget)?;
    let total_gpu_bytes = color_bytes
        .checked_add(opaque_depth_bytes)
        .and_then(|value| value.checked_add(transparent_depth_bytes))
        .and_then(|value| value.checked_add(layout.staging_size))
        .and_then(|value| value.checked_add(PUBLICATION_EXPORT_CAMERA_UNIFORM_BYTES))
        .and_then(|value| value.checked_add(budgets.gpu_driver_reserve_bytes))
        .ok_or(PublicationExportRejection::TransientGpuBudget)?;
    let readback_peak_cpu_bytes = layout
        .staging_size
        .checked_add(rgba_bytes)
        .ok_or(PublicationExportRejection::PeakCpuBudget)?;
    let jpeg_rgb_bytes = rgba_bytes
        .checked_div(4)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or(PublicationExportRejection::PeakCpuBudget)?;
    let png_encode_peak_cpu_bytes = rgba_bytes
        .checked_add(budgets.cpu_encoder_reserve_bytes)
        .ok_or(PublicationExportRejection::PeakCpuBudget)?;
    let jpeg_encode_peak_cpu_bytes = rgba_bytes
        .checked_add(jpeg_rgb_bytes)
        .and_then(|value| value.checked_add(budgets.cpu_encoder_reserve_bytes))
        .ok_or(PublicationExportRejection::PeakCpuBudget)?;
    let peak_cpu_bytes = readback_peak_cpu_bytes
        .max(png_encode_peak_cpu_bytes)
        .max(jpeg_encode_peak_cpu_bytes);

    Ok(PublicationExportResourceEstimate {
        staging_bytes: layout.staging_size,
        rgba_bytes,
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
    pub(crate) fn publication_export_request(
        &self,
        width: u32,
        height: u32,
        source_state: PublicationExportSourceState,
    ) -> PublicationExportRequest {
        PublicationExportRequest {
            width,
            height,
            needs_transparent_depth: self.transparent_instance_count > 0,
            has_measurement_overlays: self.measurement_line_count > 0,
            has_hopping_overlays: self.hopping_instance_count > 0,
            has_isosurface: self.isosurface_pipeline.is_some(),
            has_volume: self.volume_raycast_pipeline.is_some(),
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
        }
    }

    pub(crate) fn publication_render_config(
        &self,
        admission: &PublicationExportAdmissionReceipt,
        background: PublicationBackground,
    ) -> Result<PublicationRenderConfig, String> {
        self.validate_publication_export_receipt(admission)?;
        let width = admission.request.width;
        let height = admission.request.height;
        let target_format = self.gpu.surface_format();
        if !target_format.is_srgb() {
            return Err(format!(
                "publication render requires an sRGB target format, got {target_format:?}"
            ));
        }

        let background = match background {
            PublicationBackground::Transparent => wgpu::Color::TRANSPARENT,
            PublicationBackground::White => wgpu::Color::WHITE,
            PublicationBackground::Black => wgpu::Color::BLACK,
            PublicationBackground::Current => self.clear_color,
        };
        let mut camera = Camera {
            eye: self.camera.eye,
            target: self.camera.target,
            up: self.camera.up,
            fovy_deg: self.camera.fovy_deg,
            aspect: self.camera.aspect,
            znear: self.camera.znear,
            zfar: self.camera.zfar,
            is_perspective: self.camera.is_perspective,
            orthographic_scale: self.camera.orthographic_scale,
        };
        camera.set_aspect(width as f32, height as f32);

        Ok(PublicationRenderConfig {
            width,
            height,
            camera,
            background,
            alpha_mode: PublicationAlphaMode::Premultiplied,
            target_format,
            sample_count: 1,
            readback_layout: offscreen_readback_layout(width, height)
                .map_err(|error| error.to_string())?,
            admission: *admission,
        })
    }
}

#[must_use]
pub fn evaluate_publication_export_admission(
    request: PublicationExportRequest,
    limits: PublicationExportLimits,
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
    if request.has_isosurface {
        return Err(PublicationExportRejection::Isosurface);
    }
    if request.has_volume {
        return Err(PublicationExportRejection::Volume);
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
    if request.width > limits.max_texture_dimension_2d
        || request.height > limits.max_texture_dimension_2d
    {
        return Err(PublicationExportRejection::TextureDimensionLimit);
    }

    let budgets = publication_export_budgets();
    let estimate = publication_export_resource_estimate(
        request.width,
        request.height,
        request.needs_transparent_depth,
    )?;
    if estimate.staging_bytes > limits.max_buffer_size {
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
        estimate,
    })
}

pub(crate) fn validate_publication_export_receipt_fields(
    receipt: &PublicationExportAdmissionReceipt,
) -> Result<(), PublicationExportRejection> {
    if receipt.policy_version != PUBLICATION_EXPORT_POLICY_VERSION {
        return Err(PublicationExportRejection::PolicyVersion);
    }
    let expected = evaluate_publication_export_admission(receipt.request, receipt.limits)?;
    if expected != *receipt {
        return Err(PublicationExportRejection::ReceiptMismatch);
    }
    Ok(())
}

impl Renderer {
    fn validate_publication_export_receipt(
        &self,
        receipt: &PublicationExportAdmissionReceipt,
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

    /// Render the admitted structure scene through an export-owned configuration.
    pub(crate) fn render_offscreen(
        &self,
        config: &PublicationRenderConfig,
    ) -> Result<PublicationRenderResult, String> {
        self.validate_publication_export_receipt(&config.admission)?;
        if config.width != config.admission.request.width
            || config.height != config.admission.request.height
        {
            return Err("publication render dimensions do not match admission".to_owned());
        }
        if config.target_format != self.gpu.surface_format() {
            return Err("publication render target format changed after configuration".to_owned());
        }
        if config.sample_count != 1 {
            return Err("EXPORT-1B supports only single-sample publication targets".to_owned());
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
        export_camera_uniform.update_from_camera(&config.camera);
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
                    layout: &self.camera_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: export_camera_buffer.as_entire_binding(),
                    }],
                });

        let tex_format = config.target_format;

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

        let needs_transparent = config.admission.request.needs_transparent_depth;

        let (offscreen_opaque_depth, offscreen_opaque_depth_view) =
            pipeline::create_depth_texture(&self.gpu.device, width, height);
        let offscreen_transparent_depth = needs_transparent
            .then(|| pipeline::create_transparent_depth_texture(&self.gpu.device, width, height));

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

            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &export_camera_bind_group, &[]);
            if self.instance_count > 0 {
                pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
                pass.draw(0..6, 0..self.instance_count);
            }

            if self.show_cell && self.cell_line_count > 0 {
                pass.set_pipeline(&self.line_pipeline);
                pass.set_bind_group(0, &export_camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.cell_line_buffer.slice(..));
                pass.draw(0..self.cell_line_count, 0..1);
            }

            if self.show_bonds && self.bond_instance_count > 0 {
                pass.set_pipeline(&self.bond_pipeline);
                pass.set_bind_group(0, &export_camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.bond_instance_buffer.slice(..));
                pass.draw(0..72, 0..self.bond_instance_count);
            }
        }

        // ═══ Offscreen Pass 2: Transparent structure atoms ═══
        if let Some((offscreen_transparent_depth, offscreen_transparent_depth_view)) =
            &offscreen_transparent_depth
        {
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

                if self.transparent_instance_count > 0 {
                    pass.set_pipeline(&self.transparent_pipeline);
                    pass.set_bind_group(0, &export_camera_bind_group, &[]);
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
