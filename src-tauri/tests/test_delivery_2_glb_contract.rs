//! Executable hostile-contract tests for DELIVERY-2 GLB artifacts.

use crystal_canvas::blender_export::{
    build_blender_glb, validate_glb_bytes, validate_glb_export_identity,
};
use crystal_canvas::renderer::camera::Camera;
use crystal_canvas::renderer::instance::{AtomInstance, BondInstance};
use crystal_canvas::renderer::publication_look::{
    PublicationLookProfile, PublicationLookProfileId,
};
use crystal_canvas::scene_export::{
    PublicationGlbAdmission, PublicationSceneAtom, PublicationSceneSnapshot,
};
use serde_json::{Value, json};

const GLB_JSON_CHUNK: u32 = 0x4e4f_534a;
const GLB_BIN_CHUNK: u32 = 0x004e_4942;

fn hostile_snapshot() -> PublicationSceneSnapshot {
    PublicationSceneSnapshot {
        atoms: vec![PublicationSceneAtom {
            atom: AtomInstance {
                position: [1.0, -2.0, 3.0],
                radius: 1.25,
                color: [0.5, 0.25, 0.75, 0.5],
            },
            source_atom_index: 7,
            image_shift: [1, 0, -1],
        }],
        bonds: vec![BondInstance {
            start: [-1.0, 0.0, 0.0],
            radius: 0.08,
            end: [1.0, 0.0, 0.0],
            _pad: 0.0,
            color: [0.2, 0.4, 0.6, 1.0],
        }],
        cell_edges: vec![([0.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.8, 0.8, 0.8, 0.8])],
        camera: Camera::default_for_crystal(),
        look_profile: PublicationLookProfile::for_id(PublicationLookProfileId::ScientificGloss)
            .unwrap(),
        intrinsic_atom_count: 1,
        show_bonds: true,
        show_cell: true,
        glb_admission: PublicationGlbAdmission {
            atom_instances: 1,
            bonds: 1,
            nodes: 4,
            max_glb_bytes: 1024 * 1024,
            max_peak_cpu_bytes: 3 * 1024 * 1024,
        },
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn glb_root(bytes: &[u8]) -> Value {
    let json_length = read_u32(bytes, 12) as usize;
    serde_json::from_slice(&bytes[20..20 + json_length]).unwrap()
}

fn glb_binary(bytes: &[u8]) -> &[u8] {
    let json_length = read_u32(bytes, 12) as usize;
    let offset = 20 + json_length;
    let binary_length = read_u32(bytes, offset) as usize;
    &bytes[offset + 8..offset + 8 + binary_length]
}

fn forge_glb(root: &Value, binary: &[u8]) -> Vec<u8> {
    let mut json_bytes = serde_json::to_vec(root).unwrap();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 12 + 8 + json_bytes.len() + 8 + binary.len();
    let mut forged = Vec::with_capacity(total);
    forged.extend_from_slice(b"glTF");
    forged.extend_from_slice(&2u32.to_le_bytes());
    forged.extend_from_slice(&(total as u32).to_le_bytes());
    forged.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    forged.extend_from_slice(&GLB_JSON_CHUNK.to_le_bytes());
    forged.extend_from_slice(&json_bytes);
    forged.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    forged.extend_from_slice(&GLB_BIN_CHUNK.to_le_bytes());
    forged.extend_from_slice(binary);
    forged
}

#[test]
fn emitted_glb_is_parseable_and_preserves_the_publication_contract() {
    let artifact = build_blender_glb(&hostile_snapshot(), "delivery-2-real-parser").unwrap();
    validate_glb_bytes(&artifact.bytes).unwrap();
    validate_glb_export_identity(&artifact.bytes, "delivery-2-real-parser").unwrap();

    assert_eq!(&artifact.bytes[..4], b"glTF");
    assert_eq!(read_u32(&artifact.bytes, 4), 2);
    assert_eq!(read_u32(&artifact.bytes, 8) as usize, artifact.bytes.len());

    let root = glb_root(&artifact.bytes);
    assert_eq!(
        root.pointer("/asset/version").and_then(Value::as_str),
        Some("2.0")
    );
    assert_eq!(
        root.pointer("/asset/extras/crystalcanvas/export_id")
            .and_then(Value::as_str),
        Some("delivery-2-real-parser")
    );
    assert_eq!(root["nodes"].as_array().unwrap().len(), 4);
    assert_eq!(root["cameras"].as_array().unwrap().len(), 1);

    let accessors = root["accessors"].as_array().unwrap();
    let meshes = root["meshes"].as_array().unwrap();
    let mut position_counts = Vec::new();
    for mesh in meshes {
        for primitive in mesh["primitives"].as_array().unwrap() {
            let position = primitive["attributes"]["POSITION"].as_u64().unwrap() as usize;
            let accessor = &accessors[position];
            assert_eq!(accessor["type"], json!("VEC3"));
            assert_eq!(accessor["componentType"], json!(5126));
            for bound in ["min", "max"] {
                let values = accessor[bound]
                    .as_array()
                    .expect("POSITION bounds must exist");
                assert_eq!(values.len(), 3);
                assert!(
                    values
                        .iter()
                        .all(|value| value.as_f64().is_some_and(f64::is_finite))
                );
            }
            position_counts.push(accessor["count"].as_u64().unwrap());
        }
    }
    assert!(
        position_counts.contains(&221),
        "UV sphere must not regress to an octahedron"
    );
    assert!(
        position_counts.contains(&66),
        "cylinder must include sides and both caps"
    );

    let materials = root["materials"].as_array().unwrap();
    assert!(
        materials
            .iter()
            .any(|material| material["alphaMode"] == "BLEND")
    );
    let atom_material = materials
        .iter()
        .find(|material| material["alphaMode"] == "BLEND")
        .unwrap();
    let linear_red = atom_material["pbrMetallicRoughness"]["baseColorFactor"][0]
        .as_f64()
        .unwrap();
    assert!(
        (linear_red - 0.214_041_14).abs() < 1.0e-6,
        "sRGB factor must be linearized once"
    );
}

#[test]
fn parser_rejects_truncated_and_semantically_forged_glbs() {
    let artifact = build_blender_glb(&hostile_snapshot(), "delivery-2-forge").unwrap();
    let bytes = artifact.bytes;

    let mut bad_total_length = bytes.clone();
    bad_total_length[8..12].copy_from_slice(&(u32::MAX).to_le_bytes());
    assert!(validate_glb_bytes(&bad_total_length).is_err());

    let mut bad_json_chunk = bytes.clone();
    bad_json_chunk[16..20].copy_from_slice(&GLB_BIN_CHUNK.to_le_bytes());
    assert!(validate_glb_bytes(&bad_json_chunk).is_err());

    let mut missing_position_bounds = glb_root(&bytes);
    let position = missing_position_bounds["meshes"][0]["primitives"][0]["attributes"]["POSITION"]
        .as_u64()
        .unwrap() as usize;
    missing_position_bounds["accessors"][position]
        .as_object_mut()
        .unwrap()
        .remove("min");
    let forged = forge_glb(&missing_position_bounds, glb_binary(&bytes));
    assert!(validate_glb_bytes(&forged).is_err());

    let mut wrong_identity = glb_root(&bytes);
    wrong_identity["asset"]["extras"]["crystalcanvas"]["export_id"] = json!("forged");
    let forged = forge_glb(&wrong_identity, glb_binary(&bytes));
    assert!(validate_glb_export_identity(&forged, "delivery-2-forge").is_err());
}

#[test]
fn parser_rejects_non_numeric_position_bounds() {
    let artifact = build_blender_glb(&hostile_snapshot(), "delivery-2-non-numeric-bounds").unwrap();
    let mut root = glb_root(&artifact.bytes);
    let position = root["meshes"][0]["primitives"][0]["attributes"]["POSITION"]
        .as_u64()
        .unwrap() as usize;
    root["accessors"][position]["min"] = json!(["not-a-number", null, {}]);
    let forged = forge_glb(&root, glb_binary(&artifact.bytes));

    assert!(
        validate_glb_bytes(&forged).is_err(),
        "a POSITION bound must be three finite numeric values"
    );
}

#[test]
fn parser_rejects_out_of_bounds_buffer_views() {
    let artifact = build_blender_glb(&hostile_snapshot(), "delivery-2-accessor-forge").unwrap();

    let mut out_of_bounds_view = glb_root(&artifact.bytes);
    out_of_bounds_view["bufferViews"][0]["byteOffset"] = json!(u64::MAX);
    let forged = forge_glb(&out_of_bounds_view, glb_binary(&artifact.bytes));
    assert!(
        validate_glb_bytes(&forged).is_err(),
        "bufferView offsets must remain inside the BIN chunk"
    );
}

#[test]
fn parser_rejects_wrong_position_accessor_types() {
    let artifact = build_blender_glb(&hostile_snapshot(), "delivery-2-accessor-type").unwrap();
    let mut wrong_position_type = glb_root(&artifact.bytes);
    let position = wrong_position_type["meshes"][0]["primitives"][0]["attributes"]["POSITION"]
        .as_u64()
        .unwrap() as usize;
    wrong_position_type["accessors"][position]["componentType"] = json!(5121);
    wrong_position_type["accessors"][position]["type"] = json!("SCALAR");
    let forged = forge_glb(&wrong_position_type, glb_binary(&artifact.bytes));
    assert!(
        validate_glb_bytes(&forged).is_err(),
        "POSITION must remain a FLOAT VEC3 accessor"
    );
}

#[test]
fn builder_rejects_forged_admission_before_serialization() {
    let mut forged_admission = hostile_snapshot();
    forged_admission.glb_admission.max_peak_cpu_bytes = 0;
    forged_admission.glb_admission.max_glb_bytes = usize::MAX;

    assert!(
        build_blender_glb(&forged_admission, "delivery-2-forged-admission").is_err(),
        "a caller must not be able to disable publication resource admission"
    );
}

#[test]
fn builder_rejects_zero_perspective_fov() {
    let mut zero_fov = hostile_snapshot();
    zero_fov.camera.is_perspective = true;
    zero_fov.camera.fovy_deg = 0.0;
    assert!(
        build_blender_glb(&zero_fov, "delivery-2-zero-fov").is_err(),
        "perspective yfov must be strictly inside (0, pi)"
    );
}

#[test]
fn builder_rejects_overflowing_orthographic_extent() {
    let mut overflowing_orthographic_extent = hostile_snapshot();
    overflowing_orthographic_extent.camera.is_perspective = false;
    overflowing_orthographic_extent.camera.aspect = f32::MAX;
    overflowing_orthographic_extent.camera.orthographic_scale = f32::MAX;
    assert!(
        build_blender_glb(
            &overflowing_orthographic_extent,
            "delivery-2-overflowing-orthographic",
        )
        .is_err(),
        "orthographic xmag must remain finite and positive"
    );
}

#[test]
fn hostile_camera_and_resource_admission_fail_before_an_artifact_exists() {
    let mut degenerate_camera = hostile_snapshot();
    degenerate_camera.camera.target = degenerate_camera.camera.eye;
    assert!(build_blender_glb(&degenerate_camera, "delivery-2-camera").is_err());

    let mut exhausted_budget = hostile_snapshot();
    exhausted_budget.glb_admission.max_glb_bytes = 16;
    assert!(build_blender_glb(&exhausted_budget, "delivery-2-budget").is_err());

    let mut changed_scene = hostile_snapshot();
    changed_scene.glb_admission.nodes = 3;
    assert!(build_blender_glb(&changed_scene, "delivery-2-node-count").is_err());
}
