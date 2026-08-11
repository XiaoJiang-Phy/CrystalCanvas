//! Immutable, structure-only scene data for one-way publication export.

use crate::ipc::{IpcError, IpcResult};
use crate::renderer::camera::Camera;
use crate::renderer::instance::{
    AtomInstance, BondInstance, RenderAtomInstance, build_cell_lines, build_instance_data,
    build_periodic_atom_instances, build_publication_bond_instances_with_count,
    publication_bond_instance_count,
};
use crate::renderer::publication_look::{PublicationBondColorMode, PublicationLookProfile};
use crate::renderer::renderer::{PublicationExportSourceState, Renderer};
use crate::renderer::{field_scene, isosurface};
use crate::{crystal_state::CrystalState, settings::AppSettings};
use sha2::{Digest, Sha256};

const MAX_PUBLICATION_GLB_NODES: usize = 75_000;
const MAX_PUBLICATION_GLB_BYTES: usize = 96 * 1024 * 1024;
const MAX_PUBLICATION_GLB_PEAK_CPU_BYTES: usize = 320 * 1024 * 1024;
const MAX_PUBLICATION_GLB_JSON_BYTES: usize = 64 * 1024 * 1024;
const GLB_JSON_BYTES_PER_NODE: usize = 1024;
const GLB_FIXED_BYTES: usize = 256 * 1024;
pub const FIELD_ISOSURFACE: &str = "FIELD_ISOSURFACE";
pub const FIELD_SLICE: &str = "FIELD_SLICE";
pub const FIELD_CONTOUR: &str = "FIELD_CONTOUR";
pub const CONTOUR_RADIUS_ANGSTROM: f32 = 0.02;
const MAX_PUBLICATION_FIELD_PRIMITIVES: usize = 256;
const MAX_PUBLICATION_FIELD_VERTICES: usize = 12_000_000;
const MAX_PUBLICATION_FIELD_TOTAL_VERTICES: usize = 1_000_000;

#[derive(Clone)]
pub struct PublicationSceneAtom {
    pub atom: AtomInstance,
    pub source_atom_index: usize,
    pub image_shift: [i32; 3],
}

/// Fixed resource envelope for a single Blender artifact. The writer must not
/// allocate or serialize beyond this accepted snapshot.
#[derive(Clone, Copy)]
pub struct PublicationGlbAdmission {
    pub atom_instances: usize,
    pub bonds: usize,
    pub nodes: usize,
    pub max_glb_bytes: usize,
    pub max_peak_cpu_bytes: usize,
}

#[derive(Clone)]
pub struct PublicationSceneSnapshot {
    pub atoms: Vec<PublicationSceneAtom>,
    pub bonds: Vec<BondInstance>,
    pub cell_edges: Vec<([f32; 3], [f32; 3], [f32; 4])>,
    pub camera: Camera,
    pub look_profile: PublicationLookProfile,
    pub intrinsic_atom_count: usize,
    pub show_bonds: bool,
    pub show_cell: bool,
    pub glb_admission: PublicationGlbAdmission,
}

/// Core-glTF field geometry derived from an immutable, committed CPU layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortableFieldRepresentation {
    Isosurface,
    Slice,
    Contour,
}

impl PortableFieldRepresentation {
    pub const fn portable_name(self) -> &'static str {
        match self {
            Self::Isosurface => "isosurface",
            Self::Slice => "slice",
            Self::Contour => "contour",
        }
    }
}

#[derive(Clone)]
pub struct PublicationFieldPrimitive {
    pub representation: PortableFieldRepresentation,
    pub layer_id: u64,
    pub source_layer_revision: u64,
    pub scalar_unit: String,
    pub isovalue: Option<f32>,
    pub contour_level: Option<f32>,
    pub slice_plane: Option<field_scene::FieldSlicePlane>,
    pub clip_planes: Vec<field_scene::FieldClipPlane>,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
}

#[derive(Clone)]
pub struct PublicationFieldLayerProvenance {
    pub layer_id: u64,
    pub source_layer_revision: u64,
    pub source_artifact_sha256: String,
    pub normalized_layer_sha256: String,
    pub source_coordinate_unit: String,
    pub coordinate_to_angstrom: f64,
    pub normalization_conversion: String,
    pub scalar_unit: String,
    pub scalar_unit_scale: f64,
    pub clip_planes: Vec<field_scene::FieldClipPlane>,
    pub presentation: field_scene::FieldPresentationSettings,
    pub render_settings: crate::volumetric::FieldRenderSettings,
}

#[derive(Clone)]
pub struct PublicationFieldSceneSnapshot {
    pub structure: PublicationSceneSnapshot,
    pub field_primitives: Vec<PublicationFieldPrimitive>,
    pub field_layers: Vec<PublicationFieldLayerProvenance>,
    pub field_scene_hash: String,
}

impl PublicationFieldSceneSnapshot {
    pub fn has_portable_geometry(&self) -> bool {
        !self.field_primitives.is_empty()
    }
}

impl PublicationSceneSnapshot {
    fn validate(&self) -> IpcResult<()> {
        let _bond_color_mode: PublicationBondColorMode = self.look_profile.bond_color_mode;
        let scene_nodes = self
            .atoms
            .len()
            .checked_add(self.bonds.len())
            .and_then(|count| count.checked_add(self.cell_edges.len()))
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| IpcError::render("publication scene count overflow"))?;
        if self.atoms.len() != self.glb_admission.atom_instances
            || self.bonds.len() != self.glb_admission.bonds
            || scene_nodes != self.glb_admission.nodes
            || scene_nodes > MAX_PUBLICATION_GLB_NODES
            || self.glb_admission.max_glb_bytes > MAX_PUBLICATION_GLB_BYTES
            || self.glb_admission.max_peak_cpu_bytes > MAX_PUBLICATION_GLB_PEAK_CPU_BYTES
        {
            return Err(IpcError::render(
                "publication GLB scene changed after admission",
            ));
        }
        let bond_count = u32::try_from(self.bonds.len())
            .map_err(|_| IpcError::render("publication bond scene changed after admission"))?;
        if usize::try_from(bond_count)
            .map_err(|_| IpcError::render("publication bond scene changed after admission"))?
            != self.bonds.len()
        {
            return Err(IpcError::render(
                "publication bond scene changed after admission",
            ));
        }
        Ok(())
    }
}

pub fn build_publication_scene_snapshot(
    source: &CrystalState,
    settings: &AppSettings,
    renderer: &Renderer,
    look_profile: PublicationLookProfile,
) -> IpcResult<PublicationSceneSnapshot> {
    reject_nonstructural_state(source, renderer)?;
    let atom_count = source.cart_positions.len();
    if source.atomic_numbers.len() != atom_count
        || source.elements.len() != atom_count
        || source.fract_x.len() != atom_count
        || source.fract_y.len() != atom_count
        || source.fract_z.len() != atom_count
        || source.occupancies.len() != atom_count
    {
        return Err(IpcError::render(
            "publication scene has inconsistent atom arrays",
        ));
    }
    if !source
        .cart_positions
        .iter()
        .flatten()
        .all(|value| value.is_finite())
        || !source
            .fract_x
            .iter()
            .chain(&source.fract_y)
            .chain(&source.fract_z)
            .all(|value| value.is_finite())
        || !source.occupancies.iter().all(|value| value.is_finite())
    {
        return Err(IpcError::render(
            "publication scene contains a non-finite coordinate",
        ));
    }
    let expected_atoms = periodic_atom_instance_count(source)?;
    let expected_bonds = if renderer.show_bonds {
        usize::try_from(publication_bond_instance_count(
            source,
            settings,
            look_profile.bond_color_mode,
        )?)
        .map_err(|_| IpcError::render("publication GLB bond count exceeds addressable memory"))?
    } else {
        0
    };
    let expected_cell_edges = usize::from(renderer.show_cell) * 12;
    let expected_nodes = expected_atoms
        .checked_add(expected_bonds)
        .and_then(|count| count.checked_add(expected_cell_edges))
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| IpcError::render("publication GLB node count overflow"))?;
    let estimated_glb_bytes = expected_nodes
        .checked_mul(GLB_JSON_BYTES_PER_NODE)
        .and_then(|bytes| bytes.checked_add(GLB_FIXED_BYTES))
        .ok_or_else(|| IpcError::render("publication GLB byte budget overflow"))?;
    let estimated_json_bytes = expected_nodes
        .checked_mul(GLB_JSON_BYTES_PER_NODE)
        .and_then(|bytes| bytes.checked_add(GLB_FIXED_BYTES))
        .ok_or_else(|| IpcError::render("publication GLB JSON budget overflow"))?;
    let estimated_peak_cpu_bytes = estimated_glb_bytes
        .checked_mul(3)
        .and_then(|bytes| bytes.checked_add(GLB_FIXED_BYTES))
        .ok_or_else(|| IpcError::render("publication GLB peak memory budget overflow"))?;
    if expected_nodes > MAX_PUBLICATION_GLB_NODES
        || estimated_glb_bytes > MAX_PUBLICATION_GLB_BYTES
        || estimated_json_bytes > MAX_PUBLICATION_GLB_JSON_BYTES
        || estimated_peak_cpu_bytes > MAX_PUBLICATION_GLB_PEAK_CPU_BYTES
    {
        return Err(IpcError::render(
            "publication GLB exceeds the admitted resource budget",
        ));
    }
    let glb_admission = PublicationGlbAdmission {
        atom_instances: expected_atoms,
        bonds: expected_bonds,
        nodes: expected_nodes,
        max_glb_bytes: estimated_glb_bytes,
        max_peak_cpu_bytes: estimated_peak_cpu_bytes,
    };
    let intrinsic = build_instance_data(
        &source.cart_positions,
        &source.atomic_numbers,
        &source.elements,
        &source.occupancies,
        settings,
        &[],
    )?;
    let periodic: Vec<RenderAtomInstance> = build_periodic_atom_instances(source, &intrinsic)?;
    let mut atoms = Vec::new();
    atoms
        .try_reserve_exact(periodic.len())
        .map_err(|_| IpcError::render("unable to allocate publication atom scene"))?;
    for instance in periodic {
        if !instance.atom.position.iter().all(|value| value.is_finite())
            || !instance.atom.radius.is_finite()
            || instance.atom.radius <= 0.0
            || !instance
                .atom
                .color
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            return Err(IpcError::render(
                "publication atom scene contains an invalid value",
            ));
        }
        if instance.source_atom_index >= source.elements.len() {
            return Err(IpcError::render("publication atom source index is invalid"));
        }
        atoms.push(PublicationSceneAtom {
            atom: instance.atom,
            source_atom_index: instance.source_atom_index,
            image_shift: instance.image_shift,
        });
    }
    let bonds = if renderer.show_bonds {
        build_publication_bond_instances_with_count(
            source,
            settings,
            look_profile.bond_color_mode,
            u32::try_from(expected_bonds)
                .map_err(|_| IpcError::render("publication GLB bond count exceeds GPU range"))?,
        )?
    } else {
        Vec::new()
    };
    let mut cell_edges = Vec::new();
    if renderer.show_cell {
        let lines = build_cell_lines(source);
        if lines.len() != 24 {
            return Err(IpcError::render("publication cell line count is invalid"));
        }
        cell_edges
            .try_reserve_exact(lines.len() / 2)
            .map_err(|_| IpcError::render("unable to allocate publication cell scene"))?;
        for pair in lines.chunks_exact(2) {
            if !pair
                .iter()
                .flat_map(|line| line.position.iter().chain(line.color.iter()))
                .all(|value| value.is_finite())
            {
                return Err(IpcError::render(
                    "publication cell scene contains an invalid value",
                ));
            }
            cell_edges.push((pair[0].position, pair[1].position, pair[0].color));
        }
    }
    let snapshot = PublicationSceneSnapshot {
        atoms,
        bonds,
        cell_edges,
        camera: renderer.camera,
        look_profile,
        intrinsic_atom_count: source.intrinsic_sites,
        show_bonds: renderer.show_bonds,
        show_cell: renderer.show_cell,
        glb_admission,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

/// Builds the one-way portable field scene from committed CPU data. A raw scalar grid
/// and a raycast-only volume are never part of this snapshot.
/// A field texture estimate is zero because slices use vertex colors; field vertex,
/// field index, field primitive, field material, and buffer-view estimates are
/// bounded by the GLB admission before serialization.
pub fn build_publication_field_scene(
    source: &CrystalState,
    settings: &AppSettings,
    renderer: &Renderer,
    look_profile: PublicationLookProfile,
) -> IpcResult<PublicationFieldSceneSnapshot> {
    let structure = build_publication_scene_snapshot(source, settings, renderer, look_profile)?;
    build_publication_field_scene_from_snapshot(source, structure)
}

/// Realizes portable field geometry after the structure snapshot has been frozen.
/// This keeps the expensive CPU work independent of renderer and application locks.
pub fn build_publication_field_scene_from_snapshot(
    source: &CrystalState,
    structure: PublicationSceneSnapshot,
) -> IpcResult<PublicationFieldSceneSnapshot> {
    let mut field_primitives = Vec::new();
    let mut field_layers = Vec::new();
    field_primitives
        .try_reserve_exact(source.field_scene.layers.len().saturating_mul(4))
        .map_err(|_| IpcError::render("unable to allocate publication field primitives"))?;
    field_layers
        .try_reserve_exact(source.field_scene.layers.len())
        .map_err(|_| IpcError::render("unable to allocate publication field provenance"))?;

    for layer in &source.field_scene.layers {
        if !layer.render_settings.visible {
            continue;
        }
        let presentation = &layer.presentation_settings;
        presentation.validate().map_err(IpcError::render)?;
        let scalar_unit = format!("{:?}", layer.scalar_unit).to_ascii_lowercase();
        let mut clips = presentation
            .normalized_clip_planes()
            .map_err(IpcError::render)?;
        field_layers.push(PublicationFieldLayerProvenance {
            layer_id: layer.id,
            source_layer_revision: layer.revision,
            source_artifact_sha256: layer.source_sha256.clone(),
            normalized_layer_sha256: layer.normalized_sha256.clone(),
            source_coordinate_unit: match layer.source_coordinate_unit {
                crate::volumetric::FieldCoordinateUnit::Angstrom => "angstrom",
                crate::volumetric::FieldCoordinateUnit::Bohr => "bohr",
            }
            .to_owned(),
            coordinate_to_angstrom: layer.coordinate_to_angstrom,
            normalization_conversion: format!("{:?}", layer.normalization).to_ascii_lowercase(),
            scalar_unit: scalar_unit.clone(),
            scalar_unit_scale: layer.scalar_unit_scale,
            clip_planes: clips.clone(),
            presentation: presentation.clone(),
            render_settings: layer.render_settings,
        });

        let is_volume_only = matches!(
            layer.render_settings.render_mode,
            crate::volumetric::FieldRenderMode::Volume
        ) && presentation.slices.is_empty();
        if is_volume_only {
            return Err(IpcError::invalid_argument(
                "raycast-only field layer has no portable isosurface, slice, or contour representation",
            ));
        }
        let primitive_start = field_primitives.len();
        if !matches!(
            layer.render_settings.render_mode,
            crate::volumetric::FieldRenderMode::Volume
        ) {
            let positive = matches!(
                layer.render_settings.sign_mode,
                crate::volumetric::FieldSignMode::Positive | crate::volumetric::FieldSignMode::Both
            )
            .then_some(layer.render_settings.positive_isovalue)
            .filter(|value| value.is_finite() && *value > 0.0);
            let negative = matches!(
                layer.render_settings.sign_mode,
                crate::volumetric::FieldSignMode::Negative | crate::volumetric::FieldSignMode::Both
            )
            .then_some(layer.render_settings.negative_isovalue)
            .filter(|value| value.is_finite() && *value > 0.0);
            let (positive_count, negative_count) =
                isosurface::marching_cubes_signed_vertex_counts(layer, positive, negative)
                    .map_err(|_| IpcError::render("unable to admit field isosurface geometry"))?;
            for (threshold, color, signed_isovalue, expected_vertices) in [
                positive.map(|value| (value, layer.render_settings.color, value, positive_count)),
                negative.map(|value| {
                    (
                        -value,
                        layer.render_settings.color_negative,
                        -value,
                        negative_count,
                    )
                }),
            ]
            .into_iter()
            .flatten()
            {
                let remaining_vertices = remaining_field_vertex_budget(&field_primitives)?;
                if usize::try_from(expected_vertices)
                    .map_err(|_| IpcError::render("field isosurface count exceeds address space"))?
                    > remaining_vertices
                {
                    return Err(IpcError::render(
                        "publication field geometry exceeds the admitted CPU budget",
                    ));
                }
                let vertices = isosurface::marching_cubes_cpu(layer, threshold)
                    .map_err(|_| IpcError::render("unable to realize field isosurface"))?;
                let (positions, normals) =
                    clip_isosurface_vertices(&vertices, &clips, remaining_vertices)?;
                if !positions.is_empty() {
                    push_field_primitive(
                        &mut field_primitives,
                        PublicationFieldPrimitive {
                            representation: PortableFieldRepresentation::Isosurface,
                            layer_id: layer.id,
                            source_layer_revision: layer.revision,
                            scalar_unit: scalar_unit.clone(),
                            isovalue: Some(signed_isovalue),
                            contour_level: None,
                            slice_plane: None,
                            clip_planes: clips.clone(),
                            colors: vec![color; positions.len()],
                            positions,
                            normals,
                        },
                    )?;
                }
            }
        }

        for request in &presentation.slices {
            let slice_vertices = request.dimensions[0]
                .checked_sub(1)
                .and_then(|width| {
                    request.dimensions[1]
                        .checked_sub(1)
                        .and_then(|height| width.checked_mul(height))
                })
                .and_then(|cells| cells.checked_mul(6))
                .ok_or_else(|| IpcError::render("field slice vertex count overflow"))?;
            let remaining_vertices = remaining_field_vertex_budget(&field_primitives)?;
            if slice_vertices > remaining_vertices {
                return Err(IpcError::render(
                    "publication field geometry exceeds the admitted CPU budget",
                ));
            }
            let slice = field_scene::sample_field_slice(
                layer,
                layer.revision,
                request.plane,
                request.dimensions,
            )
            .map_err(IpcError::render)?;
            let scalar_range = presentation
                .display_range
                .unwrap_or([layer.data_min, layer.data_max]);
            let slice_vertices = crate::renderer::field_slice::realize_portable_slice_triangles(
                &slice,
                &clips,
                &presentation.transfer_function,
                scalar_range,
                presentation.opacity_scale,
                remaining_vertices,
            )
            .map_err(IpcError::render)?;
            let normal = slice.plane.normal.map(|value| value as f32);
            let mut positions = Vec::new();
            let mut colors = Vec::new();
            positions
                .try_reserve_exact(slice_vertices.len())
                .map_err(|_| IpcError::render("unable to allocate portable slice positions"))?;
            colors
                .try_reserve_exact(slice_vertices.len())
                .map_err(|_| IpcError::render("unable to allocate portable slice colors"))?;
            for vertex in slice_vertices {
                positions.push(vertex.position);
                colors.push(vertex.color);
            }
            let normals = vec![normal; positions.len()];
            if !positions.is_empty() {
                push_field_primitive(
                    &mut field_primitives,
                    PublicationFieldPrimitive {
                        representation: PortableFieldRepresentation::Slice,
                        layer_id: layer.id,
                        source_layer_revision: layer.revision,
                        scalar_unit: scalar_unit.clone(),
                        isovalue: None,
                        contour_level: None,
                        slice_plane: Some(request.plane),
                        clip_planes: clips.clone(),
                        positions,
                        normals,
                        colors,
                    },
                )?;
            }
            if !request.contour_levels.is_empty() {
                let contours = field_scene::extract_contours_marching_squares(
                    &slice,
                    &request.contour_levels,
                    &clips,
                )
                .map_err(IpcError::render)?;
                for &level in &request.contour_levels {
                    let contour_count = contours
                        .iter()
                        .filter(|contour| contour.level == level)
                        .count();
                    let contour_vertices = contour_count
                        .checked_mul(36)
                        .ok_or_else(|| IpcError::render("field contour vertex count overflow"))?;
                    ensure_field_vertex_budget(&field_primitives, contour_vertices)?;
                    let mut positions = Vec::new();
                    let mut normals = Vec::new();
                    positions
                        .try_reserve_exact(contour_vertices)
                        .map_err(|_| IpcError::render("unable to allocate contour positions"))?;
                    normals
                        .try_reserve_exact(contour_vertices)
                        .map_err(|_| IpcError::render("unable to allocate contour normals"))?;
                    for contour in contours.iter().filter(|contour| contour.level == level) {
                        append_contour_tube(
                            &mut positions,
                            &mut normals,
                            &slice,
                            contour.start,
                            contour.end,
                            request.plane.normal,
                        )?;
                    }
                    if positions.is_empty() {
                        continue;
                    }
                    let contour_color = crate::renderer::field_slice::transfer_color(
                        level,
                        scalar_range,
                        &presentation.transfer_function,
                        1.0,
                    )
                    .map_err(IpcError::render)?;
                    push_field_primitive(
                        &mut field_primitives,
                        PublicationFieldPrimitive {
                            representation: PortableFieldRepresentation::Contour,
                            layer_id: layer.id,
                            source_layer_revision: layer.revision,
                            scalar_unit: scalar_unit.clone(),
                            isovalue: None,
                            contour_level: Some(level),
                            slice_plane: Some(request.plane),
                            clip_planes: clips.clone(),
                            colors: vec![contour_color; positions.len()],
                            positions,
                            normals,
                        },
                    )?;
                }
            }
        }
        if field_primitives.len() == primitive_start {
            return Err(IpcError::invalid_argument(
                "selected field layer did not realize portable geometry",
            ));
        }
        clips.clear();
    }
    let mut hasher = Sha256::new();
    for layer in &field_layers {
        hasher.update(layer.normalized_layer_sha256.as_bytes());
        hasher.update(layer.layer_id.to_le_bytes());
        hasher.update(layer.source_layer_revision.to_le_bytes());
        let presentation = serde_json::to_vec(&layer.presentation)
            .map_err(|_| IpcError::render("unable to serialize field presentation identity"))?;
        let render = serde_json::to_vec(&layer.render_settings)
            .map_err(|_| IpcError::render("unable to serialize field render identity"))?;
        hasher.update(presentation);
        hasher.update(render);
    }
    Ok(PublicationFieldSceneSnapshot {
        structure,
        field_primitives,
        field_layers,
        field_scene_hash: format!("{:x}", hasher.finalize()),
    })
}

fn push_field_primitive(
    primitives: &mut Vec<PublicationFieldPrimitive>,
    primitive: PublicationFieldPrimitive,
) -> IpcResult<()> {
    if primitive.positions.len() != primitive.normals.len()
        || primitive.positions.len() != primitive.colors.len()
        || primitive.positions.len() > MAX_PUBLICATION_FIELD_VERTICES
        || primitive
            .positions
            .iter()
            .chain(&primitive.normals)
            .all(|value| value.iter().all(|component| component.is_finite()))
            == false
        || !primitive
            .colors
            .iter()
            .flatten()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
    {
        return Err(IpcError::render(
            "publication field primitive contains invalid geometry",
        ));
    }
    if primitives.len() >= MAX_PUBLICATION_FIELD_PRIMITIVES {
        return Err(IpcError::render(
            "publication field primitive count exceeds admission",
        ));
    }
    let total_vertices = primitives
        .iter()
        .try_fold(0usize, |total, item| {
            total.checked_add(item.positions.len())
        })
        .and_then(|total| total.checked_add(primitive.positions.len()))
        .ok_or_else(|| IpcError::render("publication field vertex count overflow"))?;
    if total_vertices > MAX_PUBLICATION_FIELD_TOTAL_VERTICES {
        return Err(IpcError::render(
            "publication field geometry exceeds the admitted CPU budget",
        ));
    }
    primitives.push(primitive);
    Ok(())
}

fn ensure_field_vertex_budget(
    primitives: &[PublicationFieldPrimitive],
    additional_vertices: usize,
) -> IpcResult<()> {
    let total = primitives
        .iter()
        .try_fold(0usize, |total, primitive| {
            total.checked_add(primitive.positions.len())
        })
        .and_then(|total| total.checked_add(additional_vertices))
        .ok_or_else(|| IpcError::render("publication field vertex count overflow"))?;
    if total > MAX_PUBLICATION_FIELD_TOTAL_VERTICES {
        return Err(IpcError::render(
            "publication field geometry exceeds the admitted CPU budget",
        ));
    }
    Ok(())
}

fn remaining_field_vertex_budget(primitives: &[PublicationFieldPrimitive]) -> IpcResult<usize> {
    let used = primitives
        .iter()
        .try_fold(0usize, |total, primitive| {
            total.checked_add(primitive.positions.len())
        })
        .ok_or_else(|| IpcError::render("publication field vertex count overflow"))?;
    MAX_PUBLICATION_FIELD_TOTAL_VERTICES
        .checked_sub(used)
        .ok_or_else(|| {
            IpcError::render("publication field geometry exceeds the admitted CPU budget")
        })
}

fn clip_isosurface_vertices(
    vertices: &[isosurface::IsoVertex],
    clips: &[field_scene::FieldClipPlane],
    max_vertices: usize,
) -> IpcResult<(Vec<[f32; 3]>, Vec<[f32; 3]>)> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    positions
        .try_reserve_exact(vertices.len().min(max_vertices))
        .map_err(|_| IpcError::render("unable to allocate clipped field isosurface"))?;
    normals
        .try_reserve_exact(vertices.len().min(max_vertices))
        .map_err(|_| IpcError::render("unable to allocate clipped field normals"))?;
    let scratch_capacity = 3usize
        .checked_add(clips.len())
        .ok_or_else(|| IpcError::render("clipped field polygon capacity overflow"))?;
    let mut clip_input = Vec::new();
    let mut clip_output = Vec::new();
    clip_input
        .try_reserve_exact(scratch_capacity)
        .map_err(|_| IpcError::render("unable to allocate clipped field polygon input"))?;
    clip_output
        .try_reserve_exact(scratch_capacity)
        .map_err(|_| IpcError::render("unable to allocate clipped field polygon output"))?;
    for triangle in vertices.chunks_exact(3) {
        clip_input.clear();
        clip_input.extend_from_slice(triangle);
        for plane in clips {
            if clip_input.is_empty() {
                break;
            }
            clip_output.clear();
            let mut previous = *clip_input.last().expect("non-empty isosurface polygon");
            let mut previous_inside = plane.keeps(previous.position.map(f64::from));
            for &current in &clip_input {
                let current_inside = plane.keeps(current.position.map(f64::from));
                if current_inside != previous_inside {
                    clip_output.push(interpolate_iso_vertex(previous, current, plane));
                }
                if current_inside {
                    clip_output.push(current);
                }
                previous = current;
                previous_inside = current_inside;
            }
            std::mem::swap(&mut clip_input, &mut clip_output);
        }
        if clip_input.len() >= 3 {
            let additional = (clip_input.len() - 2)
                .checked_mul(3)
                .ok_or_else(|| IpcError::render("clipped field vertex count overflow"))?;
            if positions
                .len()
                .checked_add(additional)
                .filter(|count| *count <= max_vertices)
                .is_none()
            {
                return Err(IpcError::render(
                    "publication field geometry exceeds the admitted CPU budget",
                ));
            }
            positions
                .try_reserve_exact(additional)
                .map_err(|_| IpcError::render("unable to extend clipped field isosurface"))?;
            normals
                .try_reserve_exact(additional)
                .map_err(|_| IpcError::render("unable to extend clipped field normals"))?;
            for index in 1..clip_input.len() - 1 {
                for vertex in [clip_input[0], clip_input[index], clip_input[index + 1]] {
                    positions.push(vertex.position);
                    normals.push(vertex.normal);
                }
            }
        }
    }
    Ok((positions, normals))
}

fn interpolate_iso_vertex(
    first: isosurface::IsoVertex,
    second: isosurface::IsoVertex,
    plane: &field_scene::FieldClipPlane,
) -> isosurface::IsoVertex {
    let signed_distance = |position: [f32; 3]| {
        let value = plane.normal[0] * f64::from(position[0])
            + plane.normal[1] * f64::from(position[1])
            + plane.normal[2] * f64::from(position[2])
            - plane.signed_offset_angstrom;
        if plane.keep_positive { value } else { -value }
    };
    let first_distance = signed_distance(first.position);
    let second_distance = signed_distance(second.position);
    let t = (first_distance / (first_distance - second_distance)).clamp(0.0, 1.0) as f32;
    let mix = |a: f32, b: f32| a.mul_add(1.0 - t, b * t);
    let normal = [
        mix(first.normal[0], second.normal[0]),
        mix(first.normal[1], second.normal[1]),
        mix(first.normal[2], second.normal[2]),
    ];
    let length = normal.iter().map(|value| value * value).sum::<f32>().sqrt();
    isosurface::IsoVertex {
        position: [
            mix(first.position[0], second.position[0]),
            mix(first.position[1], second.position[1]),
            mix(first.position[2], second.position[2]),
        ],
        normal: if length > f32::EPSILON {
            normal.map(|value| value / length)
        } else {
            first.normal
        },
        sign_flag: first.sign_flag,
        _pad: 0.0,
    }
}

fn slice_world(slice: &field_scene::FieldSlice, point: [f64; 2]) -> [f64; 3] {
    let center_x = (slice.dimensions[0].saturating_sub(1) as f64) * 0.5;
    let center_y = (slice.dimensions[1].saturating_sub(1) as f64) * 0.5;
    let u = (point[0] - center_x) * slice.grid_point_spacing_angstrom;
    let v = (point[1] - center_y) * slice.grid_point_spacing_angstrom;
    [
        slice.world_origin_angstrom[0] + slice.first_axis[0] * u + slice.second_axis[0] * v,
        slice.world_origin_angstrom[1] + slice.first_axis[1] * u + slice.second_axis[1] * v,
        slice.world_origin_angstrom[2] + slice.first_axis[2] * u + slice.second_axis[2] * v,
    ]
}

fn append_contour_tube(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    slice: &field_scene::FieldSlice,
    start: [f64; 2],
    end: [f64; 2],
    plane_normal: [f64; 3],
) -> IpcResult<()> {
    let start = slice_world(slice, start);
    let end = slice_world(slice, end);
    let axis = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
    let length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if !length.is_finite() || length <= f64::EPSILON {
        return Err(IpcError::render("field contour segment is degenerate"));
    }
    let tangent = axis.map(|value| value / length);
    let side = [
        plane_normal[1] * tangent[2] - plane_normal[2] * tangent[1],
        plane_normal[2] * tangent[0] - plane_normal[0] * tangent[2],
        plane_normal[0] * tangent[1] - plane_normal[1] * tangent[0],
    ];
    let side_length = (side[0] * side[0] + side[1] * side[1] + side[2] * side[2]).sqrt();
    if !side_length.is_finite() || side_length <= f64::EPSILON {
        return Err(IpcError::render("field contour plane is degenerate"));
    }
    let side = side.map(|value| value / side_length);
    let up = [
        tangent[1] * side[2] - tangent[2] * side[1],
        tangent[2] * side[0] - tangent[0] * side[2],
        tangent[0] * side[1] - tangent[1] * side[0],
    ];
    const CONTOUR_TUBE_SIDES: usize = 6;
    for side_index in 0..CONTOUR_TUBE_SIDES {
        let angle0 = std::f64::consts::TAU * side_index as f64 / CONTOUR_TUBE_SIDES as f64;
        let angle1 = std::f64::consts::TAU * (side_index + 1) as f64 / CONTOUR_TUBE_SIDES as f64;
        let radial = |angle: f64| {
            [
                side[0] * angle.cos() + up[0] * angle.sin(),
                side[1] * angle.cos() + up[1] * angle.sin(),
                side[2] * angle.cos() + up[2] * angle.sin(),
            ]
        };
        let r0 = radial(angle0);
        let r1 = radial(angle1);
        let a = [
            start[0] + r0[0] * f64::from(CONTOUR_RADIUS_ANGSTROM),
            start[1] + r0[1] * f64::from(CONTOUR_RADIUS_ANGSTROM),
            start[2] + r0[2] * f64::from(CONTOUR_RADIUS_ANGSTROM),
        ];
        let b = [
            start[0] + r1[0] * f64::from(CONTOUR_RADIUS_ANGSTROM),
            start[1] + r1[1] * f64::from(CONTOUR_RADIUS_ANGSTROM),
            start[2] + r1[2] * f64::from(CONTOUR_RADIUS_ANGSTROM),
        ];
        let c = [
            end[0] + r0[0] * f64::from(CONTOUR_RADIUS_ANGSTROM),
            end[1] + r0[1] * f64::from(CONTOUR_RADIUS_ANGSTROM),
            end[2] + r0[2] * f64::from(CONTOUR_RADIUS_ANGSTROM),
        ];
        let d = [
            end[0] + r1[0] * f64::from(CONTOUR_RADIUS_ANGSTROM),
            end[1] + r1[1] * f64::from(CONTOUR_RADIUS_ANGSTROM),
            end[2] + r1[2] * f64::from(CONTOUR_RADIUS_ANGSTROM),
        ];
        for (point, normal) in [(a, r0), (c, r0), (b, r1), (b, r1), (c, r0), (d, r1)] {
            let point = point.map(|value| value as f32);
            let normal = normal.map(|value| value as f32);
            if !point
                .iter()
                .chain(normal.iter())
                .all(|value| value.is_finite())
            {
                return Err(IpcError::render("field contour geometry is non-finite"));
            }
            positions.push(point);
            normals.push(normal);
        }
    }
    Ok(())
}

fn periodic_atom_instance_count(source: &CrystalState) -> IpcResult<usize> {
    source
        .fract_x
        .iter()
        .zip(&source.fract_y)
        .zip(&source.fract_z)
        .try_fold(0usize, |count, ((x, y), z)| {
            let shift_count = usize::from(x.abs() < 1.0e-4 || (*x - 1.0).abs() < 1.0e-4)
                .checked_add(1)
                .and_then(|x_count| {
                    x_count.checked_mul(
                        usize::from(y.abs() < 1.0e-4 || (*y - 1.0).abs() < 1.0e-4)
                            .checked_add(1)?,
                    )
                })
                .and_then(|xy_count| {
                    xy_count.checked_mul(
                        usize::from(z.abs() < 1.0e-4 || (*z - 1.0).abs() < 1.0e-4)
                            .checked_add(1)?,
                    )
                })
                .ok_or_else(|| IpcError::render("publication periodic image count overflow"))?;
            count
                .checked_add(shift_count)
                .ok_or_else(|| IpcError::render("publication periodic image count overflow"))
        })
}

fn reject_nonstructural_state(source: &CrystalState, renderer: &Renderer) -> IpcResult<()> {
    let request = renderer.publication_export_request(
        1,
        1,
        PublicationExportSourceState {
            has_measurement_state: !source.measurements.is_empty(),
            has_selection_highlights: !source.selected_atoms.is_empty(),
            has_wannier_overlay: source.wannier_overlay.is_some(),
            has_active_phonon_state: source.active_phonon_mode.is_some(),
        },
        0,
    );
    let rejected = [
        (
            "measurements",
            request.has_measurement_state || request.has_measurement_overlays,
        ),
        ("selected_atoms", request.has_selection_highlights),
        (
            "active_phonon_mode",
            request.has_active_phonon_state
                || request.has_phonon_presentation
                || request.has_atom_drag,
        ),
        ("wannier_overlay", request.has_wannier_overlay),
        ("brillouin", request.show_bz),
    ];
    if let Some((name, _)) = rejected.into_iter().find(|(_, active)| *active) {
        return Err(IpcError::render(format!(
            "publication Blender export rejects {name}"
        )));
    }
    if request.has_hopping_overlays {
        return Err(IpcError::render(
            "publication Blender export rejects hoppings",
        ));
    }
    Ok(())
}
