//! RED acceptance tests for the DELIVERY-2 portable field Blender scene.
//! These gates define the required field inventory, admission, and GLB semantics.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;

fn source(relative_path: &str) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path))
        .unwrap_or_default()
}

#[test]
fn canonical_field_scene_is_a_frozen_cpu_semantic_inventory() {
    let scene = source("src/scene_export.rs");
    let volumetric = source("src/commands/volumetric.rs");

    for required in [
        "PublicationFieldSceneSnapshot",
        "PublicationFieldPrimitive",
        "PortableFieldRepresentation",
        "build_publication_field_scene",
        "layer_id",
        "source_layer_revision",
        "source_artifact_sha256",
        "normalized_layer_sha256",
        "scalar_unit",
        "clip_planes",
        "is_finite",
        "try_reserve_exact",
        "checked_add",
        "checked_mul",
    ] {
        assert!(
            scene.contains(required),
            "portable field export needs a frozen, finite CPU inventory; missing {required:?}"
        );
    }
    assert!(
        !scene.contains("read_buffer") && !scene.contains("map_async"),
        "portable field geometry must be built from the committed CPU field snapshot, never GPU readback"
    );
    assert!(
        volumetric.contains("committed") || volumetric.contains("revision"),
        "the field export snapshot must be tied to a committed layer revision"
    );
}

#[test]
fn field_admission_rejects_nonportable_or_resource_hostile_scenes_without_omission() {
    let scene = source("src/scene_export.rs");
    let recipe = source("src/export_recipe.rs");

    for required in [
        "raycast-only",
        "raw scalar grid",
        "field vertex",
        "field index",
        "field primitive",
        "field material",
        "field texture",
        "buffer-view",
        "MAX_PUBLICATION_GLB_BYTES",
        "MAX_PUBLICATION_GLB_PEAK_CPU_BYTES",
        "reject",
    ] {
        assert!(
            scene.contains(required) || recipe.contains(required),
            "hostile or nonportable field export must reject explicitly; missing {required:?}"
        );
    }
    for forbidden in ["decimat", "smooth", "topology repair", "quantiz"] {
        assert!(
            !scene.to_ascii_lowercase().contains(forbidden),
            "DELIVERY-2 must not silently alter field geometry through {forbidden:?}"
        );
    }
}

#[test]
fn field_geometry_materials_and_metadata_have_portable_explicit_forms() {
    let blender = source("src/blender_export.rs");
    let scene = source("src/scene_export.rs");
    let recipe = source("src/export_recipe.rs");

    for required in [
        "FIELD_ISOSURFACE",
        "FIELD_SLICE",
        "FIELD_CONTOUR",
        "POSITION",
        "NORMAL",
        "COLOR_0",
        "alphaMode",
        "clipping",
        "isovalue",
        "contour_level",
        "contour_radius_angstrom",
        "0.02",
    ] {
        assert!(
            blender.contains(required) || scene.contains(required) || recipe.contains(required),
            "the portable field GLB/sidecar contract is incomplete; missing {required:?}"
        );
    }
    for forbidden in [
        "storage buffer",
        "raycast proxy",
        "OpenVDB",
        "NanoVDB",
        "USD",
        "EXT_",
    ] {
        assert!(
            !blender.contains(forbidden),
            "a portable field export must not serialize {forbidden:?}"
        );
    }
}

#[test]
fn field_sidecar_has_one_identity_and_separate_structure_and_field_counts() {
    let recipe = source("src/export_recipe.rs");
    let blender = source("src/blender_export.rs");

    for required in [
        "BlenderFieldScene",
        "field_primitives",
        "intrinsic_atoms",
        "atom_instances",
        "source_coordinate_unit",
        "normalization_conversion",
        "blender_scale",
        "coordinate_space",
        "matrix_layout",
        "column_major",
        "export_id",
        "sha256",
    ] {
        assert!(
            recipe.contains(required) || blender.contains(required),
            "sidecar must retain the semantic field-scene identity; missing {required:?}"
        );
    }
}
