//! [Overview: Adversarial FIGURE-2 contract tests for field geometry, rendering, and publication export.]
//! Implementation: executable hostile-input checks plus source gates for contracts that require a GPU-backed renderer.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crystal_canvas::renderer::field_scene::{
    FieldRenderSnapshot, FieldSliceInterpolation, FieldSlicePlane, FieldSliceRequest,
};
use serde_json::json;
use std::path::PathBuf;

fn source(relative_path: &str) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path))
        .unwrap_or_default()
}

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

fn complete_five_representation_snapshot() -> FieldRenderSnapshot {
    serde_json::from_value(json!({
        "layer_id": 7,
        "source_layer_revision": 11,
        "scalar_unit": "arbitrary",
        "scalar_range": [-2.0, 3.0],
        "representations": [
            "positive_isosurface",
            "negative_isosurface",
            "volume_raycast",
            "slice",
            "contour"
        ],
        "positive_isovalue": 1.0,
        "negative_isovalue": 1.0,
        "clip_planes": [],
        "slices": [],
        "transfer_function": {
            "color_space": "LinearRgb",
            "negative_control_points": [
                { "position": 0.0, "color_linear_rgba": [0.1, 0.2, 0.7, 0.0] },
                { "position": 1.0, "color_linear_rgba": [0.2, 0.4, 1.0, 0.7] }
            ],
            "positive_control_points": [
                { "position": 0.0, "color_linear_rgba": [0.9, 0.3, 0.1, 0.0] },
                { "position": 1.0, "color_linear_rgba": [1.0, 0.8, 0.2, 0.7] }
            ]
        },
        "transparency_method": "premultiplied_alpha_fallback",
        "display_range": [-1.0, 2.0],
        "opacity_scale": 1.0,
        "density_cutoff": 0.0,
        "colormap_mode": 0
    }))
    .expect("hostile FIGURE-2 fixture must deserialize before it reaches validation")
}

#[test]
fn admits_all_required_representations_for_one_layer_without_reusing_the_layer_cap() {
    let snapshot = complete_five_representation_snapshot();
    assert!(
        snapshot.validate().is_ok(),
        "one visible layer must be able to combine paired signed isosurfaces, volume, slice, and contour; a visible-layer cap is not a representation cap"
    );
}

#[test]
fn rejects_degenerate_one_dimensional_slices_before_cpu_or_gpu_allocation() {
    for dimensions in [[1, 512], [512, 1], [1, 1]] {
        let request = FieldSliceRequest {
            plane: FieldSlicePlane {
                normal: [0.0, 0.0, 1.0],
                signed_offset_angstrom: 0.0,
                interpolation: FieldSliceInterpolation::Trilinear,
            },
            dimensions,
            contour_levels: Vec::new(),
        };
        assert!(
            request.validate().is_err(),
            "a {dimensions:?} slice has no cells and must reject before allocating an empty field resource"
        );
    }
}

#[test]
fn adapters_and_renderers_must_share_one_declared_grid_mapping_not_implicit_n_minus_one_math() {
    let field_scene = source("src/renderer/field_scene.rs");
    let isosurface = source("src/renderer/isosurface.rs");
    let volume = source("src/renderer/volume_raycast.rs");
    let cube = source("src/io/cube_parser.rs");
    let chgcar = source("src/io/chgcar_parser.rs");
    let xsf = source("src/io/xsf_volumetric_parser.rs");
    let combined = format!("{field_scene}\n{isosurface}\n{volume}\n{cube}\n{chgcar}\n{xsf}");

    for required in [
        "FieldGridMapping",
        "AxisSampling",
        "PeriodicExclusive",
        "InclusiveBoundary",
        "sample_steps_col_major",
        "index_to_world",
        "world_to_grid",
        "sample_trilinear",
    ] {
        assert!(
            combined.contains(required),
            "FIGURE-2 must use one explicit adapter-declared grid mapping; missing `{required}`"
        );
    }
    assert!(
        !cube.contains("origin: [0.0, 0.0, 0.0]"),
        "Cube origin cannot be silently discarded before FIGURE-2 samples or renders the field"
    );
}

#[test]
fn composition_uses_one_frozen_sorted_draw_list_for_interactive_and_publication_paths() {
    let renderer = source("src/renderer/renderer.rs");
    let recipe = source("src/export_recipe.rs");

    for required in [
        "FrozenFieldDrawList",
        "stable_layer_id_ascending_then_translucent_structure",
        "draw_frozen_field_resources",
        "field_scene_hash",
    ] {
        assert!(
            renderer.contains(required) || recipe.contains(required),
            "FIGURE-2 must derive interactive and publication field draws from one frozen order; missing `{required}`"
        );
    }
    assert!(
        !renderer.contains("retained_field_layers.swap_remove"),
        "swap_remove invalidates layer-id ordering and can make the declared premultiplied fallback nondeterministic"
    );
}

#[test]
fn unlit_field_material_is_a_serialized_uniform_contract_not_fixed_shader_lighting() {
    let field_scene = source("src/renderer/field_scene.rs");
    let recipe = source("src/export_recipe.rs");
    let iso_shader = source("shaders/isosurface_render.wgsl");
    let volume_shader = source("shaders/volume_raycast.wgsl");
    let combined = format!("{field_scene}\n{recipe}\n{iso_shader}\n{volume_shader}");

    for required in ["FieldMaterialMode", "Unlit", "field_material_mode", "unlit"] {
        assert!(
            combined.contains(required),
            "FIGURE-2 must serialize and bind an unlit field-material mode; missing `{required}`"
        );
    }
    assert!(
        iso_shader.contains("if !field_material.unlit")
            && volume_shader.contains("if !params.unlit"),
        "unlit isosurfaces and volumes must bypass every lighting calculation rather than merely dimming it"
    );
}

#[test]
fn clipping_must_preserve_the_intersection_of_a_slice_triangle_not_drop_the_whole_cell() {
    let slice_cpu = source("src/renderer/field_slice.rs");
    let slice_shader = source("shaders/field_slice.wgsl");

    assert!(
        slice_cpu.contains("clip_triangle_to_half_spaces")
            || (slice_shader.contains("clip_planes") && slice_shader.contains("discard")),
        "a clipping plane that intersects one slice triangle must emit its clipped polygon or discard fragments; all-vertices-kept culling creates grid-sized holes"
    );
    assert!(
        !slice_cpu.contains("triangle.iter().all"),
        "slice clipping must not keep only triangles whose three vertices already lie in every half-space"
    );
}

#[test]
fn publication_receipt_binds_actual_field_bytes_and_volume_rejects_unrepresentable_ray_lengths() {
    let renderer = source("src/renderer/renderer.rs");
    let volume = source("src/renderer/volume_raycast.rs");
    let estimate = source_between(
        &renderer,
        "fn publication_export_resource_estimate(",
        "fn finish_publication_export_error_scopes",
    );

    for required in [
        "FieldResourceFootprint",
        "resident_field_bytes",
        "isosurface_vertex_bytes",
        "volume_storage_bytes",
        "slice_vertex_bytes",
        "contour_vertex_bytes",
        "field_scene_hash",
    ] {
        assert!(
            renderer.contains(required) || estimate.contains(required),
            "publication admission must bind real field memory rather than tile-sized placeholders; missing `{required}`"
        );
    }
    assert!(
        !volume.contains("max_steps.clamp(256, 2048)"),
        "a long thin field must reject before rendering when its required ray steps exceed the bounded budget; clamping silently truncates the volume"
    );
    assert!(
        volume.contains("required_steps") && volume.contains("VolumeRaycastPipeline::new"),
        "volume preparation must calculate and enforce the required ray-step count"
    );
}

#[test]
fn active_composition_gate_cannot_pass_by_inspecting_an_unwired_oit_shader() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert!(
        !root.join("shaders/field_composite.wgsl").exists(),
        "the unsupported OIT shader must not remain as dead evidence for a composition gate; only the admitted fallback may be tested"
    );
}
