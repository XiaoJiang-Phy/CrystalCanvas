//! [Overview: Rust integration gate for FIELD-1 field-scene architecture.]
//! Implementation: source-level contract checks for bounded field layers and IPC ownership.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_offset = source
        .find(start)
        .unwrap_or_else(|| panic!("missing source boundary `{start}`"));
    let remainder = &source[start_offset..];
    let end_offset = remainder
        .find(end)
        .unwrap_or_else(|| panic!("missing source boundary `{end}`"));
    &remainder[..end_offset]
}

#[test]
fn field_scene_is_bounded_immutable_and_replaces_the_single_committed_field() {
    let volumetric = include_str!("../src/volumetric.rs");
    let crystal_state = include_str!("../src/crystal_state.rs");

    for required in [
        "struct FieldScene",
        "struct FieldLayer",
        "FieldLayerId",
        "FieldSceneRevision",
        "Arc<[f32]>",
        "MAX_RESIDENT_FIELD_LAYERS",
        "MAX_TOTAL_FIELD_SCALAR_BYTES",
        "MAX_VISIBLE_FIELD_LAYERS_FIELD_1",
        "checked_mul",
        "checked_add",
    ] {
        assert!(
            volumetric.contains(required),
            "FIELD-1 must own bounded immutable field scenes; missing `{required}`"
        );
    }

    assert!(
        crystal_state.contains("field_scene") && crystal_state.contains("#[serde(skip)]"),
        "the Rust CrystalState must own the non-serialized field scene"
    );
    assert!(
        !crystal_state.contains("pub volumetric_data: Option<crate::volumetric::VolumetricData>"),
        "the retired single VolumetricData option cannot remain a second committed field authority"
    );
}

#[test]
fn field_layer_metadata_rejects_ambiguous_or_hostile_grid_mappings() {
    let volumetric = include_str!("../src/volumetric.rs");

    for required in [
        "grid_dims",
        "lattice_angstrom",
        "origin_angstrom",
        "periodic_axes",
        "FieldGridOrdering",
        "GridPoint",
        "Cell",
        "ScalarUnit",
        "FieldNormalization",
        "source_sha256",
        "normalized_sha256",
        "is_finite",
        "Degenerate",
        "BufferLength",
        "Undeclared",
    ] {
        assert!(
            volumetric.contains(required),
            "FIELD-1 must reject ambiguous/hostile mappings rather than infer `{required}`"
        );
    }
    assert!(
        volumetric.contains("ColMajor"),
        "field lattice layout must remain explicitly column-major"
    );
}

#[test]
fn compatibility_receipt_checks_more_than_shape_before_linear_combination() {
    let volumetric = include_str!("../src/volumetric.rs");

    for required in [
        "FieldCompatibilityReceipt",
        "GridDimensions",
        "Lattice",
        "Origin",
        "Ordering",
        "PeriodicAxes",
        "Attachment",
        "ScalarDimension",
        "ScalarUnitScale",
        "Normalization",
        "1e-5",
    ] {
        assert!(
            volumetric.contains(required),
            "FIELD-1 compatibility must identify the exact failed dimension `{required}`"
        );
    }
}

#[test]
fn linear_combination_is_finite_source_preserving_and_failure_atomic() {
    let volumetric = include_str!("../src/volumetric.rs");
    let commands = include_str!("../src/commands/volumetric.rs");
    let combine = source_between(
        commands,
        "pub fn combine_field_layers",
        "\n#[tauri::command]",
    );

    for required in [
        "combine_field_layers",
        "coefficient",
        "f64",
        "is_finite",
        "MAX_LINEAR_COMBINATION_TERMS",
        "normalized_sha256",
        "negative zero",
        "Arc<[f32]>",
    ] {
        assert!(
            volumetric.contains(required) || combine.contains(required),
            "FIELD-1 linear combination must defend `{required}`"
        );
    }
    for required in ["prepare", "revision", "commit", "field_scene_changed"] {
        assert!(
            combine.contains(required),
            "linear combination must prepare then atomically commit; missing `{required}`"
        );
    }
    assert!(
        !combine.contains("state_changed"),
        "a field-only combination must not masquerade as a structural snapshot mutation"
    );
}

#[test]
fn active_renderer_resource_is_revision_bound_and_keeps_the_previous_resource_on_failure() {
    let renderer = include_str!("../src/renderer/renderer.rs");

    for required in [
        "PreparedFieldLayer",
        "active_field_layer",
        "layer_id",
        "layer_revision",
        "prepare_field_layer",
        "commit_field_layer",
        "MAX_VISIBLE_FIELD_LAYERS_FIELD_1",
        "stale",
    ] {
        assert!(
            renderer.contains(required),
            "FIELD-1 renderer admission must bind and validate `{required}`"
        );
    }
    assert!(
        !renderer.contains(
            "pub isosurface_pipeline: Option<crate::renderer::isosurface::IsosurfacePipeline>"
        ),
        "the global single-field renderer pipeline cannot remain the field authority"
    );
}

#[test]
fn field_only_mutations_have_one_field_event_and_no_structural_transaction_side_effect() {
    let commands = include_str!("../src/commands/volumetric.rs");
    let crystal_state = include_str!("../src/crystal_state.rs");

    for required in [
        "pub fn get_field_scene_info",
        "pub fn add_field_layer",
        "pub fn remove_field_layer",
        "pub fn reorder_field_layer",
        "pub fn select_active_field_layer",
        "pub fn combine_field_layers",
        "field_scene_changed",
        "FieldSceneChangedPayload",
    ] {
        assert!(
            commands.contains(required),
            "FIELD-1 requires a typed field-scene operation/event `{required}`"
        );
    }
    assert!(
        crystal_state.contains("invalidate_structure_bound_data")
            && crystal_state.contains("field_scene"),
        "structural invalidation must clear all field layers"
    );

    for operation in [
        "pub fn add_field_layer",
        "pub fn remove_field_layer",
        "pub fn reorder_field_layer",
        "pub fn select_active_field_layer",
        "pub fn combine_field_layers",
    ] {
        let operation_source = source_between(commands, operation, "\n#[tauri::command]");
        assert!(
            !operation_source.contains("stamp_version")
                && !operation_source.contains("state_changed")
                && !operation_source.contains("undo_stack"),
            "{operation} must not change structural version, state_changed, or undo"
        );
    }
}
