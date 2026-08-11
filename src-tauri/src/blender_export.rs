//! Narrow glTF 2.0 binary writer for one-way CrystalCanvas structure scenes.

use crate::ipc::{IpcError, IpcResult};
use crate::scene_export::{
    FIELD_CONTOUR, FIELD_ISOSURFACE, FIELD_SLICE, PortableFieldRepresentation,
    PublicationFieldPrimitive, PublicationFieldSceneSnapshot, PublicationSceneSnapshot,
};
use glam::{Mat4, Quat, Vec3};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const GLB_MAGIC: &[u8; 4] = b"glTF";
const GLB_VERSION: u32 = 2u32;
const JSON_CHUNK: u32 = 0x4e4f_534a;
const BIN_CHUNK: u32 = 0x004e_4942;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;
const MAX_PUBLICATION_MATERIALS: usize = 256;
const MAX_PUBLICATION_MESHES: usize = 512;
const MAX_PUBLICATION_GLB_NODES: usize = 75_000;
const MAX_PUBLICATION_GLB_BYTES: usize = 96 * 1024 * 1024;
const MAX_PUBLICATION_GLB_PEAK_CPU_BYTES: usize = 320 * 1024 * 1024;
const MAX_PUBLICATION_GLB_JSON_BYTES: usize = 64 * 1024 * 1024;
const GLB_JSON_BYTES_PER_NODE: usize = 1024;
const GLB_FIXED_BYTES: usize = 256 * 1024;
const PUBLICATION_GLB_BUFFER_VIEW_COUNT: usize = 6;
const PUBLICATION_GLB_ACCESSOR_COUNT: usize = 6;
const CELL_EDGE_RADIUS_ANGSTROM: f32 = 0.02;
const SPHERE_LATITUDES: usize = 12;
const SPHERE_LONGITUDES: usize = 16;
const CYLINDER_SEGMENTS: usize = 16;
const FIELD_VERTEX_GLB_BYTES: usize = 44;
const FIELD_JSON_BYTES_PER_PRIMITIVE: usize = 2 * 1024;
pub(crate) const FIELD_MATERIAL_MAPPING_LIT: &str = "core_gltf_pbr_lit";
pub(crate) const FIELD_MATERIAL_MAPPING_UNLIT_FALLBACK: &str =
    "core_gltf_pbr_lit_fallback_from_unlit";

pub(crate) const fn portable_field_material_mapping(
    material_mode: crate::renderer::field_scene::FieldMaterialMode,
) -> &'static str {
    match material_mode {
        crate::renderer::field_scene::FieldMaterialMode::Lit => FIELD_MATERIAL_MAPPING_LIT,
        crate::renderer::field_scene::FieldMaterialMode::Unlit => {
            FIELD_MATERIAL_MAPPING_UNLIT_FALLBACK
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GlbSemanticInventory {
    pub intrinsic_atoms: usize,
    pub atom_instances: usize,
    pub bonds: usize,
    pub cell_edges: usize,
    pub materials: usize,
    pub meshes: usize,
    pub field_primitives: usize,
    pub field_vertices: usize,
    pub geometry_bounds: Option<GlbGeometryBounds>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GlbGeometryBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

pub struct BlenderGlbArtifact {
    pub bytes: Vec<u8>,
    pub semantic_inventory: GlbSemanticInventory,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MaterialKey {
    rgba: [u32; 4],
    roughness: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum GeometryKind {
    Sphere,
    Cylinder,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MeshKey {
    geometry: GeometryKind,
    material: usize,
}

pub fn build_blender_glb(
    snapshot: &PublicationSceneSnapshot,
    export_id: &str,
) -> IpcResult<BlenderGlbArtifact> {
    build_blender_glb_inner(snapshot, None, export_id)
}

fn build_blender_glb_inner(
    snapshot: &PublicationSceneSnapshot,
    field_scene: Option<&PublicationFieldSceneSnapshot>,
    export_id: &str,
) -> IpcResult<BlenderGlbArtifact> {
    if export_id.is_empty() {
        return Err(IpcError::render(
            "publication GLB export identity is missing",
        ));
    }
    validate_snapshot_admission(snapshot)?;
    if let Some(field_scene) = field_scene {
        validate_field_scene_admission(field_scene)?;
    }
    let admission = snapshot.glb_admission;
    let field_count = field_scene.map_or(0, |scene| scene.field_primitives.len());
    let field_binary_bytes = field_scene
        .into_iter()
        .flat_map(|scene| &scene.field_primitives)
        .try_fold(0usize, |total, primitive| {
            primitive
                .positions
                .len()
                .checked_mul(FIELD_VERTEX_GLB_BYTES)
                .and_then(|bytes| total.checked_add(bytes))
        })
        .ok_or_else(|| IpcError::render("publication field binary byte count overflow"))?;
    let binary_capacity = (64 * 1024usize)
        .checked_add(field_binary_bytes)
        .ok_or_else(|| IpcError::render("publication GLB binary capacity overflow"))?;
    let max_glb_bytes = if field_scene.is_some() {
        MAX_PUBLICATION_GLB_BYTES
    } else {
        admission.max_glb_bytes
    };
    let mut writer = GlbWriter::new(max_glb_bytes, binary_capacity)?;
    let (sphere_pos, sphere_norm, sphere_idx) = writer.shared_sphere()?;
    let (cylinder_pos, cylinder_norm, cylinder_idx) = writer.shared_cylinder()?;
    writer.reserve_field_arrays(field_count)?;
    let mut materials = Vec::<Value>::new();
    materials
        .try_reserve_exact(MAX_PUBLICATION_MATERIALS)
        .map_err(|_| IpcError::render("unable to allocate GLB materials"))?;
    let mut material_indices = BTreeMap::<MaterialKey, usize>::new();
    let mut meshes = Vec::<Value>::new();
    meshes
        .try_reserve_exact(MAX_PUBLICATION_MESHES)
        .map_err(|_| IpcError::render("unable to allocate GLB meshes"))?;
    let mut mesh_indices = BTreeMap::<MeshKey, usize>::new();
    let mut nodes = Vec::<Value>::new();
    nodes
        .try_reserve_exact(
            admission
                .nodes
                .checked_add(field_count)
                .ok_or_else(|| IpcError::render("publication GLB node capacity overflow"))?,
        )
        .map_err(|_| IpcError::render("unable to allocate GLB nodes"))?;

    for atom in &snapshot.atoms {
        let material = material_index(
            &mut materials,
            &mut material_indices,
            atom.atom.color,
            snapshot.look_profile.roughness,
        )?;
        let mesh = mesh_index(
            &mut meshes,
            &mut mesh_indices,
            GeometryKind::Sphere,
            sphere_pos,
            sphere_norm,
            sphere_idx,
            material,
        )?;
        nodes.push(json!({
            "mesh": mesh,
            "translation": atom.atom.position,
            "scale": [atom.atom.radius, atom.atom.radius, atom.atom.radius],
            "extras": {"crystalcanvas": {"source_atom_index": atom.source_atom_index, "image_shift": atom.image_shift}}
        }));
    }
    for bond in &snapshot.bonds {
        let material = material_index(
            &mut materials,
            &mut material_indices,
            bond.color,
            snapshot.look_profile.roughness,
        )?;
        let mesh = mesh_index(
            &mut meshes,
            &mut mesh_indices,
            GeometryKind::Cylinder,
            cylinder_pos,
            cylinder_norm,
            cylinder_idx,
            material,
        )?;
        nodes.push(cylinder_node(
            mesh,
            bond.start,
            bond.end,
            bond.radius,
            json!({"kind": "bond"}),
        )?);
    }
    for (start, end, color) in &snapshot.cell_edges {
        let material = material_index(
            &mut materials,
            &mut material_indices,
            *color,
            snapshot.look_profile.roughness,
        )?;
        let mesh = mesh_index(
            &mut meshes,
            &mut mesh_indices,
            GeometryKind::Cylinder,
            cylinder_pos,
            cylinder_norm,
            cylinder_idx,
            material,
        )?;
        nodes.push(cylinder_node(
            mesh,
            *start,
            *end,
            CELL_EDGE_RADIUS_ANGSTROM,
            json!({"kind": "unit_cell_edge", "radius_angstrom": CELL_EDGE_RADIUS_ANGSTROM}),
        )?);
    }
    let (camera, camera_world) = camera_json(&snapshot.camera)?;
    nodes.push(json!({"name": "CrystalCanvas Camera", "camera": 0, "matrix": camera_world}));
    if nodes.len() != admission.nodes {
        return Err(IpcError::render(
            "publication GLB node scene changed after admission",
        ));
    }
    let field_vertices = if let Some(field_scene) = field_scene {
        append_field_primitives(
            field_scene,
            &mut writer,
            &mut materials,
            &mut meshes,
            &mut nodes,
        )?
    } else {
        0
    };
    let node_indices: Vec<Value> = (0..nodes.len())
        .map(|index| {
            u32::try_from(index)
                .map(Value::from)
                .map_err(|_| IpcError::render("publication GLB node count exceeds u32"))
        })
        .collect::<Result<_, _>>()
        .map_err(|_| IpcError::render("publication GLB node count exceeds u32"))?;
    let material_count = materials.len();
    let mesh_count = meshes.len();
    let buffer_views = std::mem::take(&mut writer.buffer_views);
    let accessors = std::mem::take(&mut writer.accessors);
    let bin_length = writer.bin.len();
    let mut root = json!({
        "asset": {"version": "2.0", "generator": "CrystalCanvas", "extras": {"crystalcanvas": {
            "export_id": export_id,
            "coordinate_length_unit": "angstrom",
            "meters_per_exported_unit": 1.0e-10_f64,
            "matrix_layout": "column_major",
            "scale_policy": "scientific_visualization",
            "material_color_space": "linear_srgb",
            "alpha_policy": "blend_when_alpha_less_than_one"
        }}},
        "scene": 0,
    });
    root["scenes"] = Value::Array(vec![Value::Object(serde_json::Map::from_iter([(
        "nodes".to_owned(),
        Value::Array(node_indices),
    )]))]);
    root["nodes"] = Value::Array(nodes);
    root["meshes"] = Value::Array(meshes);
    root["materials"] = Value::Array(materials);
    root["cameras"] = Value::Array(vec![camera]);
    root["buffers"] = Value::Array(vec![json!({"byteLength": bin_length})]);
    root["bufferViews"] = Value::Array(buffer_views);
    root["accessors"] = Value::Array(accessors);
    if let Some(field_scene) = field_scene {
        root["asset"]["extras"]["crystalcanvas"]["field_scene_hash"] =
            json!(field_scene.field_scene_hash);
    }
    let bytes = writer.finish(root)?;
    let validated = parse_validated_glb(&bytes).map_err(IpcError::render)?;
    let geometry_bounds =
        glb_geometry_bounds_from_validated(&validated).map_err(IpcError::render)?;
    Ok(BlenderGlbArtifact {
        semantic_inventory: GlbSemanticInventory {
            intrinsic_atoms: snapshot.intrinsic_atom_count,
            atom_instances: snapshot.atoms.len(),
            bonds: snapshot.bonds.len(),
            cell_edges: snapshot.cell_edges.len(),
            materials: material_count,
            meshes: mesh_count,
            field_primitives: field_count,
            field_vertices,
            geometry_bounds,
        },
        bytes,
    })
}

/// Builds the stable v0.7 GLB with portable, CPU-realized field triangles.
/// The writer receives only portable CPU triangles; GPU-only resources are absent.
pub fn build_blender_glb_field_scene(
    snapshot: &PublicationFieldSceneSnapshot,
    export_id: &str,
) -> IpcResult<BlenderGlbArtifact> {
    build_blender_glb_inner(&snapshot.structure, Some(snapshot), export_id)
}

fn append_field_primitives(
    snapshot: &PublicationFieldSceneSnapshot,
    writer: &mut GlbWriter,
    materials: &mut Vec<Value>,
    meshes: &mut Vec<Value>,
    nodes: &mut Vec<Value>,
) -> IpcResult<usize> {
    let mut field_vertices = 0usize;
    for primitive in &snapshot.field_primitives {
        let field_layer = snapshot
            .field_layers
            .iter()
            .find(|layer| {
                layer.layer_id == primitive.layer_id
                    && layer.source_layer_revision == primitive.source_layer_revision
            })
            .ok_or_else(|| {
                IpcError::render("publication field primitive has no provenance layer")
            })?;
        let material_mapping =
            portable_field_material_mapping(field_layer.presentation.field_material_mode);
        if materials.len() >= MAX_PUBLICATION_MATERIALS
            || meshes.len() >= MAX_PUBLICATION_MESHES
            || nodes.len() >= MAX_PUBLICATION_GLB_NODES
        {
            return Err(IpcError::render(
                "publication field primitive exceeds GLB admission",
            ));
        }
        let position = writer.push_position_f32x3(&primitive.positions)?;
        let normal = writer.push_f32x3(&primitive.normals, ARRAY_BUFFER, None)?;
        let color = writer.push_f32x4(&primitive.colors, ARRAY_BUFFER)?;
        let indices = writer.push_sequential_u32(primitive.positions.len())?;
        let material = materials.len();
        materials.push(json!({
            "name": "CrystalCanvas Field Material",
            "pbrMetallicRoughness": {"baseColorFactor": [1.0, 1.0, 1.0, 1.0], "metallicFactor": 0.0, "roughnessFactor": 0.5},
            "alphaMode": if primitive.colors.iter().any(|color| color[3] < 1.0) { "BLEND" } else { "OPAQUE" },
            "doubleSided": true,
            "extras": {"crystalcanvas": {"material_mapping": material_mapping}}
        }));
        let node_name = match primitive.representation {
            PortableFieldRepresentation::Isosurface => FIELD_ISOSURFACE,
            PortableFieldRepresentation::Slice => FIELD_SLICE,
            PortableFieldRepresentation::Contour => FIELD_CONTOUR,
        };
        let representation = primitive.representation.portable_name();
        let mesh = meshes.len();
        meshes.push(json!({"name": node_name, "primitives": [{
            "attributes": {"POSITION": position, "NORMAL": normal, "COLOR_0": color},
            "indices": indices, "material": material, "mode": 4
        }]}));
        nodes.push(json!({"name": node_name, "mesh": mesh, "extras": {"crystalcanvas": {
            "representation": representation,
            "layer_id": primitive.layer_id,
            "source_layer_revision": primitive.source_layer_revision,
            "isovalue": primitive.isovalue,
            "contour_level": primitive.contour_level,
            "slice_plane": primitive.slice_plane,
            "scalar_unit": primitive.scalar_unit,
            "material_mapping": material_mapping,
            "contour_radius_angstrom": if matches!(primitive.representation, PortableFieldRepresentation::Contour) { 0.02 } else { 0.0 },
            "clipping": primitive.clip_planes
        }}}));
        field_vertices = field_vertices
            .checked_add(primitive.positions.len())
            .ok_or_else(|| IpcError::render("field vertex count overflow"))?;
    }
    Ok(field_vertices)
}

fn validate_field_primitive(primitive: &PublicationFieldPrimitive) -> IpcResult<()> {
    if primitive.positions.is_empty()
        || primitive.positions.len() % 3 != 0
        || primitive.positions.len() != primitive.normals.len()
        || primitive.positions.len() != primitive.colors.len()
        || primitive.scalar_unit.is_empty()
        || !primitive
            .positions
            .iter()
            .chain(&primitive.normals)
            .all(|value| value.iter().all(|component| component.is_finite()))
        || !primitive
            .colors
            .iter()
            .flatten()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        || primitive.isovalue.is_some_and(|value| !value.is_finite())
        || primitive
            .contour_level
            .is_some_and(|value| !value.is_finite())
    {
        return Err(IpcError::render(
            "publication field primitive contains invalid data",
        ));
    }
    Ok(())
}

fn validate_node_transforms(nodes: &[Value]) -> Result<(), String> {
    for node in nodes {
        if node.get("matrix").is_some()
            && (node.get("translation").is_some()
                || node.get("rotation").is_some()
                || node.get("scale").is_some())
        {
            return Err("publication GLB node mixes matrix and TRS transforms".to_owned());
        }
        if let Some(value) = node.get("matrix") {
            let _ = json_finite_array::<16>(value, "node matrix")?;
        }
        if let Some(value) = node.get("translation") {
            let _ = json_finite_array::<3>(value, "node translation")?;
        }
        if let Some(value) = node.get("rotation") {
            let rotation = json_finite_array::<4>(value, "node rotation")?;
            let norm_squared = rotation.iter().map(|value| value * value).sum::<f32>();
            if (norm_squared - 1.0).abs() > 1.0e-4 {
                return Err("publication GLB node rotation is not normalized".to_owned());
            }
        }
        if let Some(value) = node.get("scale") {
            let _ = json_finite_array::<3>(value, "node scale")?;
        }
    }
    Ok(())
}

fn validate_flat_scene_graph(
    root: &Value,
    nodes: &[Value],
    meshes: &[Value],
) -> Result<(), String> {
    if root.get("scene").and_then(Value::as_u64) != Some(0) {
        return Err("publication GLB default scene is invalid".to_owned());
    }
    let scenes = root
        .get("scenes")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB scenes are missing".to_owned())?;
    if scenes.len() != 1 {
        return Err("publication GLB scene inventory is invalid".to_owned());
    }
    let scene_nodes = scenes[0]
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB scene nodes are missing".to_owned())?;
    if scene_nodes.len() != nodes.len() {
        return Err("publication GLB scene does not contain every node".to_owned());
    }
    for (expected, index) in scene_nodes.iter().enumerate() {
        if json_value_usize(index, "scene node")? != expected {
            return Err("publication GLB scene nodes are not canonical".to_owned());
        }
    }
    for node in nodes {
        if node.get("children").is_some() {
            return Err("publication GLB node hierarchy is unsupported".to_owned());
        }
        if let Some(mesh) = node.get("mesh") {
            if json_value_usize(mesh, "node mesh")? >= meshes.len() {
                return Err("publication GLB node mesh is invalid".to_owned());
            }
        }
    }
    Ok(())
}

fn validate_material_values(materials: &[Value]) -> Result<(), String> {
    for material in materials {
        if let Some(values) = material.pointer("/pbrMetallicRoughness/baseColorFactor") {
            let color = json_finite_array::<4>(values, "material baseColorFactor")?;
            if !color.iter().all(|value| (0.0..=1.0).contains(value)) {
                return Err("publication GLB material color is outside [0, 1]".to_owned());
            }
        }
    }
    Ok(())
}

fn validate_f32x3_accessor(
    accessor: &GlbAccessorView,
    bin: &[u8],
    label: &str,
) -> Result<(), String> {
    if accessor.component_type != 5126 || accessor.kind != "VEC3" {
        return Err(format!("publication GLB {label} accessor is invalid"));
    }
    for index in 0..accessor.count {
        for axis in 0..3 {
            let value = read_f32_component(bin, accessor.offset, index, 12, axis, label)?;
            if !value.is_finite() {
                return Err(format!(
                    "publication GLB {label} contains a non-finite value"
                ));
            }
        }
    }
    Ok(())
}

fn validate_f32x4_accessor(
    accessor: &GlbAccessorView,
    bin: &[u8],
    label: &str,
) -> Result<(), String> {
    if accessor.component_type != 5126 || accessor.kind != "VEC4" {
        return Err(format!("publication GLB {label} accessor is invalid"));
    }
    for index in 0..accessor.count {
        for component in 0..4 {
            let value = read_f32_component(bin, accessor.offset, index, 16, component, label)?;
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(format!("publication GLB {label} contains an invalid value"));
            }
        }
    }
    Ok(())
}

fn read_f32_component(
    bin: &[u8],
    base_offset: usize,
    index: usize,
    stride: usize,
    component: usize,
    label: &str,
) -> Result<f32, String> {
    let offset = base_offset
        .checked_add(
            index
                .checked_mul(stride)
                .ok_or_else(|| format!("publication GLB {label} offset overflows"))?,
        )
        .and_then(|offset| offset.checked_add(component.checked_mul(4)?))
        .ok_or_else(|| format!("publication GLB {label} offset overflows"))?;
    let bytes: [u8; 4] = bin
        .get(offset..offset + 4)
        .ok_or_else(|| format!("publication GLB {label} is truncated"))?
        .try_into()
        .map_err(|_| format!("publication GLB {label} is truncated"))?;
    Ok(f32::from_le_bytes(bytes))
}

fn json_finite_array<const N: usize>(value: &Value, field: &str) -> Result<[f32; N], String> {
    let values = value
        .as_array()
        .filter(|values| values.len() == N)
        .ok_or_else(|| format!("publication GLB {field} is invalid"))?;
    let mut result = [0.0; N];
    for (index, value) in values.iter().enumerate() {
        let component = value
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| format!("publication GLB {field} is invalid"))?
            as f32;
        if !component.is_finite() {
            return Err(format!("publication GLB {field} is invalid"));
        }
        result[index] = component;
    }
    Ok(result)
}

fn node_transform(node: &Value) -> Result<Mat4, String> {
    let transform = if let Some(matrix) = node.get("matrix") {
        Mat4::from_cols_array(&json_finite_array::<16>(matrix, "node matrix")?)
    } else {
        let translation = node
            .get("translation")
            .map(|value| json_finite_array::<3>(value, "node translation"))
            .transpose()?
            .unwrap_or([0.0; 3]);
        let rotation = node
            .get("rotation")
            .map(|value| json_finite_array::<4>(value, "node rotation"))
            .transpose()?
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let scale = node
            .get("scale")
            .map(|value| json_finite_array::<3>(value, "node scale"))
            .transpose()?
            .unwrap_or([1.0; 3]);
        Mat4::from_scale_rotation_translation(
            Vec3::from_array(scale),
            Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]),
            Vec3::from_array(translation),
        )
    };
    if !transform.is_finite() {
        return Err("publication GLB node transform is invalid".to_owned());
    }
    Ok(transform)
}

fn glb_geometry_bounds_from_validated(
    validated: &ValidatedGlb<'_>,
) -> Result<Option<GlbGeometryBounds>, String> {
    let views = validated
        .root
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB buffer views are missing".to_owned())?;
    let accessors = validated
        .root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB accessors are missing".to_owned())?;
    let meshes = validated
        .root
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB meshes are missing".to_owned())?;
    let nodes = validated
        .root
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB nodes are missing".to_owned())?;
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    let mut found = false;
    for node in nodes {
        let Some(mesh_index) = node.get("mesh") else {
            continue;
        };
        let mesh_index = json_value_usize(mesh_index, "mesh")?;
        let mesh = meshes
            .get(mesh_index)
            .ok_or_else(|| "publication GLB node mesh is invalid".to_owned())?;
        let transform = node_transform(node)?;
        for primitive in mesh
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or_else(|| "publication GLB mesh primitives are missing".to_owned())?
        {
            let position_index = primitive
                .get("attributes")
                .and_then(|attributes| attributes.get("POSITION"))
                .ok_or_else(|| "publication GLB POSITION is missing".to_owned())
                .and_then(|value| json_value_usize(value, "POSITION"))?;
            let accessor = accessors
                .get(position_index)
                .ok_or_else(|| "publication GLB POSITION accessor is invalid".to_owned())?;
            let view_index = json_usize(accessor, "bufferView")?;
            let view = views
                .get(view_index)
                .ok_or_else(|| "publication GLB POSITION bufferView is invalid".to_owned())?;
            let view_offset = json_optional_usize(view, "byteOffset")?.unwrap_or(0);
            let accessor_offset = json_optional_usize(accessor, "byteOffset")?.unwrap_or(0);
            let offset = view_offset
                .checked_add(accessor_offset)
                .ok_or_else(|| "publication GLB POSITION offset overflows".to_owned())?;
            let count = json_usize(accessor, "count")?;
            if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
                || accessor.get("type").and_then(Value::as_str) != Some("VEC3")
            {
                return Err("publication GLB POSITION accessor is invalid".to_owned());
            }
            for index in 0..count {
                let local = [
                    read_f32_component(validated.bin, offset, index, 12, 0, "POSITION")?,
                    read_f32_component(validated.bin, offset, index, 12, 1, "POSITION")?,
                    read_f32_component(validated.bin, offset, index, 12, 2, "POSITION")?,
                ];
                let world = transform.transform_point3(Vec3::from_array(local));
                if !world.is_finite() {
                    return Err("publication GLB transformed geometry is non-finite".to_owned());
                }
                let world = world.to_array();
                for axis in 0..3 {
                    minimum[axis] = minimum[axis].min(world[axis]);
                    maximum[axis] = maximum[axis].max(world[axis]);
                }
                found = true;
            }
        }
    }
    Ok(found.then_some(GlbGeometryBounds {
        min: minimum,
        max: maximum,
    }))
}

fn validate_field_scene_admission(snapshot: &PublicationFieldSceneSnapshot) -> IpcResult<()> {
    let field_vertices =
        snapshot
            .field_primitives
            .iter()
            .try_fold(0usize, |total, primitive| {
                validate_field_primitive(primitive)?;
                total
                    .checked_add(primitive.positions.len())
                    .ok_or_else(|| IpcError::render("publication field vertex count overflow"))
            })?;
    let field_nodes = snapshot.field_primitives.len();
    let total_nodes = snapshot
        .structure
        .glb_admission
        .nodes
        .checked_add(field_nodes)
        .ok_or_else(|| IpcError::render("publication field node count overflow"))?;
    let field_binary = field_vertices
        .checked_mul(FIELD_VERTEX_GLB_BYTES)
        .ok_or_else(|| IpcError::render("publication field binary byte count overflow"))?;
    let field_json = field_nodes
        .checked_mul(FIELD_JSON_BYTES_PER_PRIMITIVE)
        .ok_or_else(|| IpcError::render("publication field JSON byte count overflow"))?;
    let final_glb = snapshot
        .structure
        .glb_admission
        .max_glb_bytes
        .checked_add(field_binary)
        .and_then(|value| value.checked_add(field_json))
        .ok_or_else(|| IpcError::render("publication field GLB byte budget overflow"))?;
    // The single-pass writer transiently owns its BIN, serialized JSON, final GLB,
    // and the immutable field geometry. This conservative bound is admitted first.
    let peak_cpu = final_glb
        .checked_mul(4)
        .and_then(|value| {
            field_vertices
                .checked_mul(40)
                .and_then(|geometry| value.checked_add(geometry))
        })
        .ok_or_else(|| IpcError::render("publication field CPU budget overflow"))?;
    let total_json = snapshot
        .structure
        .glb_admission
        .max_glb_bytes
        .checked_add(field_json)
        .ok_or_else(|| IpcError::render("publication field JSON budget overflow"))?;
    if total_nodes > MAX_PUBLICATION_GLB_NODES
        || final_glb > MAX_PUBLICATION_GLB_BYTES
        || total_json > MAX_PUBLICATION_GLB_JSON_BYTES
        || peak_cpu > MAX_PUBLICATION_GLB_PEAK_CPU_BYTES
    {
        return Err(IpcError::render(
            "publication field scene exceeds the admitted export budget",
        ));
    }
    Ok(())
}

fn validate_snapshot_admission(snapshot: &PublicationSceneSnapshot) -> IpcResult<()> {
    let nodes = snapshot
        .atoms
        .len()
        .checked_add(snapshot.bonds.len())
        .and_then(|count| count.checked_add(snapshot.cell_edges.len()))
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| IpcError::render("publication GLB node count overflow"))?;
    let admission = snapshot.glb_admission;
    let minimum_glb_bytes = nodes
        .checked_mul(GLB_JSON_BYTES_PER_NODE)
        .and_then(|bytes| bytes.checked_add(GLB_FIXED_BYTES))
        .ok_or_else(|| IpcError::render("publication GLB byte budget overflow"))?;
    let minimum_json_bytes = nodes
        .checked_mul(GLB_JSON_BYTES_PER_NODE)
        .and_then(|bytes| bytes.checked_add(GLB_FIXED_BYTES))
        .ok_or_else(|| IpcError::render("publication GLB JSON budget overflow"))?;
    let minimum_peak_bytes = minimum_glb_bytes
        .checked_mul(3)
        .and_then(|bytes| bytes.checked_add(GLB_FIXED_BYTES))
        .ok_or_else(|| IpcError::render("publication GLB peak memory budget overflow"))?;
    if snapshot.atoms.len() != admission.atom_instances
        || snapshot.bonds.len() != admission.bonds
        || nodes != admission.nodes
        || nodes > MAX_PUBLICATION_GLB_NODES
        || admission.max_glb_bytes < minimum_glb_bytes
        || admission.max_glb_bytes > MAX_PUBLICATION_GLB_BYTES
        || minimum_json_bytes > MAX_PUBLICATION_GLB_JSON_BYTES
        || admission.max_peak_cpu_bytes < minimum_peak_bytes
        || admission.max_peak_cpu_bytes > MAX_PUBLICATION_GLB_PEAK_CPU_BYTES
    {
        return Err(IpcError::render(
            "publication GLB scene changed after admission",
        ));
    }
    snapshot.look_profile.validate().map_err(IpcError::render)?;
    for atom in &snapshot.atoms {
        if !atom.atom.position.iter().all(|value| value.is_finite())
            || !atom.atom.radius.is_finite()
            || atom.atom.radius <= 0.0
            || !atom
                .atom
                .color
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            return Err(IpcError::render(
                "publication atom scene contains an invalid value",
            ));
        }
    }
    Ok(())
}

fn cylinder_node(
    mesh: usize,
    start: [f32; 3],
    end: [f32; 3],
    radius: f32,
    extras: Value,
) -> IpcResult<Value> {
    let start = Vec3::from_array(start);
    let end = Vec3::from_array(end);
    let delta = end - start;
    let length = delta.length();
    if !start.is_finite()
        || !end.is_finite()
        || !radius.is_finite()
        || radius <= 0.0
        || !length.is_finite()
        || length <= 1.0e-6
    {
        return Err(IpcError::render(
            "publication cylinder contains an invalid value",
        ));
    }
    let rotation = Quat::from_rotation_arc(Vec3::Y, delta / length);
    if !rotation.is_finite() {
        return Err(IpcError::render("publication cylinder rotation is invalid"));
    }
    Ok(json!({
        "mesh": mesh,
        "translation": ((start + end) * 0.5).to_array(),
        "rotation": [rotation.x, rotation.y, rotation.z, rotation.w],
        "scale": [radius, length, radius],
        "extras": {"crystalcanvas": extras}
    }))
}

fn camera_json(camera: &crate::renderer::camera::Camera) -> IpcResult<(Value, [f32; 16])> {
    if !camera.eye.is_finite()
        || !camera.target.is_finite()
        || !camera.up.is_finite()
        || !camera.fovy_deg.is_finite()
        || !camera.aspect.is_finite()
        || !camera.znear.is_finite()
        || !camera.zfar.is_finite()
        || camera.aspect <= 0.0
        || camera.znear <= 0.0
        || camera.zfar <= camera.znear
    {
        return Err(IpcError::render(
            "publication camera contains an invalid value",
        ));
    }
    let forward = camera.target - camera.eye;
    let up = camera.up;
    let right = forward.cross(up);
    if forward.length_squared() <= 1.0e-12
        || up.length_squared() <= 1.0e-12
        || right.length_squared() <= 1.0e-12
    {
        return Err(IpcError::render("publication camera basis is degenerate"));
    }
    let forward = forward.normalize();
    let right = right.normalize();
    let corrected_up = right.cross(forward).normalize();
    let world = Mat4::from_cols(
        right.extend(0.0),
        corrected_up.extend(0.0),
        (-forward).extend(0.0),
        camera.eye.extend(1.0),
    );
    let world = world.to_cols_array();
    if !world.iter().all(|value| value.is_finite()) {
        return Err(IpcError::render("publication camera matrix is invalid"));
    }
    let gltf_camera = if camera.is_perspective {
        let yfov = camera.fovy_deg.to_radians();
        if !yfov.is_finite() || yfov <= 0.0 || yfov >= std::f32::consts::PI {
            return Err(IpcError::render(
                "publication perspective camera field of view is invalid",
            ));
        }
        json!({"type": "perspective", "perspective": {"yfov": camera.fovy_deg.to_radians(), "aspectRatio": camera.aspect, "znear": camera.znear, "zfar": camera.zfar}})
    } else {
        let ymag = camera.orthographic_scale * 0.5;
        let xmag = ymag * camera.aspect;
        if !ymag.is_finite() || ymag <= 0.0 || !xmag.is_finite() || xmag <= 0.0 {
            return Err(IpcError::render(
                "publication orthographic camera scale is invalid",
            ));
        }
        json!({"type": "orthographic", "orthographic": {"xmag": xmag, "ymag": ymag, "znear": camera.znear, "zfar": camera.zfar}})
    };
    Ok((gltf_camera, world))
}

fn material_index(
    materials: &mut Vec<Value>,
    indices: &mut BTreeMap<MaterialKey, usize>,
    color: [f32; 4],
    roughness: f32,
) -> IpcResult<usize> {
    if !color
        .iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        || !roughness.is_finite()
        || !(0.04..=1.0).contains(&roughness)
    {
        return Err(IpcError::render(
            "publication material contains an invalid value",
        ));
    }
    let key = MaterialKey {
        rgba: color.map(f32::to_bits),
        roughness: roughness.to_bits(),
    };
    if let Some(index) = indices.get(&key) {
        return Ok(*index);
    }
    if materials.len() == MAX_PUBLICATION_MATERIALS {
        return Err(IpcError::render(
            "publication GLB material count exceeds admission",
        ));
    }
    let index = materials.len();
    let linear_color = [
        srgb_to_linear(color[0]),
        srgb_to_linear(color[1]),
        srgb_to_linear(color[2]),
        color[3],
    ];
    let alpha_mode = if color[3] < 1.0 { "BLEND" } else { "OPAQUE" };
    materials.push(json!({
        "name": "CrystalCanvas Publication Material",
        "pbrMetallicRoughness": {"baseColorFactor": linear_color, "metallicFactor": 0.0, "roughnessFactor": roughness},
        "alphaMode": alpha_mode
    }));
    indices.insert(key, index);
    Ok(index)
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn mesh_index(
    meshes: &mut Vec<Value>,
    indices: &mut BTreeMap<MeshKey, usize>,
    geometry: GeometryKind,
    position: usize,
    normal: usize,
    indices_accessor: usize,
    material: usize,
) -> IpcResult<usize> {
    let key = MeshKey { geometry, material };
    if let Some(index) = indices.get(&key) {
        return Ok(*index);
    }
    if meshes.len() == MAX_PUBLICATION_MESHES {
        return Err(IpcError::render(
            "publication GLB mesh count exceeds admission",
        ));
    }
    let index = meshes.len();
    meshes.push(json!({"primitives": [{"attributes": {"POSITION": position, "NORMAL": normal}, "indices": indices_accessor, "material": material, "mode": 4}]}));
    indices.insert(key, index);
    Ok(index)
}

struct GlbWriter {
    bin: Vec<u8>,
    buffer_views: Vec<Value>,
    accessors: Vec<Value>,
    max_glb_bytes: usize,
}

impl GlbWriter {
    fn new(max_glb_bytes: usize, binary_capacity: usize) -> IpcResult<Self> {
        let mut bin = Vec::new();
        bin.try_reserve_exact(binary_capacity)
            .map_err(|_| IpcError::render("unable to allocate GLB binary buffer"))?;
        let mut buffer_views = Vec::new();
        buffer_views
            .try_reserve_exact(6)
            .map_err(|_| IpcError::render("unable to allocate GLB buffer views"))?;
        let mut accessors = Vec::new();
        accessors
            .try_reserve_exact(6)
            .map_err(|_| IpcError::render("unable to allocate GLB accessors"))?;
        Ok(Self {
            bin,
            buffer_views,
            accessors,
            max_glb_bytes,
        })
    }

    fn reserve_field_arrays(&mut self, field_count: usize) -> IpcResult<()> {
        let additional = field_count
            .checked_mul(4)
            .ok_or_else(|| IpcError::render("field GLB accessor capacity overflow"))?;
        self.buffer_views
            .try_reserve_exact(additional)
            .map_err(|_| IpcError::render("unable to reserve field GLB buffer views"))?;
        self.accessors
            .try_reserve_exact(additional)
            .map_err(|_| IpcError::render("unable to reserve field GLB accessors"))?;
        Ok(())
    }

    fn align(&mut self) {
        while self.bin.len() % 4 != 0 {
            self.bin.push(0);
        }
    }

    fn push_position_f32x3(&mut self, values: &[[f32; 3]]) -> IpcResult<usize> {
        let (min, max) = finite_bounds(values)?;
        self.push_f32x3(values, ARRAY_BUFFER, Some((min, max)))
    }

    fn push_f32x3(
        &mut self,
        values: &[[f32; 3]],
        target: u32,
        bounds: Option<([f32; 3], [f32; 3])>,
    ) -> IpcResult<usize> {
        let offset = self.bin.len();
        let byte_len = values
            .len()
            .checked_mul(12)
            .ok_or_else(|| IpcError::render("GLB geometry byte count overflow"))?;
        self.ensure_bin_capacity(byte_len)?;
        for value in values {
            if !value.iter().all(|component| component.is_finite()) {
                return Err(IpcError::render("GLB geometry contains non-finite data"));
            }
            for component in value {
                self.bin.extend_from_slice(&component.to_le_bytes());
            }
        }
        self.push_view_accessor(offset, values.len(), 5126, "VEC3", target, bounds)
    }

    fn push_u16(&mut self, values: &[u16]) -> IpcResult<usize> {
        let offset = self.bin.len();
        let byte_len = values
            .len()
            .checked_mul(2)
            .ok_or_else(|| IpcError::render("GLB index byte count overflow"))?;
        self.ensure_bin_capacity(byte_len)?;
        for value in values {
            self.bin.extend_from_slice(&value.to_le_bytes());
        }
        self.push_view_accessor(
            offset,
            values.len(),
            5123,
            "SCALAR",
            ELEMENT_ARRAY_BUFFER,
            None,
        )
    }

    fn push_f32x4(&mut self, values: &[[f32; 4]], target: u32) -> IpcResult<usize> {
        let offset = self.bin.len();
        let byte_len = values
            .len()
            .checked_mul(16)
            .ok_or_else(|| IpcError::render("GLB geometry byte count overflow"))?;
        self.ensure_bin_capacity(byte_len)?;
        for value in values {
            for component in value {
                self.bin.extend_from_slice(&component.to_le_bytes());
            }
        }
        self.push_view_accessor(offset, values.len(), 5126, "VEC4", target, None)
    }

    fn push_sequential_u32(&mut self, count: usize) -> IpcResult<usize> {
        let offset = self.bin.len();
        let byte_len = count
            .checked_mul(4)
            .ok_or_else(|| IpcError::render("field index byte count overflow"))?;
        self.ensure_bin_capacity(byte_len)?;
        for index in 0..count {
            self.bin.extend_from_slice(
                &u32::try_from(index)
                    .map_err(|_| IpcError::render("field index exceeds u32"))?
                    .to_le_bytes(),
            );
        }
        self.push_view_accessor(offset, count, 5125, "SCALAR", ELEMENT_ARRAY_BUFFER, None)
    }

    fn ensure_bin_capacity(&self, additional: usize) -> IpcResult<()> {
        let next_len = self
            .bin
            .len()
            .checked_add(additional)
            .ok_or_else(|| IpcError::render("GLB binary byte count overflow"))?;
        if next_len > self.max_glb_bytes {
            return Err(IpcError::render("publication GLB binary exceeds admission"));
        }
        Ok(())
    }

    fn push_view_accessor(
        &mut self,
        offset: usize,
        count: usize,
        component_type: u32,
        kind: &str,
        target: u32,
        bounds: Option<([f32; 3], [f32; 3])>,
    ) -> IpcResult<usize> {
        let byte_length = self
            .bin
            .len()
            .checked_sub(offset)
            .ok_or_else(|| IpcError::render("GLB byteLength underflow"))?;
        let view = self.buffer_views.len();
        self.buffer_views.push(
            json!({"buffer": 0, "byteOffset": offset, "byteLength": byte_length, "target": target}),
        );
        let accessor = self.accessors.len();
        let mut value = json!({"bufferView": view, "componentType": component_type, "count": count, "type": kind});
        if let Some((min, max)) = bounds {
            value["min"] = json!(min);
            value["max"] = json!(max);
        }
        self.accessors.push(value);
        self.align();
        Ok(accessor)
    }

    fn shared_sphere(&mut self) -> IpcResult<(usize, usize, usize)> {
        let vertex_count = (SPHERE_LATITUDES + 1)
            .checked_mul(SPHERE_LONGITUDES + 1)
            .ok_or_else(|| IpcError::render("sphere vertex count overflow"))?;
        let index_count = SPHERE_LATITUDES
            .checked_mul(SPHERE_LONGITUDES)
            .and_then(|count| count.checked_mul(6))
            .ok_or_else(|| IpcError::render("sphere index count overflow"))?;
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        positions
            .try_reserve_exact(vertex_count)
            .map_err(|_| IpcError::render("unable to allocate sphere vertices"))?;
        indices
            .try_reserve_exact(index_count)
            .map_err(|_| IpcError::render("unable to allocate sphere indices"))?;
        for latitude in 0..=SPHERE_LATITUDES {
            let theta = std::f32::consts::PI * latitude as f32 / SPHERE_LATITUDES as f32;
            let sin_theta = theta.sin();
            for longitude in 0..=SPHERE_LONGITUDES {
                let phi = std::f32::consts::TAU * longitude as f32 / SPHERE_LONGITUDES as f32;
                positions.push([sin_theta * phi.cos(), theta.cos(), sin_theta * phi.sin()]);
            }
        }
        for latitude in 0..SPHERE_LATITUDES {
            for longitude in 0..SPHERE_LONGITUDES {
                let a = latitude * (SPHERE_LONGITUDES + 1) + longitude;
                let b = a + SPHERE_LONGITUDES + 1;
                indices.extend_from_slice(&[
                    u16::try_from(a)
                        .map_err(|_| IpcError::render("sphere vertex index exceeds u16"))?,
                    u16::try_from(b)
                        .map_err(|_| IpcError::render("sphere vertex index exceeds u16"))?,
                    u16::try_from(a + 1)
                        .map_err(|_| IpcError::render("sphere vertex index exceeds u16"))?,
                    u16::try_from(a + 1)
                        .map_err(|_| IpcError::render("sphere vertex index exceeds u16"))?,
                    u16::try_from(b)
                        .map_err(|_| IpcError::render("sphere vertex index exceeds u16"))?,
                    u16::try_from(b + 1)
                        .map_err(|_| IpcError::render("sphere vertex index exceeds u16"))?,
                ]);
            }
        }
        Ok((
            self.push_position_f32x3(&positions)?,
            self.push_f32x3(&positions, ARRAY_BUFFER, None)?,
            self.push_u16(&indices)?,
        ))
    }

    fn shared_cylinder(&mut self) -> IpcResult<(usize, usize, usize)> {
        let vertex_count = CYLINDER_SEGMENTS
            .checked_mul(4)
            .and_then(|count| count.checked_add(2))
            .ok_or_else(|| IpcError::render("cylinder vertex count overflow"))?;
        let index_count = CYLINDER_SEGMENTS
            .checked_mul(12)
            .ok_or_else(|| IpcError::render("cylinder index count overflow"))?;
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut indices = Vec::new();
        positions
            .try_reserve_exact(vertex_count)
            .map_err(|_| IpcError::render("unable to allocate cylinder vertices"))?;
        normals
            .try_reserve_exact(vertex_count)
            .map_err(|_| IpcError::render("unable to allocate cylinder normals"))?;
        indices
            .try_reserve_exact(index_count)
            .map_err(|_| IpcError::render("unable to allocate cylinder indices"))?;
        for segment in 0..CYLINDER_SEGMENTS {
            let angle = std::f32::consts::TAU * segment as f32 / CYLINDER_SEGMENTS as f32;
            let x = angle.cos();
            let z = angle.sin();
            positions.extend_from_slice(&[[x, -0.5, z], [x, 0.5, z], [x, -0.5, z], [x, 0.5, z]]);
            normals.extend_from_slice(&[
                [x, 0.0, z],
                [x, 0.0, z],
                [0.0, -1.0, 0.0],
                [0.0, 1.0, 0.0],
            ]);
        }
        let bottom_center = positions.len();
        positions.extend_from_slice(&[[0.0, -0.5, 0.0], [0.0, 0.5, 0.0]]);
        normals.extend_from_slice(&[[0.0, -1.0, 0.0], [0.0, 1.0, 0.0]]);
        for segment in 0..CYLINDER_SEGMENTS {
            let next = (segment + 1) % CYLINDER_SEGMENTS;
            let bottom_side = segment * 4;
            let top_side = bottom_side + 1;
            let next_bottom_side = next * 4;
            let next_top_side = next_bottom_side + 1;
            let bottom_cap = bottom_side + 2;
            let top_cap = bottom_side + 3;
            let next_bottom_cap = next * 4 + 2;
            let next_top_cap = next * 4 + 3;
            indices.extend_from_slice(&[
                u16::try_from(bottom_side)
                    .map_err(|_| IpcError::render("cylinder vertex index exceeds u16"))?,
                u16::try_from(top_side)
                    .map_err(|_| IpcError::render("cylinder vertex index exceeds u16"))?,
                u16::try_from(next_top_side)
                    .map_err(|_| IpcError::render("cylinder vertex index exceeds u16"))?,
                u16::try_from(bottom_side)
                    .map_err(|_| IpcError::render("cylinder vertex index exceeds u16"))?,
                u16::try_from(next_top_side)
                    .map_err(|_| IpcError::render("cylinder vertex index exceeds u16"))?,
                u16::try_from(next_bottom_side)
                    .map_err(|_| IpcError::render("cylinder vertex index exceeds u16"))?,
                u16::try_from(bottom_center)
                    .map_err(|_| IpcError::render("cylinder vertex index exceeds u16"))?,
                u16::try_from(bottom_cap)
                    .map_err(|_| IpcError::render("cylinder vertex index exceeds u16"))?,
                u16::try_from(next_bottom_cap)
                    .map_err(|_| IpcError::render("cylinder vertex index exceeds u16"))?,
                u16::try_from(bottom_center + 1)
                    .map_err(|_| IpcError::render("cylinder vertex index exceeds u16"))?,
                u16::try_from(next_top_cap)
                    .map_err(|_| IpcError::render("cylinder vertex index exceeds u16"))?,
                u16::try_from(top_cap)
                    .map_err(|_| IpcError::render("cylinder vertex index exceeds u16"))?,
            ]);
        }
        Ok((
            self.push_position_f32x3(&positions)?,
            self.push_f32x3(&normals, ARRAY_BUFFER, None)?,
            self.push_u16(&indices)?,
        ))
    }

    fn finish(mut self, root: Value) -> IpcResult<Vec<u8>> {
        let mut json_padded = serde_json::to_vec(&root)
            .map_err(|error| IpcError::render(format!("unable to serialize GLB JSON: {error}")))?;
        while json_padded.len() % 4 != 0 {
            json_padded.push(b' ');
        }
        let json_length = json_padded.len();
        drop(root);
        self.align();
        let GlbWriter { bin, .. } = self;
        let total = 12usize
            .checked_add(8)
            .and_then(|value| value.checked_add(json_length))
            .and_then(|value| value.checked_add(8))
            .and_then(|value| value.checked_add(bin.len()))
            .ok_or_else(|| IpcError::render("GLB size overflow"))?;
        if total > self.max_glb_bytes {
            return Err(IpcError::render(
                "publication GLB exceeds the admitted byte budget",
            ));
        }
        let total_u32 = u32::try_from(total).map_err(|_| IpcError::render("GLB exceeds 4 GiB"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| IpcError::render("unable to allocate GLB"))?;
        bytes.extend_from_slice(GLB_MAGIC);
        bytes.extend_from_slice(&GLB_VERSION.to_le_bytes());
        bytes.extend_from_slice(&total_u32.to_le_bytes());
        bytes.extend_from_slice(
            &(u32::try_from(json_length).map_err(|_| IpcError::render("GLB JSON too large"))?)
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&JSON_CHUNK.to_le_bytes());
        bytes.extend_from_slice(&json_padded);
        drop(json_padded);
        bytes.extend_from_slice(
            &(u32::try_from(bin.len()).map_err(|_| IpcError::render("GLB binary too large"))?)
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&BIN_CHUNK.to_le_bytes());
        bytes.extend_from_slice(&bin);
        drop(bin);
        Ok(bytes)
    }
}

fn finite_bounds(values: &[[f32; 3]]) -> IpcResult<([f32; 3], [f32; 3])> {
    let first = values
        .first()
        .ok_or_else(|| IpcError::render("GLB POSITION accessor is empty"))?;
    if !first.iter().all(|value| value.is_finite()) {
        return Err(IpcError::render("GLB geometry contains non-finite data"));
    }
    let mut min = *first;
    let mut max = *first;
    for value in &values[1..] {
        for axis in 0..3 {
            if !value[axis].is_finite() {
                return Err(IpcError::render("GLB geometry contains non-finite data"));
            }
            min[axis] = min[axis].min(value[axis]);
            max[axis] = max[axis].max(value[axis]);
        }
    }
    Ok((min, max))
}

#[derive(Clone, Copy)]
struct GlbAccessorView {
    offset: usize,
    count: usize,
    component_type: u32,
    kind: &'static str,
}

struct ValidatedGlb<'a> {
    root: Value,
    bin: &'a [u8],
}

fn parse_validated_glb(glb: &[u8]) -> Result<ValidatedGlb<'_>, String> {
    if glb.len() < 20 || &glb[..4] != GLB_MAGIC {
        return Err("publication GLB bytes are invalid".to_owned());
    }
    if glb.len() > MAX_PUBLICATION_GLB_BYTES {
        return Err("publication GLB exceeds the validation byte budget".to_owned());
    }
    let read_u32 = |offset: usize| -> Result<u32, String> {
        let end = offset
            .checked_add(4)
            .ok_or_else(|| "publication GLB offset overflows".to_owned())?;
        let bytes: [u8; 4] = glb
            .get(offset..end)
            .ok_or_else(|| "publication GLB is truncated".to_owned())?
            .try_into()
            .map_err(|_| "publication GLB is truncated".to_owned())?;
        Ok(u32::from_le_bytes(bytes))
    };
    if read_u32(4)? != GLB_VERSION
        || usize::try_from(read_u32(8)?)
            .map_err(|_| "publication GLB length is invalid".to_owned())?
            != glb.len()
    {
        return Err("publication GLB header is invalid".to_owned());
    }
    let json_length = usize::try_from(read_u32(12)?)
        .map_err(|_| "publication GLB JSON length is invalid".to_owned())?;
    if json_length > MAX_PUBLICATION_GLB_JSON_BYTES
        || json_length % 4 != 0
        || read_u32(16)? != JSON_CHUNK
    {
        return Err("publication GLB JSON chunk is invalid".to_owned());
    }
    let json_end = 20usize
        .checked_add(json_length)
        .ok_or_else(|| "publication GLB JSON length overflows".to_owned())?;
    let bin_header_end = json_end
        .checked_add(8)
        .ok_or_else(|| "publication GLB chunk header overflows".to_owned())?;
    if bin_header_end > glb.len() || read_u32(json_end + 4)? != BIN_CHUNK {
        return Err("publication GLB binary chunk is invalid".to_owned());
    }
    let bin_length = usize::try_from(read_u32(json_end)?)
        .map_err(|_| "publication GLB binary length is invalid".to_owned())?;
    if bin_length > MAX_PUBLICATION_GLB_BYTES
        || bin_length % 4 != 0
        || bin_header_end.checked_add(bin_length) != Some(glb.len())
    {
        return Err("publication GLB chunk lengths are invalid".to_owned());
    }
    let bin = &glb[bin_header_end..];
    let root: Value = serde_json::from_slice(&glb[20..json_end])
        .map_err(|_| "publication GLB JSON is invalid".to_owned())?;
    if root.pointer("/asset/version").and_then(Value::as_str) != Some("2.0") {
        return Err("publication GLB asset version is invalid".to_owned());
    }
    let buffers = root
        .get("buffers")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB buffers are missing".to_owned())?;
    if buffers.len() != 1 || json_usize(&buffers[0], "byteLength")? != bin.len() {
        return Err("publication GLB buffer length is invalid".to_owned());
    }
    let buffer_views = root
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB buffer views are missing".to_owned())?;
    if buffer_views.len() < PUBLICATION_GLB_BUFFER_VIEW_COUNT {
        return Err("publication GLB bufferView schema is invalid".to_owned());
    }
    let mut views = Vec::new();
    views
        .try_reserve_exact(buffer_views.len())
        .map_err(|_| "unable to validate publication GLB buffer views".to_owned())?;
    for view in buffer_views {
        if json_usize(view, "buffer")? != 0 {
            return Err("publication GLB bufferView buffer is invalid".to_owned());
        }
        if view.get("byteStride").is_some() {
            return Err("publication GLB bufferView byteStride is unsupported".to_owned());
        }
        let offset = json_optional_usize(view, "byteOffset")?.unwrap_or(0);
        let length = json_usize(view, "byteLength")?;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| "publication GLB bufferView range overflows".to_owned())?;
        let target = json_usize(view, "target")?;
        if end > bin.len()
            || (target != ARRAY_BUFFER as usize && target != ELEMENT_ARRAY_BUFFER as usize)
        {
            return Err("publication GLB bufferView is invalid".to_owned());
        }
        views.push((offset, length));
    }
    let accessors = root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB accessors are missing".to_owned())?;
    if accessors.len() < PUBLICATION_GLB_ACCESSOR_COUNT {
        return Err("publication GLB accessor schema is invalid".to_owned());
    }
    let mut parsed_accessors = Vec::new();
    parsed_accessors
        .try_reserve_exact(accessors.len())
        .map_err(|_| "unable to validate publication GLB accessors".to_owned())?;
    for accessor in accessors {
        let view_index = json_usize(accessor, "bufferView")?;
        let (view_offset, view_length) = *views
            .get(view_index)
            .ok_or_else(|| "publication GLB accessor bufferView is invalid".to_owned())?;
        let component_type = u32::try_from(json_usize(accessor, "componentType")?)
            .map_err(|_| "publication GLB accessor componentType is invalid".to_owned())?;
        let kind = accessor
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| "publication GLB accessor type is missing".to_owned())?;
        let component_count = match kind {
            "SCALAR" => 1usize,
            "VEC3" => 3usize,
            "VEC4" => 4usize,
            _ => return Err("publication GLB accessor type is unsupported".to_owned()),
        };
        let component_size = match component_type {
            5123 => 2usize,
            5125 | 5126 => 4usize,
            _ => return Err("publication GLB accessor componentType is unsupported".to_owned()),
        };
        let count = json_usize(accessor, "count")?;
        if count == 0 {
            return Err("publication GLB accessor count is invalid".to_owned());
        }
        let byte_offset = json_optional_usize(accessor, "byteOffset")?.unwrap_or(0);
        let byte_length = count
            .checked_mul(component_count)
            .and_then(|value| value.checked_mul(component_size))
            .ok_or_else(|| "publication GLB accessor range overflows".to_owned())?;
        let view_end = byte_offset
            .checked_add(byte_length)
            .ok_or_else(|| "publication GLB accessor range overflows".to_owned())?;
        if view_end > view_length {
            return Err("publication GLB accessor exceeds bufferView".to_owned());
        }
        let offset = view_offset
            .checked_add(byte_offset)
            .ok_or_else(|| "publication GLB accessor offset overflows".to_owned())?;
        parsed_accessors.push(GlbAccessorView {
            offset,
            count,
            component_type,
            kind: match kind {
                "SCALAR" => "SCALAR",
                "VEC3" => "VEC3",
                "VEC4" => "VEC4",
                _ => unreachable!(),
            },
        });
    }
    let materials = root
        .get("materials")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB materials are missing".to_owned())?;
    if materials.len() > MAX_PUBLICATION_MATERIALS {
        return Err("publication GLB material schema is invalid".to_owned());
    }
    let meshes = root
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB meshes are missing".to_owned())?;
    if meshes.len() > MAX_PUBLICATION_MESHES {
        return Err("publication GLB mesh schema is invalid".to_owned());
    }
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB nodes are missing".to_owned())?;
    if nodes.len() > MAX_PUBLICATION_GLB_NODES {
        return Err("publication GLB node schema is invalid".to_owned());
    }
    validate_flat_scene_graph(&root, nodes, meshes)?;
    validate_node_transforms(nodes)?;
    validate_material_values(materials)?;
    for mesh in meshes {
        for primitive in mesh
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or_else(|| "publication GLB primitive is invalid".to_owned())?
        {
            if json_usize(primitive, "mode")? != 4 {
                return Err("publication GLB primitive mode is invalid".to_owned());
            }
            let attributes = primitive
                .get("attributes")
                .ok_or_else(|| "publication GLB primitive attributes are missing".to_owned())?;
            let position = accessor_at(&parsed_accessors, json_usize(attributes, "POSITION")?)?;
            let normal = accessor_at(&parsed_accessors, json_usize(attributes, "NORMAL")?)?;
            let color = attributes
                .get("COLOR_0")
                .map(|_| {
                    json_usize(attributes, "COLOR_0")
                        .and_then(|index| accessor_at(&parsed_accessors, index).map(Some))
                })
                .transpose()?
                .flatten();
            let indices = accessor_at(&parsed_accessors, json_usize(primitive, "indices")?)?;
            if position.component_type != 5126
                || position.kind != "VEC3"
                || normal.component_type != 5126
                || normal.kind != "VEC3"
                || normal.count != position.count
                || (indices.component_type != 5123 && indices.component_type != 5125)
                || indices.kind != "SCALAR"
                || json_usize(primitive, "material")? >= materials.len()
                || color.is_some_and(|color| {
                    color.component_type != 5126
                        || color.kind != "VEC4"
                        || color.count != position.count
                })
            {
                return Err("publication GLB primitive accessor is invalid".to_owned());
            }
            validate_position_bounds(
                accessors,
                json_usize(attributes, "POSITION")?,
                position,
                bin,
            )?;
            validate_f32x3_accessor(normal, bin, "NORMAL")?;
            if let Some(color) = color {
                validate_f32x4_accessor(color, bin, "COLOR_0")?;
            }
            for index in 0..indices.count {
                let offset = indices
                    .offset
                    .checked_add(
                        index
                            .checked_mul(if indices.component_type == 5123 { 2 } else { 4 })
                            .ok_or_else(|| "publication GLB index offset overflows".to_owned())?,
                    )
                    .ok_or_else(|| "publication GLB index offset overflows".to_owned())?;
                let value = if indices.component_type == 5123 {
                    let bytes: [u8; 2] = bin
                        .get(offset..offset + 2)
                        .ok_or_else(|| "publication GLB index is truncated".to_owned())?
                        .try_into()
                        .map_err(|_| "publication GLB index is truncated".to_owned())?;
                    usize::from(u16::from_le_bytes(bytes))
                } else {
                    let bytes: [u8; 4] = bin
                        .get(offset..offset + 4)
                        .ok_or_else(|| "publication GLB index is truncated".to_owned())?
                        .try_into()
                        .map_err(|_| "publication GLB index is truncated".to_owned())?;
                    usize::try_from(u32::from_le_bytes(bytes)).map_err(|_| {
                        "publication GLB index exceeds addressable memory".to_owned()
                    })?
                };
                if value >= position.count {
                    return Err("publication GLB index exceeds POSITION count".to_owned());
                }
            }
        }
    }
    Ok(ValidatedGlb { root, bin })
}

pub fn validate_glb_bytes(glb: &[u8]) -> Result<(), String> {
    parse_validated_glb(glb).map(|_| ())
}

fn json_usize(value: &Value, field: &str) -> Result<usize, String> {
    let value = value
        .get(field)
        .ok_or_else(|| format!("publication GLB {field} is invalid"))?;
    json_value_usize(value, field)
}

fn json_value_usize(value: &Value, field: &str) -> Result<usize, String> {
    usize::try_from(
        value
            .as_u64()
            .ok_or_else(|| format!("publication GLB {field} is invalid"))?,
    )
    .map_err(|_| format!("publication GLB {field} exceeds addressable memory"))
}

fn json_optional_usize(value: &Value, field: &str) -> Result<Option<usize>, String> {
    value
        .get(field)
        .map(|value| {
            usize::try_from(
                value
                    .as_u64()
                    .ok_or_else(|| format!("publication GLB {field} is invalid"))?,
            )
            .map_err(|_| format!("publication GLB {field} exceeds addressable memory"))
        })
        .transpose()
}

fn accessor_at<'a>(
    accessors: &'a [GlbAccessorView],
    index: usize,
) -> Result<&'a GlbAccessorView, String> {
    accessors
        .get(index)
        .ok_or_else(|| "publication GLB accessor index is invalid".to_owned())
}

fn validate_position_bounds(
    accessors: &[Value],
    accessor_index: usize,
    position: &GlbAccessorView,
    bin: &[u8],
) -> Result<(), String> {
    let accessor = accessors
        .get(accessor_index)
        .ok_or_else(|| "publication GLB POSITION accessor is invalid".to_owned())?;
    let min = json_finite_vec3(accessor, "min")?;
    let max = json_finite_vec3(accessor, "max")?;
    if min.iter().zip(max.iter()).any(|(min, max)| min > max) {
        return Err("publication GLB POSITION bounds are inverted".to_owned());
    }
    let mut actual_min = [f32::INFINITY; 3];
    let mut actual_max = [f32::NEG_INFINITY; 3];
    for index in 0..position.count {
        for axis in 0..3 {
            let offset = position
                .offset
                .checked_add(
                    index
                        .checked_mul(12)
                        .ok_or_else(|| "publication GLB POSITION offset overflows".to_owned())?,
                )
                .and_then(|offset| offset.checked_add(axis * 4))
                .ok_or_else(|| "publication GLB POSITION offset overflows".to_owned())?;
            let bytes: [u8; 4] = bin
                .get(offset..offset + 4)
                .ok_or_else(|| "publication GLB POSITION is truncated".to_owned())?
                .try_into()
                .map_err(|_| "publication GLB POSITION is truncated".to_owned())?;
            let value = f32::from_le_bytes(bytes);
            if !value.is_finite() {
                return Err("publication GLB POSITION contains a non-finite value".to_owned());
            }
            actual_min[axis] = actual_min[axis].min(value);
            actual_max[axis] = actual_max[axis].max(value);
        }
    }
    if min
        .iter()
        .copied()
        .zip(actual_min)
        .chain(max.iter().copied().zip(actual_max))
        .any(|(declared, actual)| (declared - actual).abs() > 1.0e-5)
    {
        return Err("publication GLB POSITION bounds do not match binary data".to_owned());
    }
    Ok(())
}

fn json_finite_vec3(value: &Value, field: &str) -> Result<[f32; 3], String> {
    json_finite_array::<3>(
        value
            .get(field)
            .ok_or_else(|| format!("publication GLB POSITION {field} is invalid"))?,
        &format!("POSITION {field}"),
    )
}

fn json_finite_number(value: &Value, field: &str) -> Result<f32, String> {
    let number = value
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("publication GLB {field} is invalid"))? as f32;
    number
        .is_finite()
        .then_some(number)
        .ok_or_else(|| format!("publication GLB {field} is invalid"))
}

fn structure_node_counts(nodes: &[Value]) -> Result<(usize, usize, usize, usize), String> {
    let mut atom_instances = 0usize;
    let mut bonds = 0usize;
    let mut cell_edges = 0usize;
    let mut field_nodes = 0usize;
    for node in nodes {
        if node.get("camera").is_some() {
            continue;
        }
        let extras = node.pointer("/extras/crystalcanvas");
        if extras
            .and_then(|value| value.get("representation"))
            .is_some()
        {
            field_nodes = field_nodes
                .checked_add(1)
                .ok_or_else(|| "publication GLB field node count overflows".to_owned())?;
            continue;
        }
        match extras
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str)
        {
            Some("bond") => {
                bonds = bonds
                    .checked_add(1)
                    .ok_or_else(|| "publication GLB bond count overflows".to_owned())?;
            }
            Some("unit_cell_edge") => {
                cell_edges = cell_edges
                    .checked_add(1)
                    .ok_or_else(|| "publication GLB cell-edge count overflows".to_owned())?;
            }
            Some(other) => {
                return Err(format!(
                    "publication GLB contains an unsupported structure node kind `{other}`"
                ));
            }
            None if extras
                .and_then(|value| value.get("source_atom_index"))
                .is_some() =>
            {
                atom_instances = atom_instances
                    .checked_add(1)
                    .ok_or_else(|| "publication GLB atom count overflows".to_owned())?;
            }
            None => {
                return Err("publication GLB contains an unclassified structure node".to_owned());
            }
        }
    }
    Ok((atom_instances, bonds, cell_edges, field_nodes))
}

fn validate_recipe_camera(
    root: &Value,
    nodes: &[Value],
    camera: &crate::export_recipe::RecipeCamera,
) -> Result<(), String> {
    let cameras = root
        .get("cameras")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB cameras are missing".to_owned())?;
    if cameras.len() != 1 {
        return Err("publication GLB camera inventory is invalid".to_owned());
    }
    let mut camera_node = None;
    for node in nodes {
        if node.get("camera").is_some() {
            if camera_node.replace(node).is_some() {
                return Err("publication GLB camera node inventory is invalid".to_owned());
            }
        }
    }
    let camera_node =
        camera_node.ok_or_else(|| "publication GLB camera node inventory is invalid".to_owned())?;
    if json_usize(camera_node, "camera")? != 0
        || !camera_node
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| name == "CrystalCanvas Camera")
    {
        return Err("publication GLB camera node inventory is invalid".to_owned());
    }
    let expected_camera = crate::renderer::camera::Camera {
        eye: Vec3::from_array(camera.eye),
        target: Vec3::from_array(camera.target),
        up: Vec3::from_array(camera.up),
        fovy_deg: camera.fovy_deg,
        aspect: crate::export_recipe::recipe_glb_camera_aspect(&camera.aspect_policy)?,
        znear: camera.znear,
        zfar: camera.zfar,
        is_perspective: camera.projection == "perspective",
        orthographic_scale: camera.orthographic_scale,
    };
    let (expected_json, expected_world) =
        camera_json(&expected_camera).map_err(|error| error.message)?;
    let actual = &cameras[0];
    if actual.get("type") != expected_json.get("type") {
        return Err("publication GLB camera projection differs from its recipe".to_owned());
    }
    let actual_projection = actual
        .get(camera.projection.as_str())
        .ok_or_else(|| "publication GLB camera projection is missing".to_owned())?;
    let expected_projection = expected_json
        .get(camera.projection.as_str())
        .ok_or_else(|| "publication recipe camera projection is missing".to_owned())?;
    for field in ["znear", "zfar"] {
        let actual_value = json_finite_number(
            actual_projection
                .get(field)
                .ok_or_else(|| format!("publication GLB camera {field} is missing"))?,
            &format!("camera {field}"),
        )?;
        let expected_value = json_finite_number(
            expected_projection
                .get(field)
                .ok_or_else(|| format!("publication recipe camera {field} is missing"))?,
            &format!("recipe camera {field}"),
        )?;
        if (actual_value - expected_value).abs() > 1.0e-5 {
            return Err("publication GLB camera clipping differs from its recipe".to_owned());
        }
    }
    if camera.projection == "perspective" {
        let actual_yfov = json_finite_number(
            actual_projection
                .get("yfov")
                .ok_or_else(|| "publication GLB camera yfov is missing".to_owned())?,
            "camera yfov",
        )?;
        if (actual_yfov - camera.fovy_deg.to_radians()).abs() > 1.0e-5 {
            return Err("publication GLB camera field of view differs from its recipe".to_owned());
        }
        let aspect = json_finite_number(
            actual_projection
                .get("aspectRatio")
                .ok_or_else(|| "publication GLB camera aspect ratio is missing".to_owned())?,
            "camera aspect ratio",
        )?;
        let expected_aspect = json_finite_number(
            expected_projection
                .get("aspectRatio")
                .ok_or_else(|| "publication recipe camera aspect ratio is missing".to_owned())?,
            "recipe camera aspect ratio",
        )?;
        if (aspect - expected_aspect).abs() > 1.0e-5 {
            return Err("publication GLB camera aspect ratio is invalid".to_owned());
        }
    } else {
        let actual_ymag = json_finite_number(
            actual_projection
                .get("ymag")
                .ok_or_else(|| "publication GLB camera ymag is missing".to_owned())?,
            "camera ymag",
        )?;
        if (actual_ymag - camera.orthographic_scale * 0.5).abs() > 1.0e-5 {
            return Err("publication GLB camera scale differs from its recipe".to_owned());
        }
        let xmag = json_finite_number(
            actual_projection
                .get("xmag")
                .ok_or_else(|| "publication GLB camera xmag is missing".to_owned())?,
            "camera xmag",
        )?;
        let expected_xmag = json_finite_number(
            expected_projection
                .get("xmag")
                .ok_or_else(|| "publication recipe camera xmag is missing".to_owned())?,
            "recipe camera xmag",
        )?;
        if (xmag - expected_xmag).abs() > 1.0e-5 {
            return Err("publication GLB camera xmag is invalid".to_owned());
        }
    }
    let actual_world = node_transform(camera_node)?;
    if actual_world
        .to_cols_array()
        .iter()
        .zip(expected_world.iter())
        .any(|(actual, expected)| (actual - expected).abs() > 1.0e-5)
    {
        return Err("publication GLB camera transform differs from its recipe".to_owned());
    }
    Ok(())
}

pub fn validate_glb_export_identity(glb: &[u8], export_id: &str) -> Result<(), String> {
    let validated = parse_validated_glb(glb)?;
    let glb_export_id = validated
        .root
        .pointer("/asset/extras/crystalcanvas/export_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "publication GLB export identity is missing".to_owned())?;
    if glb_export_id != export_id {
        return Err("publication GLB and recipe export identities differ".to_owned());
    }
    Ok(())
}

/// Verifies that a field-aware GLB and its sidecar describe the same realized scene.
pub fn validate_glb_recipe_semantics(
    glb: &[u8],
    recipe: &crate::export_recipe::PublicationGlbRecipe,
) -> Result<(), String> {
    let validated = parse_validated_glb(glb)?;
    let root = &validated.root;
    let glb_export_id = root
        .pointer("/asset/extras/crystalcanvas/export_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "publication GLB export identity is missing".to_owned())?;
    if glb_export_id != recipe.export_id {
        return Err("publication GLB and recipe export identities differ".to_owned());
    }
    let materials = root
        .get("materials")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB materials are missing".to_owned())?;
    let meshes = root
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB meshes are missing".to_owned())?;
    if materials.len() != recipe.semantic_inventory.materials
        || meshes.len() != recipe.semantic_inventory.meshes
    {
        return Err("publication GLB semantic inventory differs from its recipe".to_owned());
    }
    let accessors = root
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB accessors are missing".to_owned())?;
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "publication GLB nodes are missing".to_owned())?;
    validate_recipe_camera(&root, nodes, &recipe.camera)?;
    let (atom_instances, bonds, cell_edges, field_nodes) = structure_node_counts(nodes)?;
    if recipe.semantic_inventory.intrinsic_atoms != recipe.source.intrinsic_atom_count
        || atom_instances != recipe.semantic_inventory.atom_instances
        || bonds != recipe.semantic_inventory.bonds
        || cell_edges != recipe.semantic_inventory.cell_edges
        || field_nodes != recipe.semantic_inventory.field_primitives
    {
        return Err(
            "publication GLB structure semantic inventory differs from its recipe".to_owned(),
        );
    }
    let actual_geometry_bounds = glb_geometry_bounds_from_validated(&validated)?;
    if actual_geometry_bounds != recipe.semantic_inventory.geometry_bounds {
        return Err("publication GLB geometry bounds differ from its recipe".to_owned());
    }
    let crate::export_recipe::ExportRecipeKind::BlenderFieldScene = recipe.kind else {
        if recipe.field_scene.is_some()
            || recipe.semantic_inventory.field_primitives != 0
            || recipe.semantic_inventory.field_vertices != 0
        {
            return Err("publication Blender structure recipe contains field metadata".to_owned());
        }
        return Ok(());
    };
    let field_scene = recipe
        .field_scene
        .as_ref()
        .ok_or_else(|| "publication Blender field recipe is missing its field scene".to_owned())?;
    if root
        .pointer("/asset/extras/crystalcanvas/field_scene_hash")
        .and_then(Value::as_str)
        != Some(field_scene.field_scene_hash.as_str())
    {
        return Err("publication GLB and recipe field-scene hashes differ".to_owned());
    }
    let mut expected_index = 0usize;
    let mut field_vertices = 0usize;
    for node in nodes {
        let extras = node.pointer("/extras/crystalcanvas");
        let Some(representation) = extras
            .and_then(|value| value.get("representation"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let expected = field_scene
            .primitives
            .get(expected_index)
            .ok_or_else(|| "publication GLB contains an unexpected field primitive".to_owned())?;
        let expected_representation = expected.representation.as_str();
        let expected_layer = field_scene
            .layers
            .iter()
            .find(|layer| {
                layer.layer_id == expected.layer_id
                    && layer.source_layer_revision == expected.source_layer_revision
            })
            .ok_or_else(|| {
                "publication recipe field primitive has no provenance layer".to_owned()
            })?;
        if representation != expected_representation
            || extras
                .and_then(|value| value.get("layer_id"))
                .and_then(Value::as_u64)
                != Some(expected.layer_id)
            || extras
                .and_then(|value| value.get("source_layer_revision"))
                .and_then(Value::as_u64)
                != Some(expected.source_layer_revision)
            || extras
                .and_then(|value| value.get("scalar_unit"))
                .and_then(Value::as_str)
                != Some(expected.scalar_unit.as_str())
            || extras.and_then(|value| value.get("isovalue"))
                != Some(
                    &serde_json::to_value(expected.isovalue)
                        .map_err(|_| "publication recipe isovalue is invalid".to_owned())?,
                )
            || extras.and_then(|value| value.get("contour_level"))
                != Some(
                    &serde_json::to_value(expected.contour_level)
                        .map_err(|_| "publication recipe contour level is invalid".to_owned())?,
                )
            || extras.and_then(|value| value.get("slice_plane"))
                != Some(
                    &serde_json::to_value(&expected.slice_plane)
                        .map_err(|_| "publication recipe slice plane is invalid".to_owned())?,
                )
            || extras.and_then(|value| value.get("clipping"))
                != Some(
                    &serde_json::to_value(&expected_layer.clip_planes)
                        .map_err(|_| "publication recipe clipping is invalid".to_owned())?,
                )
            || extras
                .and_then(|value| value.get("material_mapping"))
                .and_then(Value::as_str)
                != Some(
                    expected_layer
                        .presentation
                        .portable_material_mapping
                        .as_str(),
                )
        {
            return Err("publication GLB field primitive differs from its recipe".to_owned());
        }
        let mesh_index = json_usize(node, "mesh")?;
        let mesh = meshes
            .get(mesh_index)
            .ok_or_else(|| "publication GLB field mesh is invalid".to_owned())?;
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .filter(|items| items.len() == 1)
            .ok_or_else(|| "publication GLB field mesh primitive is invalid".to_owned())?;
        let glb_primitive = &primitives[0];
        let position_index = glb_primitive
            .get("attributes")
            .and_then(|value| value.get("POSITION"))
            .ok_or_else(|| "publication GLB field POSITION is missing".to_owned())
            .and_then(|value| json_value_usize(value, "POSITION"))?;
        let position = accessors
            .get(position_index)
            .ok_or_else(|| "publication GLB field POSITION accessor is invalid".to_owned())?;
        field_vertices = field_vertices
            .checked_add(json_usize(position, "count")?)
            .ok_or_else(|| "publication GLB field vertex count overflows".to_owned())?;
        let material_index = json_usize(glb_primitive, "material")?;
        if materials
            .get(material_index)
            .and_then(|value| value.pointer("/extras/crystalcanvas/material_mapping"))
            .and_then(Value::as_str)
            != Some(
                expected_layer
                    .presentation
                    .portable_material_mapping
                    .as_str(),
            )
        {
            return Err("publication GLB field material differs from its recipe".to_owned());
        }
        expected_index += 1;
    }
    if expected_index != field_scene.primitives.len()
        || expected_index != recipe.semantic_inventory.field_primitives
        || field_vertices != recipe.semantic_inventory.field_vertices
    {
        return Err("publication GLB field semantic inventory differs from its recipe".to_owned());
    }
    Ok(())
}
