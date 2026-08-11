//! LOOK-2 adversarial publication-look admission gates.
//!
//! These tests intentionally inspect source contracts because the publication
//! render path requires a window-backed GPU. They must be RED until the fixed
//! profile system, publication-only shading, midpoint bonds, and recipe v9 are
//! implemented together.

use std::path::PathBuf;

fn source(relative_path: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(relative_path)
        .try_exists()
        .ok()
        .filter(|exists| *exists)
        .and_then(|_| {
            std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path))
                .ok()
        })
        .unwrap_or_default()
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

#[test]
fn fixed_profiles_are_explicit_and_reject_hostile_parameters() {
    let look = source("src/renderer/publication_look.rs");

    for required in [
        "pub enum PublicationLookProfileId",
        "ScientificGloss",
        "Studio",
        "Unlit",
        "pub struct PublicationLookProfile",
        "pub struct PublicationLookUniform",
        "pub fn validate",
        "is_finite",
        "normalize",
        "ambient",
        "roughness",
        "specular",
        "exposure",
        "opacity",
        "cell_line_width_pixels",
        "return Err",
    ] {
        assert!(
            look.contains(required),
            "LOOK-2 must expose fixed, validated publication profiles; missing {required:?}"
        );
    }

    assert!(
        !look.contains("MaterialNode")
            && !look.contains("RenderGraph")
            && !look.contains("PipelineCache")
            && !look.contains("async "),
        "LOOK-2 must reject a general material/render-graph/cache/async expansion"
    );
}

#[test]
fn unlit_is_a_strict_profile_not_a_dimmed_lit_mode() {
    let look = source("src/renderer/publication_look.rs");
    let atom_shader = source("shaders/impostor_sphere.wgsl");
    let bond_shader = source("shaders/bond_cylinder.wgsl");

    for required in [
        "Unlit",
        "DepthEnhancement::Disabled",
        "ToneMapping::Disabled",
        "BondColorMode",
    ] {
        assert!(
            look.contains(required),
            "Unlit must explicitly disable every visual modulation; missing {required:?}"
        );
    }
    for shader in [&atom_shader, &bond_shader] {
        assert!(
            shader.contains("fs_publication")
                && shader.contains("unlit")
                && shader.contains("srgb_to_linear"),
            "publication shaders must have an explicit Unlit branch operating on declared sRGB base colors"
        );
    }
}

#[test]
fn recipe_v9_serializes_the_complete_validated_look_snapshot() {
    let recipe = source("src/export_recipe.rs");

    assert!(
        recipe.contains("EXPORT_RECIPE_SCHEMA_VERSION: u32 = 10"),
        "RELEASE-2 adds reproducible framing and cell-line contrast to the LOOK-2 recipe, so the combined baseline is schema v9"
    );
    for required in [
        "PublicationLookRecipe",
        "profile_id",
        "profile_version",
        "key_direction",
        "fill_direction",
        "rim_direction",
        "ambient",
        "roughness",
        "specular",
        "opacity",
        "exposure",
        "tone_mapping",
        "input_color_space",
        "output_color_space",
        "bond_color_mode",
        "cell_line_width_pixels",
        "depth_enhancement",
        "look_profile",
    ] {
        assert!(
            recipe.contains(required),
            "recipe v9 must serialize the effective publication look, not only a profile name; missing {required:?}"
        );
    }
    assert!(
        recipe.contains("validate") && recipe.contains("is_finite"),
        "recipe parsing must reject a forged non-finite or incomplete look snapshot"
    );
}

#[test]
fn publication_shaders_take_profile_uniforms_and_use_projection_correct_rays() {
    let atom_shader = source("shaders/impostor_sphere.wgsl");
    let bond_shader = source("shaders/bond_cylinder.wgsl");
    let look = source("src/renderer/publication_look.rs");

    for required in [
        "PublicationLookUniform",
        "fs_publication",
        "key_direction",
        "fill_direction",
        "rim_direction",
        "tone_map_publication",
        "srgb_to_linear",
    ] {
        assert!(
            atom_shader.contains(required),
            "atom publication shading must consume the selected profile rather than hard-code {required:?}"
        );
    }
    let uniform_builder = source_between(&look, "impl PublicationLookUniform", "\n}\n");
    for (cpu_field, shader_use) in [
        ("profile.roughness", "look.material.y"),
        ("profile.specular", "look.material.z"),
        ("profile.exposure", "exposure_tone_unlit_projection.x"),
    ] {
        assert!(
            uniform_builder.contains(cpu_field)
                && atom_shader.contains(shader_use)
                && bond_shader.contains(shader_use),
            "{cpu_field} must travel through PublicationLookUniform and be consumed by both publication shaders"
        );
    }
    assert!(
        atom_shader.contains("exposure_tone_unlit_projection.w")
            && atom_shader.contains("select(orthographic_ray")
            && atom_shader.contains("normalize(ray_pos)")
            && atom_shader.contains("vec3<f32>(0.0, 0.0, -1.0)"),
        "sphere intersections must use a camera-origin ray for perspective and a parallel ray for orthographic projection"
    );
    for required in [
        "PublicationLookUniform",
        "fs_publication",
        "key_direction",
        "srgb_to_linear",
    ] {
        assert!(
            bond_shader.contains(required),
            "bond publication shading must consume the selected profile rather than hard-code {required:?}"
        );
    }
}

#[test]
fn selected_profile_is_a_single_snapshot_for_recipe_and_render() {
    let recipe = source("src/export_recipe.rs");
    let renderer = source("src/renderer/renderer.rs");
    let command = source("src/commands/file_io.rs");
    let recipe_builder = source_between(
        &recipe,
        "pub fn from_current_scene(",
        "\n    pub fn validate(&self)",
    );
    let command_signature = source_between(&command, "pub fn export_image(", ") -> IpcResult");

    assert!(
        recipe.contains("look_profile: PublicationLookProfile")
            && renderer.contains("look_profile: PublicationLookProfile"),
        "recipe and render configuration must receive one already-resolved fixed profile, not select defaults independently"
    );
    assert!(
        command_signature.contains("publication_profile")
            && !command_signature.contains("PublicationLookProfileId::ScientificGloss")
            && command.contains("PublicationLookProfile::for_id(")
            && command.contains("publication_render_config_with_profile("),
        "the export command must accept a selected profile rather than hard-code Scientific Gloss, then pass one resolved snapshot to both the sidecar and renderer"
    );
    assert!(
        !recipe_builder.contains("PublicationLookProfileId::ScientificGloss"),
        "recipe construction must not silently replace the selected profile with Scientific Gloss"
    );
}

#[test]
fn publication_bond_builder_rejects_malformed_and_non_finite_scene_data_without_panicking() {
    use crystal_canvas::crystal_state::CrystalState;
    use crystal_canvas::renderer::instance::build_publication_bond_instances;
    use crystal_canvas::renderer::publication_look::PublicationBondColorMode;
    use crystal_canvas::settings::AppSettings;

    let settings = AppSettings::default();
    let mut malformed = CrystalState::default();
    malformed.cart_positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
    malformed.atomic_numbers = vec![6];
    malformed.elements = vec!["C".to_owned()];
    let malformed_result = std::panic::catch_unwind(|| {
        build_publication_bond_instances(
            &malformed,
            &settings,
            PublicationBondColorMode::ByElements,
        )
    });
    assert!(
        malformed_result.is_ok(),
        "parallel-array corruption must be rejected as an error, never panic through an unchecked index"
    );
    assert!(
        malformed_result.unwrap().is_err(),
        "parallel-array corruption must not produce a partial publication bond scene"
    );

    let mut non_finite = CrystalState::default();
    non_finite.cart_positions = vec![[0.0, 0.0, 0.0], [f32::NAN, 0.0, 0.0]];
    non_finite.atomic_numbers = vec![6, 6];
    non_finite.elements = vec!["C".to_owned(), "C".to_owned()];
    assert!(
        build_publication_bond_instances(
            &non_finite,
            &settings,
            PublicationBondColorMode::ByElements,
        )
        .is_err(),
        "NaN coordinates must reject the export instead of silently dropping a bond"
    );

    let mut extreme = CrystalState::default();
    extreme.cart_positions = vec![[0.0, 0.0, 0.0], [f32::MAX, 0.0, 0.0]];
    extreme.atomic_numbers = vec![6, 6];
    extreme.elements = vec!["C".to_owned(), "C".to_owned()];
    assert!(
        build_publication_bond_instances(
            &extreme,
            &settings,
            PublicationBondColorMode::ByElements,
        )
        .is_err(),
        "overflowing Cartesian distances must reject rather than silently alter publication topology"
    );

    let mut hostile_settings = AppSettings::default();
    hostile_settings.bond_tolerance = f32::NAN;
    let mut finite = CrystalState::default();
    finite.cart_positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
    finite.atomic_numbers = vec![6, 6];
    finite.elements = vec!["C".to_owned(), "C".to_owned()];
    assert!(
        build_publication_bond_instances(
            &finite,
            &hostile_settings,
            PublicationBondColorMode::ByElements,
        )
        .is_err(),
        "non-finite bond settings must reject rather than silently suppress all bonds"
    );

    let empty = CrystalState::default();
    assert!(
        build_publication_bond_instances(&empty, &settings, PublicationBondColorMode::ByElements)
            .unwrap()
            .is_empty(),
        "an empty structure must remain an empty publication scene without allocating phantom bonds"
    );
}

#[test]
fn bond_modes_are_exact_midpoint_splits_and_reject_numerical_disasters() {
    let instance = source("src/renderer/instance.rs");

    for required in [
        "PublicationBondColorMode",
        "Uniform",
        "ByElements",
        "split_publication_bond",
        "midpoint",
        "is_finite",
        "try_reserve",
        "element_color(",
    ] {
        assert!(
            instance.contains(required),
            "LOOK-2 bond preparation must defend exact midpoint topology under hostile input; missing {required:?}"
        );
    }
    assert!(
        instance.contains("start: midpoint")
            && instance.contains("end: midpoint")
            && instance.contains("BondInstance"),
        "By Elements must produce two cylinders sharing one exact Cartesian midpoint without a gap or overlap"
    );
}

#[test]
fn look_resources_are_bound_once_before_tiling_and_never_created_per_tile() {
    let renderer = source("src/renderer/renderer.rs");
    let export = source_between(
        &renderer,
        "pub(crate) fn render_offscreen(",
        "\n    fn render_offscreen_tile(",
    );
    let tile = source_between(
        &renderer,
        "fn render_offscreen_tile(",
        "\n    /// Clear volumetric pipelines",
    );
    let tile_loop = export
        .find("for tile_row in")
        .expect("publication export must retain deterministic tiling");

    for label in [
        "Publication Look Uniform Buffer",
        "Publication Look Bind Group",
    ] {
        let creation = export
            .find(label)
            .unwrap_or_else(|| panic!("LOOK-2 must create {label} before tile rendering"));
        assert!(
            creation < tile_loop,
            "{label} must be created once before the tile loop, not per tile"
        );
        assert!(
            !tile.contains(label),
            "tile rendering must not create {label}"
        );
    }
    assert!(
        tile.contains("set_bind_group(1"),
        "every publication atom/bond pass must bind the shared look uniform"
    );
}

#[test]
fn endpoint_split_bond_resources_are_admission_bound_before_gpu_allocation() {
    let renderer = source("src/renderer/renderer.rs");
    let export = source_between(
        &renderer,
        "pub(crate) fn render_offscreen(",
        "\n    fn render_offscreen_tile(",
    );
    let estimate = source_between(
        &renderer,
        "fn publication_export_resource_estimate(",
        "\nfn finish_publication_export_error_scopes",
    );

    for required in [
        "publication_bond_instance_count",
        "publication_bond_bytes",
        "std::mem::size_of::<crate::renderer::instance::BondInstance>()",
    ] {
        assert!(
            renderer.contains(required) || estimate.contains(required),
            "endpoint-split bonds must be counted and byte-budgeted in the receipt before GPU allocation; missing {required:?}"
        );
    }
    let count_validation = export
        .find("u32::try_from(config.publication_bond_instances.len())")
        .expect("endpoint bond count must be range-checked");
    let allocation = export
        .find("Publication Element Bond Instance Buffer")
        .expect("publication By Elements rendering must identify its vertex buffer");
    assert!(
        count_validation < allocation,
        "endpoint bond count must be checked before its GPU buffer is allocated"
    );
}

#[test]
fn endpoint_split_bonds_reuse_the_effective_atom_color_policy() {
    let instance = source("src/renderer/instance.rs");
    let publication_builder = source_between(
        &instance,
        "pub fn build_publication_bond_instances_with_count(",
        "\npub struct RenderLineScene",
    );

    assert!(
        publication_builder.contains("effective_element_color(settings")
            && instance.contains("fn effective_element_color(")
            && instance.contains("custom_atom_colors"),
        "By Elements bond endpoints must use the same custom atom-color policy serialized in the recipe"
    );
}

#[test]
fn publication_pipeline_contract_preserves_render_2_and_excluded_domain_rejection() {
    let renderer = source("src/renderer/renderer.rs");
    let pipeline = source("src/renderer/pipeline.rs");

    for required in [
        "PublicationLookUniform",
        "Publication Look Bind Group Layout",
        "create_publication",
        "selected_samples",
    ] {
        assert!(
            pipeline.contains(required) || renderer.contains(required),
            "LOOK-2 must add publication-only pipeline support without replacing RENDER-2 sampling; missing {required:?}"
        );
    }
    for rejection in [
        "MeasurementOverlays",
        "HoppingOverlays",
        "Isosurface",
        "Volume",
        "PhononPresentation",
        "BrillouinZone",
        "SelectionHighlights",
        "WannierOverlay",
    ] {
        assert!(
            renderer.contains(rejection),
            "LOOK-2 must retain active rejection for excluded domain {rejection}"
        );
    }
}
