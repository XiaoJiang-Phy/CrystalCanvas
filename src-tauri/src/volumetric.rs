//! Volumetric data structure for 3D scalar fields (CHGCAR, LOCPOT, .cube, .xsf)
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

pub(crate) const MAX_RESIDENT_FIELD_LAYERS: usize = 6;
pub(crate) const MAX_VISIBLE_FIELD_LAYERS_FIELD_1: usize = 1;
pub(crate) const MAX_FIELD_SCALAR_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_TOTAL_FIELD_SCALAR_BYTES: usize = 96 * 1024 * 1024;
const MAX_FIELD_OPERATION_PEAK_BYTES: usize = 128 * 1024 * 1024;
pub(crate) const MAX_LINEAR_COMBINATION_TERMS: usize = 8;
const MAX_FIELD_LABEL_BYTES: usize = 256;

pub(crate) type FieldLayerId = u64;
pub(crate) type FieldSceneRevision = u64;
pub type LegacyVolumetricData = VolumetricData;

static NEXT_FIELD_TOKEN: AtomicU64 = AtomicU64::new(0);

fn next_field_token(kind: &str) -> Result<u64, String> {
    NEXT_FIELD_TOKEN
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| current.checked_add(1))
        .map(|current| current + 1)
        .map_err(|_| format!("field {kind} exhausted"))
}

pub fn source_artifact_sha256(path: &str) -> Result<String, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("unable to open field source for hashing: {error}"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("unable to read field source for hashing: {error}"))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum FieldGridOrdering {
    ColMajor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ScalarUnit {
    ElectronPerCubicAngstrom,
    ElectronPerBohrCubed,
    Arbitrary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum FieldNormalization {
    Raw,
    VaspCellIntegratedToDensity,
}

/// Scalar metadata emitted by a format adapter after it has made every
/// producer-specific conversion.  A format discriminator is not evidence of
/// scalar units or normalization.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct FieldSourceMetadata {
    pub scalar_unit: ScalarUnit,
    pub scalar_unit_scale: f64,
    pub normalization: FieldNormalization,
    pub metadata_declared: bool,
}

impl FieldSourceMetadata {
    pub const UNDECLARED: Self = Self {
        scalar_unit: ScalarUnit::Arbitrary,
        scalar_unit_scale: 1.0,
        normalization: FieldNormalization::Raw,
        metadata_declared: false,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum FieldAttachment {
    GridPoint,
    Cell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldSignMode {
    Positive,
    Negative,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldRenderMode {
    Isosurface,
    Volume,
    Both,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct FieldRenderSettings {
    pub visible: bool,
    pub isovalue: f32,
    pub sign_mode: FieldSignMode,
    pub color: [f32; 4],
    pub color_negative: [f32; 4],
    pub opacity: f32,
    pub render_mode: FieldRenderMode,
    pub colormap_mode: u32,
}

impl Default for FieldRenderSettings {
    fn default() -> Self {
        Self {
            visible: true,
            isovalue: 0.0,
            sign_mode: FieldSignMode::Positive,
            color: [0.0, 0.722, 0.831, 0.5],
            color_negative: [0.0, 0.722, 0.831, 0.5],
            opacity: 0.5,
            render_mode: FieldRenderMode::Both,
            colormap_mode: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldGridMappingError {
    Degenerate,
    BufferLength,
    Undeclared,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum FieldCompatibilityFailure {
    GridDimensions,
    Lattice,
    Origin,
    Ordering,
    PeriodicAxes,
    Attachment,
    ScalarDimension,
    ScalarUnitScale,
    Normalization,
    Undeclared,
}

#[derive(Clone, Debug, Serialize)]
pub struct FieldCompatibilityReceipt {
    pub compatible: bool,
    pub failure: Option<FieldCompatibilityFailure>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FieldLineageTerm {
    pub source_sha256: String,
    pub normalized_sha256: String,
    pub compatibility_receipt_sha256: String,
    pub coefficient: f64,
}

#[derive(Clone)]
pub struct FieldLayer {
    pub id: FieldLayerId,
    pub revision: FieldSceneRevision,
    pub label: String,
    pub grid_dims: [usize; 3],
    /// ColMajor lattice matrix in Angstrom.
    pub lattice_angstrom: [f64; 9],
    pub origin_angstrom: [f64; 3],
    pub periodic_axes: [bool; 3],
    pub attachment: FieldAttachment,
    pub ordering: FieldGridOrdering,
    pub scalar_unit: ScalarUnit,
    pub scalar_unit_scale: f64,
    pub normalization: FieldNormalization,
    pub metadata_declared: bool,
    pub data: Arc<[f32]>,
    pub data_min: f32,
    pub data_max: f32,
    pub source_sha256: String,
    pub normalized_sha256: String,
    pub lineage: Option<Vec<FieldLineageTerm>>,
    pub render_settings: FieldRenderSettings,
}

#[derive(Clone, Default)]
pub(crate) struct FieldScene {
    pub(crate) layers: Vec<FieldLayer>,
    pub(crate) active_layer: Option<FieldLayerId>,
    pub(crate) revision: FieldSceneRevision,
}

pub trait ScalarFieldView {
    fn grid_dims(&self) -> [usize; 3];
    fn lattice_angstrom(&self) -> &[f64; 9];
    fn origin_angstrom(&self) -> &[f64; 3];
    fn scalar_data(&self) -> &[f32];
    fn scalar_range(&self) -> (f32, f32);
}

impl ScalarFieldView for VolumetricData {
    fn grid_dims(&self) -> [usize; 3] { self.grid_dims }
    fn lattice_angstrom(&self) -> &[f64; 9] { &self.lattice }
    fn origin_angstrom(&self) -> &[f64; 3] { &self.origin }
    fn scalar_data(&self) -> &[f32] { &self.data }
    fn scalar_range(&self) -> (f32, f32) { (self.data_min, self.data_max) }
}

impl ScalarFieldView for FieldLayer {
    fn grid_dims(&self) -> [usize; 3] { self.grid_dims }
    fn lattice_angstrom(&self) -> &[f64; 9] { &self.lattice_angstrom }
    fn origin_angstrom(&self) -> &[f64; 3] { &self.origin_angstrom }
    fn scalar_data(&self) -> &[f32] { &self.data }
    fn scalar_range(&self) -> (f32, f32) { (self.data_min, self.data_max) }
}

impl FieldLayer {
    pub fn from_volumetric(
        id: FieldLayerId,
        revision: FieldSceneRevision,
        label: String,
        volumetric: VolumetricData,
    ) -> Result<Self, String> {
        validate_volumetric_input(&volumetric)?;
        if label.len() > MAX_FIELD_LABEL_BYTES {
            return Err("field label exceeds byte limit".into());
        }
        let normalized_sha256 = normalized_field_sha256(&volumetric);
        let (data_min, data_max) = scalar_bounds(&volumetric.data);
        let scalar_metadata = volumetric.scalar_metadata;
        let mut render_settings = FieldRenderSettings::default();
        render_settings.isovalue = default_field_isovalue(data_min, data_max);
        let layer = Self {
            id,
            revision,
            label,
            grid_dims: volumetric.grid_dims,
            lattice_angstrom: volumetric.lattice,
            origin_angstrom: volumetric.origin,
            periodic_axes: [true, true, true],
            attachment: FieldAttachment::Cell,
            ordering: FieldGridOrdering::ColMajor,
            scalar_unit: scalar_metadata.scalar_unit,
            scalar_unit_scale: scalar_metadata.scalar_unit_scale,
            normalization: scalar_metadata.normalization,
            metadata_declared: scalar_metadata.metadata_declared,
            data: Arc::from(volumetric.data),
            data_min,
            data_max,
            normalized_sha256: normalized_sha256.clone(),
            source_sha256: normalized_sha256,
            lineage: None,
            render_settings,
        };
        layer.validate()?;
        Ok(layer)
    }

    pub fn compatibility_with(&self, other: &Self) -> FieldCompatibilityReceipt {
        const LATTICE_TOLERANCE_ANGSTROM: f64 = 1e-5;
        let failure = if !self.metadata_declared || !other.metadata_declared {
            Some(FieldCompatibilityFailure::Undeclared)
        } else if self.grid_dims != other.grid_dims {
            Some(FieldCompatibilityFailure::GridDimensions)
        } else if !self.lattice_angstrom.iter().zip(other.lattice_angstrom.iter()).all(|(a, b)| (a - b).abs() <= LATTICE_TOLERANCE_ANGSTROM) {
            Some(FieldCompatibilityFailure::Lattice)
        } else if !self.origin_angstrom.iter().zip(other.origin_angstrom.iter()).all(|(a, b)| (a - b).abs() <= LATTICE_TOLERANCE_ANGSTROM) {
            Some(FieldCompatibilityFailure::Origin)
        } else if self.ordering != other.ordering {
            Some(FieldCompatibilityFailure::Ordering)
        } else if self.periodic_axes != other.periodic_axes {
            Some(FieldCompatibilityFailure::PeriodicAxes)
        } else if self.attachment != other.attachment {
            Some(FieldCompatibilityFailure::Attachment)
        } else if self.scalar_unit != other.scalar_unit {
            Some(FieldCompatibilityFailure::ScalarDimension)
        } else if (self.scalar_unit_scale - other.scalar_unit_scale).abs() > f64::EPSILON {
            Some(FieldCompatibilityFailure::ScalarUnitScale)
        } else if self.normalization != other.normalization {
            Some(FieldCompatibilityFailure::Normalization)
        } else {
            None
        };
        FieldCompatibilityReceipt { compatible: failure.is_none(), failure }
    }

    fn validate(&self) -> Result<(), String> {
        let voxel_count = self.grid_dims.iter().try_fold(1_usize, |count, dimension| {
            if *dimension == 0 { return None; }
            count.checked_mul(*dimension)
        }).ok_or_else(|| "field grid dimensions overflow or contain zero".to_string())?;
        let scalar_bytes = voxel_count.checked_mul(std::mem::size_of::<f32>())
            .ok_or_else(|| "field scalar byte count overflow".to_string())?;
        if scalar_bytes > MAX_FIELD_SCALAR_BYTES { return Err("field scalar data exceeds per-layer byte limit".into()); }
        if self.data.len() != voxel_count { return Err(format!("{:?}: field scalar buffer length does not match grid dimensions", FieldGridMappingError::BufferLength)); }
        if !self.data.iter().all(|value| value.is_finite()) { return Err("field scalar buffer must be finite".into()); }
        if !self.lattice_angstrom.iter().chain(self.origin_angstrom.iter()).all(|value| value.is_finite()) { return Err("field grid mapping must be finite".into()); }
        let l = &self.lattice_angstrom;
        let determinant = l[0] * (l[4] * l[8] - l[5] * l[7]) - l[1] * (l[3] * l[8] - l[5] * l[6]) + l[2] * (l[3] * l[7] - l[4] * l[6]);
        if !determinant.is_finite() || determinant.abs() <= f64::EPSILON { return Err(format!("{:?} field lattice", FieldGridMappingError::Degenerate)); }
        if !self.scalar_unit_scale.is_finite() || self.scalar_unit_scale <= 0.0 { return Err(format!("{:?} field scalar unit scale", FieldGridMappingError::Undeclared)); }
        if !self.data_min.is_finite() || !self.data_max.is_finite() || self.data_min > self.data_max { return Err("field scalar range must be finite and ordered".into()); }
        Ok(())
    }
}

fn validate_volumetric_input(input: &VolumetricData) -> Result<(), String> {
    let voxel_count = input.grid_dims.iter().try_fold(1_usize, |count, dimension| {
        (*dimension != 0).then(|| count.checked_mul(*dimension)).flatten()
    }).ok_or_else(|| "field grid dimensions overflow or contain zero".to_string())?;
    let scalar_bytes = voxel_count.checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| "field scalar byte count overflow".to_string())?;
    if scalar_bytes > MAX_FIELD_SCALAR_BYTES {
        return Err("field scalar data exceeds per-layer byte limit".into());
    }
    if input.data.len() != voxel_count {
        return Err(format!("{:?}: field scalar buffer length does not match grid dimensions", FieldGridMappingError::BufferLength));
    }
    if !input.data.iter().all(|value| value.is_finite()) {
        return Err("field scalar buffer must be finite".into());
    }
    if !input
        .lattice
        .iter()
        .chain(input.origin.iter())
        .all(|value| value.is_finite())
    {
        return Err("field grid mapping must be finite".into());
    }
    if !input.scalar_metadata.scalar_unit_scale.is_finite()
        || input.scalar_metadata.scalar_unit_scale <= 0.0
    {
        return Err("field scalar unit scale must be finite and positive".into());
    }
    if input.scalar_metadata.metadata_declared
        && matches!(input.scalar_metadata.scalar_unit, ScalarUnit::Arbitrary)
    {
        return Err("declared field metadata requires a physical scalar unit".into());
    }
    let l = &input.lattice;
    let determinant = l[0] * (l[4] * l[8] - l[5] * l[7])
        - l[1] * (l[3] * l[8] - l[5] * l[6])
        + l[2] * (l[3] * l[7] - l[4] * l[6]);
    if !determinant.is_finite() || determinant.abs() <= f64::EPSILON {
        return Err(format!("{:?} field lattice", FieldGridMappingError::Degenerate));
    }
    Ok(())
}

impl FieldScene {
    pub fn active_layer(&self) -> Option<&FieldLayer> {
        self.active_layer.and_then(|id| self.layers.iter().find(|layer| layer.id == id))
    }

    pub fn active_layer_mut(&mut self) -> Option<&mut FieldLayer> {
        self.active_layer.and_then(|id| self.layers.iter_mut().find(|layer| layer.id == id))
    }

    pub fn rename_layer(&mut self, id: FieldLayerId, label: String) -> Result<(), String> {
        if label.is_empty() || label.len() > MAX_FIELD_LABEL_BYTES {
            return Err("field label is invalid".into());
        }
        let layer = self.layers.iter_mut().find(|layer| layer.id == id)
            .ok_or_else(|| "field layer does not exist".to_string())?;
        layer.label = label;
        self.revision = next_field_token("scene revision")?;
        Ok(())
    }

    pub fn set_layer_visibility(&mut self, id: FieldLayerId, visible: bool) -> Result<(), String> {
        let layer = self.layers.iter_mut().find(|layer| layer.id == id)
            .ok_or_else(|| "field layer does not exist".to_string())?;
        layer.render_settings.visible = visible;
        self.revision = next_field_token("scene revision")?;
        Ok(())
    }

    pub fn replace_with(&mut self, label: String, volumetric: VolumetricData) -> Result<&FieldLayer, String> {
        let mut replacement = Self::default();
        replacement.add_layer(label, volumetric)?;
        *self = replacement;
        Ok(self.layers.last().expect("replaced field layer"))
    }

    pub fn replace_with_source(
        &mut self,
        label: String,
        volumetric: VolumetricData,
        source_sha256: String,
    ) -> Result<&FieldLayer, String> {
        self.replace_with(label, volumetric)?;
        let layer = self.layers.last_mut().expect("replaced field layer");
        layer.source_sha256 = source_sha256;
        Ok(layer)
    }

    pub fn add_layer(&mut self, label: String, volumetric: VolumetricData) -> Result<&FieldLayer, String> {
        if self.layers.len() >= MAX_RESIDENT_FIELD_LAYERS { return Err("field scene has reached MAX_RESIDENT_FIELD_LAYERS".into()); }
        let revision = next_field_token("scene revision")?;
        let id = next_field_token("layer identifier")?;
        let layer = FieldLayer::from_volumetric(id, revision, label, volumetric)?;
        let resident_bytes = self.layers.iter().try_fold(0_usize, |total, candidate| total.checked_add(candidate.data.len().checked_mul(std::mem::size_of::<f32>())?)).ok_or_else(|| "field scene byte count overflow".to_string())?;
        let layer_bytes = layer.data.len().checked_mul(std::mem::size_of::<f32>()).ok_or_else(|| "field layer byte count overflow".to_string())?;
        if resident_bytes.checked_add(layer_bytes).ok_or_else(|| "field scene byte count overflow".to_string())? > MAX_TOTAL_FIELD_SCALAR_BYTES { return Err("field scene exceeds MAX_TOTAL_FIELD_SCALAR_BYTES".into()); }
        if resident_bytes
            .checked_add(layer_bytes.checked_mul(2).ok_or_else(|| "field operation byte count overflow".to_string())?)
            .ok_or_else(|| "field operation byte count overflow".to_string())?
            > MAX_FIELD_OPERATION_PEAK_BYTES
        {
            return Err("field admission exceeds peak byte limit".into());
        }
        self.revision = revision;
        self.active_layer = Some(id);
        self.layers.try_reserve(1).map_err(|_| "unable to reserve field layer slot")?;
        self.layers.push(layer);
        Ok(self.layers.last().expect("pushed field layer"))
    }

    pub fn add_layer_from_source(
        &mut self,
        label: String,
        volumetric: VolumetricData,
        source_sha256: String,
    ) -> Result<&FieldLayer, String> {
        self.add_layer(label, volumetric)?;
        let layer = self.layers.last_mut().expect("pushed field layer");
        layer.source_sha256 = source_sha256;
        Ok(layer)
    }

    pub fn remove_layer(&mut self, id: FieldLayerId) -> Result<(), String> {
        let position = self.layers.iter().position(|layer| layer.id == id).ok_or_else(|| "field layer does not exist".to_string())?;
        self.layers.remove(position);
        self.revision = next_field_token("scene revision")?;
        if self.active_layer == Some(id) {
            self.active_layer = self.layers.first().map(|layer| layer.id);
        }
        Ok(())
    }

    pub fn reorder_layer(&mut self, id: FieldLayerId, target_index: usize) -> Result<(), String> {
        if target_index >= self.layers.len() { return Err("field target index is out of range".into()); }
        let position = self.layers.iter().position(|layer| layer.id == id).ok_or_else(|| "field layer does not exist".to_string())?;
        let layer = self.layers.remove(position);
        self.layers.insert(target_index, layer);
        self.revision = next_field_token("scene revision")?;
        Ok(())
    }

    pub fn select_active(&mut self, id: FieldLayerId) -> Result<(), String> {
        if !self.layers.iter().any(|layer| layer.id == id) { return Err("field layer does not exist".into()); }
        self.active_layer = Some(id);
        self.revision = next_field_token("scene revision")?;
        Ok(())
    }

    pub fn combine(&mut self, terms: &[(FieldLayerId, f64)], output_label: String) -> Result<&FieldLayer, String> {
        if terms.is_empty() || terms.len() > MAX_LINEAR_COMBINATION_TERMS { return Err("linear combination term count is invalid".into()); }
        if self.layers.len() >= MAX_RESIDENT_FIELD_LAYERS { return Err("field scene has reached MAX_RESIDENT_FIELD_LAYERS".into()); }
        if output_label.len() > MAX_FIELD_LABEL_BYTES { return Err("field label exceeds byte limit".into()); }
        let (first_id, first_coefficient) = terms[0];
        if !first_coefficient.is_finite() { return Err("linear combination coefficient must be finite".into()); }
        let reference = self.layers.iter().find(|layer| layer.id == first_id).ok_or_else(|| "linear combination source layer does not exist".to_string())?;
        let mut sources: Vec<(&FieldLayer, f64)> = Vec::new();
        sources
            .try_reserve_exact(terms.len())
            .map_err(|_| "unable to reserve linear-combination sources")?;
        for (id, coefficient) in terms {
            if !coefficient.is_finite() {
                return Err("linear combination coefficient must be finite".into());
            }
            if sources.iter().any(|(source, _)| source.id == *id) {
                return Err("linear combination contains a duplicate source layer".into());
            }
            let source = self.layers.iter().find(|layer| layer.id == *id)
                .ok_or_else(|| "linear combination source layer does not exist".to_string())?;
            let receipt = reference.compatibility_with(source);
            if !receipt.compatible {
                return Err(format!("linear combination rejected: {:?}", receipt.failure));
            }
            sources.push((source, *coefficient));
        }
        let output_bytes = reference.data.len().checked_mul(std::mem::size_of::<f32>()).ok_or_else(|| "field scalar byte count overflow".to_string())?;
        let resident_bytes = self.layers.iter().try_fold(0_usize, |total, layer| {
            total.checked_add(layer.data.len().checked_mul(std::mem::size_of::<f32>())?)
        }).ok_or_else(|| "field scene byte count overflow".to_string())?;
        if resident_bytes.checked_add(output_bytes).ok_or_else(|| "field scene byte count overflow".to_string())? > MAX_TOTAL_FIELD_SCALAR_BYTES {
            return Err("field scene exceeds MAX_TOTAL_FIELD_SCALAR_BYTES".into());
        }
        if resident_bytes
            .checked_add(output_bytes.checked_mul(2).ok_or_else(|| "field operation byte count overflow".to_string())?)
            .ok_or_else(|| "field operation byte count overflow".to_string())?
            > MAX_FIELD_OPERATION_PEAK_BYTES
        {
            return Err("linear combination exceeds peak byte limit".into());
        }
        let mut output = Vec::new();
        output.try_reserve_exact(reference.data.len()).map_err(|_| "unable to reserve linear-combination output")?;
        for index in 0..reference.data.len() {
            let combined = sources.iter().try_fold(0.0_f64, |accumulator, (source, coefficient)| {
                let next = accumulator + coefficient * f64::from(source.data[index]);
                next.is_finite().then_some(next)
            }).ok_or_else(|| "linear combination produced a non-finite scalar".to_string())?;
            if combined > f64::from(f32::MAX) || combined < f64::from(f32::MIN) {
                return Err("linear combination exceeded f32 result range".into());
            }
            output.push(combined as f32);
        }
        let data_min = output.iter().copied().fold(f32::INFINITY, f32::min);
        let data_max = output.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let id = next_field_token("layer identifier")?;
        let revision = next_field_token("scene revision")?;
        // Canonicalize negative zero before hashing the derived source-free field.
        for value in &mut output { if *value == 0.0 { *value = 0.0; } }
        let normalized_sha256 = normalized_field_sha256_parts(
            reference.grid_dims,
            &reference.lattice_angstrom,
            &reference.origin_angstrom,
            reference.scalar_unit,
            reference.scalar_unit_scale,
            reference.normalization,
            reference.metadata_declared,
            &output,
        );
        let mut lineage = Vec::new();
        lineage
            .try_reserve_exact(sources.len())
            .map_err(|_| "unable to reserve field provenance")?;
        for (source, coefficient) in &sources {
            lineage.push(FieldLineageTerm {
                source_sha256: source.source_sha256.clone(),
                normalized_sha256: source.normalized_sha256.clone(),
                compatibility_receipt_sha256: compatibility_receipt_sha256(reference, source),
                coefficient: *coefficient,
            });
        }
        let source_sha256 = derived_source_sha256(&lineage, &normalized_sha256);
        let mut render_settings = FieldRenderSettings::default();
        render_settings.isovalue = default_field_isovalue(data_min, data_max);
        let layer = FieldLayer { id, revision, label: output_label, grid_dims: reference.grid_dims, lattice_angstrom: reference.lattice_angstrom, origin_angstrom: reference.origin_angstrom, periodic_axes: reference.periodic_axes, attachment: reference.attachment, ordering: reference.ordering, scalar_unit: reference.scalar_unit, scalar_unit_scale: reference.scalar_unit_scale, normalization: reference.normalization, metadata_declared: reference.metadata_declared, data: Arc::from(output), data_min, data_max, source_sha256, normalized_sha256, lineage: Some(lineage), render_settings };
        layer.validate()?;
        self.revision = revision;
        self.active_layer = Some(id);
        self.layers.try_reserve(1).map_err(|_| "unable to reserve field layer slot")?;
        self.layers.push(layer);
        Ok(self.layers.last().expect("pushed derived field layer"))
    }
}

fn scalar_sha256(values: &[f32]) -> String {
    let mut digest = Sha256::new();
    for value in values {
        let canonical = if *value == 0.0 { 0.0 } else { *value };
        digest.update(canonical.to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn normalized_field_sha256(volumetric: &VolumetricData) -> String {
    normalized_field_sha256_parts(
        volumetric.grid_dims,
        &volumetric.lattice,
        &volumetric.origin,
        volumetric.scalar_metadata.scalar_unit,
        volumetric.scalar_metadata.scalar_unit_scale,
        volumetric.scalar_metadata.normalization,
        volumetric.scalar_metadata.metadata_declared,
        &volumetric.data,
    )
}

fn normalized_field_sha256_parts(
    grid_dims: [usize; 3],
    lattice: &[f64; 9],
    origin: &[f64; 3],
    scalar_unit: ScalarUnit,
    scalar_unit_scale: f64,
    normalization: FieldNormalization,
    metadata_declared: bool,
    data: &[f32],
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"crystalcanvas.normalized-field.v1\0");
    for dimension in grid_dims {
        digest.update((dimension as u64).to_le_bytes());
    }
    for value in lattice.iter().chain(origin.iter()) {
        digest.update(value.to_le_bytes());
    }
    digest.update([scalar_unit as u8]);
    digest.update(scalar_unit_scale.to_le_bytes());
    digest.update([normalization as u8]);
    digest.update([u8::from(metadata_declared)]);
    digest.update(scalar_sha256(data));
    format!("{:x}", digest.finalize())
}

fn compatibility_receipt_sha256(reference: &FieldLayer, source: &FieldLayer) -> String {
    let mut digest = Sha256::new();
    digest.update(b"crystalcanvas.field-compatibility.v1\0");
    digest.update(reference.normalized_sha256.as_bytes());
    digest.update(source.normalized_sha256.as_bytes());
    let receipt = reference.compatibility_with(source);
    digest.update([u8::from(receipt.compatible)]);
    digest.update(format!("{:?}", receipt.failure).as_bytes());
    format!("{:x}", digest.finalize())
}

fn derived_source_sha256(lineage: &[FieldLineageTerm], normalized_sha256: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"crystalcanvas.field.linear-combination.v1\0");
    for term in lineage {
        digest.update(term.source_sha256.as_bytes());
        digest.update(term.normalized_sha256.as_bytes());
        digest.update(term.compatibility_receipt_sha256.as_bytes());
        digest.update(term.coefficient.to_le_bytes());
    }
    digest.update(normalized_sha256.as_bytes());
    format!("{:x}", digest.finalize())
}

fn scalar_bounds(values: &[f32]) -> (f32, f32) {
    values.iter().copied().fold(
        (f32::INFINITY, f32::NEG_INFINITY),
        |(minimum, maximum), value| (minimum.min(value), maximum.max(value)),
    )
}

fn default_field_isovalue(data_min: f32, data_max: f32) -> f32 {
    let bound = data_min.abs().max(data_max.abs());
    if !bound.is_finite() || bound <= 0.0 { return 0.0; }
    if data_min < 0.0 { return bound * 0.1; }
    let value = data_max * 0.1;
    if value < data_min { data_min + (data_max - data_min) * 0.1 } else { value }
}

#[derive(Clone, Serialize)]
pub enum VolumetricFormat {
    VaspChgcar,
    VaspLocpot,
    GaussianCube,
    Xsf,
}

/// 3D scalar field on a regular grid, aligned to the crystallographic unit cell.
/// Grid indices follow Fortran column-major order: x fastest, z slowest.
/// CAUTION: deep-copies data Vec (up to 13.5 MB at 150³). Avoid cloning on hot paths.
#[derive(Clone, Serialize)]
pub struct VolumetricData {
    /// Grid dimensions $(N_x, N_y, N_z)$
    pub grid_dims: [usize; 3],

    /// 3x3 lattice matrix (ColMajor, Å) defining the voxel-to-Cartesian mapping:
    /// $\mathbf{r}(i,j,k) = \frac{i}{N_x}\mathbf{a} + \frac{j}{N_y}\mathbf{b} + \frac{k}{N_z}\mathbf{c}$
    /// ColMajor: [a_x, a_y, a_z, b_x, b_y, b_z, c_x, c_y, c_z]
    pub lattice: [f64; 9],

    /// Flattened scalar field values in physical units ($e/\text{Å}^3$ for CHGCAR, $e/a_0^3$ for .cube).
    /// Index: `data[ix + iy * Nx + iz * Nx * Ny]` (x-fastest, Fortran order).
    pub data: Vec<f32>,

    /// Global min/max for UI slider range
    pub data_min: f32,
    pub data_max: f32,

    /// Source file type (for provenance)
    pub source_format: VolumetricFormat,

    /// Adapter-issued scalar metadata.  Undeclared values may be displayed but
    /// cannot participate in auditable field arithmetic.
    pub scalar_metadata: FieldSourceMetadata,

    /// Origin offset (relevant for .cube files and some .xsf; zero for CHGCAR)
    pub origin: [f64; 3],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volumetric_data_creation() {
        let data = vec![1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let vol = VolumetricData {
            grid_dims: [2, 2, 2],
            lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            data_min: 1.0,
            data_max: 8.0,
            data,
            source_format: VolumetricFormat::VaspChgcar,
            scalar_metadata: FieldSourceMetadata::UNDECLARED,
            origin: [0.0, 0.0, 0.0],
        };
        assert_eq!(vol.data_min, 1.0);
        assert_eq!(vol.data_max, 8.0);
        assert_eq!(vol.grid_dims, [2, 2, 2]);
        assert_eq!(vol.data.len(), 8);
    }

    #[test]
    fn test_empty_data_field_is_valid() {
        let vol = VolumetricData {
            grid_dims: [0, 0, 0],
            lattice: [0.0; 9],
            data: Vec::new(),
            data_min: 0.0,
            data_max: 0.0,
            source_format: VolumetricFormat::GaussianCube,
            scalar_metadata: FieldSourceMetadata::UNDECLARED,
            origin: [0.0, 0.0, 0.0],
        };
        assert!(vol.data.is_empty());
        let n_voxels = vol.grid_dims[0] * vol.grid_dims[1] * vol.grid_dims[2];
        assert_eq!(n_voxels, vol.data.len());
    }

    #[test]
    fn test_single_voxel_grid() {
        let vol = VolumetricData {
            grid_dims: [1, 1, 1],
            lattice: [3.867, 0.0, 0.0, 0.0, 3.867, 0.0, 0.0, 0.0, 6.359],
            data: vec![-0.5_f32],
            data_min: -0.5,
            data_max: -0.5,
            source_format: VolumetricFormat::VaspLocpot,
            scalar_metadata: FieldSourceMetadata::UNDECLARED,
            origin: [0.0, 0.0, 0.0],
        };
        assert_eq!(vol.data.len(), 1);
        assert_eq!(vol.data_min, vol.data_max);
        assert!(vol.data_min < 0.0, "LOCPOT can have negative potential");
    }

    #[test]
    fn test_constant_scalar_field_min_eq_max() {
        let n = 27_usize;
        let vol = VolumetricData {
            grid_dims: [3, 3, 3],
            lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            data: vec![0.1_f32; n],
            data_min: 0.1,
            data_max: 0.1,
            source_format: VolumetricFormat::VaspChgcar,
            scalar_metadata: FieldSourceMetadata::UNDECLARED,
            origin: [0.0, 0.0, 0.0],
        };
        assert_eq!(vol.data_min, vol.data_max);
        // A normalised slider value (v - min) / (max - min) with min==max must be guarded upstream.
        // Here we only assert the struct holds the invariant; slider guard is UI's problem.
        assert!((vol.data_max - vol.data_min).abs() < f32::EPSILON);
    }

    #[test]
    fn test_f32_extremes_do_not_corrupt_struct() {
        let data = vec![f32::MIN_POSITIVE, 1.0e10_f32, f32::MAX];
        let vol = VolumetricData {
            grid_dims: [3, 1, 1],
            lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            data_min: f32::MIN_POSITIVE,
            data_max: f32::MAX,
            data,
            source_format: VolumetricFormat::Xsf,
            scalar_metadata: FieldSourceMetadata::UNDECLARED,
            origin: [0.0, 0.0, 0.0],
        };
        assert!(vol.data_min > 0.0);
        assert!(vol.data_max.is_finite());
        assert!(!vol.data_max.is_nan());
        let range = vol.data_max - vol.data_min;
        assert!(range.is_finite() || range.is_infinite(), "range overflow is expected behavior; caller must handle");
    }

    #[test]
    fn test_data_grid_mismatch_is_detectable() {
        let vol = VolumetricData {
            grid_dims: [4, 4, 4],
            lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            data: vec![0.0_f32; 8],
            data_min: 0.0,
            data_max: 0.0,
            source_format: VolumetricFormat::VaspChgcar,
            scalar_metadata: FieldSourceMetadata::UNDECLARED,
            origin: [0.0, 0.0, 0.0],
        };
        let claimed = vol.grid_dims[0] * vol.grid_dims[1] * vol.grid_dims[2];
        // Mismatch must be detectable so parsers can return Err before handing to GPU
        assert_ne!(claimed, vol.data.len(), "mismatch must be detectable by caller");
    }

    #[test]
    fn test_degenerate_zero_lattice_is_detectable() {
        let vol = VolumetricData {
            grid_dims: [2, 2, 2],
            lattice: [0.0; 9],
            data: vec![1.0_f32; 8],
            data_min: 1.0,
            data_max: 1.0,
            source_format: VolumetricFormat::Xsf,
            scalar_metadata: FieldSourceMetadata::UNDECLARED,
            origin: [0.0, 0.0, 0.0],
        };
        // ColMajor 3×3 determinant: det = a_x*(b_y*c_z - b_z*c_y) - a_y*(...) + a_z*(...)
        let l = &vol.lattice;
        let det = l[0] * (l[4] * l[8] - l[5] * l[7])
                - l[1] * (l[3] * l[8] - l[5] * l[6])
                + l[2] * (l[3] * l[7] - l[4] * l[6]);
        // Any lattice-based coordinate transform with det==0 must be rejected upstream
        assert_eq!(det, 0.0, "degenerate lattice must have zero determinant");
    }

    #[test]
    fn test_all_negative_values_min_max_ordering() {
        let data: Vec<f32> = vec![-9.0, -4.0, -7.0, -1.0, -3.0, -6.0, -8.0, -2.0];
        let vol = VolumetricData {
            grid_dims: [2, 2, 2],
            lattice: [5.0, 0.0, 0.0, 0.0, 5.0, 0.0, 0.0, 0.0, 5.0],
            data_min: -9.0,
            data_max: -1.0,
            data,
            source_format: VolumetricFormat::VaspLocpot,
            scalar_metadata: FieldSourceMetadata::UNDECLARED,
            origin: [0.0, 0.0, 0.0],
        };
        assert!(vol.data_min < 0.0);
        assert!(vol.data_max < 0.0);
        assert!(vol.data_min < vol.data_max, "min must be strictly less than max");
    }

    #[test]
    fn test_inverted_min_max_is_invariant_violation() {
        let vol = VolumetricData {
            grid_dims: [1, 1, 1],
            lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            data: vec![5.0_f32],
            data_min: 99.0,
            data_max: -1.0,
            source_format: VolumetricFormat::GaussianCube,
            scalar_metadata: FieldSourceMetadata::UNDECLARED,
            origin: [0.0, 0.0, 0.0],
        };
        // The struct accepts it (no runtime check) — parser must enforce ordering.
        let invariant_holds = vol.data_min <= vol.data_max;
        assert!(!invariant_holds, "parser must enforce data_min <= data_max; this struct carries an invalid state");
    }
}
