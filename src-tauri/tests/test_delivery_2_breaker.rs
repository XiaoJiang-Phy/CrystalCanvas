//! DELIVERY-2 adversarial acceptance gates.
//!
//! These tests define the one-way Blender scene contract before the exporter
//! exists. They intentionally remain RED until Builder implements the complete
//! GLB, sidecar, IPC, and UI boundary. They do not require a window-backed GPU
//! or a Blender installation.

use std::path::PathBuf;

fn source(relative_path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    std::fs::read_to_string(path).unwrap_or_default()
}

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start = source
        .find(start)
        .unwrap_or_else(|| panic!("missing source boundary {start:?}"));
    let remainder = &source[start..];
    let end = remainder
        .find(end)
        .unwrap_or_else(|| panic!("missing source boundary {end:?}"));
    &remainder[..end]
}

fn command_body<'a>(source: &'a str, command: &str) -> &'a str {
    source_between(source, &format!("pub fn {command}("), "\n#[tauri::command]")
}

#[test]
fn one_way_glb_export_has_a_separate_scene_snapshot_and_no_reverse_path() {
    let lib = source("src/lib.rs");
    let scene_export = source("src/scene_export.rs");
    let blender_export = source("src/blender_export.rs");
    let file_io = source("src/commands/file_io.rs");

    assert!(
        lib.contains("pub mod scene_export;") && lib.contains("pub mod blender_export;"),
        "DELIVERY-2 must isolate normalized scene construction from GLB byte serialization"
    );
    for required in [
        "PublicationSceneSnapshot",
        "RenderAtomInstance",
        "build_periodic_atom_instances",
        "source_atom_index",
        "image_shift",
        "build_cell_lines",
        "PublicationBondColorMode",
        "show_bonds",
        "show_cell",
    ] {
        assert!(
            scene_export.contains(required),
            "the Blender scene must retain renderer-derived structure identity and visibility; missing {required:?}"
        );
    }
    assert!(
        file_io.contains("pub fn export_blender_scene("),
        "Blender delivery requires an explicit export-only IPC command"
    );
    for forbidden in [
        "import_blender",
        "import_glb",
        "parse_glb",
        "round_trip",
        "live_link",
        "synchronize_blender",
    ] {
        assert!(
            !lib.contains(forbidden)
                && !scene_export.contains(forbidden)
                && !blender_export.contains(forbidden)
                && !file_io.contains(forbidden),
            "DELIVERY-2 is strictly one-way; forbidden reverse-path marker {forbidden:?} found"
        );
    }
}

#[test]
fn hostile_scene_data_is_rejected_before_glb_allocation_or_serialization() {
    let scene_export = source("src/scene_export.rs");
    let blender_export = source("src/blender_export.rs");

    for required in [
        "validate",
        "is_finite",
        "checked_add",
        "checked_mul",
        "try_reserve_exact",
        "u32::try_from",
        "publication bond scene changed after admission",
    ] {
        assert!(
            scene_export.contains(required) || blender_export.contains(required),
            "empty, non-finite, overflowed, or resource-meltdown scenes must reject before GLB output; missing {required:?}"
        );
    }
    for rejected_domain in [
        "measurements",
        "selected_atoms",
        "active_phonon_mode",
        "wannier_overlay",
        "brillouin",
    ] {
        assert!(
            scene_export.contains(rejected_domain) && scene_export.contains("reject"),
            "scene admission must actively reject {rejected_domain:?}, not merely omit it accidentally"
        );
    }
    assert!(
        !scene_export.contains("(\"isosurface\", request.has_isosurface)")
            && !scene_export.contains("(\"volume\", request.has_volume)"),
        "DELIVERY-2 must admit portable realized field geometry; it may reject only a raycast-only scene"
    );
}

#[test]
fn glb_writer_emits_a_bounds_checked_core_asset_with_shared_mesh_data() {
    let blender_export = source("src/blender_export.rs");

    for required in [
        "b\"glTF\"",
        "2u32",
        "JSON",
        "BIN",
        "align",
        "checked_add",
        "byteOffset",
        "byteLength",
        "bufferViews",
        "accessors",
        "POSITION",
        "NORMAL",
        "meshes",
        "materials",
        "cameras",
        "extras",
        "shared_sphere",
        "shared_cylinder",
    ] {
        assert!(
            blender_export.contains(required),
            "GLB writer must declare a checked core glTF 2.0 contract; missing {required:?}"
        );
    }
    for forbidden in [
        "EXT_mesh_gpu_instancing",
        "RenderGraph",
        "MaterialNode",
        "async ",
        "OpenVDB",
        "USD",
    ] {
        assert!(
            !blender_export.contains(forbidden),
            "DELIVERY-2 must not add low-yield exporter scope {forbidden:?}"
        );
    }
}

#[test]
fn sidecar_declares_truthful_units_identity_camera_and_material_mapping() {
    let recipe = source("src/export_recipe.rs");
    let scene_export = source("src/scene_export.rs");
    let blender_export = source("src/blender_export.rs");

    for required in [
        "PublicationGlbRecipe",
        "export_id",
        "source_length_unit",
        "coordinate_length_unit",
        "meters_per_exported_unit",
        "matrix_layout",
        "column_major",
        "source_atom_index",
        "image_shift",
        "semantic_inventory",
        "material_mapping",
        "camera",
        "sha256",
    ] {
        assert!(
            recipe.contains(required)
                || scene_export.contains(required)
                || blender_export.contains(required),
            "the GLB sidecar must make its unit, identity, material, and camera semantics auditable; missing {required:?}"
        );
    }
    assert!(
        !recipe.contains("gltf_normative_meter_scale_applied: true"),
        "the sidecar must not falsely claim metre-scale glTF conformance when CrystalCanvas emits scientific display coordinates"
    );
}

#[test]
fn glb_and_sidecar_are_an_atomic_non_overwriting_artifact_pair() {
    let recipe = source("src/export_recipe.rs");
    let file_io = source("src/commands/file_io.rs");
    let command = command_body(&file_io, "export_blender_scene");

    for required in [
        "validate_publication_glb_targets",
        "write_publication_glb_pair",
        "publication_sidecar_path",
        "ensure_output_path_available",
        "temporary_sibling",
        "rename",
        "remove_file",
        "sha256",
        "export_id",
    ] {
        assert!(
            recipe.contains(required),
            "GLB pair commit must prevent overwrite and clean staged artifacts; missing {required:?}"
        );
    }
    assert!(
        command.contains("PublicationGlbRecipe")
            && command.contains("write_publication_glb_pair")
            && !command.contains("std::fs::write"),
        "IPC must commit only the validated GLB/sidecar pair, never write a lone final artifact"
    );
}

#[test]
fn ipc_and_ui_offer_one_explicit_blender_action_and_browser_rejects_it() {
    let file_io = source("src/commands/file_io.rs");
    let contracts = source("../src/ipc/contracts.ts");
    let inventory = source("../ipc/inventory.json");
    let modal = source("../src/components/layout/ExportImageModal.tsx");
    let tauri_mock = source("../src/utils/tauri-mock.ts");

    let command = command_body(&file_io, "export_blender_scene");
    assert!(
        command.contains("publication_profile")
            && command.contains("IpcEnumInput<PublicationLookProfileId>"),
        "Blender export must receive the selected fixed publication profile through typed snake_case IPC"
    );
    for required in ["export_blender_scene", "publicationProfile"] {
        assert!(
            contracts.contains(required) && inventory.contains(required),
            "typed TypeScript contract and IPC inventory must expose {required:?}"
        );
    }
    for required in [
        "Blender Scene",
        "export_blender_scene",
        "publicationProfile",
        "isExporting",
        "sidecar",
    ] {
        assert!(
            modal.contains(required),
            "the existing export modal must present one deterministic Blender workflow; missing {required:?}"
        );
    }
    assert!(
        tauri_mock.contains("not_in_tauri")
            && tauri_mock.contains("browser_policy_for")
            && inventory.contains("export_blender_scene"),
        "browser mode must reject Blender export through the existing external-I/O policy"
    );
}
