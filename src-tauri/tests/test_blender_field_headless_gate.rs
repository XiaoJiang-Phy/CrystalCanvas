//! [Overview: macOS Blender 4.4 headless acceptance gate for portable field GLB export.]
//! Implementation: exports a canonical structure-plus-field scene and verifies Blender imports its geometry and colors.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crystal_canvas::blender_export::build_blender_glb_field_scene;
use crystal_canvas::crystal_state::CrystalState;
use crystal_canvas::export_recipe::{PublicationGlbRecipe, write_publication_glb_pair};
use crystal_canvas::renderer::camera::Camera;
use crystal_canvas::renderer::publication_look::{
    PublicationLookProfile, PublicationLookProfileId,
};
use crystal_canvas::scene_export::{
    PortableFieldRepresentation, PublicationFieldLayerProvenance, PublicationFieldPrimitive,
    PublicationFieldSceneSnapshot, PublicationGlbAdmission, PublicationSceneSnapshot,
};
use crystal_canvas::volumetric::FieldRenderSettings;
use std::path::{Path, PathBuf};
use std::process::Command;

const BLENDER_44: &str = "/Applications/Blender.app/Contents/MacOS/Blender";

fn canonical_field_scene() -> PublicationFieldSceneSnapshot {
    PublicationFieldSceneSnapshot {
        structure: PublicationSceneSnapshot {
            atoms: Vec::new(),
            bonds: Vec::new(),
            cell_edges: Vec::new(),
            camera: Camera::default_for_crystal(),
            look_profile: PublicationLookProfile::for_id(PublicationLookProfileId::ScientificGloss)
                .expect("scientific profile must exist"),
            intrinsic_atom_count: 0,
            show_bonds: false,
            show_cell: false,
            glb_admission: PublicationGlbAdmission {
                atom_instances: 0,
                bonds: 0,
                nodes: 1,
                max_glb_bytes: 1024 * 1024,
                max_peak_cpu_bytes: 3 * 1024 * 1024,
            },
        },
        field_primitives: vec![PublicationFieldPrimitive {
            representation: PortableFieldRepresentation::Isosurface,
            layer_id: 41,
            source_layer_revision: 7,
            scalar_unit: "electron_per_cubic_angstrom".to_owned(),
            isovalue: Some(0.25),
            contour_level: None,
            slice_plane: None,
            clip_planes: Vec::new(),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            colors: vec![[0.2, 0.7, 0.4, 0.5]; 3],
        }],
        field_layers: vec![PublicationFieldLayerProvenance {
            layer_id: 41,
            source_layer_revision: 7,
            source_artifact_sha256: "a".repeat(64),
            normalized_layer_sha256: "b".repeat(64),
            source_coordinate_unit: "angstrom".to_owned(),
            coordinate_to_angstrom: 1.0,
            normalization_conversion: "raw".to_owned(),
            scalar_unit: "electron_per_cubic_angstrom".to_owned(),
            scalar_unit_scale: 1.0,
            clip_planes: Vec::new(),
            presentation: Default::default(),
            render_settings: FieldRenderSettings::default(),
        }],
        field_scene_hash: "c".repeat(64),
    }
}

fn blender_binary() -> Option<PathBuf> {
    std::env::var_os("CRYSTALCANVAS_BLENDER_BIN")
        .map(PathBuf::from)
        .or_else(|| {
            Path::new(BLENDER_44)
                .is_file()
                .then(|| PathBuf::from(BLENDER_44))
        })
}

fn release_artifact_directory() -> PathBuf {
    PathBuf::from(
        std::env::var("CRYSTALCANVAS_RELEASE_ARTIFACT_DIR")
            .expect("set CRYSTALCANVAS_RELEASE_ARTIFACT_DIR to generate a release fixture"),
    )
}

#[test]
#[ignore = "writes an explicit software-only v0.8 field GLB fixture"]
fn writes_release_v0_8_software_field_glb_fixture() {
    let directory = release_artifact_directory();
    std::fs::create_dir_all(&directory).expect("release fixture directory must be creatable");
    let primary_path = directory.join("software-contract-field.glb");
    let sidecar_path =
        crystal_canvas::export_recipe::publication_sidecar_path(&primary_path).unwrap();
    let marker_path =
        crystal_canvas::export_recipe::publication_pair_commit_path(&primary_path).unwrap();
    assert!(
        !primary_path.exists() && !sidecar_path.exists() && !marker_path.exists(),
        "release fixture generation never overwrites an existing artifact triplet"
    );

    let scene = canonical_field_scene();
    let mut source = CrystalState::default();
    source.name = "software-contract-field".to_owned();
    let mut recipe = PublicationGlbRecipe::from_field_scene(&source, &scene)
        .expect("canonical field recipe must be valid");
    let artifact = build_blender_glb_field_scene(&scene, &recipe.export_id)
        .expect("canonical field GLB must build");
    recipe.semantic_inventory = artifact.semantic_inventory;
    write_publication_glb_pair(&primary_path, &artifact.bytes, recipe)
        .expect("software field GLB fixture must be emitted as a complete artifact triplet");
}

#[test]
#[ignore = "requires an explicitly selected Blender 4.4 binary and native Metal-capable host"]
fn blender_44_headless_imports_canonical_structure_and_field_artifact() {
    let Some(blender) = blender_binary() else {
        panic!("set CRYSTALCANVAS_BLENDER_BIN to the selected Blender 4.4 executable");
    };
    let version = Command::new(&blender)
        .arg("--version")
        .output()
        .expect("Blender executable must run");
    assert!(version.status.success(), "Blender --version failed");
    let version_text = String::from_utf8_lossy(&version.stdout);
    assert!(
        version_text.contains("Blender 4.4"),
        "DELIVERY-2 selected import gate requires Blender 4.4, got {version_text:?}"
    );

    let scene = canonical_field_scene();
    let mut source = CrystalState::default();
    source.name = "canonical-blender-field-gate".to_owned();
    let mut recipe = PublicationGlbRecipe::from_field_scene(&source, &scene)
        .expect("canonical field recipe must be valid");
    let artifact = build_blender_glb_field_scene(&scene, &recipe.export_id)
        .expect("canonical field GLB must build");
    recipe.semantic_inventory = artifact.semantic_inventory;
    let temporary = tempfile::tempdir().expect("temporary Blender gate directory");
    let glb_path = temporary.path().join("canonical-field.glb");
    let sidecar_path = write_publication_glb_pair(&glb_path, &artifact.bytes, recipe)
        .expect("canonical field GLB and sidecar must write");
    let sidecar = std::fs::read_to_string(&sidecar_path).expect("field sidecar must be readable");
    assert!(sidecar.contains("field_scene_hash"));
    assert!(sidecar.contains("isosurface"));

    let check = r#"
import bpy, sys
path = sys.argv[sys.argv.index('--') + 1]
bpy.ops.import_scene.gltf(filepath=path)
fields = [obj for obj in bpy.data.objects if obj.name.startswith('FIELD_')]
assert len(fields) == 1, f'expected one field object, got {len(fields)}'
field = fields[0]
assert field.type == 'MESH', f'field object type is {field.type}'
assert len(field.data.vertices) == 3, f'field vertex count is {len(field.data.vertices)}'
assert len(field.data.polygons) == 1, f'field polygon count is {len(field.data.polygons)}'
assert len(field.data.materials) == 1, 'field material was not imported'
assert len(field.data.color_attributes) > 0, 'COLOR_0 was not imported'
print('CRYSTALCANVAS_BLENDER_FIELD_IMPORT_OK')
"#;
    let imported = Command::new(&blender)
        .args([
            "--background",
            "--factory-startup",
            "--python-expr",
            check,
            "--",
        ])
        .arg(&glb_path)
        .output()
        .expect("Blender headless import must run");
    assert!(
        imported.status.success(),
        "Blender failed to import canonical field GLB:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&imported.stdout),
        String::from_utf8_lossy(&imported.stderr),
    );
    assert!(
        String::from_utf8_lossy(&imported.stdout).contains("CRYSTALCANVAS_BLENDER_FIELD_IMPORT_OK"),
        "Blender did not complete the field import assertions"
    );
}
