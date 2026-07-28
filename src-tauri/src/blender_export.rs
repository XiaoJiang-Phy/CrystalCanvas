//! Narrow glTF 2.0 binary writer for one-way CrystalCanvas structure scenes.

use crate::ipc::{IpcError, IpcResult};
use crate::scene_export::{PublicationGlbAdmission, PublicationSceneSnapshot};
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GlbSemanticInventory {
    pub intrinsic_atoms: usize,
    pub atom_instances: usize,
    pub bonds: usize,
    pub cell_edges: usize,
    pub materials: usize,
    pub meshes: usize,
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
    if export_id.is_empty() {
        return Err(IpcError::render(
            "publication GLB export identity is missing",
        ));
    }
    validate_snapshot_admission(snapshot)?;
    let admission = snapshot.glb_admission;
    let mut writer = GlbWriter::new(admission)?;
    let (sphere_pos, sphere_norm, sphere_idx) = writer.shared_sphere()?;
    let (cylinder_pos, cylinder_norm, cylinder_idx) = writer.shared_cylinder()?;
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
        .try_reserve_exact(admission.nodes)
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
    let node_indices: Vec<u32> = (0..nodes.len())
        .map(u32::try_from)
        .collect::<Result<_, _>>()
        .map_err(|_| IpcError::render("publication GLB node count exceeds u32"))?;
    let root = json!({
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
        "scenes": [{"nodes": node_indices}],
        "nodes": nodes,
        "meshes": meshes,
        "materials": materials,
        "cameras": [camera],
        "buffers": [{"byteLength": writer.bin.len()}],
        "bufferViews": writer.buffer_views,
        "accessors": writer.accessors,
    });
    let bytes = writer.finish(root)?;
    validate_glb_bytes(&bytes).map_err(IpcError::render)?;
    Ok(BlenderGlbArtifact {
        semantic_inventory: GlbSemanticInventory {
            intrinsic_atoms: snapshot.intrinsic_atom_count,
            atom_instances: snapshot.atoms.len(),
            bonds: snapshot.bonds.len(),
            cell_edges: snapshot.cell_edges.len(),
            materials: material_indices.len(),
            meshes: mesh_indices.len(),
        },
        bytes,
    })
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
    admission: PublicationGlbAdmission,
}

impl GlbWriter {
    fn new(admission: PublicationGlbAdmission) -> IpcResult<Self> {
        let mut bin = Vec::new();
        bin.try_reserve_exact(64 * 1024)
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
            admission,
        })
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

    fn ensure_bin_capacity(&self, additional: usize) -> IpcResult<()> {
        let next_len = self
            .bin
            .len()
            .checked_add(additional)
            .ok_or_else(|| IpcError::render("GLB binary byte count overflow"))?;
        if next_len > self.admission.max_glb_bytes {
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
        let json_bytes = serde_json::to_vec(&root)
            .map_err(|error| IpcError::render(format!("unable to serialize GLB JSON: {error}")))?;
        let mut json_padded = json_bytes;
        while json_padded.len() % 4 != 0 {
            json_padded.push(b' ');
        }
        self.align();
        let total = 12usize
            .checked_add(8)
            .and_then(|value| value.checked_add(json_padded.len()))
            .and_then(|value| value.checked_add(8))
            .and_then(|value| value.checked_add(self.bin.len()))
            .ok_or_else(|| IpcError::render("GLB size overflow"))?;
        if total > self.admission.max_glb_bytes {
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
            &(u32::try_from(json_padded.len())
                .map_err(|_| IpcError::render("GLB JSON too large"))?)
            .to_le_bytes(),
        );
        bytes.extend_from_slice(&JSON_CHUNK.to_le_bytes());
        bytes.extend_from_slice(&json_padded);
        bytes.extend_from_slice(
            &(u32::try_from(self.bin.len())
                .map_err(|_| IpcError::render("GLB binary too large"))?)
            .to_le_bytes(),
        );
        bytes.extend_from_slice(&BIN_CHUNK.to_le_bytes());
        bytes.extend_from_slice(&self.bin);
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

pub fn validate_glb_bytes(glb: &[u8]) -> Result<(), String> {
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
    if buffer_views.len() != PUBLICATION_GLB_BUFFER_VIEW_COUNT {
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
    if accessors.len() != PUBLICATION_GLB_ACCESSOR_COUNT {
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
            _ => return Err("publication GLB accessor type is unsupported".to_owned()),
        };
        let component_size = match component_type {
            5123 => 2usize,
            5126 => 4usize,
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
    if root
        .get("nodes")
        .and_then(Value::as_array)
        .map_or(true, |nodes| nodes.len() > MAX_PUBLICATION_GLB_NODES)
    {
        return Err("publication GLB node schema is invalid".to_owned());
    }
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
            let indices = accessor_at(&parsed_accessors, json_usize(primitive, "indices")?)?;
            if position.component_type != 5126
                || position.kind != "VEC3"
                || normal.component_type != 5126
                || normal.kind != "VEC3"
                || normal.count != position.count
                || indices.component_type != 5123
                || indices.kind != "SCALAR"
                || json_usize(primitive, "material")? >= materials.len()
            {
                return Err("publication GLB primitive accessor is invalid".to_owned());
            }
            validate_position_bounds(
                accessors,
                json_usize(attributes, "POSITION")?,
                position,
                bin,
            )?;
            for index in 0..indices.count {
                let offset = indices
                    .offset
                    .checked_add(
                        index
                            .checked_mul(2)
                            .ok_or_else(|| "publication GLB index offset overflows".to_owned())?,
                    )
                    .ok_or_else(|| "publication GLB index offset overflows".to_owned())?;
                let bytes: [u8; 2] = bin
                    .get(offset..offset + 2)
                    .ok_or_else(|| "publication GLB index is truncated".to_owned())?
                    .try_into()
                    .map_err(|_| "publication GLB index is truncated".to_owned())?;
                if usize::from(u16::from_le_bytes(bytes)) >= position.count {
                    return Err("publication GLB index exceeds POSITION count".to_owned());
                }
            }
        }
    }
    Ok(())
}

fn json_usize(value: &Value, field: &str) -> Result<usize, String> {
    usize::try_from(
        value
            .get(field)
            .and_then(Value::as_u64)
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
    let values = value
        .get(field)
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| format!("publication GLB POSITION {field} is invalid"))?;
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        let component = value
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| format!("publication GLB POSITION {field} is invalid"))?
            as f32;
        if !component.is_finite() {
            return Err(format!("publication GLB POSITION {field} is invalid"));
        }
        result[index] = component;
    }
    Ok(result)
}

pub fn validate_glb_export_identity(glb: &[u8], export_id: &str) -> Result<(), String> {
    validate_glb_bytes(glb)?;
    let json_length = u32::from_le_bytes(
        glb[12..16]
            .try_into()
            .map_err(|_| "publication GLB JSON length is invalid".to_owned())?,
    ) as usize;
    let root: Value = serde_json::from_slice(&glb[20..20 + json_length])
        .map_err(|_| "publication GLB JSON is invalid".to_owned())?;
    let glb_export_id = root
        .pointer("/asset/extras/crystalcanvas/export_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "publication GLB export identity is missing".to_owned())?;
    if glb_export_id != export_id {
        return Err("publication GLB and recipe export identities differ".to_owned());
    }
    Ok(())
}
