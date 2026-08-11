//! [Overview: Validated FIGURE-2 field representations, clipping, slices, contours, and transfer functions.]
//! Implementation: bounded renderer-owned snapshot derived from immutable column-major scalar layers.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::volumetric::{
    AxisSampling, FieldGridMapping, FieldGridOrdering, FieldLayerId, FieldSceneRevision,
    ScalarFieldView,
};

pub const MAX_VISIBLE_FIELD_LAYERS_FIGURE_2: usize = 4;
pub const MAX_FIELD_REPRESENTATIONS: usize = 5;
pub const MAX_FIELD_CLIP_PLANES: usize = 6;
pub const MAX_FIELD_SLICES: usize = 4;
pub const MAX_FIELD_CONTOUR_LEVELS: usize = 32;
pub const MAX_FIELD_TRANSFER_POINTS: usize = 16;
pub const MAX_FIELD_SLICE_DIMENSION: usize = 512;
const MAX_FIELD_SLICE_SAMPLES: usize = MAX_FIELD_SLICE_DIMENSION * MAX_FIELD_SLICE_DIMENSION;
const MAX_FIELD_CONTOUR_SEGMENTS: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldRepresentation {
    PositiveIsosurface,
    NegativeIsosurface,
    VolumeRaycast,
    Slice,
    Contour,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldSignIdentity {
    Positive,
    Negative,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldSliceInterpolation {
    Trilinear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldContourAlgorithm {
    MarchingSquares,
    AsymptoticDecider,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldTransparencyMethod {
    WeightedBlendedOit,
    PremultipliedAlphaFallback,
}

/// The field material is explicit serialized state; unlit bypasses all lighting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldMaterialMode {
    #[default]
    Lit,
    Unlit,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct FieldClipPlane {
    pub normal: [f64; 3],
    pub signed_offset_angstrom: f64,
    pub keep_positive: bool,
}

impl FieldClipPlane {
    pub fn normalized(
        normal: [f64; 3],
        signed_offset_angstrom: f64,
        keep_positive: bool,
    ) -> Result<Self, String> {
        if !normal.iter().all(|value| value.is_finite()) || !signed_offset_angstrom.is_finite() {
            return Err("field clip plane must be finite".to_owned());
        }
        let length_squared = normal.iter().map(|value| value * value).sum::<f64>();
        if !length_squared.is_finite() || length_squared <= f64::EPSILON {
            return Err("field clip plane normal is degenerate".to_owned());
        }
        let inverse_length = length_squared.sqrt().recip();
        Ok(Self {
            normal: [
                normal[0] * inverse_length,
                normal[1] * inverse_length,
                normal[2] * inverse_length,
            ],
            signed_offset_angstrom,
            keep_positive,
        })
    }

    #[must_use]
    pub fn keeps(&self, world_position: [f64; 3]) -> bool {
        let signed_distance = self.normal[0].mul_add(
            world_position[0],
            self.normal[1].mul_add(world_position[1], self.normal[2] * world_position[2]),
        ) - self.signed_offset_angstrom;
        if self.keep_positive {
            signed_distance >= 0.0
        } else {
            signed_distance <= 0.0
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct FieldSlicePlane {
    pub normal: [f64; 3],
    pub signed_offset_angstrom: f64,
    pub interpolation: FieldSliceInterpolation,
}

/// One bounded planar scalar sample and its optional contour levels.  The
/// plane is expressed in the normalized renderer coordinate system (Å).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FieldSliceRequest {
    pub plane: FieldSlicePlane,
    pub dimensions: [usize; 2],
    #[serde(default)]
    pub contour_levels: Vec<f32>,
}

impl FieldSliceRequest {
    pub fn validate(&self) -> Result<(), String> {
        FieldSlicePlane::normalized(
            self.plane.normal,
            self.plane.signed_offset_angstrom,
            self.plane.interpolation,
        )?;
        if self.dimensions[0] < 2
            || self.dimensions[1] < 2
            || self.dimensions[0] > MAX_FIELD_SLICE_DIMENSION
            || self.dimensions[1] > MAX_FIELD_SLICE_DIMENSION
            || self.dimensions[0]
                .checked_mul(self.dimensions[1])
                .filter(|count| *count <= MAX_FIELD_SLICE_SAMPLES)
                .is_none()
        {
            return Err("field slice dimensions are invalid".to_owned());
        }
        if self.contour_levels.len() > MAX_FIELD_CONTOUR_LEVELS {
            return Err("field contour level count is invalid".to_owned());
        }
        let mut previous = f32::NEG_INFINITY;
        for &level in &self.contour_levels {
            if !level.is_finite() || level <= previous {
                return Err(
                    "field contour levels must be finite and strictly increasing".to_owned(),
                );
            }
            previous = level;
        }
        Ok(())
    }
}

impl FieldSlicePlane {
    pub fn normalized(
        normal: [f64; 3],
        signed_offset_angstrom: f64,
        interpolation: FieldSliceInterpolation,
    ) -> Result<Self, String> {
        let clip = FieldClipPlane::normalized(normal, signed_offset_angstrom, true)?;
        Ok(Self {
            normal: clip.normal,
            signed_offset_angstrom,
            interpolation,
        })
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct FieldTransferControlPoint {
    pub position: f32,
    pub color_linear_rgba: [f32; 4],
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FieldTransferFunction {
    /// LinearRgb color interpolation and linear opacity interpolation are fixed by FIGURE-2.
    pub color_space: String,
    pub negative_control_points: Vec<FieldTransferControlPoint>,
    pub positive_control_points: Vec<FieldTransferControlPoint>,
}

impl Default for FieldTransferFunction {
    fn default() -> Self {
        Self {
            color_space: "LinearRgb".to_owned(),
            negative_control_points: vec![
                FieldTransferControlPoint {
                    position: 0.0,
                    color_linear_rgba: [0.13, 0.20, 0.70, 0.0],
                },
                FieldTransferControlPoint {
                    position: 1.0,
                    color_linear_rgba: [0.18, 0.42, 1.0, 0.65],
                },
            ],
            positive_control_points: vec![
                FieldTransferControlPoint {
                    position: 0.0,
                    color_linear_rgba: [0.95, 0.45, 0.04, 0.0],
                },
                FieldTransferControlPoint {
                    position: 1.0,
                    color_linear_rgba: [1.0, 0.73, 0.18, 0.65],
                },
            ],
        }
    }
}

impl FieldTransferFunction {
    pub fn validate(&self) -> Result<(), String> {
        if self.color_space != "LinearRgb" {
            return Err("field transfer color space must be LinearRgb".to_owned());
        }
        for branch in [&self.negative_control_points, &self.positive_control_points] {
            if branch.len() < 2 || branch.len() > MAX_FIELD_TRANSFER_POINTS {
                return Err("field transfer control point count is invalid".to_owned());
            }
            let mut previous = -1.0_f32;
            for point in branch {
                if !point.position.is_finite()
                    || !(0.0..=1.0).contains(&point.position)
                    || !point
                        .color_linear_rgba
                        .iter()
                        .all(|value| value.is_finite())
                    || point
                        .color_linear_rgba
                        .iter()
                        .any(|value| !(0.0..=1.0).contains(value))
                    || point.position <= previous
                {
                    return Err(
                        "field transfer control points must be finite and strictly increasing"
                            .to_owned(),
                    );
                }
                previous = point.position;
            }
        }
        Ok(())
    }

    pub fn maximum_opacity(&self) -> Result<f32, String> {
        self.validate()?;
        self.negative_control_points
            .iter()
            .chain(&self.positive_control_points)
            .map(|point| point.color_linear_rgba[3])
            .reduce(f32::max)
            .ok_or_else(|| "field transfer function has no control points".to_owned())
    }
}

/// Persisted FIGURE-2 presentation inputs owned by one source layer. The
/// scalar array remains immutable; changing this state only rebuilds derived
/// renderer resources for the same layer revision.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FieldPresentationSettings {
    pub clip_planes: Vec<FieldClipPlane>,
    #[serde(default)]
    pub slices: Vec<FieldSliceRequest>,
    pub transfer_function: FieldTransferFunction,
    #[serde(default)]
    pub use_explicit_transfer_function: bool,
    /// `None` preserves the complete finite source range.  A display range is
    /// presentation state and never rescales source scalar data.
    #[serde(default)]
    pub display_range: Option<[f32; 2]>,
    #[serde(default = "default_field_opacity_scale")]
    pub opacity_scale: f32,
    #[serde(default)]
    pub density_cutoff: f32,
    #[serde(default = "default_field_transparency_method")]
    pub transparency_method: FieldTransparencyMethod,
    #[serde(default)]
    pub field_material_mode: FieldMaterialMode,
}

const fn default_field_opacity_scale() -> f32 {
    3.0
}

const fn default_field_transparency_method() -> FieldTransparencyMethod {
    FieldTransparencyMethod::PremultipliedAlphaFallback
}

impl Default for FieldPresentationSettings {
    fn default() -> Self {
        Self {
            clip_planes: Vec::new(),
            slices: Vec::new(),
            transfer_function: FieldTransferFunction::default(),
            use_explicit_transfer_function: false,
            display_range: None,
            opacity_scale: default_field_opacity_scale(),
            density_cutoff: 0.0,
            transparency_method: default_field_transparency_method(),
            field_material_mode: FieldMaterialMode::Lit,
        }
    }
}

impl FieldPresentationSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.clip_planes.len() > MAX_FIELD_CLIP_PLANES {
            return Err("field clip-plane count exceeds its bound".to_owned());
        }
        for plane in &self.clip_planes {
            FieldClipPlane::normalized(
                plane.normal,
                plane.signed_offset_angstrom,
                plane.keep_positive,
            )?;
        }
        if self.slices.len() > MAX_FIELD_SLICES {
            return Err("field slice count exceeds its bound".to_owned());
        }
        for slice in &self.slices {
            slice.validate()?;
        }
        if let Some([minimum, maximum]) = self.display_range {
            if !minimum.is_finite() || !maximum.is_finite() || minimum >= maximum {
                return Err("field display range is invalid".to_owned());
            }
        }
        if !self.opacity_scale.is_finite()
            || !(0.0..=10.0).contains(&self.opacity_scale)
            || !self.density_cutoff.is_finite()
            || self.density_cutoff < 0.0
        {
            return Err("field opacity mapping is invalid".to_owned());
        }
        if matches!(
            self.transparency_method,
            FieldTransparencyMethod::WeightedBlendedOit
        ) {
            return Err(
                "weighted blended OIT is not admitted by the active FIGURE-2 capability policy"
                    .to_owned(),
            );
        }
        self.transfer_function.validate()
    }

    pub fn normalized_clip_planes(&self) -> Result<Vec<FieldClipPlane>, String> {
        self.validate()?;
        let mut normalized = Vec::new();
        normalized
            .try_reserve_exact(self.clip_planes.len())
            .map_err(|_| "unable to reserve normalized field clip planes".to_owned())?;
        for plane in &self.clip_planes {
            normalized.push(FieldClipPlane::normalized(
                plane.normal,
                plane.signed_offset_angstrom,
                plane.keep_positive,
            )?);
        }
        Ok(normalized)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FieldRenderSnapshot {
    pub layer_id: FieldLayerId,
    pub source_layer_revision: FieldSceneRevision,
    #[serde(default)]
    pub source_origin_angstrom: Option<[f64; 3]>,
    pub scalar_unit: String,
    pub scalar_range: [f32; 2],
    pub representations: Vec<FieldRepresentation>,
    pub positive_isovalue: Option<f32>,
    pub negative_isovalue: Option<f32>,
    pub positive_color: [f32; 4],
    pub negative_color: [f32; 4],
    pub clip_planes: Vec<FieldClipPlane>,
    pub slices: Vec<FieldSliceRequest>,
    pub transfer_function: FieldTransferFunction,
    pub use_explicit_transfer_function: bool,
    pub transparency_method: FieldTransparencyMethod,
    #[serde(default)]
    pub field_material_mode: FieldMaterialMode,
    pub display_range: Option<[f32; 2]>,
    pub opacity_scale: f32,
    pub density_cutoff: f32,
    pub colormap_mode: u32,
}

impl FieldRenderSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        let [data_min, data_max] = self.scalar_range;
        if self
            .source_origin_angstrom
            .is_some_and(|origin| !origin.iter().all(|value| value.is_finite()))
            || self.scalar_unit.is_empty()
            || !data_min.is_finite()
            || !data_max.is_finite()
            || data_min > data_max
        {
            return Err("field scalar range is invalid".to_owned());
        }
        if self.representations.is_empty()
            || self.representations.len() > MAX_FIELD_REPRESENTATIONS
            || self.clip_planes.len() > MAX_FIELD_CLIP_PLANES
            || self.slices.len() > MAX_FIELD_SLICES
            || self
                .representations
                .iter()
                .enumerate()
                .any(|(index, representation)| {
                    self.representations[..index].contains(representation)
                })
        {
            return Err("field representation or clip-plane count is invalid".to_owned());
        }
        let requires_positive = self
            .representations
            .contains(&FieldRepresentation::PositiveIsosurface);
        let requires_negative = self
            .representations
            .contains(&FieldRepresentation::NegativeIsosurface);
        validate_isovalue(
            self.positive_isovalue,
            data_max,
            requires_positive,
            "positive",
        )?;
        if ![self.positive_color, self.negative_color]
            .iter()
            .flatten()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            return Err("field isosurface color is invalid".to_owned());
        }
        validate_isovalue(
            self.negative_isovalue,
            -data_min,
            requires_negative,
            "negative",
        )?;
        for plane in &self.clip_planes {
            FieldClipPlane::normalized(
                plane.normal,
                plane.signed_offset_angstrom,
                plane.keep_positive,
            )?;
        }
        for slice in &self.slices {
            slice.validate()?;
        }
        FieldPresentationSettings {
            clip_planes: self.clip_planes.clone(),
            slices: self.slices.clone(),
            transfer_function: self.transfer_function.clone(),
            use_explicit_transfer_function: self.use_explicit_transfer_function,
            display_range: self.display_range,
            opacity_scale: self.opacity_scale,
            density_cutoff: self.density_cutoff,
            transparency_method: self.transparency_method,
            field_material_mode: self.field_material_mode,
        }
        .validate()?;
        if self.colormap_mode > 9 {
            return Err("field colormap mode is invalid".to_owned());
        }
        Ok(())
    }
}

fn validate_isovalue(
    value: Option<f32>,
    branch_maximum: f32,
    required: bool,
    sign_identity: &str,
) -> Result<(), String> {
    match value {
        Some(value) if value.is_finite() && value > 0.0 && value <= branch_maximum => Ok(()),
        None if !required => Ok(()),
        _ => Err(format!(
            "{sign_identity} isovalue is invalid for the selected representation"
        )),
    }
}

#[derive(Clone, Debug)]
pub struct FieldSlice {
    pub plane: FieldSlicePlane,
    pub source_layer_revision: FieldSceneRevision,
    pub world_origin_angstrom: [f64; 3],
    pub first_axis: [f64; 3],
    pub second_axis: [f64; 3],
    pub grid_point_spacing_angstrom: f64,
    pub dimensions: [usize; 2],
    pub values: Vec<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContourSegment {
    pub level: f32,
    pub start: [f64; 2],
    pub end: [f64; 2],
}

pub fn sample_field_slice(
    field: &impl ScalarFieldView,
    source_layer_revision: FieldSceneRevision,
    plane: FieldSlicePlane,
    dimensions: [usize; 2],
) -> Result<FieldSlice, String> {
    if dimensions[0] == 0
        || dimensions[1] == 0
        || dimensions[0] > MAX_FIELD_SLICE_DIMENSION
        || dimensions[1] > MAX_FIELD_SLICE_DIMENSION
    {
        return Err("field slice dimensions are invalid".to_owned());
    }
    let sample_count = dimensions[0]
        .checked_mul(dimensions[1])
        .filter(|count| *count <= MAX_FIELD_SLICE_SAMPLES)
        .ok_or_else(|| "field slice sample count exceeds its bound".to_owned())?;
    let plane = FieldSlicePlane::normalized(
        plane.normal,
        plane.signed_offset_angstrom,
        plane.interpolation,
    )?;
    if !matches!(plane.interpolation, FieldSliceInterpolation::Trilinear) {
        return Err("field slice interpolation is unsupported".to_owned());
    }
    let (first_axis, second_axis) = plane_axes(plane.normal)?;
    let spacing = field_voxel_spacing(field)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(sample_count)
        .map_err(|_| "unable to allocate field slice samples".to_owned())?;
    let origin = [
        plane.normal[0] * plane.signed_offset_angstrom,
        plane.normal[1] * plane.signed_offset_angstrom,
        plane.normal[2] * plane.signed_offset_angstrom,
    ];
    let center_x = (dimensions[0].saturating_sub(1) as f64) * 0.5;
    let center_y = (dimensions[1].saturating_sub(1) as f64) * 0.5;
    for y in 0..dimensions[1] {
        for x in 0..dimensions[0] {
            let dx = (x as f64 - center_x) * spacing;
            let dy = (y as f64 - center_y) * spacing;
            let world = [
                origin[0] + first_axis[0] * dx + second_axis[0] * dy,
                origin[1] + first_axis[1] * dx + second_axis[1] * dy,
                origin[2] + first_axis[2] * dx + second_axis[2] * dy,
            ];
            values.push(sample_trilinear_col_major(field, world).unwrap_or(f32::NAN));
        }
    }
    Ok(FieldSlice {
        plane,
        source_layer_revision,
        world_origin_angstrom: origin,
        first_axis,
        second_axis,
        grid_point_spacing_angstrom: spacing,
        dimensions,
        values,
    })
}

pub fn extract_contours_marching_squares(
    slice: &FieldSlice,
    contour_levels: &[f32],
    clip_planes: &[FieldClipPlane],
) -> Result<Vec<ContourSegment>, String> {
    if contour_levels.is_empty() || contour_levels.len() > MAX_FIELD_CONTOUR_LEVELS {
        return Err("field contour level count is invalid".to_owned());
    }
    if clip_planes.len() > MAX_FIELD_CLIP_PLANES {
        return Err("field contour clip-plane count is invalid".to_owned());
    }
    let mut previous = f32::NEG_INFINITY;
    for &level in contour_levels {
        if !level.is_finite() || level <= previous {
            return Err("field contour levels must be finite and strictly increasing".to_owned());
        }
        previous = level;
    }
    if slice.dimensions[0] < 2 || slice.dimensions[1] < 2 {
        return Err("field contour slice is too small".to_owned());
    }
    let mut output = Vec::new();
    let segment_capacity = (slice.dimensions[0] - 1)
        .checked_mul(slice.dimensions[1] - 1)
        .and_then(|cells| cells.checked_mul(contour_levels.len()))
        .and_then(|segments| segments.checked_mul(2))
        .filter(|segments| *segments <= MAX_FIELD_CONTOUR_SEGMENTS)
        .ok_or_else(|| "field contour segment budget exceeded".to_owned())?;
    output
        .try_reserve_exact(segment_capacity)
        .map_err(|_| "unable to reserve field contour segments".to_owned())?;
    for &level in contour_levels {
        for y in 0..(slice.dimensions[1] - 1) {
            for x in 0..(slice.dimensions[0] - 1) {
                let values = [
                    slice.values[y * slice.dimensions[0] + x],
                    slice.values[y * slice.dimensions[0] + x + 1],
                    slice.values[(y + 1) * slice.dimensions[0] + x + 1],
                    slice.values[(y + 1) * slice.dimensions[0] + x],
                ];
                if !values.iter().all(|value| value.is_finite()) {
                    continue;
                }
                let case_index = values
                    .iter()
                    .enumerate()
                    .fold(0_u8, |mask, (index, value)| {
                        mask | u8::from(*value >= level) << index
                    });
                append_marching_squares_segments(
                    &mut output,
                    level,
                    x as f64,
                    y as f64,
                    values,
                    case_index,
                )?;
            }
        }
    }
    let mut normalized_clip_planes = Vec::new();
    normalized_clip_planes
        .try_reserve_exact(clip_planes.len())
        .map_err(|_| "unable to reserve normalized contour clip planes".to_owned())?;
    for clip_plane in clip_planes {
        normalized_clip_planes.push(FieldClipPlane::normalized(
            clip_plane.normal,
            clip_plane.signed_offset_angstrom,
            clip_plane.keep_positive,
        )?);
    }
    output.retain_mut(|segment| clip_contour_segment(segment, slice, &normalized_clip_planes));
    Ok(output)
}

fn append_marching_squares_segments(
    output: &mut Vec<ContourSegment>,
    level: f32,
    x: f64,
    y: f64,
    values: [f32; 4],
    case_index: u8,
) -> Result<(), String> {
    let edge = |edge_index: u8| -> [f64; 2] {
        let (first, second) = match edge_index {
            0 => (0, 1),
            1 => (1, 2),
            2 => (3, 2),
            _ => (0, 3),
        };
        let denominator = f64::from(values[second] - values[first]);
        let fraction = if denominator.abs() <= f64::EPSILON {
            0.5
        } else {
            (f64::from(level - values[first]) / denominator).clamp(0.0, 1.0)
        };
        match edge_index {
            0 => [x + fraction, y],
            1 => [x + 1.0, y + fraction],
            2 => [x + fraction, y + 1.0],
            _ => [x, y + fraction],
        }
    };
    let mut add = |first: u8, second: u8| -> Result<(), String> {
        if output.len() >= MAX_FIELD_CONTOUR_SEGMENTS {
            return Err("field contour segment budget exceeded".to_owned());
        }
        output.push(ContourSegment {
            level,
            start: edge(first),
            end: edge(second),
        });
        Ok(())
    };
    match case_index {
        0 | 15 => Ok(()),
        1 | 14 => add(3, 0),
        2 | 13 => add(0, 1),
        3 | 12 => add(3, 1),
        4 | 11 => add(1, 2),
        5 | 10 => {
            // The bilinear asymptotic decider uses the determinant, not the
            // arithmetic center value, so skewed saddles retain their topology.
            let q = (values[0] - level) * (values[2] - level)
                - (values[1] - level) * (values[3] - level);
            if (q >= 0.0) == (case_index == 5) {
                add(3, 2)?;
                add(0, 1)
            } else {
                add(3, 0)?;
                add(1, 2)
            }
        }
        6 | 9 => add(0, 2),
        7 | 8 => add(3, 2),
        _ => Err("MarchingSquares produced an invalid case index".to_owned()),
    }
}

fn clip_contour_segment(
    segment: &mut ContourSegment,
    slice: &FieldSlice,
    clip_planes: &[FieldClipPlane],
) -> bool {
    let center_x = (slice.dimensions[0].saturating_sub(1) as f64) * 0.5;
    let center_y = (slice.dimensions[1].saturating_sub(1) as f64) * 0.5;
    let world = |point: [f64; 2]| -> [f64; 3] {
        let u = (point[0] - center_x) * slice.grid_point_spacing_angstrom;
        let v = (point[1] - center_y) * slice.grid_point_spacing_angstrom;
        [
            slice.world_origin_angstrom[0] + slice.first_axis[0] * u + slice.second_axis[0] * v,
            slice.world_origin_angstrom[1] + slice.first_axis[1] * u + slice.second_axis[1] * v,
            slice.world_origin_angstrom[2] + slice.first_axis[2] * u + slice.second_axis[2] * v,
        ]
    };
    let mut start = world(segment.start);
    let mut end = world(segment.end);
    for plane in clip_planes {
        let signed_distance =
            |point: [f64; 3]| dot(plane.normal, point) - plane.signed_offset_angstrom;
        let start_distance = signed_distance(start);
        let end_distance = signed_distance(end);
        let start_kept = if plane.keep_positive {
            start_distance >= 0.0
        } else {
            start_distance <= 0.0
        };
        let end_kept = if plane.keep_positive {
            end_distance >= 0.0
        } else {
            end_distance <= 0.0
        };
        if !start_kept && !end_kept {
            return false;
        }
        if start_kept != end_kept {
            let fraction = (start_distance / (start_distance - end_distance)).clamp(0.0, 1.0);
            let intersection = [
                start[0] + (end[0] - start[0]) * fraction,
                start[1] + (end[1] - start[1]) * fraction,
                start[2] + (end[2] - start[2]) * fraction,
            ];
            if start_kept {
                end = intersection;
            } else {
                start = intersection;
            }
        }
    }
    let grid = |point: [f64; 3]| -> [f64; 2] {
        let delta = [
            point[0] - slice.world_origin_angstrom[0],
            point[1] - slice.world_origin_angstrom[1],
            point[2] - slice.world_origin_angstrom[2],
        ];
        [
            dot(delta, slice.first_axis) / slice.grid_point_spacing_angstrom + center_x,
            dot(delta, slice.second_axis) / slice.grid_point_spacing_angstrom + center_y,
        ]
    };
    segment.start = grid(start);
    segment.end = grid(end);
    true
}

fn plane_axes(normal: [f64; 3]) -> Result<([f64; 3], [f64; 3]), String> {
    let seed = if normal[0].abs() < 0.8 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let first = normalize(cross(normal, seed))?;
    let second = normalize(cross(normal, first))?;
    Ok((first, second))
}

fn field_voxel_spacing(field: &impl ScalarFieldView) -> Result<f64, String> {
    let dims = field.grid_dims();
    if dims.iter().any(|dimension| *dimension == 0) {
        return Err("field grid dimensions are invalid".to_owned());
    }
    if dims.iter().any(|dimension| *dimension < 2) {
        return Err("field grid has insufficient points for interpolation".to_owned());
    }
    let mapping: FieldGridMapping = field.grid_mapping();
    let _axis_sampling: [AxisSampling; 3] = mapping.axis_sampling;
    let _sampling_contract = mapping.axis_sampling.map(|axis| match axis {
        AxisSampling::PeriodicExclusive => true,
        AxisSampling::InclusiveBoundary => false,
    });
    let _grid_origin = mapping.index_to_world([0, 0, 0]);
    let sample_steps_col_major = mapping.sample_steps_col_major;
    let spacing = [
        norm([
            sample_steps_col_major[0],
            sample_steps_col_major[1],
            sample_steps_col_major[2],
        ]),
        norm([
            sample_steps_col_major[3],
            sample_steps_col_major[4],
            sample_steps_col_major[5],
        ]),
        norm([
            sample_steps_col_major[6],
            sample_steps_col_major[7],
            sample_steps_col_major[8],
        ]),
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min);
    if !spacing.is_finite() || spacing <= 0.0 {
        return Err("field slice spacing is invalid".to_owned());
    }
    Ok(spacing)
}

fn sample_trilinear_col_major(field: &impl ScalarFieldView, world: [f64; 3]) -> Option<f32> {
    if !matches!(field.grid_ordering(), FieldGridOrdering::ColMajor) {
        return None;
    }
    let mapping = field.grid_mapping();
    mapping.world_to_grid(world)?;
    mapping
        .sample_trilinear(field.scalar_data(), world)
        .filter(|value| value.is_finite())
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0].mul_add(right[0], left[1].mul_add(right[1], left[2] * right[2]))
}

fn norm(vector: [f64; 3]) -> f64 {
    dot(vector, vector).sqrt()
}

fn normalize(vector: [f64; 3]) -> Result<[f64; 3], String> {
    let length = norm(vector);
    if !length.is_finite() || length <= f64::EPSILON {
        return Err("field slice plane normal is degenerate".to_owned());
    }
    Ok([vector[0] / length, vector[1] / length, vector[2] / length])
}
