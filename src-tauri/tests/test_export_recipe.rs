use crystal_canvas::export_recipe::{
    EXPORT_RECIPE_SCHEMA, EXPORT_RECIPE_SCHEMA_VERSION, ExportRecipeKind, PublicationRasterRecipe,
    RecipeArtifact, RecipeCamera, RecipeCodec, RecipeColorProfile, RecipeMaterials, RecipeOutput,
    RecipeRendering, RecipeScene, RecipeSource, parse_publication_recipe, publication_sidecar_path,
    write_publication_raster_pair,
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
    let publication_admission = evaluate_publication_export_admission(
        PublicationExportRequest {
            width,
            height,
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
            aspect_policy: "export_dimensions_override_interactive_aspect".to_owned(),
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
            material_profile: "legacy_interactive".to_owned(),
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
            color_value_space: "legacy_renderer_input".to_owned(),
        },
        rendering: RecipeRendering {
            lighting_policy: "legacy_fixed_shader".to_owned(),
            ssao: "disabled".to_owned(),
            shadows: "disabled".to_owned(),
            requested_samples: 1,
            selected_samples: 1,
            selected_capabilities: vec![
                "single_sample_color".to_owned(),
                "rgba8_readback".to_owned(),
            ],
            fallback_policy: "reject_on_render_or_encode_failure".to_owned(),
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
            tile_layout: [1, 1],
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
fn admission_receipt_serializes_the_complete_v4_policy_and_rejects_tampering() {
    let recipe = valid_recipe();
    let value = serde_json::to_value(&recipe).unwrap();
    let admission = &value["rendering"]["publication_admission"];

    assert_eq!(admission["policy_version"], 4);
    assert_eq!(admission["request"]["width"], 1);
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
