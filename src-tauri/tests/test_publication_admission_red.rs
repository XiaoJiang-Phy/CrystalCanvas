//! Actual hostile-input admission tests for PUB-0 / EXPORT-1A.
//!
//! This test intentionally calls the policy seam without creating a window or
//! GPU device.  It is RED until that seam is exported to integration tests.

use crystal_canvas::renderer::renderer::{
    PublicationExportLimits, PublicationExportRejection, PublicationExportRequest,
    evaluate_publication_export_admission,
};

const TEST_MAX_TEXTURE_DIMENSION: u32 = 8_192;
const TEST_MAX_BUFFER_SIZE: u64 = 256 * 1024 * 1024;

fn test_limits() -> PublicationExportLimits {
    PublicationExportLimits {
        max_texture_dimension_2d: TEST_MAX_TEXTURE_DIMENSION,
        max_buffer_size: TEST_MAX_BUFFER_SIZE,
        publication_msaa_x4: true,
    }
}

fn structure_only_request(width: u32, height: u32) -> PublicationExportRequest {
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
    }
}

fn assert_rejected(request: PublicationExportRequest, expected: PublicationExportRejection) {
    assert_eq!(
        evaluate_publication_export_admission(request, test_limits()),
        Err(expected),
        "hostile publication export input must be rejected for its actual admission reason"
    );
}

#[test]
fn rejects_zero_and_arithmetic_disaster_dimensions_without_a_gpu() {
    assert_rejected(
        structure_only_request(0, 1),
        PublicationExportRejection::ZeroDimensions,
    );
    assert_rejected(
        structure_only_request(1, 0),
        PublicationExportRejection::ZeroDimensions,
    );

    let tiled = evaluate_publication_export_admission(
        structure_only_request(TEST_MAX_TEXTURE_DIMENSION + 1, 1),
        test_limits(),
    )
    .expect("a bounded multi-tile export must not be rejected only for exceeding one texture");
    let tiled_value = serde_json::to_value(tiled).unwrap();
    assert_eq!(tiled_value["render_plan"]["tile_layout"], serde_json::json!([2, 1]));
    assert_eq!(
        tiled_value["render_plan"]["tile_dimensions"],
        serde_json::json!([TEST_MAX_TEXTURE_DIMENSION, 1])
    );

    let overflow = structure_only_request(u32::MAX, 1);
    assert_eq!(
        evaluate_publication_export_admission(
            overflow,
            PublicationExportLimits {
                max_texture_dimension_2d: u32::MAX,
                max_buffer_size: u64::MAX,
                publication_msaa_x4: true,
            },
        ),
        Err(PublicationExportRejection::RowLayoutLimit)
    );
}

#[test]
fn rejects_resource_meltdowns_before_offscreen_allocation() {
    let constrained_staging = structure_only_request(4_096, 4_096);
    assert_eq!(
        evaluate_publication_export_admission(
            constrained_staging,
            PublicationExportLimits {
                max_texture_dimension_2d: TEST_MAX_TEXTURE_DIMENSION,
                max_buffer_size: 1,
                publication_msaa_x4: true,
            },
        ),
        Err(PublicationExportRejection::DeviceBufferLimit),
    );

    let mut oversized_transparent_export = structure_only_request(8_192, 3_072);
    oversized_transparent_export.needs_transparent_depth = true;
    assert_rejected(
        oversized_transparent_export,
        PublicationExportRejection::TransientGpuBudget,
    );
}

#[test]
fn rejects_every_excluded_scene_domain() {
    let mut measurement_overlay = structure_only_request(512, 512);
    measurement_overlay.has_measurement_overlays = true;
    assert_rejected(
        measurement_overlay,
        PublicationExportRejection::MeasurementOverlays,
    );

    let mut hopping_overlay = structure_only_request(512, 512);
    hopping_overlay.has_hopping_overlays = true;
    assert_rejected(hopping_overlay, PublicationExportRejection::HoppingOverlays);

    let mut isosurface = structure_only_request(512, 512);
    isosurface.has_isosurface = true;
    assert_rejected(isosurface, PublicationExportRejection::Isosurface);

    let mut volume = structure_only_request(512, 512);
    volume.has_volume = true;
    assert_rejected(volume, PublicationExportRejection::Volume);

    let mut phonon_presentation = structure_only_request(512, 512);
    phonon_presentation.has_phonon_presentation = true;
    assert_rejected(
        phonon_presentation,
        PublicationExportRejection::PhononPresentation,
    );

    let mut atom_drag = structure_only_request(512, 512);
    atom_drag.has_atom_drag = true;
    assert_rejected(atom_drag, PublicationExportRejection::AtomDrag);

    let mut brillouin_zone = structure_only_request(512, 512);
    brillouin_zone.show_bz = true;
    assert_rejected(brillouin_zone, PublicationExportRejection::BrillouinZone);

    let mut measurement_state = structure_only_request(512, 512);
    measurement_state.has_measurement_state = true;
    assert_rejected(
        measurement_state,
        PublicationExportRejection::MeasurementState,
    );

    let mut selection_highlights = structure_only_request(512, 512);
    selection_highlights.has_selection_highlights = true;
    assert_rejected(
        selection_highlights,
        PublicationExportRejection::SelectionHighlights,
    );

    let mut wannier_overlay = structure_only_request(512, 512);
    wannier_overlay.has_wannier_overlay = true;
    assert_rejected(wannier_overlay, PublicationExportRejection::WannierOverlay);

    let mut active_phonon_state = structure_only_request(512, 512);
    active_phonon_state.has_active_phonon_state = true;
    assert_rejected(
        active_phonon_state,
        PublicationExportRejection::ActivePhononState,
    );
}

#[test]
fn admits_the_smallest_structure_only_control_case() {
    assert!(
        evaluate_publication_export_admission(structure_only_request(1, 1), test_limits()).is_ok()
    );
}

#[test]
fn capability_selected_fallback_and_peak_resources_are_receipt_bound() {
    let msaa = evaluate_publication_export_admission(
        structure_only_request(1024, 1024),
        test_limits(),
    )
    .unwrap();
    let msaa_value = serde_json::to_value(msaa).unwrap();
    assert_eq!(msaa_value["render_plan"]["requested_samples"], 4);
    assert_eq!(msaa_value["render_plan"]["selected_samples"], 4);
    assert_eq!(msaa_value["render_plan"]["tile_overlap_pixels"], 0);
    assert!(msaa_value["estimate"]["msaa_color_bytes"].as_u64().unwrap() > 0);
    assert!(
        msaa_value["estimate"]["opaque_depth_bytes"].as_u64().unwrap()
            >= msaa_value["estimate"]["msaa_color_bytes"].as_u64().unwrap()
    );
    assert!(
        msaa_value["estimate"]["peak_cpu_bytes"].as_u64().unwrap()
            >= msaa_value["estimate"]["rgba_bytes"].as_u64().unwrap()
    );

    let fallback = evaluate_publication_export_admission(
        structure_only_request(1024, 1024),
        PublicationExportLimits {
            publication_msaa_x4: false,
            ..test_limits()
        },
    )
    .unwrap();
    let fallback_value = serde_json::to_value(fallback).unwrap();
    assert_eq!(fallback_value["render_plan"]["requested_samples"], 4);
    assert_eq!(fallback_value["render_plan"]["selected_samples"], 1);
    assert_eq!(fallback_value["estimate"]["msaa_color_bytes"], 0);
}
