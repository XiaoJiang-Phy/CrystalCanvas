use crystal_canvas::crystal_state::{
    validate_fractional_position, validate_lattice_parameters, validate_slab_request,
    validate_supercell_request, CrystalState,
};

#[test]
fn valid_skew_lattice_is_accepted() {
    assert!(validate_lattice_parameters(4.0, 5.0, 6.0, 70.0, 80.0, 75.0).is_ok());
}

#[test]
fn nonpositive_and_nonfinite_lattice_lengths_are_rejected() {
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(validate_lattice_parameters(invalid, 5.0, 6.0, 70.0, 80.0, 75.0).is_err());
        assert!(validate_lattice_parameters(4.0, invalid, 6.0, 70.0, 80.0, 75.0).is_err());
        assert!(validate_lattice_parameters(4.0, 5.0, invalid, 70.0, 80.0, 75.0).is_err());
    }
}

#[test]
fn out_of_range_and_nonfinite_lattice_angles_are_rejected() {
    for invalid in [
        -1.0,
        0.0,
        180.0,
        181.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        assert!(validate_lattice_parameters(4.0, 5.0, 6.0, invalid, 80.0, 75.0).is_err());
        assert!(validate_lattice_parameters(4.0, 5.0, 6.0, 70.0, invalid, 75.0).is_err());
        assert!(validate_lattice_parameters(4.0, 5.0, 6.0, 70.0, 80.0, invalid).is_err());
    }
}

#[test]
fn degenerate_and_extremely_ill_conditioned_lattices_are_rejected() {
    assert!(validate_lattice_parameters(1.0, 1.0, 1.0, 120.0, 120.0, 120.0).is_err());
    assert!(validate_lattice_parameters(1.0, 1.0, 1.0, 90.0, 90.0, 1.0e-13).is_err());
    assert!(validate_lattice_parameters(1.0e-20, 1.0, 1.0, 90.0, 90.0, 90.0).is_err());
    assert!(validate_lattice_parameters(f64::MAX, f64::MAX, f64::MAX, 90.0, 90.0, 90.0).is_err());
}

#[test]
fn fractional_positions_require_only_finite_components() {
    assert!(validate_fractional_position([100.0, -100.0, 0.5]).is_ok());
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(validate_fractional_position([invalid, 0.0, 0.0]).is_err());
        assert!(validate_fractional_position([0.0, invalid, 0.0]).is_err());
        assert!(validate_fractional_position([0.0, 0.0, invalid]).is_err());
    }
}

#[test]
fn slab_request_rejects_zero_miller_and_invalid_layers() {
    assert!(validate_slab_request([0, 0, 0], 3, 15.0).is_err());
    assert!(validate_slab_request([1, 0, 0], 0, 15.0).is_err());
    assert!(validate_slab_request([1, 0, 0], -1, 15.0).is_err());
    assert!(validate_slab_request([1, 0, 0], i32::MAX, 15.0).is_err());
}

#[test]
fn slab_request_rejects_negative_and_nonfinite_vacuum() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1] {
        assert!(validate_slab_request([1, 1, 0], 3, invalid).is_err());
    }
    for valid in [0.0, 4.9, 5.0, 100.0, 100.1] {
        assert!(validate_slab_request([1, 1, 0], 3, valid).is_ok());
    }
}

#[test]
fn supercell_request_rejects_empty_singular_and_negative_expansions() {
    assert!(validate_supercell_request(&[0; 9], 1).is_err());
    assert!(validate_supercell_request(&[1, 0, 0, 0, 1, 0, 0, 0, 0], 1).is_err());
    assert!(validate_supercell_request(&[-1, 0, 0, 0, 1, 0, 0, 0, 1], 1).is_err());
    assert!(validate_supercell_request(&[1, 0, 0, 0, 1, 0, 0, 0, 1], 0).is_err());
}

#[test]
fn supercell_request_rejects_integer_and_atom_count_resource_meltdowns() {
    let huge = [i32::MAX, 0, 0, 0, i32::MAX, 0, 0, 0, i32::MAX];
    assert!(validate_supercell_request(&huge, 1).is_err());
    assert!(validate_supercell_request(&[2, 0, 0, 0, 1, 0, 0, 0, 1], usize::MAX).is_err());
    assert!(validate_supercell_request(&[2, 0, 0, 0, 2, 0, 0, 0, 2], 1_251).is_err());
}

#[test]
fn supercell_request_returns_checked_output_size() {
    assert_eq!(
        validate_supercell_request(&[2, 0, 0, 0, 2, 0, 0, 0, 1], 1_000),
        Ok(4_000)
    );
}

#[test]
fn supercell_rejects_cartesian_coordinates_not_representable_as_f32() {
    let scale = f64::from(f32::MAX) * 4.0;
    let state = CrystalState {
        name: "overflow-probe".to_string(),
        cell_a: scale,
        cell_b: scale,
        cell_c: scale,
        cell_alpha: 90.0,
        cell_beta: 90.0,
        cell_gamma: 90.0,
        labels: vec!["C1".to_string()],
        elements: vec!["C".to_string()],
        fract_x: vec![0.5],
        fract_y: vec![0.5],
        fract_z: vec![0.5],
        occupancies: vec![1.0],
        atomic_numbers: vec![6],
        intrinsic_sites: 1,
        ..CrystalState::default()
    };

    let identity = [1, 0, 0, 0, 1, 0, 0, 0, 1];
    assert!(state.generate_supercell(&identity).is_err());
}
