use crystal_canvas::crystal_state::CrystalState;

fn valid_state() -> CrystalState {
    let mut state = CrystalState {
        name: "invariant-probe".to_string(),
        cell_a: 4.0,
        cell_b: 5.0,
        cell_c: 6.0,
        cell_alpha: 70.0,
        cell_beta: 80.0,
        cell_gamma: 75.0,
        labels: vec!["C1".to_string()],
        elements: vec!["C".to_string()],
        fract_x: vec![0.25],
        fract_y: vec![0.5],
        fract_z: vec![0.75],
        occupancies: vec![1.0],
        atomic_numbers: vec![6],
        intrinsic_sites: 1,
        ..CrystalState::default()
    };
    state.fractional_to_cartesian();
    state
}

#[test]
fn accepts_a_finite_aligned_structure() {
    assert!(valid_state().validate_structural_invariants().is_ok());
}

#[test]
fn rejects_misaligned_soa_and_intrinsic_site_counts() {
    let mut state = valid_state();
    state.labels.clear();
    assert!(state.validate_structural_invariants().is_err());

    let mut state = valid_state();
    state.elements.clear();
    assert!(state.validate_structural_invariants().is_err());

    let mut state = valid_state();
    state.cart_positions.clear();
    assert!(state.validate_structural_invariants().is_err());

    let mut state = valid_state();
    state.intrinsic_sites = 2;
    assert!(state.validate_structural_invariants().is_err());
}

#[test]
fn rejects_zero_and_ill_conditioned_nonempty_lattices() {
    let mut state = valid_state();
    state.cell_a = 0.0;
    state.cell_b = 0.0;
    state.cell_c = 0.0;
    assert!(state.validate_structural_invariants().is_err());

    let mut state = valid_state();
    state.cell_gamma = 1.0e-13;
    assert!(state.validate_structural_invariants().is_err());
}

#[test]
fn rejects_an_empty_structure_with_a_degenerate_lattice() {
    assert!(CrystalState::default().validate_structural_invariants().is_err());
}

#[test]
fn rejects_nonfinite_fractional_and_cartesian_components() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut state = valid_state();
        state.fract_x[0] = invalid;
        assert!(state.validate_structural_invariants().is_err());
    }

    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut state = valid_state();
        state.cart_positions[0][2] = invalid;
        assert!(state.validate_structural_invariants().is_err());
    }
}

#[test]
fn rejects_zero_identity_and_invalid_occupancy() {
    let mut state = valid_state();
    state.atomic_numbers[0] = 0;
    assert!(state.validate_structural_invariants().is_err());

    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0e-6, 1.000001] {
        let mut state = valid_state();
        state.occupancies[0] = invalid;
        assert!(state.validate_structural_invariants().is_err());
    }
}

#[test]
fn rejects_atom_count_resource_meltdown_before_array_walks() {
    let mut state = CrystalState::default();
    state.labels = vec!["X".to_string(); 10_001];
    assert!(state.validate_structural_invariants().is_err());
}

#[test]
fn structural_commit_boundaries_reference_the_shared_gate() {
    for source in [
        include_str!("../src/transaction.rs"),
        include_str!("../src/commands/file_io.rs"),
        include_str!("../src/commands/volumetric.rs"),
        include_str!("../src/commands/editing.rs"),
        include_str!("../src/commands/analysis.rs"),
        include_str!("../src/main.rs"),
    ] {
        assert!(source.contains("validate_structural_invariants"));
    }
}

#[test]
fn interactive_phonon_load_rejects_mismatched_structural_attachment() {
    let source = include_str!("../src/commands/analysis.rs");
    assert!(source.contains("new_state.intrinsic_sites != data.n_atoms"));
    assert!(source.contains("IpcError::invalid_argument"));
}
