//! RENDER-2 adversarial publication-rendering gates.
//!
//! These checks stay source-level because the render path needs a window-backed
//! GPU. They make unsupported sample modes, malformed tile layouts, and
//! accidental full-frame GPU allocations fail before Builder implementation is
//! admitted.

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start = source
        .find(start)
        .unwrap_or_else(|| panic!("missing source boundary `{start}`"));
    let remainder = &source[start..];
    let end = remainder
        .find(end)
        .unwrap_or_else(|| panic!("missing source boundary `{end}`"));
    &remainder[..end]
}

#[test]
fn publication_sampling_uses_the_active_format_capabilities_and_has_only_the_4x_to_1x_fallback() {
    let gpu_context = include_str!("../src/renderer/gpu_context.rs");
    let render_config = include_str!("../src/renderer/render_config.rs");
    let renderer = include_str!("../src/renderer/renderer.rs");
    let pipeline = include_str!("../src/renderer/pipeline.rs");

    assert!(
        gpu_context.contains("get_texture_format_features")
            && gpu_context.contains("Depth32Float")
            && render_config.contains("MULTISAMPLE_X4"),
        "RENDER-2 must capture active color/depth format sample capabilities during GPU initialization; device limits alone cannot authorize MSAA"
    );
    assert!(
        render_config.contains("requested_samples")
            && render_config.contains("selected_samples")
            && render_config.contains("[4, 1]"),
        "RENDER-2 must record a deterministic requested/selected sampling decision with exactly the 4x-to-1x fallback chain"
    );
    assert!(
        renderer.contains("Publication Resolve Color Texture")
            && renderer.contains("resolve_target: if config.selected_samples > 1")
            && !renderer.contains("EXPORT-1B supports only single-sample publication targets"),
        "a selected multisample target must resolve into a single-sample copy/readback target, not be rejected by the retired EXPORT-1B guard"
    );
    assert!(
        pipeline.contains("sample_count")
            && !pipeline.contains("MultisampleState {\n            count: 1"),
        "all publication pipeline and depth attachments must receive the selected sample count; a hard-coded single-sample attachment can produce incompatible passes"
    );
}

#[test]
fn tiled_export_is_projection_cropped_composed_sequentially_and_never_uses_overlap() {
    let camera = include_str!("../src/renderer/camera.rs");
    let renderer = include_str!("../src/renderer/renderer.rs");
    let recipe = include_str!("../src/export_recipe.rs");
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    /// Clear volumetric pipelines",
    );

    assert!(
        camera.contains("tile")
            && camera.contains("projection")
            && camera.contains("orthographic")
            && camera.contains("perspective"),
        "RENDER-2 must apply the same tile crop transform to perspective and orthographic export projections"
    );
    assert!(
        offscreen.contains("tile")
            && offscreen.contains("copy_texture_to_buffer")
            && offscreen.contains("full_output")
            && offscreen.contains("current_tile"),
        "RENDER-2 must render one bounded tile at a time and compose it into one CPU output; allocating every tile or one oversized GPU target violates the export budget"
    );
    assert!(
        recipe.contains("tile_dimensions")
            && recipe.contains("tile_overlap_pixels")
            && recipe.contains("tile_layout")
            && recipe.contains("tile_overlap_pixels: 0"),
        "the recipe must record grid, actual tile dimensions, and an explicit zero-overlap policy so a tiled image is reproducible"
    );
}

#[test]
fn multisampled_transparent_tiles_never_copy_depth_and_resolve_only_after_the_final_pass() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let offscreen = source_between(
        renderer,
        "fn render_offscreen_tile(",
        "\n    /// Clear volumetric pipelines",
    );

    assert!(
        offscreen.contains("if config.selected_samples == 1")
            && offscreen.contains("Publication Opaque Depth Replay Pass"),
        "a multisampled depth attachment cannot be copied; RENDER-2 must use an MSAA-safe opaque depth replay for transparent structure atoms"
    );
    assert!(
        offscreen.contains("resolve_target: if config.selected_samples > 1 && !needs_transparent")
            && offscreen.contains("resolve_target: if config.selected_samples > 1 {"),
        "opaque tiles may resolve only when there is no transparent pass; transparent tiles must resolve in their final pass"
    );
}

#[test]
fn tile_composition_and_admission_use_checked_plan_bound_resources() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    /// Clear volumetric pipelines",
    );
    let estimate = source_between(
        renderer,
        "fn publication_export_resource_estimate(",
        "\nfn finish_publication_export_error_scopes",
    );

    for required_guard in [
        "checked_mul(config.tile_dimensions",
        "checked_sub(tile_y)",
        "checked_sub(tile_x)",
        "checked_add(tile_row_bytes)",
        ".get_mut(destination_start..destination_end)",
    ] {
        assert!(
            offscreen.contains(required_guard),
            "tile composition must reject hostile offset arithmetic with `{required_guard}`"
        );
    }
    for required_resource in [
        "resolve_color_bytes",
        "msaa_color_bytes",
        "opaque_depth_bytes",
        "transparent_depth_bytes",
        "depth_replay_color_bytes",
        "tile_rgba_bytes",
        "cpu_encoder_reserve_bytes",
    ] {
        assert!(
            estimate.contains(required_resource),
            "admission estimate must account for `{required_resource}`"
        );
    }
}

#[test]
fn recipe_validation_rejects_malformed_render_2_sampling_and_tile_metadata() {
    let recipe = include_str!("../src/export_recipe.rs");

    for required_guard in [
        "requested_samples != 4",
        "selected_samples != 4",
        "selected_samples != 1",
        "tile_layout[0] == 0",
        "tile_layout[1] == 0",
        "tile_dimensions[0] == 0",
        "tile_dimensions[1] == 0",
        "tile_overlap_pixels != 0",
    ] {
        assert!(
            recipe.contains(required_guard),
            "recipe validation must actively reject hostile RENDER-2 metadata (`{required_guard}`)"
        );
    }
    assert!(
        recipe.contains("EXPORT_RECIPE_SCHEMA_VERSION: u32 = 9"),
        "RELEASE-2 extends the combined rendering recipe with framing and cell-line contrast, so the baseline is schema v9"
    );
}

#[test]
fn publication_pipelines_are_created_once_before_tiling() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let export = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    fn render_offscreen_tile(",
    );
    let tile = source_between(
        renderer,
        "fn render_offscreen_tile(",
        "\n    /// Clear volumetric pipelines",
    );

    let pipeline_creations: Vec<_> = export.match_indices("PublicationPipelines::new(").collect();
    let tile_loop = export
        .find("for tile_row in")
        .expect("publication export must iterate tiles");
    assert!(
        pipeline_creations.len() == 1,
        "publication export must create exactly one pipeline set, found {}",
        pipeline_creations.len()
    );
    assert!(
        pipeline_creations[0].0 < tile_loop,
        "publication pipelines must be created before the tile loop"
    );
    assert!(
        tile.contains("publication_pipelines: &PublicationPipelines"),
        "each tile must receive the shared publication pipelines"
    );

    let mut pipeline_factories = Vec::new();
    let mut remainder = tile;
    while let Some(offset) = remainder.find("create_") {
        let candidate = &remainder[offset..];
        let identifier_len = candidate
            .bytes()
            .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            .count();
        let identifier = &candidate[..identifier_len];
        if identifier.ends_with("_pipeline")
            && candidate[identifier_len..].trim_start().starts_with('(')
        {
            pipeline_factories.push(identifier);
        }
        remainder = &candidate["create_".len()..];
    }
    assert!(
        pipeline_factories.is_empty(),
        "tile rendering must not create a pipeline: {pipeline_factories:?}"
    );
}
