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
    let recipe_position = export_image
        .find("PublicationRasterRecipe::from_current_scene")
        .expect("export must build a publication recipe");
    let config_position = export_image
        .find(".publication_render_config_with_profile(")
        .expect("export must derive its render config from the validated receipt");
    let render_position = export_image
        .find(".render_offscreen(&publication_config)")
        .expect("export must render only through the validated publication config");
    assert!(
        recipe_position < config_position && config_position < render_position,
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
        "\nfn publication_export_resource_estimate(",
    );
    let gate = source_between(
        renderer,
        "pub fn evaluate_publication_export_admission(",
        "\npub(crate) fn validate_publication_export_receipt_fields",
    );
    let config = source_between(
        renderer,
        "pub(crate) fn publication_render_config_with_profile(",
        "\n}\n\n#[must_use]",
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
        gate.contains("publication_render_plan(request, limits, field_resources.as_ref())")
            && gate.contains("estimate.staging_bytes > limits.max_buffer_size")
            && gate.contains("budgets.max_readback_bytes"),
        "the gate must derive a field-aware bounded tile plan, then enforce device staging-buffer and CPU-image budgets"
    );
    assert!(
        recipe_builder.contains("evaluate_publication_export_admission(")
            && config.contains("let plan = admission.render_plan")
            && config.contains("self.validate_publication_export_receipt(admission)?"),
        "the export path must consume the receipt-bound render plan before resource allocation"
    );
}

#[test]
fn rejects_a_render_path_that_can_return_after_mutating_the_interactive_camera() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    /// Clear volumetric pipelines",
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
fn field_domains_require_a_frozen_snapshot_while_legacy_publication_still_rejects_them() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let recipe = include_str!("../src/export_recipe.rs");
    let legacy_admission = source_between(
        renderer,
        "pub fn evaluate_publication_export_admission(",
        "\nfn evaluate_publication_export_admission_inner(",
    );
    let field_admission = source_between(
        renderer,
        "pub fn evaluate_field_publication_export_admission(",
        "\npub fn evaluate_publication_export_admission(",
    );
    let recipe_builder = source_between(
        recipe,
        "pub fn from_current_scene(",
        "\n    pub fn validate(&self)",
    );

    assert!(
        renderer.contains("reject_legacy_unadmitted_field(request)?"),
        "legacy publication admission must not silently admit field layers"
    );
    assert!(
        renderer.contains("if field_resources.is_none()"),
        "field admission must fail closed when no frozen GPU-resource snapshot exists"
    );
    assert!(
        renderer.contains("PublicationExportRejection::Isosurface"),
        "unadmitted isosurfaces must remain explicit publication rejections"
    );
    assert!(
        renderer.contains("PublicationExportRejection::Volume"),
        "unadmitted volumes must remain explicit publication rejections"
    );
    assert!(
        legacy_admission
            .contains("evaluate_publication_export_admission_inner(request, limits, None)"),
        "the legacy admission path must provide no field snapshot"
    );
    assert!(
        field_admission.contains("field_publication_resources"),
        "FIGURE-2 admission must consume the frozen field resource snapshot"
    );
    assert!(
        !field_admission.contains("reject_legacy_unadmitted_field"),
        "the explicit FIGURE-2 field path must not reuse the blanket legacy rejection"
    );
    assert!(
        recipe_builder.contains("match field_snapshot.as_ref()")
            && recipe_builder.contains("evaluate_field_publication_export_admission(")
            && recipe_builder.contains("evaluate_publication_export_admission("),
        "recipe generation must choose field admission only when it serializes the same snapshot"
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
        "\n    /// Clear volumetric pipelines",
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
fn publication_export_uses_render_2_sampling_instead_of_the_retired_single_sample_baseline() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let config = source_between(
        renderer,
        "pub(crate) fn publication_render_config_with_profile(",
        "\n}\n\n#[must_use]",
    );
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    /// Clear volumetric pipelines",
    );

    assert!(
        config.contains("requested_samples")
            && config.contains("selected_samples")
            && !config.contains("sample_count: 1"),
        "RENDER-2 must select publication sample count from the captured GPU capabilities; it must not retain a hard-coded single-sample baseline"
    );
    assert!(
        !offscreen.contains("config.sample_count != 1")
            && offscreen.contains("Publication Resolve Color Texture")
            && offscreen.contains("resolve_target: if config.selected_samples > 1"),
        "RENDER-2 must permit the selected MSAA count and resolve each multisample tile to its single-sample readback target"
    );
}

#[test]
fn publication_export_records_the_versioned_camera_material_and_profile_contract() {
    let file_io = include_str!("../src/commands/file_io.rs");
    let recipe = include_str!("../src/export_recipe.rs");
    let export_image = command_body(file_io, "export_image");

    for required_contract in [
        "PublicationRasterRecipe::from_current_scene",
        "write_publication_raster_pair",
        "PublicationLookProfile",
        "RecipeCamera",
        "RecipeMaterials",
        "PublicationLookRecipe",
    ] {
        assert!(
            export_image.contains(required_contract) || recipe.contains(required_contract),
            "publication export must retain the reproducible `{required_contract}` contract"
        );
    }
}

#[test]
fn publication_alpha_and_channel_order_are_declared_at_the_new_boundaries() {
    let file_io = include_str!("../src/commands/file_io.rs");
    let renderer = include_str!("../src/renderer/renderer.rs");
    let recipe = include_str!("../src/export_recipe.rs");
    let export_image = command_body(file_io, "export_image");
    let unpack = source_between(
        renderer,
        "fn unpack_publication_readback(",
        "\nfn drag_instances(",
    );

    assert!(export_image.contains("ExportImageBackground::Transparent"));
    assert!(unpack.contains("BGRA -> RGBA"));
    assert!(
        renderer.contains("PublicationAlphaMode::Premultiplied")
            && recipe.contains("readback_alpha_policy: \"premultiplied\"")
            && recipe.contains("unpremultiply_rgba(&mut rgba)"),
        "publication readback, recipe metadata, and PNG encoding must agree on premultiplied-to-straight alpha conversion"
    );
}

#[test]
fn publication_quality_contract_is_not_limited_to_pixel_dimensions() {
    let file_io = include_str!("../src/commands/file_io.rs");
    let renderer = include_str!("../src/renderer/renderer.rs");
    let recipe = include_str!("../src/export_recipe.rs");
    let export_image = command_body(file_io, "export_image");
    let offscreen = source_between(
        renderer,
        "pub(crate) fn render_offscreen(",
        "\n    /// Clear volumetric pipelines",
    );

    assert!(export_image.contains("width") && export_image.contains("height"));
    for required_quality_contract in ["look_profile", "bond_color_mode"] {
        assert!(
            offscreen.contains(required_quality_contract)
                || export_image.contains(required_quality_contract)
                || recipe.contains(required_quality_contract),
            "publication export must retain `{required_quality_contract}` independently of image dimensions"
        );
    }
}
