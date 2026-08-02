use crystal_canvas::export_recipe::{
    EXPORT_RECIPE_SCHEMA, EXPORT_RECIPE_SCHEMA_VERSION, ExportRecipeKind, PublicationLookRecipe,
    PublicationRasterRecipe, RecipeArtifact, RecipeCamera, RecipeCodec, RecipeColorProfile,
    RecipeMaterials, RecipeOutput, RecipeRendering, RecipeScene, RecipeSource,
    parse_publication_recipe, publication_sidecar_path, write_publication_raster_pair,
};
use crystal_canvas::renderer::publication_look::{
    PublicationLookProfile, PublicationLookProfileId,
};
use crystal_canvas::renderer::renderer::{
    PublicationExportLimits, PublicationExportRequest, evaluate_publication_export_admission,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn valid_recipe() -> PublicationRasterRecipe {
    valid_recipe_for(1, 1)
}

fn valid_recipe_for(width: u32, height: u32) -> PublicationRasterRecipe {
    let look_profile = PublicationLookRecipe::from_profile(
        PublicationLookProfile::for_id(PublicationLookProfileId::ScientificGloss).unwrap(),
    );
    let publication_admission = evaluate_publication_export_admission(
        PublicationExportRequest {
            width,
            height,
            publication_bond_instance_count: 0,
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
        },
        PublicationExportLimits {
            max_texture_dimension_2d: 8192,
            max_buffer_size: 256 * 1024 * 1024,
            publication_msaa_x4: true,
        },
    )
    .unwrap();

    PublicationRasterRecipe {
        schema: EXPORT_RECIPE_SCHEMA.to_owned(),
        schema_version: EXPORT_RECIPE_SCHEMA_VERSION,
        kind: ExportRecipeKind::PublicationRaster,
        application_version: "0.7.0-test".to_owned(),
        generated_at_unix_ms: 1_784_000_000_000,
        success: true,
        source: RecipeSource {
            structure_name: "NaCl fixture".to_owned(),
            source_version: 7,
            intrinsic_atom_count: 2,
            structure_hash: Some("0".repeat(64)),
            structure_hash_algorithm: Some("sha256-canonical-crystal-state-v1".to_owned()),
            source_length_unit: "angstrom".to_owned(),
            coordinate_space: "cartesian_right_handed_y_up".to_owned(),
        },
        camera: RecipeCamera {
            eye: [0.0, 10.0, 30.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            projection: "orthographic".to_owned(),
            fovy_deg: 45.0,
            orthographic_scale: 30.0,
            znear: 0.1,
            zfar: 200.0,
            aspect_policy: "fit_visible_structure_to_export_aspect_with_margin_v1".to_owned(),
            fit_visible_structure_to_export: true,
            publication_framing_margin: 0.08,
        },
        scene: RecipeScene {
            atoms: true,
            bonds: true,
            unit_cell: true,
            measurements: false,
            hoppings: false,
            isosurface: false,
            volume: false,
            stable_periodic_image_policy: "current_renderer_visible_images".to_owned(),
        },
        materials: RecipeMaterials {
            material_profile: look_profile.profile_id.clone(),
            look_profile,
            atom_radius_policy: "mapped_covalent_radius_angstrom_scaled".to_owned(),
            atom_radius_scale: 1.0,
            bond_tolerance: 0.45,
            bond_radius: 0.08,
            radius_length_unit: "angstrom".to_owned(),
            bond_color_rgba: [0.65, 0.65, 0.65, 1.0],
            custom_atom_colors_rgba: BTreeMap::from([
                ("Cl".to_owned(), [0.0, 1.0, 0.0, 1.0]),
                ("Na".to_owned(), [0.0, 0.0, 1.0, 1.0]),
            ]),
            color_value_space: "sRGB_straight_alpha".to_owned(),
            cell_line_color_rgba: [0.20, 0.28, 0.40, 1.0],
        },
        rendering: RecipeRendering {
            lighting_policy: "publication_profile_v1".to_owned(),
            ssao: "disabled".to_owned(),
            shadows: "disabled".to_owned(),
            requested_samples: 4,
            selected_samples: 4,
            selected_capabilities: vec![
                "msaa_x4".to_owned(),
                "depth32float_msaa_x4".to_owned(),
                "rgba8_readback".to_owned(),
            ],
            fallback_policy: "fallback_4x_to_1x_on_unsupported_active_format".to_owned(),
            applied_fallbacks: Vec::new(),
            adapter_name: "test adapter".to_owned(),
            backend: "test backend".to_owned(),
            device_type: "test device".to_owned(),
            render_target_format: "Bgra8UnormSrgb".to_owned(),
            max_texture_dimension_2d: 8192,
            max_buffer_size: 256 * 1024 * 1024,
            max_storage_buffer_size: 128 * 1024 * 1024,
            supports_compute_shaders: true,
            publication_admission,
            field_scene: None,
        },
        output: RecipeOutput {
            width,
            height,
            raster_format: "png".to_owned(),
            bit_depth_per_channel: 8,
            color_space: "sRGB".to_owned(),
            color_profile: RecipeColorProfile::srgb().unwrap(),
            requested_background: "transparent".to_owned(),
            effective_background: "transparent".to_owned(),
            effective_background_rgba_linear: [0.0, 0.0, 0.0, 0.0],
            readback_alpha_policy: "premultiplied".to_owned(),
            encoded_alpha_policy: "straight".to_owned(),
            codec: RecipeCodec::Png {
                compression: "balanced".to_owned(),
                filter: "adaptive".to_owned(),
            },
            tile_layout: [width.div_ceil(8192), height.div_ceil(8192)],
            tile_dimensions: [width.min(8192), height.min(8192)],
            tile_overlap_pixels: 0,
        },
        artifact: Some(RecipeArtifact {
            file_name: "figure.png".to_owned(),
            sha256: "0".repeat(64),
        }),
    }
}

#[test]
fn sidecar_path_is_a_sibling_with_the_versioned_recipe_suffix() {
    assert_eq!(
        publication_sidecar_path(std::path::Path::new("/tmp/figure.png")).unwrap(),
        std::path::Path::new("/tmp/figure.crystalcanvas.json")
    );
    assert_eq!(
        publication_sidecar_path(std::path::Path::new("figure.final.jpeg")).unwrap(),
        std::path::Path::new("figure.final.crystalcanvas.json")
    );
}

#[test]
fn validation_rejects_unknown_versions_non_finite_values_and_missing_units() {
    let mut unknown_version = valid_recipe();
    unknown_version.schema_version += 1;
    let bytes = serde_json::to_vec(&unknown_version).unwrap();
    assert!(
        parse_publication_recipe(&bytes)
            .unwrap_err()
            .contains("unsupported export recipe schema version")
    );

    let mut non_finite = valid_recipe();
    non_finite.camera.eye[0] = f32::NAN;
    assert!(non_finite.validate().unwrap_err().contains("camera"));

    let mut missing_source_unit = valid_recipe();
    missing_source_unit.source.source_length_unit.clear();
    assert!(
        missing_source_unit
            .validate()
            .unwrap_err()
            .contains("source units")
    );

    let mut missing_radius_unit = valid_recipe();
    missing_radius_unit.materials.radius_length_unit.clear();
    assert!(
        missing_radius_unit
            .validate()
            .unwrap_err()
            .contains("radius length unit")
    );
}

#[test]
fn validation_rejects_forged_fixed_profile_snapshots() {
    let mut forged_roughness = valid_recipe();
    forged_roughness.materials.look_profile.roughness = 0.73;
    assert!(
        forged_roughness.validate().is_err(),
        "a bounded but non-preset roughness must not masquerade as Scientific Gloss"
    );

    let mut forged_bond_mode = valid_recipe();
    forged_bond_mode.materials.look_profile.bond_color_mode = "uniform".to_owned();
    assert!(
        forged_bond_mode.validate().is_err(),
        "a fixed profile id must determine its bond-color policy"
    );

    let mut forged_direction = valid_recipe();
    forged_direction.materials.look_profile.key_direction = [0.0, 1.0, 0.0];
    assert!(
        forged_direction.validate().is_err(),
        "a normalized but substituted light direction must not be accepted as a fixed profile"
    );

    let mut forged_cell_line_color = valid_recipe();
    forged_cell_line_color.materials.cell_line_color_rgba = [1.0, 0.0, 1.0, 1.0];
    assert!(
        forged_cell_line_color.validate().is_err(),
        "the recorded cell-line color must be derived from the effective background"
    );
}

#[test]
fn validation_rejects_zero_dimensions_and_inconsistent_alpha_contracts() {
    let mut zero_width = valid_recipe();
    zero_width.output.width = 0;
    assert!(
        zero_width
            .validate()
            .unwrap_err()
            .contains("dimensions must be non-zero")
    );

    let mut invalid_alpha = valid_recipe();
    invalid_alpha.output.encoded_alpha_policy = "premultiplied".to_owned();
    assert!(
        invalid_alpha
            .validate()
            .unwrap_err()
            .contains("alpha policy")
    );
}

#[test]
fn validation_rejects_tampered_v9_plan_capability_and_fallback_metadata() {
    let recipe = valid_recipe_for(8193, 1);
    assert_eq!(recipe.output.tile_layout, [2, 1]);
    assert_eq!(recipe.output.tile_dimensions, [8192, 1]);

    let mut overlap = recipe.clone();
    overlap.output.tile_overlap_pixels = 1;
    assert!(overlap.validate().unwrap_err().contains("tile metadata"));

    let mut capabilities = recipe.clone();
    capabilities.rendering.selected_capabilities.clear();
    assert!(
        capabilities
            .validate()
            .unwrap_err()
            .contains("rendering policy")
    );

    let mut false_fallback = recipe.clone();
    false_fallback
        .rendering
        .applied_fallbacks
        .push("msaa_x4_unavailable".to_owned());
    assert!(
        false_fallback
            .validate()
            .unwrap_err()
            .contains("rendering policy")
    );

    let mut detached_plan = recipe;
    detached_plan.output.tile_dimensions = [4096, 1];
    detached_plan.output.tile_layout = [3, 1];
    assert!(
        detached_plan
            .validate()
            .unwrap_err()
            .contains("admission plan")
    );
}

#[test]
fn admission_receipt_serializes_the_complete_v7_policy_and_rejects_tampering() {
    let recipe = valid_recipe();
    let value = serde_json::to_value(&recipe).unwrap();
    let admission = &value["rendering"]["publication_admission"];

    assert_eq!(admission["policy_version"], 7);
    assert_eq!(admission["request"]["width"], 1);
    assert_eq!(admission["request"]["publication_bond_instance_count"], 0);
    assert_eq!(admission["estimate"]["publication_bond_bytes"], 0);
    assert_eq!(admission["request"]["has_measurement_state"], false);
    assert_eq!(
        admission["budgets"]["cpu_encoder_reserve_bytes"],
        16 * 1024 * 1024
    );
    assert_eq!(admission["budgets"]["encoded_overhead_bytes"], 1024 * 1024);
    assert_eq!(admission["budgets"]["max_recipe_bytes"], 1024 * 1024);
    assert_eq!(
        admission["estimate"]["peak_cpu_bytes"],
        admission["estimate"]["jpeg_encode_peak_cpu_bytes"]
    );

    for pointer in [
        "/rendering/publication_admission/policy_version",
        "/rendering/publication_admission/request/width",
        "/rendering/publication_admission/budgets/cpu_encoder_reserve_bytes",
    ] {
        let mut tampered = value.clone();
        let field = tampered.pointer_mut(pointer).unwrap();
        *field = serde_json::json!(field.as_u64().unwrap() + 1);
        let error = parse_publication_recipe(&serde_json::to_vec(&tampered).unwrap()).unwrap_err();
        assert!(
            error.contains("receipt") || error.contains("policy") || error.contains("dimensions"),
            "tampered field {pointer} was rejected for an unexpected reason: {error}"
        );
    }
}

#[test]
fn publication_encoding_streams_to_a_bounded_file_instead_of_an_encoded_vec() {
    let source = include_str!("../src/export_recipe.rs");
    assert!(source.contains("BoundedFileWriter"));
    assert!(source.contains("encode_raster_to_staged_file"));
    assert!(!source.contains("BoundedImageWriter"));
    assert!(!source.contains("Cursor<Vec<u8>>"));
    assert!(!source.contains("try_reserve_exact(max_len)"));
    assert!(source.contains("serde_json::to_writer_pretty"));
    assert!(!source.contains("serde_json::to_vec_pretty(&recipe)"));
}

#[test]
fn fixed_recipe_serialization_is_deterministic() {
    let recipe = valid_recipe();
    let first = serde_json::to_vec_pretty(&recipe).unwrap();
    let second = serde_json::to_vec_pretty(&recipe).unwrap();
    assert_eq!(first, second);

    let cl_position = first
        .windows(4)
        .position(|window| window == br#""Cl""#)
        .unwrap();
    let na_position = first
        .windows(4)
        .position(|window| window == br#""Na""#)
        .unwrap();
    assert!(
        cl_position < na_position,
        "custom colors must use sorted keys"
    );
}

#[test]
fn paired_write_creates_a_hash_bound_image_and_sidecar() {
    let directory = tempfile::tempdir().unwrap();
    let image_path = directory.path().join("figure.png");
    let input_premultiplied_rgba = vec![64, 0, 0, 128];

    let recipe_path =
        write_publication_raster_pair(&image_path, input_premultiplied_rgba, valid_recipe())
            .unwrap();

    assert!(image_path.is_file());
    assert!(recipe_path.is_file());
    assert_eq!(
        recipe_path,
        directory.path().join("figure.crystalcanvas.json")
    );

    let image_bytes = std::fs::read(&image_path).unwrap();
    let parsed = parse_publication_recipe(&std::fs::read(&recipe_path).unwrap()).unwrap();
    let artifact = parsed.artifact.unwrap();
    assert_eq!(artifact.file_name, "figure.png");
    assert_eq!(
        artifact.sha256,
        format!("{:x}", Sha256::digest(&image_bytes))
    );

    let decoded = image::load_from_memory_with_format(&image_bytes, image::ImageFormat::Png)
        .unwrap()
        .to_rgba8();
    assert_eq!(
        decoded.get_pixel(0, 0).0,
        [90, 0, 0, 128],
        "transparent PNG must encode straight alpha"
    );
}

#[test]
fn transparent_jpeg_is_explicitly_composited_onto_white() {
    let directory = tempfile::tempdir().unwrap();
    let image_path = directory.path().join("figure.jpg");
    let mut recipe = valid_recipe_for(8, 8);
    recipe.output.raster_format = "jpeg".to_owned();
    recipe.output.effective_background = "white".to_owned();
    recipe.output.effective_background_rgba_linear = [1.0, 1.0, 1.0, 1.0];
    recipe.materials.cell_line_color_rgba = [0.18, 0.22, 0.28, 1.0];
    recipe.output.encoded_alpha_policy = "none".to_owned();
    recipe.output.codec = RecipeCodec::Jpeg {
        quality: 95,
        chroma_subsampling: "4:4:4".to_owned(),
    };
    let input = [64, 0, 0, 128].repeat(64);

    write_publication_raster_pair(&image_path, input, recipe).unwrap();

    let decoded = image::open(&image_path).unwrap().to_rgb8();
    let pixel = decoded.get_pixel(4, 4).0;
    for (actual, expected) in pixel.into_iter().zip([196_u8, 187, 187]) {
        assert!(
            actual.abs_diff(expected) <= 4,
            "JPEG white composition channel {actual} differs from {expected}"
        );
    }
}

#[test]
fn existing_primary_or_sidecar_is_never_silently_overwritten() {
    let primary_directory = tempfile::tempdir().unwrap();
    let primary_path = primary_directory.path().join("figure.png");
    std::fs::write(&primary_path, b"existing image").unwrap();

    let error =
        write_publication_raster_pair(&primary_path, vec![0, 0, 0, 0], valid_recipe()).unwrap_err();
    assert!(error.contains("already exists"));
    assert_eq!(std::fs::read(&primary_path).unwrap(), b"existing image");
    assert!(
        !primary_directory
            .path()
            .join("figure.crystalcanvas.json")
            .exists()
    );

    let sidecar_directory = tempfile::tempdir().unwrap();
    let image_path = sidecar_directory.path().join("figure.png");
    let recipe_path = sidecar_directory.path().join("figure.crystalcanvas.json");
    std::fs::write(&recipe_path, b"existing recipe").unwrap();

    let error =
        write_publication_raster_pair(&image_path, vec![0, 0, 0, 0], valid_recipe()).unwrap_err();
    assert!(error.contains("already exists"));
    assert_eq!(std::fs::read(&recipe_path).unwrap(), b"existing recipe");
    assert!(!image_path.exists());
}
