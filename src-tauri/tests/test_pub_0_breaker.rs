//! PUB-0 adversarial contracts for publication export.
//!
//! These source-level tests avoid a window and GPU while ensuring hostile input
//! is rejected before offscreen resource allocation. The remaining
//! `baseline_...` tests retain evidence for work that PUB-0 does not yet admit.

use sha2::{Digest, Sha256};

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

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
fn baseline_fixtures_are_hash_pinned_to_existing_3d_validation_inputs() {
    // The publication task reuses these files only as renderer fixtures.  This
    // test does not make a new claim about either material.
    assert_eq!(
        sha256_hex(include_bytes!("../../tests/data/nacl.cif")),
        "cff7516be4caa365d3254c62b5879d611bef7faaa355edd4b9a9c31fc7fa1e7b",
        "NaCl renderer fixture changed; renew its PUB-0 provenance before comparing images"
    );
    assert_eq!(
        sha256_hex(include_bytes!("../../tests/data/rutile.cif")),
        "4315cc29f3ac4df9ceee46827f9f36672250eaae47b9ebbd2380b6ddcd94deb8",
        "rutile renderer fixture changed; renew its PUB-0 provenance before comparing images"
    );
}

#[test]
fn rejects_zero_dimensions_before_offscreen_resource_allocation() {
    let file_io = include_str!("../src/commands/file_io.rs");
    let renderer = include_str!("../src/renderer/renderer.rs");
    let recipe = include_str!("../src/export_recipe.rs");
    let gate = source_between(
        renderer,
        "pub fn evaluate_publication_export_admission(",
        "\npub(crate) fn validate_publication_export_receipt_fields",
    );
    let recipe_builder = source_between(
        recipe,
        "pub fn from_current_scene(",
        "\n    pub fn validate(&self)",
    );
    let export_image = command_body(file_io, "export_image");

    assert!(
        gate.contains("if request.width == 0 || request.height == 0")
            && gate.contains("PublicationExportRejection::ZeroDimensions"),
        "zero dimensions must be explicitly rejected by the publication gate"
    );
    assert!(
        recipe_builder.contains("evaluate_publication_export_admission("),
        "recipe construction must enter the gate before creating an export request"
    );
    assert!(
        export_image
            .find("PublicationRasterRecipe::from_current_scene")
            .expect("export must build a publication recipe")
            < export_image
                .find(
                    ".render_offscreen(&recipe.rendering.publication_admission, bg_mode.as_str())"
                )
                .expect("export must render only after recipe validation"),
        "zero-size requests must fail before offscreen rendering starts"
    );
}

#[test]
fn rejects_extreme_dimensions_before_gpu_or_cpu_resource_meltdown() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let recipe = include_str!("../src/export_recipe.rs");
    let layout = source_between(
        renderer,
        "fn offscreen_readback_layout(",
        "\nfn drag_instances(",
    );
    let gate = source_between(
        renderer,
        "pub fn evaluate_publication_export_admission(",
        "\npub(crate) fn validate_publication_export_receipt_fields",
    );
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    pub fn prepare_volumetric",
    );
    let recipe_builder = source_between(
        recipe,
        "pub fn from_current_scene(",
        "\n    pub fn validate(&self)",
    );

    assert!(
        layout.contains(".checked_mul(4)")
            && layout.contains(".checked_add(alignment - 1)")
            && layout.contains("usize::try_from(staging_size)")
            && layout.contains("u32::try_from(padded_bytes_per_row)"),
        "readback layout must reject arithmetic and address-space overflow"
    );
    assert!(
        gate.contains("request.width > limits.max_texture_dimension_2d")
            && gate.contains("estimate.staging_bytes > limits.max_buffer_size")
            && gate.contains("budgets.max_readback_bytes"),
        "the gate must enforce device texture, staging-buffer, and CPU-image budgets"
    );
    assert!(
        recipe_builder.contains("evaluate_publication_export_admission(")
            && offscreen
                .find("offscreen_readback_layout(width, height).map_err")
                .expect("offscreen renderer must compute a checked layout")
                < offscreen
                    .find("// Choose background clear color")
                    .expect("offscreen renderer must allocate only after layout validation"),
        "the export path must gate dimensions before resource allocation"
    );
}

#[test]
fn rejects_a_render_path_that_can_return_after_mutating_the_interactive_camera() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    pub fn prepare_volumetric",
    );
    if let Some(camera_mutation) = offscreen.find("self.camera.set_aspect") {
        let restore = offscreen[camera_mutation..]
            .find("self.update_camera();")
            .map(|offset| camera_mutation + offset)
            .expect("a mutated interactive camera must have an unconditional restore path");
        assert!(
            !offscreen[camera_mutation..restore].contains('?'),
            "fallible target allocation, command submission, or readback can leave the interactive camera at export aspect; use an export-owned camera or an unconditional cleanup guard"
        );
    }
}

#[test]
fn rejects_publication_exports_with_unrecorded_transient_or_nonstructural_state() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let recipe = include_str!("../src/export_recipe.rs");
    let recipe_builder = source_between(
        recipe,
        "pub fn from_current_scene(",
        "\n    pub fn validate(&self)",
    );
    let admission_surface = format!("{renderer}\n{recipe_builder}");

    for (state_marker, rejected_domain) in [
        (
            "source.measurements",
            "measurement state not reflected by renderer buffers",
        ),
        (
            "source.selected_atoms",
            "selection-highlighted atoms or bonds",
        ),
        ("source.wannier_overlay", "Wannier-generated ghost atoms"),
        ("source.active_phonon_mode", "phonon presentation"),
        (
            "self.phonon_presentation",
            "renderer-owned phonon presentation",
        ),
        ("self.atom_drag", "transient atom-drag preview"),
        ("self.show_bz", "Brillouin-zone viewport replacement"),
    ] {
        assert!(
            admission_surface.contains(state_marker),
            "PUB-0 must actively reject or serialize {rejected_domain}; current recipe claims a structure-only scene"
        );
    }
}

#[test]
fn rejects_out_of_scope_overlays_before_offscreen_rendering() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let recipe = include_str!("../src/export_recipe.rs");
    let gate = source_between(
        renderer,
        "pub(crate) fn publication_export_request(",
        "\n}\n\n#[must_use]",
    );
    let recipe_builder = source_between(
        recipe,
        "pub fn from_current_scene(",
        "\n    pub fn validate(&self)",
    );
    let admission_surface = format!("{gate}\n{renderer}");

    for (active_condition, rejection) in [
        (
            "self.measurement_line_count > 0",
            "publication export currently rejects measurement overlays",
        ),
        ("self.hopping_instance_count > 0", "has_hopping_overlays"),
        ("self.isosurface_pipeline.is_some()", "has_isosurface"),
        ("self.volume_raycast_pipeline.is_some()", "has_volume"),
    ] {
        assert!(
            admission_surface.contains(active_condition) && admission_surface.contains(rejection),
            "PUB-0 must actively reject the excluded export domain `{active_condition}`"
        );
    }
    assert!(
        recipe_builder.contains("evaluate_publication_export_admission("),
        "scope rejection must be reached from the public export path"
    );
}

#[test]
fn rejects_a_readback_only_budget_for_a_full_resolution_export() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let gate = source_between(
        renderer,
        "pub fn evaluate_publication_export_admission(",
        "\npub(crate) fn validate_publication_export_receipt_fields",
    );
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    pub fn prepare_volumetric",
    );

    assert!(
        gate.contains("budgets.max_transient_gpu_bytes")
            && gate.contains("transient_gpu")
            && gate.contains("peak_cpu"),
        "a 192 MiB readback cap still permits roughly 768 MiB of color, two Depth32Float textures, and staging memory at 48 megapixels; admission must enforce checked total GPU and peak CPU budgets"
    );
    assert!(
        offscreen.contains("needs_transparent")
            && offscreen.contains("create_transparent_depth_texture"),
        "the budget must account for the second depth attachment when transparent structure atoms require it"
    );
}

#[test]
fn publication_admission_must_be_executable_without_a_window_or_gpu() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let recipe = include_str!("../src/export_recipe.rs");
    let admission_surface = format!("{renderer}\n{recipe}");

    assert!(
        admission_surface.contains("PublicationExportRequest")
            && admission_surface.contains("PublicationExportAdmissionReceipt")
            && admission_surface.contains("evaluate_publication_export_admission"),
        "source-string checks cannot prove active rejection; expose a pure admission policy and test zero dimensions, 48-megapixel requests, and every excluded scene domain with concrete values"
    );
}

#[test]
fn baseline_offscreen_export_is_single_sample_without_a_capability_checked_fallback() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    pub fn prepare_volumetric",
    );

    assert!(
        offscreen.contains("sample_count: 1"),
        "the existing offscreen color target is the single-sample baseline"
    );
    for unsupported_contract in ["RenderConfig", "fallback", "msaa", "MSAA", "sample_count:"] {
        if unsupported_contract == "sample_count:" {
            continue;
        }
        assert!(
            !offscreen.contains(unsupported_contract),
            "the current offscreen path has no publication sampling capability contract (`{unsupported_contract}`)"
        );
    }
}

#[test]
fn baseline_export_has_no_versioned_recipe_or_explicit_camera_material_contract() {
    let file_io = include_str!("../src/commands/file_io.rs");
    let export_image = command_body(file_io, "export_image");

    for missing_contract in [
        "crystalcanvas.export-recipe",
        "schema_version",
        "sidecar",
        "camera",
        "projection",
        "material",
        "light",
        "profile",
    ] {
        assert!(
            !export_image.contains(missing_contract),
            "the current image IPC does not record `{missing_contract}` for reproducible publication export"
        );
    }
}

#[test]
fn baseline_alpha_comparison_semantics_are_undefined() {
    let file_io = include_str!("../src/commands/file_io.rs");
    let renderer = include_str!("../src/renderer/renderer.rs");
    let export_image = command_body(file_io, "export_image");
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    pub fn prepare_volumetric",
    );

    assert!(export_image.contains("ExportImageBackground::Transparent"));
    assert!(offscreen.contains("BGRA -> RGBA"));
    for unspecified_semantic in ["premultiplied", "straight alpha", "alpha_mode"] {
        assert!(
            !export_image.contains(unspecified_semantic)
                && !offscreen.contains(unspecified_semantic),
            "current export has no declared `{unspecified_semantic}` comparison semantics"
        );
    }
}

#[test]
fn baseline_native_quality_is_not_separable_from_pixel_dimensions() {
    let file_io = include_str!("../src/commands/file_io.rs");
    let renderer = include_str!("../src/renderer/renderer.rs");
    let export_image = command_body(file_io, "export_image");
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    pub fn prepare_volumetric",
    );

    assert!(export_image.contains("width") && export_image.contains("height"));
    for missing_quality_contract in [
        "perceptual_metric",
        "image_comparison_metric",
        "approved_tolerance",
        "publication_profile",
        "scientific_gloss",
        "bond_color_mode",
    ] {
        assert!(
            !offscreen.contains(missing_quality_contract)
                && !export_image.contains(missing_quality_contract),
            "current export cannot distinguish `{missing_quality_contract}` quality acceptance from dimensions alone"
        );
    }
}
