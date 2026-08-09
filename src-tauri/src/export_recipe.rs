//! Versioned publication-export recipes and paired artifact writes.

use crate::crystal_state::CrystalState;
use crate::renderer::publication_look::{PublicationLookProfile, PublicationLookProfileId};
use crate::renderer::renderer::{
    FieldPublicationSnapshot, MAX_PUBLICATION_RECIPE_BYTES, PublicationBackground,
    PublicationExportAdmissionReceipt, PublicationExportSourceState, Renderer,
    cell_line_style_for_background, evaluate_field_publication_export_admission,
    evaluate_publication_export_admission, validate_publication_export_receipt_fields,
};
use crate::settings::AppSettings;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

pub const EXPORT_RECIPE_SCHEMA: &str = "crystalcanvas.export-recipe";
pub const EXPORT_RECIPE_SCHEMA_VERSION: u32 = 10;

const MAX_RECIPE_STRUCTURE_NAME_BYTES: usize = 4 * 1024;
const MAX_RECIPE_CUSTOM_ATOM_COLORS: usize = 118;
const MAX_RECIPE_ELEMENT_SYMBOL_BYTES: usize = 3;
const SRGB_PROFILE_NAME: &str = "sRGB IEC61966-2.1";
const SRGB_PROFILE_VERSION: &str = "ICC v4.3";
const FIXED_ICC_CREATION_DATE: [u8; 12] = [
    0x07, 0xea, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const JPEG_QUALITY: u8 = 95;
const JPEG_CHROMA_SUBSAMPLING: &str = "4:4:4";
const PNG_COMPRESSION: &str = "balanced";
const PNG_FILTER: &str = "adaptive";

static TEMP_FILE_NONCE: AtomicU64 = AtomicU64::new(0);
static SRGB_ICC_PROFILE: OnceLock<Vec<u8>> = OnceLock::new();

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportRecipeKind {
    PublicationRaster,
    BlenderStructureScene,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct PublicationRasterRecipe {
    pub schema: String,
    pub schema_version: u32,
    pub kind: ExportRecipeKind,
    pub application_version: String,
    pub generated_at_unix_ms: u64,
    pub success: bool,
    pub source: RecipeSource,
    pub camera: RecipeCamera,
    pub scene: RecipeScene,
    pub materials: RecipeMaterials,
    pub rendering: RecipeRendering,
    pub output: RecipeOutput,
    pub artifact: Option<RecipeArtifact>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct PublicationGlbRecipe {
    pub schema: String,
    pub schema_version: u32,
    pub kind: ExportRecipeKind,
    pub application_version: String,
    pub generated_at_unix_ms: u64,
    pub success: bool,
    pub export_id: String,
    pub source: RecipeSource,
    pub camera: RecipeCamera,
    pub look_profile: PublicationLookRecipe,
    pub coordinate_length_unit: String,
    pub meters_per_exported_unit: f64,
    pub matrix_layout: String,
    pub scale_policy: String,
    pub material_mapping: String,
    pub semantic_inventory: crate::blender_export::GlbSemanticInventory,
    pub artifact: Option<RecipeArtifact>,
}

impl PublicationGlbRecipe {
    pub fn from_scene(
        source: &CrystalState,
        scene: &crate::scene_export::PublicationSceneSnapshot,
    ) -> Result<Self, String> {
        validate_recipe_metadata(source.name.as_str(), 0, std::iter::empty())?;
        let camera = &scene.camera;
        let recipe = Self {
            schema: EXPORT_RECIPE_SCHEMA.to_owned(), schema_version: EXPORT_RECIPE_SCHEMA_VERSION, kind: ExportRecipeKind::BlenderStructureScene,
            application_version: env!("CARGO_PKG_VERSION").to_owned(), generated_at_unix_ms: unix_time_ms()?, success: true,
            export_id: format!("{}-{}", canonical_structure_sha256(source), unix_time_ms()?),
            source: RecipeSource { structure_name: source.name.clone(), source_version: source.version, intrinsic_atom_count: scene.intrinsic_atom_count, structure_hash: Some(canonical_structure_sha256(source)), structure_hash_algorithm: Some("sha256-canonical-crystal-state-v1".to_owned()), source_length_unit: "angstrom".to_owned(), coordinate_space: "cartesian_right_handed_y_up".to_owned() },
            camera: RecipeCamera { eye: camera.eye.to_array(), target: camera.target.to_array(), up: camera.up.to_array(), projection: if camera.is_perspective { "perspective".to_owned() } else { "orthographic".to_owned() }, fovy_deg: camera.fovy_deg, orthographic_scale: camera.orthographic_scale, znear: camera.znear, zfar: camera.zfar, aspect_policy: "current_renderer_camera".to_owned(), fit_visible_structure_to_export: false, publication_framing_margin: 0.0 },
            look_profile: PublicationLookRecipe::from_profile(scene.look_profile), coordinate_length_unit: "angstrom".to_owned(), meters_per_exported_unit: 1.0e-10,
            matrix_layout: "column_major".to_owned(), scale_policy: "scientific_visualization".to_owned(), material_mapping: "gltf_pbr_metallic_roughness; sRGB input colors are converted once to linear-sRGB factors; alpha is OPAQUE at 1.0 and BLEND below 1.0; renderer lighting and tone mapping are not baked".to_owned(),
            semantic_inventory: crate::blender_export::GlbSemanticInventory { intrinsic_atoms: scene.intrinsic_atom_count, atom_instances: scene.atoms.len(), bonds: scene.bonds.len(), cell_edges: scene.cell_edges.len(), materials: 0, meshes: 0 }, artifact: None,
        };
        recipe.validate()?;
        Ok(recipe)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != EXPORT_RECIPE_SCHEMA
            || self.schema_version != EXPORT_RECIPE_SCHEMA_VERSION
            || self.kind != ExportRecipeKind::BlenderStructureScene
            || !self.success
            || self.export_id.is_empty()
            || self.source.source_length_unit != "angstrom"
            || self.coordinate_length_unit != "angstrom"
            || self.meters_per_exported_unit != 1.0e-10
            || self.matrix_layout != "column_major"
            || self.scale_policy != "scientific_visualization"
            || !self.meters_per_exported_unit.is_finite()
        {
            return Err("publication GLB recipe metadata is invalid".to_owned());
        }
        self.look_profile.validate()?;
        if !self
            .camera
            .eye
            .iter()
            .chain(self.camera.target.iter())
            .chain(self.camera.up.iter())
            .chain(
                [
                    self.camera.fovy_deg,
                    self.camera.orthographic_scale,
                    self.camera.znear,
                    self.camera.zfar,
                ]
                .iter(),
            )
            .all(|value| value.is_finite())
            || self.camera.znear <= 0.0
            || self.camera.zfar <= self.camera.znear
            || !matches!(
                self.camera.projection.as_str(),
                "perspective" | "orthographic"
            )
            || (self.camera.projection == "perspective"
                && !(0.0 < self.camera.fovy_deg && self.camera.fovy_deg < 180.0))
            || (self.camera.projection == "orthographic" && self.camera.orthographic_scale <= 0.0)
        {
            return Err("publication GLB camera is invalid".to_owned());
        }
        if let Some(artifact) = &self.artifact {
            if artifact.file_name.is_empty() || !is_sha256(&artifact.sha256) {
                return Err("publication GLB artifact metadata is invalid".to_owned());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RecipeSource {
    pub structure_name: String,
    pub source_version: u32,
    pub intrinsic_atom_count: usize,
    pub structure_hash: Option<String>,
    pub structure_hash_algorithm: Option<String>,
    pub source_length_unit: String,
    pub coordinate_space: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RecipeCamera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub projection: String,
    pub fovy_deg: f32,
    pub orthographic_scale: f32,
    pub znear: f32,
    pub zfar: f32,
    pub aspect_policy: String,
    pub fit_visible_structure_to_export: bool,
    pub publication_framing_margin: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RecipeScene {
    pub atoms: bool,
    pub bonds: bool,
    pub unit_cell: bool,
    pub measurements: bool,
    pub hoppings: bool,
    pub isosurface: bool,
    pub volume: bool,
    pub stable_periodic_image_policy: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RecipeMaterials {
    pub material_profile: String,
    pub look_profile: PublicationLookRecipe,
    pub atom_radius_policy: String,
    pub atom_radius_scale: f32,
    pub bond_tolerance: f32,
    pub bond_radius: f32,
    pub radius_length_unit: String,
    pub bond_color_rgba: [f32; 4],
    pub custom_atom_colors_rgba: BTreeMap<String, [f32; 4]>,
    pub color_value_space: String,
    pub cell_line_color_rgba: [f32; 4],
}

/// Complete fixed publication profile snapshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct PublicationLookRecipe {
    pub profile_id: String,
    pub profile_version: String,
    pub key_direction: [f32; 3],
    pub key_intensity: f32,
    pub fill_direction: [f32; 3],
    pub fill_intensity: f32,
    pub rim_direction: [f32; 3],
    pub rim_intensity: f32,
    pub ambient: f32,
    pub roughness: f32,
    pub specular: f32,
    pub opacity: f32,
    pub exposure: f32,
    pub tone_mapping: String,
    pub input_color_space: String,
    pub output_color_space: String,
    pub bond_color_mode: String,
    pub cell_line_width_pixels: f32,
    pub depth_enhancement: String,
}

impl PublicationLookRecipe {
    pub fn from_profile(profile: PublicationLookProfile) -> Self {
        Self {
            profile_id: profile.id.as_str().to_owned(),
            profile_version: profile.version.to_owned(),
            key_direction: profile.key_direction,
            key_intensity: profile.key_intensity,
            fill_direction: profile.fill_direction,
            fill_intensity: profile.fill_intensity,
            rim_direction: profile.rim_direction,
            rim_intensity: profile.rim_intensity,
            ambient: profile.ambient,
            roughness: profile.roughness,
            specular: profile.specular,
            opacity: profile.opacity,
            exposure: profile.exposure,
            tone_mapping: profile.tone_mapping.as_str().to_owned(),
            input_color_space: "sRGB_straight_alpha".to_owned(),
            output_color_space: "sRGB".to_owned(),
            bond_color_mode: profile.bond_color_mode.as_str().to_owned(),
            cell_line_width_pixels: profile.cell_line_width_pixels,
            depth_enhancement: profile.depth_enhancement.as_str().to_owned(),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if !matches!(
            self.profile_id.as_str(),
            "scientific_gloss" | "studio" | "unlit"
        ) || self.profile_version != "v1"
            || self.input_color_space != "sRGB_straight_alpha"
            || self.output_color_space != "sRGB"
            || !matches!(self.tone_mapping.as_str(), "disabled" | "aces_fitted")
            || !matches!(self.bond_color_mode.as_str(), "uniform" | "by_elements")
            || self.depth_enhancement != "disabled"
        {
            return Err("publication look profile metadata is unsupported".to_owned());
        }
        let scalars = [
            self.key_intensity,
            self.fill_intensity,
            self.rim_intensity,
            self.ambient,
            self.roughness,
            self.specular,
            self.opacity,
            self.exposure,
            self.cell_line_width_pixels,
        ];
        if !scalars.iter().all(|value| value.is_finite())
            || !self
                .key_direction
                .iter()
                .chain(self.fill_direction.iter())
                .chain(self.rim_direction.iter())
                .all(|value| value.is_finite())
            || !normalized_direction(self.key_direction)
            || !normalized_direction(self.fill_direction)
            || !normalized_direction(self.rim_direction)
            || !(0.0..=4.0).contains(&self.key_intensity)
            || !(0.0..=4.0).contains(&self.fill_intensity)
            || !(0.0..=4.0).contains(&self.rim_intensity)
            || !(0.0..=1.0).contains(&self.ambient)
            || !(0.04..=1.0).contains(&self.roughness)
            || !(0.0..=1.0).contains(&self.specular)
            || !(0.0..=1.0).contains(&self.opacity)
            || !(-4.0..=4.0).contains(&self.exposure)
            || self.cell_line_width_pixels != 1.0
        {
            return Err("publication look profile is outside fixed bounds".to_owned());
        }
        if self.profile_id == "unlit"
            && (self.key_intensity != 0.0
                || self.fill_intensity != 0.0
                || self.rim_intensity != 0.0
                || self.ambient != 0.0
                || self.specular != 0.0
                || self.exposure != 0.0
                || self.tone_mapping != "disabled")
        {
            return Err("Unlit publication look must bypass visual modulation".to_owned());
        }
        Ok(())
    }
}

fn normalized_direction(direction: [f32; 3]) -> bool {
    let squared_length =
        direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2];
    squared_length.is_finite() && (0.999..=1.001).contains(&squared_length)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RecipeRendering {
    pub lighting_policy: String,
    pub ssao: String,
    pub shadows: String,
    pub requested_samples: u32,
    pub selected_samples: u32,
    pub selected_capabilities: Vec<String>,
    pub fallback_policy: String,
    pub applied_fallbacks: Vec<String>,
    pub adapter_name: String,
    pub backend: String,
    pub device_type: String,
    pub render_target_format: String,
    pub max_texture_dimension_2d: u32,
    pub max_buffer_size: u64,
    pub max_storage_buffer_size: u64,
    pub supports_compute_shaders: bool,
    pub publication_admission: PublicationExportAdmissionReceipt,
    #[serde(default)]
    pub field_scene: Option<RecipeFieldPublicationScene>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RecipeFieldPublicationScene {
    pub layers: Vec<RecipeFieldLayer>,
    pub composition_method: String,
    pub composition_order: String,
    #[serde(default)]
    pub field_scene_hash: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RecipeFieldLayer {
    pub layer_id: u64,
    pub source_layer_revision: u64,
    #[serde(default)]
    pub source_origin_angstrom: Option<[f64; 3]>,
    pub scalar_unit: String,
    pub scalar_range: [f32; 2],
    pub display_range: Option<[f32; 2]>,
    pub representations: Vec<String>,
    pub positive_isovalue: Option<f32>,
    pub negative_isovalue: Option<f32>,
    pub positive_color: [f32; 4],
    pub negative_color: [f32; 4],
    pub clip_planes: Vec<RecipeFieldClipPlane>,
    pub slices: Vec<RecipeFieldSlice>,
    pub transfer_function: RecipeFieldTransferFunction,
    pub use_explicit_transfer_function: bool,
    pub colormap_mode: u32,
    pub opacity_scale: f32,
    pub density_cutoff: f32,
    #[serde(default = "default_recipe_field_material_mode")]
    pub field_material_mode: String,
}

fn default_recipe_field_material_mode() -> String {
    "lit".to_owned()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RecipeFieldClipPlane {
    pub normal: [f64; 3],
    pub signed_offset_angstrom: f64,
    pub keep_positive: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RecipeFieldSlice {
    pub normal: [f64; 3],
    pub signed_offset_angstrom: f64,
    pub interpolation: String,
    pub dimensions: [usize; 2],
    pub contour_levels: Vec<f32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RecipeFieldTransferFunction {
    pub color_space: String,
    pub negative_control_points: Vec<RecipeFieldTransferControlPoint>,
    pub positive_control_points: Vec<RecipeFieldTransferControlPoint>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RecipeFieldTransferControlPoint {
    pub position: f32,
    pub color_linear_rgba: [f32; 4],
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RecipeOutput {
    pub width: u32,
    pub height: u32,
    pub raster_format: String,
    pub bit_depth_per_channel: u8,
    pub color_space: String,
    pub color_profile: RecipeColorProfile,
    pub requested_background: String,
    pub effective_background: String,
    pub effective_background_rgba_linear: [f64; 4],
    pub readback_alpha_policy: String,
    pub encoded_alpha_policy: String,
    pub codec: RecipeCodec,
    pub tile_layout: [u32; 2],
    pub tile_dimensions: [u32; 2],
    pub tile_overlap_pixels: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RecipeColorProfile {
    pub name: String,
    pub version: String,
    pub sha256: String,
}

impl RecipeColorProfile {
    pub fn srgb() -> Result<Self, String> {
        Ok(Self {
            name: SRGB_PROFILE_NAME.to_owned(),
            version: SRGB_PROFILE_VERSION.to_owned(),
            sha256: hex_sha256(srgb_icc_profile()?),
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RecipeCodec {
    Png {
        compression: String,
        filter: String,
    },
    Jpeg {
        quality: u8,
        chroma_subsampling: String,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RecipeArtifact {
    pub file_name: String,
    pub sha256: String,
}

fn recipe_field_scene(
    snapshot: &FieldPublicationSnapshot,
) -> Result<RecipeFieldPublicationScene, String> {
    let mut layers = Vec::new();
    layers
        .try_reserve_exact(snapshot.field_snapshots.len())
        .map_err(|_| "unable to allocate publication field recipe".to_owned())?;
    for field in &snapshot.field_snapshots {
        let mut representations = Vec::new();
        representations
            .try_reserve_exact(field.representations.len())
            .map_err(|_| "unable to allocate field representations".to_owned())?;
        for representation in &field.representations {
            representations.push(
                match representation {
                    crate::renderer::field_scene::FieldRepresentation::PositiveIsosurface => {
                        "positive_isosurface"
                    }
                    crate::renderer::field_scene::FieldRepresentation::NegativeIsosurface => {
                        "negative_isosurface"
                    }
                    crate::renderer::field_scene::FieldRepresentation::VolumeRaycast => {
                        "volume_raycast"
                    }
                    crate::renderer::field_scene::FieldRepresentation::Slice => "slice",
                    crate::renderer::field_scene::FieldRepresentation::Contour => "contour",
                }
                .to_owned(),
            );
        }
        let convert_points = |points: &[crate::renderer::field_scene::FieldTransferControlPoint]| -> Result<Vec<RecipeFieldTransferControlPoint>, String> {
            let mut recipe_points = Vec::new();
            recipe_points.try_reserve_exact(points.len()).map_err(|_| "unable to allocate field transfer recipe".to_owned())?;
            for point in points {
                recipe_points.push(RecipeFieldTransferControlPoint { position: point.position, color_linear_rgba: point.color_linear_rgba });
            }
            Ok(recipe_points)
        };
        let mut clip_planes = Vec::new();
        clip_planes
            .try_reserve_exact(field.clip_planes.len())
            .map_err(|_| "unable to allocate field clip recipe".to_owned())?;
        for plane in &field.clip_planes {
            clip_planes.push(RecipeFieldClipPlane {
                normal: plane.normal,
                signed_offset_angstrom: plane.signed_offset_angstrom,
                keep_positive: plane.keep_positive,
            });
        }
        let mut slices = Vec::new();
        slices
            .try_reserve_exact(field.slices.len())
            .map_err(|_| "unable to allocate publication field slices".to_owned())?;
        for slice in &field.slices {
            let mut contour_levels = Vec::new();
            contour_levels
                .try_reserve_exact(slice.contour_levels.len())
                .map_err(|_| "unable to allocate publication contour levels".to_owned())?;
            contour_levels.extend_from_slice(&slice.contour_levels);
            slices.push(RecipeFieldSlice {
                normal: slice.plane.normal,
                signed_offset_angstrom: slice.plane.signed_offset_angstrom,
                interpolation: match slice.plane.interpolation {
                    crate::renderer::field_scene::FieldSliceInterpolation::Trilinear => "trilinear",
                }
                .to_owned(),
                dimensions: slice.dimensions,
                contour_levels,
            });
        }
        layers.push(RecipeFieldLayer {
            layer_id: field.layer_id,
            source_layer_revision: field.source_layer_revision,
            source_origin_angstrom: field.source_origin_angstrom,
            scalar_unit: field.scalar_unit.clone(),
            scalar_range: field.scalar_range,
            display_range: field.display_range,
            representations,
            positive_isovalue: field.positive_isovalue,
            negative_isovalue: field.negative_isovalue,
            positive_color: field.positive_color,
            negative_color: field.negative_color,
            clip_planes,
            slices,
            transfer_function: RecipeFieldTransferFunction {
                color_space: field.transfer_function.color_space.clone(),
                negative_control_points: convert_points(
                    &field.transfer_function.negative_control_points,
                )?,
                positive_control_points: convert_points(
                    &field.transfer_function.positive_control_points,
                )?,
            },
            use_explicit_transfer_function: field.use_explicit_transfer_function,
            colormap_mode: field.colormap_mode,
            opacity_scale: field.opacity_scale,
            density_cutoff: field.density_cutoff,
            field_material_mode: match field.field_material_mode {
                crate::renderer::field_scene::FieldMaterialMode::Lit => "lit",
                crate::renderer::field_scene::FieldMaterialMode::Unlit => "unlit",
            }
            .to_owned(),
        });
    }
    let composition_method = match snapshot.composition_method {
        crate::renderer::field_scene::FieldTransparencyMethod::WeightedBlendedOit => {
            "weighted_blended_oit"
        }
        crate::renderer::field_scene::FieldTransparencyMethod::PremultipliedAlphaFallback => {
            "premultiplied_alpha_fallback"
        }
    }
    .to_owned();
    let composition_order = match snapshot.composition_method {
        crate::renderer::field_scene::FieldTransparencyMethod::WeightedBlendedOit => {
            "order_independent"
        }
        crate::renderer::field_scene::FieldTransparencyMethod::PremultipliedAlphaFallback => {
            "stable_layer_id_ascending_then_translucent_structure"
        }
    }
    .to_owned();
    Ok(RecipeFieldPublicationScene {
        layers,
        composition_method,
        composition_order,
        field_scene_hash: snapshot.field_scene_hash.clone(),
    })
}

fn validate_recipe_field_scene(
    field_scene: Option<&RecipeFieldPublicationScene>,
    admission: &PublicationExportAdmissionReceipt,
) -> Result<(), String> {
    if admission.field_admitted != field_scene.is_some()
        || admission.field_scene_hash.as_deref()
            != field_scene.map(|scene| scene.field_scene_hash.as_str())
    {
        return Err("publication field recipe does not match admission".to_owned());
    }
    let Some(field_scene) = field_scene else {
        return Ok(());
    };
    if field_scene.layers.len() != usize::from(admission.field_layer_count)
        || !matches!(
            field_scene.composition_method.as_str(),
            "weighted_blended_oit" | "premultiplied_alpha_fallback"
        )
        || !matches!(
            (
                field_scene.composition_method.as_str(),
                field_scene.composition_order.as_str()
            ),
            ("weighted_blended_oit", "order_independent")
                | (
                    "premultiplied_alpha_fallback",
                    "stable_layer_id_ascending_then_translucent_structure"
                )
        )
    {
        return Err("publication field scene metadata is invalid".to_owned());
    }
    let mut has_isosurface = false;
    let mut has_volume = false;
    for layer in &field_scene.layers {
        let positive = layer
            .representations
            .iter()
            .any(|item| item == "positive_isosurface");
        let negative = layer
            .representations
            .iter()
            .any(|item| item == "negative_isosurface");
        has_isosurface |= positive || negative;
        has_volume |= layer
            .representations
            .iter()
            .any(|item| item == "volume_raycast");
        if !matches!(
            layer.scalar_unit.as_str(),
            "electron_per_cubic_angstrom" | "electron_per_bohr_cubed" | "arbitrary"
        ) || !layer.scalar_range.iter().all(|value| value.is_finite())
            || layer.scalar_range[0] > layer.scalar_range[1]
            || layer.representations.is_empty()
            || (positive
                && !layer
                    .positive_isovalue
                    .is_some_and(|value| value.is_finite() && value > 0.0))
            || (negative
                && !layer
                    .negative_isovalue
                    .is_some_and(|value| value.is_finite() && value > 0.0))
            || layer.clip_planes.len() > crate::renderer::field_scene::MAX_FIELD_CLIP_PLANES
            || layer.slices.len() > crate::renderer::field_scene::MAX_FIELD_SLICES
            || layer.transfer_function.color_space != "LinearRgb"
            || ![layer.positive_color, layer.negative_color]
                .iter()
                .flatten()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            || layer.colormap_mode > 9
            || !layer.opacity_scale.is_finite()
            || !(0.0..=10.0).contains(&layer.opacity_scale)
            || !layer.density_cutoff.is_finite()
            || layer.density_cutoff < 0.0
            || layer.display_range.is_some_and(|range| {
                !range.iter().all(|value| value.is_finite()) || range[0] >= range[1]
            })
        {
            return Err("publication field layer is invalid".to_owned());
        }
        for plane in &layer.clip_planes {
            if !plane.signed_offset_angstrom.is_finite()
                || !plane.normal.iter().all(|value| value.is_finite())
            {
                return Err("publication field clip plane is invalid".to_owned());
            }
        }
        for points in [
            &layer.transfer_function.negative_control_points,
            &layer.transfer_function.positive_control_points,
        ] {
            if points.len() < 2
                || points.len() > crate::renderer::field_scene::MAX_FIELD_TRANSFER_POINTS
            {
                return Err("publication field transfer control point count is invalid".to_owned());
            }
            let mut previous = -1.0_f32;
            for point in points {
                if !point.position.is_finite()
                    || !(0.0..=1.0).contains(&point.position)
                    || point.position <= previous
                    || !point
                        .color_linear_rgba
                        .iter()
                        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
                {
                    return Err("publication field transfer control points are invalid".to_owned());
                }
                previous = point.position;
            }
        }
        for slice in &layer.slices {
            if slice.interpolation != "trilinear"
                || slice.dimensions[0] == 0
                || slice.dimensions[1] == 0
                || slice.dimensions.iter().any(|dimension| {
                    *dimension > crate::renderer::field_scene::MAX_FIELD_SLICE_DIMENSION
                })
                || slice.dimensions[0]
                    .checked_mul(slice.dimensions[1])
                    .is_none_or(|count| {
                        count
                            > crate::renderer::field_scene::MAX_FIELD_SLICE_DIMENSION
                                * crate::renderer::field_scene::MAX_FIELD_SLICE_DIMENSION
                    })
                || slice.contour_levels.len()
                    > crate::renderer::field_scene::MAX_FIELD_CONTOUR_LEVELS
                || !slice.normal.iter().all(|value| value.is_finite())
                || !slice.signed_offset_angstrom.is_finite()
            {
                return Err("publication field slice is invalid".to_owned());
            }
            let mut previous = f32::NEG_INFINITY;
            for &level in &slice.contour_levels {
                if !level.is_finite() || level <= previous {
                    return Err("publication field contour levels are invalid".to_owned());
                }
                previous = level;
            }
        }
    }
    if has_isosurface != admission.request.has_isosurface
        || has_volume != admission.request.has_volume
    {
        return Err("publication field representations do not match admission".to_owned());
    }
    Ok(())
}

impl PublicationRasterRecipe {
    pub fn from_current_scene(
        source: &CrystalState,
        settings: &AppSettings,
        renderer: &Renderer,
        look_profile: PublicationLookProfile,
        width: u32,
        height: u32,
        requested_background: &str,
        raster_format: &str,
    ) -> Result<(Self, Vec<crate::renderer::instance::BondInstance>), String> {
        look_profile.validate_fixed()?;
        let publication_bond_instance_count = if renderer.show_bonds {
            crate::renderer::instance::publication_bond_instance_count(
                source,
                settings,
                look_profile.bond_color_mode,
            )
            .map_err(|error| error.message)?
        } else {
            0
        };
        let request = renderer.publication_export_request(
            width,
            height,
            PublicationExportSourceState {
                has_measurement_state: !source.measurements.is_empty(),
                has_selection_highlights: !source.selected_atoms.is_empty(),
                has_wannier_overlay: source.wannier_overlay.is_some(),
                has_active_phonon_state: source.active_phonon_mode.is_some(),
            },
            publication_bond_instance_count,
        );
        let field_snapshot = renderer.field_publication_snapshot();
        let admission = match field_snapshot.as_ref() {
            Some(snapshot) => evaluate_field_publication_export_admission(
                request,
                renderer.publication_export_limits(),
                snapshot,
            ),
            None => {
                evaluate_publication_export_admission(request, renderer.publication_export_limits())
            }
        }
        .map_err(|error| error.to_string())?;
        let recipe_field_scene = field_snapshot
            .as_ref()
            .map(recipe_field_scene)
            .transpose()?;
        let publication_bond_instances = if renderer.show_bonds {
            crate::renderer::instance::build_publication_bond_instances_with_count(
                source,
                settings,
                look_profile.bond_color_mode,
                publication_bond_instance_count,
            )
            .map_err(|error| error.message)?
        } else {
            Vec::new()
        };
        let camera =
            renderer.publication_export_camera(width, height, &publication_bond_instances)?;
        let config = &renderer.gpu.render_config;
        let render_target_format = renderer.gpu.surface_format();
        let render_plan = admission.render_plan;
        if !render_target_format.is_srgb() {
            return Err(format!(
                "publication raster requires an sRGB render target, got {render_target_format:?}"
            ));
        }

        let (effective_background, effective_background_rgba_linear) =
            effective_background(renderer, requested_background, raster_format)?;
        let background = match effective_background.as_str() {
            "transparent" => PublicationBackground::Transparent,
            "white" => PublicationBackground::White,
            "black" => PublicationBackground::Black,
            "default" => PublicationBackground::Current,
            _ => return Err("publication cell-line background is unsupported".to_owned()),
        };
        let cell_line_style = cell_line_style_for_background(background, renderer.clear_color)?;
        let look_recipe = PublicationLookRecipe::from_profile(look_profile);
        validate_recipe_metadata_inputs(source, settings)?;
        let encoded_alpha_policy = match (raster_format, effective_background.as_str()) {
            ("png", "transparent") => "straight",
            ("png", _) => "opaque",
            ("jpeg", _) => "none",
            _ => "unsupported",
        };
        let codec = codec_for_raster_format(raster_format)?;
        let mut custom_atom_colors_rgba = BTreeMap::new();
        for (element, color) in &settings.custom_atom_colors {
            custom_atom_colors_rgba.insert(element.clone(), *color);
        }

        let recipe = Self {
            schema: EXPORT_RECIPE_SCHEMA.to_owned(),
            schema_version: EXPORT_RECIPE_SCHEMA_VERSION,
            kind: ExportRecipeKind::PublicationRaster,
            application_version: env!("CARGO_PKG_VERSION").to_owned(),
            generated_at_unix_ms: unix_time_ms()?,
            success: true,
            source: RecipeSource {
                structure_name: source.name.clone(),
                source_version: source.version,
                intrinsic_atom_count: source.intrinsic_sites,
                structure_hash: Some(canonical_structure_sha256(source)),
                structure_hash_algorithm: Some("sha256-canonical-crystal-state-v1".to_owned()),
                source_length_unit: "angstrom".to_owned(),
                coordinate_space: "cartesian_right_handed_y_up".to_owned(),
            },
            camera: RecipeCamera {
                eye: camera.eye.to_array(),
                target: camera.target.to_array(),
                up: camera.up.to_array(),
                projection: if camera.is_perspective {
                    "perspective".to_owned()
                } else {
                    "orthographic".to_owned()
                },
                fovy_deg: camera.fovy_deg,
                orthographic_scale: camera.orthographic_scale,
                znear: camera.znear,
                zfar: camera.zfar,
                aspect_policy: "fit_visible_structure_to_export_aspect_with_margin_v1".to_owned(),
                fit_visible_structure_to_export: true,
                publication_framing_margin: 0.08,
            },
            scene: RecipeScene {
                atoms: true,
                bonds: renderer.show_bonds,
                unit_cell: renderer.show_cell,
                measurements: false,
                hoppings: false,
                isosurface: request.has_isosurface,
                volume: request.has_volume,
                stable_periodic_image_policy: "current_renderer_visible_images".to_owned(),
            },
            materials: RecipeMaterials {
                material_profile: look_recipe.profile_id.clone(),
                look_profile: look_recipe,
                atom_radius_policy: "mapped_covalent_radius_angstrom_scaled".to_owned(),
                atom_radius_scale: settings.atom_scale,
                bond_tolerance: settings.bond_tolerance,
                bond_radius: settings.bond_radius,
                radius_length_unit: "angstrom".to_owned(),
                bond_color_rgba: settings.bond_color,
                custom_atom_colors_rgba,
                color_value_space: "sRGB_straight_alpha".to_owned(),
                cell_line_color_rgba: cell_line_style.cell_line_color_rgba,
            },
            rendering: RecipeRendering {
                lighting_policy: "publication_profile_v1".to_owned(),
                ssao: "disabled".to_owned(),
                shadows: "disabled".to_owned(),
                requested_samples: render_plan.requested_samples,
                selected_samples: render_plan.selected_samples,
                selected_capabilities: if render_plan.selected_samples == 4 {
                    vec![
                        "msaa_x4".to_owned(),
                        "depth32float_msaa_x4".to_owned(),
                        "rgba8_readback".to_owned(),
                    ]
                } else {
                    vec![
                        "single_sample_color".to_owned(),
                        "rgba8_readback".to_owned(),
                    ]
                },
                fallback_policy: "fallback_4x_to_1x_on_unsupported_active_format".to_owned(),
                applied_fallbacks: if render_plan.selected_samples == 1 {
                    vec!["msaa_x4_unavailable".to_owned()]
                } else {
                    Vec::new()
                },
                adapter_name: config.device_name.clone(),
                backend: config.backend_name.clone(),
                device_type: config.device_type.clone(),
                render_target_format: format!("{render_target_format:?}"),
                max_texture_dimension_2d: config.max_texture_dimension_2d,
                max_buffer_size: config.max_buffer_size,
                max_storage_buffer_size: config.max_storage_buffer_size,
                supports_compute_shaders: config.supports_compute_shaders,
                publication_admission: admission,
                field_scene: recipe_field_scene,
            },
            output: RecipeOutput {
                width,
                height,
                raster_format: raster_format.to_owned(),
                bit_depth_per_channel: 8,
                color_space: "sRGB".to_owned(),
                color_profile: RecipeColorProfile::srgb()?,
                requested_background: requested_background.to_owned(),
                effective_background,
                effective_background_rgba_linear,
                readback_alpha_policy: "premultiplied".to_owned(),
                encoded_alpha_policy: encoded_alpha_policy.to_owned(),
                codec,
                tile_layout: render_plan.tile_layout,
                tile_dimensions: render_plan.tile_dimensions,
                tile_overlap_pixels: render_plan.tile_overlap_pixels,
            },
            artifact: None,
        };
        recipe.validate_fields(false)?;
        Ok((recipe, publication_bond_instances))
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_fields(true)
    }

    fn validate_fields(&self, require_artifact: bool) -> Result<(), String> {
        if self.schema != EXPORT_RECIPE_SCHEMA {
            return Err(format!("unknown export recipe schema `{}`", self.schema));
        }
        if self.schema_version != EXPORT_RECIPE_SCHEMA_VERSION {
            return Err(format!(
                "unsupported export recipe schema version {}",
                self.schema_version
            ));
        }
        if self.kind != ExportRecipeKind::PublicationRaster {
            return Err("publication raster sidecar has an invalid recipe kind".to_owned());
        }
        if self.application_version.is_empty() {
            return Err("application version must not be empty".to_owned());
        }
        if self.generated_at_unix_ms == 0 {
            return Err("generation timestamp must be non-zero".to_owned());
        }
        if !self.success {
            return Err("a completed publication recipe must report success".to_owned());
        }
        if self.source.source_length_unit != "angstrom"
            || self.source.coordinate_space != "cartesian_right_handed_y_up"
        {
            return Err("source units or coordinate space are not declared".to_owned());
        }
        validate_recipe_metadata(
            &self.source.structure_name,
            self.materials.custom_atom_colors_rgba.len(),
            self.materials
                .custom_atom_colors_rgba
                .keys()
                .map(String::as_str),
        )?;
        match (
            self.source.structure_hash.as_deref(),
            self.source.structure_hash_algorithm.as_deref(),
        ) {
            (Some(hash), Some("sha256-canonical-crystal-state-v1")) if is_sha256(hash) => {}
            _ => return Err("source structure hash metadata is invalid".to_owned()),
        }
        if self.source.intrinsic_atom_count == 0 {
            return Err("publication export requires at least one intrinsic atom".to_owned());
        }

        let camera_finite = self
            .camera
            .eye
            .iter()
            .chain(self.camera.target.iter())
            .chain(self.camera.up.iter())
            .chain(
                [
                    self.camera.fovy_deg,
                    self.camera.orthographic_scale,
                    self.camera.znear,
                    self.camera.zfar,
                ]
                .iter(),
            )
            .all(|value| value.is_finite());
        if !camera_finite
            || self.camera.znear <= 0.0
            || self.camera.zfar <= self.camera.znear
            || self.camera.fovy_deg <= 0.0
            || self.camera.fovy_deg >= 180.0
            || self.camera.orthographic_scale <= 0.0
        {
            return Err("camera contains non-finite or invalid projection values".to_owned());
        }
        if !matches!(
            self.camera.projection.as_str(),
            "perspective" | "orthographic"
        ) {
            return Err("camera projection is unsupported".to_owned());
        }
        if self.camera.aspect_policy != "fit_visible_structure_to_export_aspect_with_margin_v1"
            || !self.camera.fit_visible_structure_to_export
            || self.camera.publication_framing_margin != 0.08
        {
            return Err("camera aspect policy is unsupported".to_owned());
        }
        let eye_target_distance_squared = self
            .camera
            .eye
            .iter()
            .zip(self.camera.target.iter())
            .map(|(eye, target)| (*eye - *target).powi(2))
            .sum::<f32>();
        let up_length_squared = self
            .camera
            .up
            .iter()
            .map(|value| value.powi(2))
            .sum::<f32>();
        if eye_target_distance_squared <= f32::EPSILON || up_length_squared <= f32::EPSILON {
            return Err("camera eye, target, or up vector is degenerate".to_owned());
        }

        let request = &self.rendering.publication_admission.request;
        if !self.scene.atoms
            || self.scene.measurements
            || self.scene.hoppings
            || self.scene.stable_periodic_image_policy != "current_renderer_visible_images"
            || self.scene.measurements != request.has_measurement_overlays
            || self.scene.hoppings != request.has_hopping_overlays
            || self.scene.isosurface != request.has_isosurface
            || self.scene.volume != request.has_volume
        {
            return Err("recipe scene does not match its admitted publication scene".to_owned());
        }
        if (request.has_isosurface || request.has_volume)
            != self.rendering.publication_admission.field_admitted
        {
            return Err("field publication scene is not bound to its admission".to_owned());
        }

        let material_finite = std::iter::once(&self.materials.atom_radius_scale)
            .chain(std::iter::once(&self.materials.bond_tolerance))
            .chain(std::iter::once(&self.materials.bond_radius))
            .chain(self.materials.bond_color_rgba.iter())
            .chain(self.materials.cell_line_color_rgba.iter())
            .chain(self.materials.custom_atom_colors_rgba.values().flatten())
            .all(|value| value.is_finite());
        if !material_finite
            || self.materials.atom_radius_scale <= 0.0
            || self.materials.bond_tolerance < 0.0
            || self.materials.bond_radius < 0.0
            || !self
                .materials
                .bond_color_rgba
                .iter()
                .chain(self.materials.cell_line_color_rgba.iter())
                .chain(self.materials.custom_atom_colors_rgba.values().flatten())
                .all(|value| (0.0..=1.0).contains(value))
        {
            return Err("material settings contain non-finite or invalid values".to_owned());
        }
        if self.materials.radius_length_unit != "angstrom" {
            return Err("material radius length unit is not declared".to_owned());
        }
        let effective_background = match self.output.effective_background.as_str() {
            "transparent" => PublicationBackground::Transparent,
            "white" => PublicationBackground::White,
            "black" => PublicationBackground::Black,
            "default" => PublicationBackground::Current,
            _ => return Err("publication cell-line background is unsupported".to_owned()),
        };
        let background_rgba = self.output.effective_background_rgba_linear;
        let expected_cell_line_style = cell_line_style_for_background(
            effective_background,
            wgpu::Color {
                r: background_rgba[0],
                g: background_rgba[1],
                b: background_rgba[2],
                a: background_rgba[3],
            },
        )?;
        if self.materials.cell_line_color_rgba != expected_cell_line_style.cell_line_color_rgba {
            return Err(
                "material cell-line color does not match the effective background".to_owned(),
            );
        }
        if self.materials.atom_radius_policy != "mapped_covalent_radius_angstrom_scaled" {
            return Err("material policy is unsupported".to_owned());
        }
        self.materials.look_profile.validate()?;
        let fixed_profile_id = match self.materials.look_profile.profile_id.as_str() {
            "scientific_gloss" => PublicationLookProfileId::ScientificGloss,
            "studio" => PublicationLookProfileId::Studio,
            "unlit" => PublicationLookProfileId::Unlit,
            _ => return Err("publication look profile metadata is unsupported".to_owned()),
        };
        let fixed_look =
            PublicationLookRecipe::from_profile(PublicationLookProfile::for_id(fixed_profile_id)?);
        if self.materials.look_profile != fixed_look {
            return Err("publication look does not match its fixed profile snapshot".to_owned());
        }
        if self.materials.material_profile != self.materials.look_profile.profile_id
            || self.materials.color_value_space != "sRGB_straight_alpha"
        {
            return Err("publication material profile is inconsistent".to_owned());
        }

        if self.output.width == 0 || self.output.height == 0 {
            return Err("export dimensions must be non-zero".to_owned());
        }
        validate_publication_export_receipt_fields(&self.rendering.publication_admission)
            .map_err(|error| error.to_string())?;
        validate_recipe_field_scene(
            self.rendering.field_scene.as_ref(),
            &self.rendering.publication_admission,
        )?;
        if self.rendering.publication_admission.request.width != self.output.width
            || self.rendering.publication_admission.request.height != self.output.height
            || self
                .rendering
                .publication_admission
                .limits
                .max_texture_dimension_2d
                != self.rendering.max_texture_dimension_2d
            || self.rendering.publication_admission.limits.max_buffer_size
                != self.rendering.max_buffer_size
        {
            return Err(
                "publication admission receipt does not match recipe output or limits".to_owned(),
            );
        }
        if !matches!(self.output.raster_format.as_str(), "png" | "jpeg") {
            return Err(format!(
                "unsupported publication raster format `{}`",
                self.output.raster_format
            ));
        }
        if self.output.bit_depth_per_channel != 8 || self.output.color_space != "sRGB" {
            return Err("only 8-bit sRGB publication output is supported".to_owned());
        }
        validate_color_profile_contract(&self.output.color_profile)?;
        if !self
            .output
            .effective_background_rgba_linear
            .iter()
            .all(|value| value.is_finite())
        {
            return Err("effective background contains a non-finite value".to_owned());
        }
        validate_background_contract(&self.output)?;
        validate_alpha_contract(&self.output)?;
        validate_codec_contract(&self.output)?;
        if self.output.tile_layout[0] == 0
            || self.output.tile_layout[1] == 0
            || self.output.tile_dimensions[0] == 0
            || self.output.tile_dimensions[1] == 0
            || self.output.tile_overlap_pixels != 0
            || self.output.tile_dimensions[0] > self.output.width
            || self.output.tile_dimensions[1] > self.output.height
            || self.output.tile_layout[0]
                != self.output.width.div_ceil(self.output.tile_dimensions[0])
            || self.output.tile_layout[1]
                != self.output.height.div_ceil(self.output.tile_dimensions[1])
        {
            return Err("publication tile metadata is invalid".to_owned());
        }

        let plan = self.rendering.publication_admission.render_plan;
        if self.output.tile_layout != plan.tile_layout
            || self.output.tile_dimensions != plan.tile_dimensions
            || self.output.tile_overlap_pixels != plan.tile_overlap_pixels
            || self.rendering.requested_samples != plan.requested_samples
            || self.rendering.selected_samples != plan.selected_samples
        {
            return Err("recipe rendering metadata does not match its admission plan".to_owned());
        }

        if self.rendering.lighting_policy != "publication_profile_v1"
            || self.rendering.ssao != "disabled"
            || self.rendering.shadows != "disabled"
            || self.rendering.requested_samples != 4
            || (self.rendering.selected_samples != 4 && self.rendering.selected_samples != 1)
            || self.rendering.fallback_policy != "fallback_4x_to_1x_on_unsupported_active_format"
            || (self.rendering.selected_samples == 4
                && (self.rendering.selected_capabilities
                    != ["msaa_x4", "depth32float_msaa_x4", "rgba8_readback"]
                    || !self.rendering.applied_fallbacks.is_empty()))
            || (self.rendering.selected_samples == 1
                && (self.rendering.selected_capabilities
                    != ["single_sample_color", "rgba8_readback"]
                    || self.rendering.applied_fallbacks != ["msaa_x4_unavailable"]))
        {
            return Err("rendering policy is unsupported".to_owned());
        }
        if self.rendering.adapter_name.is_empty()
            || self.rendering.backend.is_empty()
            || self.rendering.device_type.is_empty()
            || !matches!(
                self.rendering.render_target_format.as_str(),
                "Bgra8UnormSrgb" | "Rgba8UnormSrgb"
            )
        {
            return Err("renderer adapter identity is incomplete".to_owned());
        }

        if require_artifact {
            let artifact = self
                .artifact
                .as_ref()
                .ok_or_else(|| "completed recipe is missing its primary artifact".to_owned())?;
            if artifact.file_name.is_empty()
                || Path::new(&artifact.file_name)
                    .file_name()
                    .and_then(|name| name.to_str())
                    != Some(artifact.file_name.as_str())
                || !is_sha256(&artifact.sha256)
            {
                return Err("primary artifact name or SHA-256 is invalid".to_owned());
            }
        }
        Ok(())
    }
}

pub fn parse_publication_recipe(bytes: &[u8]) -> Result<PublicationRasterRecipe, String> {
    if bytes.len()
        > usize::try_from(MAX_PUBLICATION_RECIPE_BYTES)
            .map_err(|_| "publication recipe limit exceeds addressable memory".to_owned())?
    {
        return Err("publication recipe exceeds the maximum supported size".to_owned());
    }
    let recipe: PublicationRasterRecipe = serde_json::from_slice(bytes)
        .map_err(|error| format!("unable to parse publication recipe: {error}"))?;
    recipe.validate()?;
    Ok(recipe)
}

pub fn publication_sidecar_path(primary_path: &Path) -> Result<PathBuf, String> {
    let file_stem = primary_path
        .file_stem()
        .ok_or_else(|| "export path must include a file name".to_owned())?;
    let mut sidecar_name = OsString::from(file_stem);
    sidecar_name.push(".crystalcanvas.json");
    Ok(primary_path.with_file_name(sidecar_name))
}

pub(crate) fn validate_publication_raster_targets(
    primary_path: &Path,
) -> Result<&'static str, String> {
    let format = raster_format_from_path(primary_path)?;
    let sidecar_path = publication_sidecar_path(primary_path)?;
    ensure_output_path_available(primary_path, "publication image")?;
    ensure_output_path_available(&sidecar_path, "export recipe")?;

    let parent = primary_path.parent().unwrap_or_else(|| Path::new("."));
    let metadata = std::fs::metadata(parent)
        .map_err(|error| format!("unable to inspect publication output directory: {error}"))?;
    if !metadata.is_dir() {
        return Err("publication output parent is not a directory".to_owned());
    }
    Ok(format.as_str())
}

pub fn write_publication_raster_pair(
    primary_path: &Path,
    rgba: Vec<u8>,
    mut recipe: PublicationRasterRecipe,
) -> Result<PathBuf, String> {
    recipe.validate_fields(false)?;
    let path_format = validate_publication_raster_targets(primary_path)?;
    if path_format != recipe.output.raster_format {
        return Err(format!(
            "path raster format `{path_format}` does not match recipe format `{}`",
            recipe.output.raster_format
        ));
    }
    let primary_file_name = primary_path
        .file_name()
        .ok_or_else(|| "export path must include a file name".to_owned())?
        .to_string_lossy()
        .into_owned();

    let sidecar_path = publication_sidecar_path(primary_path)?;
    let image_temp = temporary_sibling(primary_path, "image")?;
    let recipe_temp = temporary_sibling(&sidecar_path, "recipe")?;
    let format = raster_format_from_recipe(&recipe.output.raster_format)?;
    if let Err(error) = encode_raster_to_staged_file(
        &recipe.output,
        rgba,
        format,
        usize::try_from(
            recipe
                .rendering
                .publication_admission
                .estimate
                .max_encoded_bytes,
        )
        .map_err(|_| "encoded publication image budget exceeds addressable memory".to_owned())?,
        &image_temp,
    ) {
        return Err(error);
    }
    let artifact_sha256 = match hex_sha256_file(&image_temp) {
        Ok(sha256) => sha256,
        Err(error) => {
            return Err(with_staged_cleanup_error(
                error,
                &image_temp,
                "publication image",
            ));
        }
    };
    recipe.artifact = Some(RecipeArtifact {
        file_name: primary_file_name,
        sha256: artifact_sha256,
    });
    if let Err(error) = recipe.validate() {
        return Err(with_staged_cleanup_error(
            error,
            &image_temp,
            "publication image",
        ));
    }
    let max_recipe_bytes = match usize::try_from(
        recipe
            .rendering
            .publication_admission
            .budgets
            .max_recipe_bytes,
    ) {
        Ok(max_recipe_bytes) => max_recipe_bytes,
        Err(error) => {
            return Err(with_staged_cleanup_error(
                format!("export recipe budget exceeds addressable memory: {error}"),
                &image_temp,
                "publication image",
            ));
        }
    };

    if let Err(error) = stage_recipe_file(&recipe_temp, &recipe, max_recipe_bytes) {
        return Err(with_staged_cleanup_error(
            error,
            &image_temp,
            "publication image",
        ));
    }

    commit_staged_pair(&image_temp, &recipe_temp, primary_path, &sidecar_path)?;
    sync_parent_directory(primary_path)?;

    Ok(sidecar_path)
}

pub(crate) fn validate_publication_glb_targets(primary_path: &Path) -> Result<(), String> {
    let extension = primary_path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "publication Blender path must include a .glb extension".to_owned())?;
    if extension != "glb" {
        return Err("publication Blender export requires a .glb path".to_owned());
    }
    let sidecar_path = publication_sidecar_path(primary_path)?;
    ensure_output_path_available(primary_path, "publication GLB")?;
    ensure_output_path_available(&sidecar_path, "publication GLB recipe")?;
    let parent = primary_path.parent().unwrap_or_else(|| Path::new("."));
    if !std::fs::metadata(parent)
        .map_err(|error| format!("unable to inspect publication output directory: {error}"))?
        .is_dir()
    {
        return Err("publication output parent is not a directory".to_owned());
    }
    Ok(())
}

pub fn write_publication_glb_pair(
    primary_path: &Path,
    glb: &[u8],
    mut recipe: PublicationGlbRecipe,
) -> Result<PathBuf, String> {
    validate_publication_glb_targets(primary_path)?;
    recipe.validate()?;
    crate::blender_export::validate_glb_export_identity(glb, &recipe.export_id)?;
    let sidecar_path = publication_sidecar_path(primary_path)?;
    let glb_temp = temporary_sibling(primary_path, "glb")?;
    let recipe_temp = temporary_sibling(&sidecar_path, "recipe")?;
    let stage = |path: &Path, bytes: &[u8], label: &str| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| format!("unable to stage {label}: {error}"))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("unable to stage {label}: {error}"))
    };
    if let Err(error) = stage(&glb_temp, glb, "publication GLB") {
        let _ = std::fs::remove_file(&glb_temp);
        return Err(error);
    }
    recipe.artifact = Some(RecipeArtifact {
        file_name: primary_path
            .file_name()
            .ok_or_else(|| "export path must include a file name".to_owned())?
            .to_string_lossy()
            .into_owned(),
        sha256: hex_sha256(glb),
    });
    recipe.validate()?;
    let stage_recipe = || -> Result<(), String> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&recipe_temp)
            .map_err(|error| format!("unable to stage publication GLB recipe: {error}"))?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, &recipe)
            .map_err(|error| format!("unable to serialize publication GLB recipe: {error}"))?;
        writer
            .flush()
            .and_then(|()| writer.get_ref().sync_all())
            .map_err(|error| format!("unable to stage publication GLB recipe: {error}"))
    };
    if let Err(error) = stage_recipe() {
        let _ = std::fs::remove_file(&glb_temp);
        let _ = std::fs::remove_file(&recipe_temp);
        return Err(error);
    }
    if let Err(error) = commit_no_replace(&glb_temp, primary_path, "publication GLB") {
        let _ = std::fs::remove_file(&glb_temp);
        let _ = std::fs::remove_file(&recipe_temp);
        return Err(error);
    }
    if let Err(error) = commit_no_replace(&recipe_temp, &sidecar_path, "publication GLB recipe") {
        let rollback = std::fs::remove_file(primary_path);
        let _ = std::fs::remove_file(&recipe_temp);
        return Err(match rollback {
            Ok(()) => error,
            Err(rollback_error) => format!(
                "{error}; unable to remove_file incomplete publication GLB: {rollback_error}"
            ),
        });
    }
    sync_parent_directory(primary_path)?;
    Ok(sidecar_path)
}

fn effective_background(
    renderer: &Renderer,
    requested_background: &str,
    raster_format: &str,
) -> Result<(String, [f64; 4]), String> {
    let requested_rgba = match requested_background {
        "transparent" => [0.0, 0.0, 0.0, 0.0],
        "white" => [1.0, 1.0, 1.0, 1.0],
        "black" => [0.0, 0.0, 0.0, 1.0],
        "default" => {
            let color = renderer.clear_color;
            [color.r, color.g, color.b, color.a]
        }
        value => return Err(format!("unsupported export background `{value}`")),
    };

    if raster_format == "jpeg" && requested_background == "transparent" {
        Ok(("white".to_owned(), [1.0, 1.0, 1.0, 1.0]))
    } else {
        Ok((requested_background.to_owned(), requested_rgba))
    }
}

fn validate_alpha_contract(output: &RecipeOutput) -> Result<(), String> {
    if output.readback_alpha_policy != "premultiplied" {
        return Err("renderer readback alpha policy is unsupported".to_owned());
    }
    match (
        output.raster_format.as_str(),
        output.effective_background.as_str(),
        output.encoded_alpha_policy.as_str(),
    ) {
        ("png", "transparent", "straight")
        | ("png", "white" | "black" | "default", "opaque")
        | ("jpeg", "white" | "black" | "default", "none") => Ok(()),
        _ => Err("raster format, background, and alpha policy are inconsistent".to_owned()),
    }
}

fn validate_codec_contract(output: &RecipeOutput) -> Result<(), String> {
    match (&output.raster_format[..], &output.codec) {
        (
            "png",
            RecipeCodec::Png {
                compression,
                filter,
            },
        ) if compression == PNG_COMPRESSION && filter == PNG_FILTER => Ok(()),
        (
            "jpeg",
            RecipeCodec::Jpeg {
                quality,
                chroma_subsampling,
            },
        ) if *quality == JPEG_QUALITY && chroma_subsampling == JPEG_CHROMA_SUBSAMPLING => Ok(()),
        _ => Err("raster format and codec policy are inconsistent".to_owned()),
    }
}

fn validate_color_profile_contract(profile: &RecipeColorProfile) -> Result<(), String> {
    let expected_profile = srgb_icc_profile()?;
    if profile.name != SRGB_PROFILE_NAME
        || profile.version != SRGB_PROFILE_VERSION
        || profile.sha256 != hex_sha256(expected_profile)
    {
        return Err("color profile metadata does not match the embedded sRGB profile".to_owned());
    }
    Ok(())
}

fn validate_recipe_metadata_inputs(
    source: &CrystalState,
    settings: &AppSettings,
) -> Result<(), String> {
    validate_recipe_metadata(
        &source.name,
        settings.custom_atom_colors.len(),
        settings.custom_atom_colors.keys().map(String::as_str),
    )
}

fn validate_recipe_metadata<'a>(
    structure_name: &str,
    custom_atom_color_count: usize,
    mut custom_atom_color_keys: impl Iterator<Item = &'a str>,
) -> Result<(), String> {
    if structure_name.len() > MAX_RECIPE_STRUCTURE_NAME_BYTES {
        return Err("structure name exceeds the publication recipe limit".to_owned());
    }
    if custom_atom_color_count > MAX_RECIPE_CUSTOM_ATOM_COLORS {
        return Err("custom atom color count exceeds the publication recipe limit".to_owned());
    }
    if custom_atom_color_keys.any(|element| {
        element.is_empty()
            || element.len() > MAX_RECIPE_ELEMENT_SYMBOL_BYTES
            || !element.bytes().all(|byte| byte.is_ascii_alphabetic())
    }) {
        return Err("custom atom color key is not a bounded element symbol".to_owned());
    }
    Ok(())
}

fn validate_background_contract(output: &RecipeOutput) -> Result<(), String> {
    let expected_effective = match (
        output.raster_format.as_str(),
        output.requested_background.as_str(),
    ) {
        ("jpeg", "transparent") => "white",
        ("png" | "jpeg", background @ ("white" | "black" | "default")) => background,
        ("png", "transparent") => "transparent",
        _ => return Err("requested background is unsupported".to_owned()),
    };
    if output.effective_background != expected_effective {
        return Err("requested and effective backgrounds are inconsistent".to_owned());
    }

    let rgba = output.effective_background_rgba_linear;
    if !rgba.iter().all(|value| (0.0..=1.0).contains(value)) {
        return Err("effective background must use normalized linear RGBA".to_owned());
    }
    let expected_rgba = match output.effective_background.as_str() {
        "transparent" => Some([0.0, 0.0, 0.0, 0.0]),
        "white" => Some([1.0, 1.0, 1.0, 1.0]),
        "black" => Some([0.0, 0.0, 0.0, 1.0]),
        "default" => None,
        _ => return Err("effective background is unsupported".to_owned()),
    };
    if expected_rgba.is_some_and(|expected| rgba != expected)
        || (output.effective_background == "default" && rgba[3] != 1.0)
    {
        return Err("effective background RGBA does not match its policy".to_owned());
    }
    Ok(())
}

fn raster_format_from_path(primary_path: &Path) -> Result<RasterFormat, String> {
    let extension = primary_path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "publication export path must include a raster extension".to_owned())?;
    match extension.as_str() {
        "png" => Ok(RasterFormat::Png),
        "jpg" | "jpeg" => Ok(RasterFormat::Jpeg),
        _ => Err(format!(
            "unsupported publication raster extension `{extension}`"
        )),
    }
}

fn ensure_output_path_available(path: &Path, label: &str) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err(format!("{label} target already exists")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("unable to inspect {label} target: {error}")),
    }
}

fn encode_raster_to_staged_file(
    output: &RecipeOutput,
    mut rgba: Vec<u8>,
    format: RasterFormat,
    max_encoded_bytes: usize,
    staged_path: &Path,
) -> Result<(), String> {
    let expected = usize::try_from(output.width)
        .ok()
        .and_then(|width| {
            usize::try_from(output.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "export byte count overflow".to_owned())?;
    if rgba.len() != expected {
        return Err("offscreen image byte count does not match output dimensions".to_owned());
    }

    if max_encoded_bytes < expected {
        return Err("encoded publication image budget is below the raw image size".to_owned());
    }
    let icc_profile = srgb_icc_profile()?.to_vec();
    let staged_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(staged_path)
        .map_err(|error| format!("unable to stage publication image: {error}"))?;
    let mut writer = BoundedFileWriter::new(BufWriter::new(staged_file), max_encoded_bytes);
    let result = match format {
        RasterFormat::Png => {
            if output.encoded_alpha_policy == "straight" {
                unpremultiply_rgba(&mut rgba);
            }
            let mut encoder = PngEncoder::new_with_quality(
                &mut writer,
                CompressionType::Default,
                FilterType::Adaptive,
            );
            encoder
                .set_icc_profile(icc_profile)
                .map_err(|error| format!("unable to set publication PNG color profile: {error}"))?;
            encoder
                .write_image(&rgba, output.width, output.height, ExtendedColorType::Rgba8)
                .map_err(|error| format!("unable to encode publication PNG: {error}"))
        }
        RasterFormat::Jpeg => {
            let composite_white = output.requested_background == "transparent";
            let rgb_len = expected
                .checked_div(4)
                .and_then(|pixels| pixels.checked_mul(3))
                .ok_or_else(|| "JPEG byte count overflow".to_owned())?;
            let mut rgb = Vec::new();
            rgb.try_reserve_exact(rgb_len)
                .map_err(|_| "unable to allocate JPEG RGB buffer".to_owned())?;
            for pixel in rgba.chunks_exact(4) {
                if composite_white {
                    rgb.extend_from_slice(&composite_premultiplied_srgb_over_white(pixel));
                } else {
                    rgb.extend_from_slice(&pixel[..3]);
                }
            }
            let mut encoder = JpegEncoder::new_with_quality(&mut writer, JPEG_QUALITY);
            encoder.set_icc_profile(icc_profile).map_err(|error| {
                format!("unable to set publication JPEG color profile: {error}")
            })?;
            encoder
                .write_image(&rgb, output.width, output.height, ExtendedColorType::Rgb8)
                .map_err(|error| format!("unable to encode publication JPEG: {error}"))
        }
    };
    let result = result.and_then(|()| sync_bounded_writer(writer, "publication image"));
    if let Err(error) = result {
        return Err(with_staged_cleanup_error(
            error,
            staged_path,
            "publication image",
        ));
    }
    Ok(())
}

struct BoundedFileWriter<W> {
    writer: W,
    max_len: usize,
    written: usize,
}

impl<W> BoundedFileWriter<W> {
    const fn new(writer: W, max_len: usize) -> Self {
        Self {
            writer,
            max_len,
            written: 0,
        }
    }

    fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> Write for BoundedFileWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self
            .written
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("encoded image cursor position overflow"))?;
        if end > self.max_len {
            return Err(std::io::Error::other(
                "encoded publication image exceeds its admission budget",
            ));
        }
        let written = self.writer.write(bytes)?;
        self.written = self
            .written
            .checked_add(written)
            .ok_or_else(|| std::io::Error::other("encoded image cursor position overflow"))?;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

fn srgb_icc_profile() -> Result<&'static [u8], String> {
    if let Some(profile) = SRGB_ICC_PROFILE.get() {
        return Ok(profile);
    }

    let mut profile = moxcms::ColorProfile::new_srgb()
        .encode()
        .map_err(|error| format!("unable to build the sRGB ICC profile: {error}"))?;
    if profile.len() < 40 || &profile[36..40] != b"acsp" {
        return Err("generated sRGB ICC profile is invalid".to_owned());
    }
    profile[24..36].copy_from_slice(&FIXED_ICC_CREATION_DATE);
    let _ = SRGB_ICC_PROFILE.set(profile);
    SRGB_ICC_PROFILE
        .get()
        .map(Vec::as_slice)
        .ok_or_else(|| "unable to initialize the sRGB ICC profile".to_owned())
}

fn unpremultiply_rgba(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = f32::from(pixel[3]) / 255.0;
        if alpha == 0.0 {
            pixel[..3].fill(0);
        } else if alpha < 1.0 {
            for channel in &mut pixel[..3] {
                *channel = linear_to_srgb_byte(srgb_byte_to_linear(*channel) / alpha);
            }
        }
    }
}

fn composite_premultiplied_srgb_over_white(pixel: &[u8]) -> [u8; 3] {
    let alpha = f32::from(pixel[3]) / 255.0;
    let inverse_alpha = 1.0 - alpha;
    [
        linear_to_srgb_byte((srgb_byte_to_linear(pixel[0]) + inverse_alpha).min(1.0)),
        linear_to_srgb_byte((srgb_byte_to_linear(pixel[1]) + inverse_alpha).min(1.0)),
        linear_to_srgb_byte((srgb_byte_to_linear(pixel[2]) + inverse_alpha).min(1.0)),
    ]
}

fn srgb_byte_to_linear(value: u8) -> f32 {
    let srgb = f32::from(value) / 255.0;
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb_byte(value: f32) -> u8 {
    let linear = value.clamp(0.0, 1.0);
    let srgb = if linear <= 0.0031308 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round() as u8
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RasterFormat {
    Png,
    Jpeg,
}

impl RasterFormat {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
        }
    }
}

fn codec_for_raster_format(value: &str) -> Result<RecipeCodec, String> {
    match value {
        "png" => Ok(RecipeCodec::Png {
            compression: PNG_COMPRESSION.to_owned(),
            filter: PNG_FILTER.to_owned(),
        }),
        "jpeg" => Ok(RecipeCodec::Jpeg {
            quality: JPEG_QUALITY,
            chroma_subsampling: JPEG_CHROMA_SUBSAMPLING.to_owned(),
        }),
        _ => Err(format!("unsupported publication raster format `{value}`")),
    }
}

fn raster_format_from_recipe(value: &str) -> Result<RasterFormat, String> {
    match value {
        "png" => Ok(RasterFormat::Png),
        "jpeg" => Ok(RasterFormat::Jpeg),
        _ => Err(format!("unsupported publication raster format `{value}`")),
    }
}

fn temporary_sibling(path: &Path, role: &str) -> Result<PathBuf, String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| "export path must include a file name".to_owned())?;
    let nonce = TEMP_FILE_NONCE.fetch_add(1, Ordering::Relaxed);
    let mut temporary = OsString::from(".");
    temporary.push(file_name);
    temporary.push(format!(
        ".{role}.{}.{}.{}",
        std::process::id(),
        unix_time_ms()?,
        nonce
    ));
    Ok(parent.join(temporary))
}

#[cfg(test)]
fn stage_new_file(path: &Path, bytes: &[u8], label: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("unable to stage {label}: {error}"))?;
    let result = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    result.map_err(|error| {
        with_staged_cleanup_error(format!("unable to stage {label}: {error}"), path, label)
    })
}

fn stage_recipe_file(
    path: &Path,
    recipe: &PublicationRasterRecipe,
    max_recipe_bytes: usize,
) -> Result<(), String> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("unable to stage export recipe: {error}"))?;
    let mut writer = BoundedFileWriter::new(BufWriter::new(file), max_recipe_bytes);
    let result = serde_json::to_writer_pretty(&mut writer, recipe)
        .map_err(|error| format!("unable to serialize export recipe: {error}"))
        .and_then(|()| sync_bounded_writer(writer, "export recipe"));
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(with_staged_cleanup_error(error, path, "export recipe")),
    }
}

fn sync_bounded_writer(
    mut writer: BoundedFileWriter<BufWriter<File>>,
    label: &str,
) -> Result<(), String> {
    writer
        .flush()
        .map_err(|error| format!("unable to flush {label}: {error}"))?;
    writer
        .into_inner()
        .into_inner()
        .map_err(|error| format!("unable to finalize {label}: {error}"))?
        .sync_all()
        .map_err(|error| format!("unable to sync {label}: {error}"))
}

fn commit_no_replace(staged_path: &Path, final_path: &Path, label: &str) -> Result<(), String> {
    #[cfg(any(target_vendor = "apple", target_os = "linux"))]
    {
        rustix::fs::renameat_with(
            rustix::fs::CWD,
            staged_path,
            rustix::fs::CWD,
            final_path,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|error| {
            format!("unable to commit {label} without overwriting an existing file: {error}")
        })
    }
    #[cfg(target_os = "windows")]
    {
        std::fs::rename(staged_path, final_path).map_err(|error| {
            format!("unable to commit {label} without overwriting an existing file: {error}")
        })
    }
    #[cfg(not(any(target_vendor = "apple", target_os = "linux", target_os = "windows")))]
    {
        let _ = (staged_path, final_path);
        Err(format!(
            "atomic no-replace commit is unavailable for {label} on this platform"
        ))
    }
}

fn commit_staged_pair(
    image_temp: &Path,
    recipe_temp: &Path,
    primary_path: &Path,
    sidecar_path: &Path,
) -> Result<(), String> {
    if let Err(error) = commit_no_replace(image_temp, primary_path, "publication image") {
        let error = with_staged_cleanup_error(error, image_temp, "publication image");
        return Err(with_staged_cleanup_error(
            error,
            recipe_temp,
            "export recipe",
        ));
    }
    if let Err(error) = commit_no_replace(recipe_temp, sidecar_path, "export recipe") {
        let error = with_staged_cleanup_error(error, recipe_temp, "export recipe");
        let error = with_staged_cleanup_error(error, image_temp, "publication image");
        let error = format!(
            "{error}; partial commit preserved the publication image at {}",
            primary_path.display()
        );
        return Err(with_directory_sync_error(error, primary_path));
    }
    let error = with_staged_cleanup_error(String::new(), image_temp, "publication image");
    let error = with_staged_cleanup_error(error, recipe_temp, "export recipe");
    if error.is_empty() {
        Ok(())
    } else {
        let error =
            format!("publication pair was committed but staged-file cleanup failed: {error}");
        Err(with_directory_sync_error(error, primary_path))
    }
}

fn with_staged_cleanup_error(error: String, path: &Path, label: &str) -> String {
    match std::fs::remove_file(path) {
        Ok(()) => error,
        Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => error,
        Err(cleanup_error) if error.is_empty() => {
            format!(
                "unable to remove staged {label} {}: {cleanup_error}",
                path.display()
            )
        }
        Err(cleanup_error) => format!(
            "{error}; unable to remove staged {label} {}: {cleanup_error}",
            path.display()
        ),
    }
}

fn sync_parent_directory(path: &Path) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "publication export pair was committed but its directory could not be synced: {error}"
            )
        })
}

fn with_directory_sync_error(error: String, path: &Path) -> String {
    match sync_parent_directory(path) {
        Ok(()) => error,
        Err(sync_error) => format!("{error}; {sync_error}"),
    }
}

fn unix_time_ms() -> Result<u64, String> {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?
        .as_millis();
    u64::try_from(millis).map_err(|_| "generation timestamp exceeds u64".to_owned())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn hex_sha256_file(path: &Path) -> Result<String, String> {
    let file = File::open(path)
        .map_err(|error| format!("unable to read staged publication image: {error}"))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("unable to hash staged publication image: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn canonical_structure_sha256(source: &CrystalState) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"crystal-canvas:canonical-structure:v1\0");
    hash_string(&mut hasher, &source.name);
    for value in [
        source.cell_a,
        source.cell_b,
        source.cell_c,
        source.cell_alpha,
        source.cell_beta,
        source.cell_gamma,
    ] {
        hasher.update(value.to_le_bytes());
    }
    hasher.update(source.spacegroup_number.to_le_bytes());
    hash_string(&mut hasher, &source.spacegroup_hm);
    hasher.update(
        u64::try_from(source.intrinsic_sites)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update([u8::from(source.is_2d)]);
    hasher.update(
        source
            .vacuum_axis
            .map(|axis| u64::try_from(axis).unwrap_or(u64::MAX))
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hash_strings(&mut hasher, &source.labels);
    hash_strings(&mut hasher, &source.elements);
    hash_f64s(&mut hasher, &source.fract_x);
    hash_f64s(&mut hasher, &source.fract_y);
    hash_f64s(&mut hasher, &source.fract_z);
    hash_f64s(&mut hasher, &source.occupancies);
    hasher.update(
        u64::try_from(source.atomic_numbers.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(&source.atomic_numbers);
    format!("{:x}", hasher.finalize())
}

fn hash_string(hasher: &mut Sha256, value: &str) {
    hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn hash_strings(hasher: &mut Sha256, values: &[String]) {
    hasher.update(
        u64::try_from(values.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for value in values {
        hash_string(hasher, value);
    }
}

fn hash_f64s(hasher: &mut Sha256, values: &[f64]) {
    hasher.update(
        u64::try_from(values.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for value in values {
        hasher.update(value.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_pixel_output() -> RecipeOutput {
        RecipeOutput {
            width: 1,
            height: 1,
            raster_format: "png".to_owned(),
            bit_depth_per_channel: 8,
            color_space: "sRGB".to_owned(),
            color_profile: RecipeColorProfile::srgb().unwrap(),
            requested_background: "transparent".to_owned(),
            effective_background: "transparent".to_owned(),
            effective_background_rgba_linear: [0.0, 0.0, 0.0, 0.0],
            readback_alpha_policy: "premultiplied".to_owned(),
            encoded_alpha_policy: "straight".to_owned(),
            codec: RecipeCodec::Png {
                compression: PNG_COMPRESSION.to_owned(),
                filter: PNG_FILTER.to_owned(),
            },
            tile_layout: [1, 1],
            tile_dimensions: [1, 1],
            tile_overlap_pixels: 0,
        }
    }

    #[test]
    fn bounded_encoder_removes_its_staged_file_when_the_codec_exceeds_budget() {
        let directory = tempfile::tempdir().unwrap();
        let staged_path = directory.path().join(".figure.image");
        let error = encode_raster_to_staged_file(
            &one_pixel_output(),
            vec![0, 0, 0, 0],
            RasterFormat::Png,
            4,
            &staged_path,
        )
        .unwrap_err();

        assert!(error.contains("admission budget"));
        assert!(!staged_path.exists());
    }

    #[test]
    fn sidecar_commit_race_preserves_the_committed_image_without_path_based_rollback() {
        let directory = tempfile::tempdir().unwrap();
        let image_temp = directory.path().join(".figure.image");
        let recipe_temp = directory.path().join(".figure.recipe");
        let primary_path = directory.path().join("figure.png");
        let sidecar_path = directory.path().join("figure.crystalcanvas.json");
        stage_new_file(&image_temp, b"new image", "test image").unwrap();
        stage_new_file(&recipe_temp, b"new recipe", "test recipe").unwrap();
        std::fs::write(&sidecar_path, b"racing recipe").unwrap();

        let error = commit_staged_pair(&image_temp, &recipe_temp, &primary_path, &sidecar_path)
            .unwrap_err();

        assert!(error.contains("partial commit preserved"));
        assert_eq!(std::fs::read(&primary_path).unwrap(), b"new image");
        assert_eq!(std::fs::read(&sidecar_path).unwrap(), b"racing recipe");
        assert!(!image_temp.exists());
        assert!(!recipe_temp.exists());
    }
}
