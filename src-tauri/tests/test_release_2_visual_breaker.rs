//! RELEASE-2 adversarial gates derived from the Intel/Metal publication run.
//!
//! These tests are intentionally RED until the publication renderer can tile
//! to its resource budget, make cell lines legible against its chosen
//! background, and compute one immutable output framing before tile rendering.

use crystal_canvas::renderer::renderer::{
    PublicationExportLimits, PublicationExportRequest, evaluate_publication_export_admission,
};
use serde_json::Value;
use std::path::PathBuf;

fn source(relative_path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    std::fs::read_to_string(path).unwrap_or_default()
}

fn structure_only_request(width: u32, height: u32) -> PublicationExportRequest {
    PublicationExportRequest {
        width,
        height,
        publication_bond_instance_count: 10,
        needs_transparent_depth: false,
        has_measurement_overlays: false,
        has_hopping_overlays: false,
        has_isosurface: false,
        has_volume: false,
        has_phonon_presentation: false,
        has_atom_drag: false,
        show_bz: false,
        has_measurement_state: false,
        has_selection_highlights: false,
        has_wannier_overlay: false,
        has_active_phonon_state: false,
    }
}

fn intel_metal_limits() -> PublicationExportLimits {
    PublicationExportLimits {
        max_texture_dimension_2d: 8192,
        max_buffer_size: 256 * 1024 * 1024,
        publication_msaa_x4: true,
    }
}

fn integer_pair(value: &Value, pointer: &str) -> [u64; 2] {
    let values = value
        .pointer(pointer)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing {pointer}"));
    assert_eq!(values.len(), 2, "{pointer} must contain exactly two values");
    [
        values[0]
            .as_u64()
            .unwrap_or_else(|| panic!("invalid {pointer}[0]")),
        values[1]
            .as_u64()
            .unwrap_or_else(|| panic!("invalid {pointer}[1]")),
    ]
}

#[test]
fn intel_metal_budget_tiles_both_the_observed_7280_and_standard_8k_exports() {
    for (width, height) in [(7280, 4320), (7680, 4320)] {
        let receipt = evaluate_publication_export_admission(
            structure_only_request(width, height),
            intel_metal_limits(),
        )
        .unwrap_or_else(|error| {
            panic!(
                "{width}x{height} must be tiled to the Intel/Metal policy budget instead of being rejected: {error}"
            )
        });
        let receipt = serde_json::to_value(receipt).unwrap();
        let tile_dimensions = integer_pair(&receipt, "/render_plan/tile_dimensions");
        let tile_layout = integer_pair(&receipt, "/render_plan/tile_layout");

        assert!(
            tile_dimensions[0] < u64::from(width) || tile_dimensions[1] < u64::from(height),
            "a budgeted high-resolution export must not retain one full-frame temporary tile"
        );
        assert!(
            tile_layout[0] > 1 || tile_layout[1] > 1,
            "a reduced tile must produce a multi-tile partition"
        );
        assert_eq!(
            u64::from(width).div_ceil(tile_dimensions[0]),
            tile_layout[0],
            "tile columns must cover the full frame exactly"
        );
        assert_eq!(
            u64::from(height).div_ceil(tile_dimensions[1]),
            tile_layout[1],
            "tile rows must cover the full frame exactly"
        );

        let transient_gpu_bytes = receipt
            .pointer("/estimate/transient_gpu_bytes")
            .and_then(Value::as_u64)
            .expect("resource estimate must record transient GPU bytes");
        let gpu_budget = receipt
            .pointer("/budgets/max_transient_gpu_bytes")
            .and_then(Value::as_u64)
            .expect("receipt must record its GPU policy budget");
        assert!(
            transient_gpu_bytes <= gpu_budget,
            "each chosen tile must fit the declared transient GPU budget"
        );
    }
}

#[test]
fn cell_lines_are_background_contrast_aware_and_recorded_in_the_recipe() {
    let look = source("src/renderer/publication_look.rs");
    let renderer = source("src/renderer/renderer.rs");
    let recipe = source("src/export_recipe.rs");

    for required in [
        "PublicationCellLineStyle",
        "for_background",
        "PublicationBackground::White",
        "PublicationBackground::Black",
        "cell_line_color_rgba",
    ] {
        assert!(
            look.contains(required) || renderer.contains(required) || recipe.contains(required),
            "publication cell lines must have an explicit background-aware contrast contract; missing {required:?}"
        );
    }
    for (background, color) in [
        ("white", "[0.18, 0.22, 0.28, 1.0]"),
        ("black", "[0.82, 0.86, 0.92, 1.0]"),
        ("transparent", "[0.20, 0.28, 0.40, 1.0]"),
    ] {
        assert!(
            look.contains(color),
            "the {background} publication background must select a deterministic opaque cell-line contrast color"
        );
    }
    assert!(
        !look.contains("cell_line_width_pixels: 2.0"),
        "the recipe must not claim a two-pixel line while the active LineList pipeline renders one physical pixel"
    );
    assert!(
        renderer.contains("cell_line_style_for_background"),
        "the selected export background must select the cell-line style before any tile pass"
    );
    assert!(
        renderer.contains("PublicationBackground::Current")
            && renderer.contains("current_background")
            && renderer.contains("luminance"),
        "the Current option must derive cell-line contrast from the actual renderer background"
    );
    assert!(
        renderer.contains("publication_srgb_rgba_to_linear")
            && renderer.contains("color: publication_cell_line_color"),
        "the selected sRGB cell-line color must be converted exactly once before upload to the publication-only line buffer"
    );
    assert!(
        recipe.contains("cell_line_color_rgba"),
        "the sidecar must record the effective cell-line contrast color"
    );
}

#[test]
fn export_framing_is_fitted_once_for_the_final_aspect_and_never_refitted_per_tile() {
    let renderer = source("src/renderer/renderer.rs");
    let recipe = source("src/export_recipe.rs");

    for required in [
        "fit_visible_structure_to_export",
        "publication_framing_margin",
        "fit_visible_structure_to_export_aspect_with_margin_v1",
        "cell_lines",
        "bonds",
    ] {
        assert!(
            renderer.contains(required) || recipe.contains(required),
            "publication export must record one deterministic fit-to-output framing policy; missing {required:?}"
        );
    }

    let tile_loop_start = renderer
        .find("for tile_row in 0..config.tile_layout[1]")
        .expect("publication renderer must retain the tile loop");
    let tile_loop_end = renderer[tile_loop_start..]
        .find("\n    fn render_offscreen_tile(")
        .map(|offset| tile_loop_start + offset)
        .expect("tile composition must end before the tile renderer");
    let tile_loop = &renderer[tile_loop_start..tile_loop_end];
    assert!(
        !tile_loop.contains("fit_visible_structure_to_export"),
        "tile rendering must crop one precomputed camera, never refit a local tile camera"
    );
}
