//! EXPORT-1B adversarial contract tests.
//!
//! These tests are deliberately source-level: publication rendering needs a
//! window-backed GPU, while the failure modes below must be rejected on every
//! supported host before a device allocation is attempted.

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

fn command_body<'a>(source: &'a str, command: &str) -> &'a str {
    source_between(source, &format!("pub fn {command}("), "\n#[tauri::command]")
}

#[test]
fn export_uses_a_typed_validated_render_config_instead_of_a_stringly_gpu_boundary() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let file_io = include_str!("../src/commands/file_io.rs");
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    /// Clear volumetric pipelines",
    );
    let export_image = command_body(file_io, "export_image");

    assert!(
        renderer.contains("struct PublicationRenderConfig"),
        "EXPORT-1B must make dimensions, target format, background, alpha semantics, and the admission receipt one validated render configuration"
    );
    assert!(
        renderer.contains("struct PublicationRenderResult"),
        "the renderer must return declared pixel semantics rather than an untyped byte vector"
    );
    assert!(
        !offscreen.contains("bg_mode: &str"),
        "an arbitrary string must not cross into the GPU render boundary"
    );
    assert!(
        !offscreen.contains("Result<Vec<u8>, String>"),
        "a raw Vec loses the alpha, channel-order, and row-order contract before encoding"
    );
    assert!(
        export_image.contains("PublicationRenderConfig")
            && export_image.contains("render_offscreen(&publication_config)"),
        "IPC must construct one validated publication config and pass that exact config to the renderer"
    );
}

#[test]
fn export_draw_list_cannot_leak_rejected_nonstructural_overlays_after_admission() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    /// Clear volumetric pipelines",
    );

    for forbidden_draw_state in [
        "measurement_line_count",
        "measurement_line_buffer",
        "hopping_instance_count",
        "hopping_instance_buffer",
        "show_hoppings",
        "isosurface_pipeline",
        "volume_raycast_pipeline",
    ] {
        assert!(
            !offscreen.contains(forbidden_draw_state),
            "publication render path still contains rejected non-structural state `{forbidden_draw_state}`; admission alone is not a sufficient containment boundary"
        );
    }
}

#[test]
fn readback_unpack_is_a_single_checked_conversion_boundary_for_hostile_row_layouts() {
    let renderer = include_str!("../src/renderer/renderer.rs");

    assert!(
        renderer.contains("fn unpack_publication_readback("),
        "EXPORT-1B must isolate padded-row removal and channel conversion in a testable CPU boundary"
    );
    let unpack = source_between(
        renderer,
        "fn unpack_publication_readback(",
        "\nfn drag_instances(",
    );

    for required_guard in [
        "checked_mul",
        "checked_add",
        "try_reserve_exact",
        "chunks_exact(4)",
        "BGRA -> RGBA",
    ] {
        assert!(
            unpack.contains(required_guard),
            "readback unpack must defend hostile padded rows with `{required_guard}`"
        );
    }
    assert!(
        !unpack.contains("flip"),
        "publication readback must declare one top-to-bottom convention instead of silently flipping rows"
    );
}

#[test]
fn export_camera_and_background_are_snapshot_owned_not_interactive_renderer_mutations() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    /// Clear volumetric pipelines",
    );

    assert!(
        renderer.contains("fn publication_render_config_with_profile(")
            && renderer.contains("PublicationBackground"),
        "all four backgrounds and the export camera snapshot must be resolved before target allocation"
    );
    assert!(
        offscreen.contains("config.camera") && offscreen.contains("config.background"),
        "the offscreen pass must consume the frozen export camera and background, not read mutable interactive values during rendering"
    );
    for forbidden_interactive_mutation in [
        "self.camera.set_aspect",
        "self.clear_color =",
        "self.update_camera()",
        "self.render_config",
    ] {
        assert!(
            !offscreen.contains(forbidden_interactive_mutation),
            "a failed export can corrupt interactive state through `{forbidden_interactive_mutation}`"
        );
    }
}
