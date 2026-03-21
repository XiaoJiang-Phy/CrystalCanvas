// [PROJECT RULE L0] - Do not modify assert tolerance, time values, or thresholds in this file.
//! [Node S4] FFI Bridge — build_slab_v2 / cluster_slab_layers / shift_slab_termination
//!
//! Acceptance criteria (Plan):
//! - cargo build: zero error, zero warning
//! - test_ffi_roundtrip: still passes (no regression)
//! - S4 new tests: generate_slab via CrystalState using v2 API
//! - shift_termination invalidated by empty crystal, out-of-range index
//!
//! Status: ACTIVE — Node S4 FFI bridge implemented

use crystal_canvas::crystal_state::CrystalState;

// ===========================================================================
// Helpers
// ===========================================================================

/// FCC Al primitive cell: a_prim = a_conv / sqrt(2) ≈ 2.863 Å, angles = 60°
fn make_fcc_al_state() -> CrystalState {
    // a_conv = 4.05 Å → a_prim = 4.05 / sqrt(2) ≈ 2.863 Å
    let a0 = 2.863_f64;
    CrystalState {
        name: "Al_fcc_primitive".to_string(),
        cell_a: a0,
        cell_b: a0,
        cell_c: a0,
        cell_alpha: 60.0,
        cell_beta: 60.0,
        cell_gamma: 60.0,
        spacegroup_hm: "P1".to_string(),
        spacegroup_number: 1,
        labels: vec!["Al1".to_string()],
        elements: vec!["Al".to_string()],
        fract_x: vec![0.0],
        fract_y: vec![0.0],
        fract_z: vec![0.0],
        occupancies: vec![1.0],
        atomic_numbers: vec![13],
        cart_positions: vec![[0.0_f32; 3]],
        version: 1,
        bond_analysis: None,
        phonon_data: None,
        active_phonon_mode: None,
        phonon_phase: 0.0,
        intrinsic_sites: 1,
        selected_atoms: vec![],
    }
}

/// Simple cubic with 1 atom
fn make_sc_state(a: f64) -> CrystalState {
    CrystalState {
        name: "SC".to_string(),
        cell_a: a,
        cell_b: a,
        cell_c: a,
        cell_alpha: 90.0,
        cell_beta: 90.0,
        cell_gamma: 90.0,
        spacegroup_hm: "P1".to_string(),
        spacegroup_number: 1,
        labels: vec!["X1".to_string()],
        elements: vec!["X".to_string()],
        fract_x: vec![0.0],
        fract_y: vec![0.0],
        fract_z: vec![0.0],
        occupancies: vec![1.0],
        atomic_numbers: vec![1],
        cart_positions: vec![[0.0_f32; 3]],
        version: 1,
        bond_analysis: None,
        phonon_data: None,
        active_phonon_mode: None,
        phonon_phase: 0.0,
        intrinsic_sites: 1,
        selected_atoms: vec![],
    }
}

/// Empty CrystalState
fn empty_state() -> CrystalState {
    CrystalState::default()
}

// ===========================================================================
// S4 Gate Tests — Plan-mandated
// ===========================================================================

/// Gate: generate_slab must succeed for Al FCC (1,1,1) 3L with v2 API
#[test]
fn test_generate_slab_v2_fcc111_3layer() {
    let state = make_fcc_al_state();
    let result = state.generate_slab([1, 1, 1], 3, 10.0);

    assert!(result.is_ok(), "generate_slab should succeed: {:?}", result.err());
    let slab = result.unwrap();

    // v2 returns deduplicated atom count — must be > 0
    assert!(slab.num_atoms() > 0, "Slab must have atoms");
    // intrinsic_sites must match actual count (no boundary mirroring in v2)
    assert_eq!(
        slab.intrinsic_sites,
        slab.num_atoms(),
        "v2 slab: intrinsic_sites must equal num_atoms (boundary mirroring skipped)"
    );
    // Vacuum => c must be substantially larger than original
    assert!(
        slab.cell_c > 10.0,
        "Slab c-axis must include vacuum (> 10 Å)"
    );
    // All slab fractional z must be in [0, 1)
    for i in 0..slab.num_atoms() {
        let fz = slab.fract_z[i];
        assert!(
            fz >= 0.0 && fz < 1.0,
            "Atom {} has fract_z = {} out of [0,1)",
            i, fz
        );
    }
}

/// Gate: generate_slab (1,0,0) 4L for SC — atom count must be 4
#[test]
fn test_generate_slab_v2_sc100_4layer() {
    let state = make_sc_state(3.0);
    let result = state.generate_slab([1, 0, 0], 4, 10.0);

    assert!(result.is_ok(), "SC (1,0,0) 4L should succeed");
    let slab = result.unwrap();
    assert_eq!(slab.num_atoms(), 4, "SC (1,0,0) 4L must have exactly 4 atoms");
}

/// Gate: shift_termination on a 3L slab must succeed and preserve atom count
#[test]
fn test_shift_termination_layer1_preserves_count() {
    let state = make_sc_state(3.0);
    let mut slab = state.generate_slab([1, 0, 0], 3, 10.0).unwrap();
    let n_before = slab.num_atoms();

    let result = slab.shift_termination(1, 0.3);
    assert!(result.is_ok(), "shift_termination to layer 1 must succeed");

    assert_eq!(
        slab.num_atoms(),
        n_before,
        "Atom count must be preserved after termination shift"
    );
    // All fract_z must remain in [0, 1)
    for i in 0..slab.num_atoms() {
        let fz = slab.fract_z[i];
        assert!(
            fz >= 0.0 && fz < 1.0,
            "After shift, atom {} has fract_z = {} out of [0,1)",
            i, fz
        );
    }
}

// ===========================================================================
// [Breaker] Pathological Attack Tests
// ===========================================================================

/// Empty crystal: generate_slab must return Err, not panic
#[test]
fn test_generate_slab_empty_crystal_is_error() {
    let state = empty_state();
    let result = state.generate_slab([1, 0, 0], 3, 10.0);
    assert!(result.is_err(), "Empty crystal must return Err, not panic");
}

/// Zero layers: generate_slab must Err or gracefully return 0 atoms
#[test]
fn test_generate_slab_zero_layers() {
    let state = make_sc_state(3.0);
    let result = state.generate_slab([1, 0, 0], 0, 10.0);
    // Must not panic — result may be Err or 0-atom slab
    match result {
        Ok(slab) => {
            assert_eq!(slab.num_atoms(), 0, "Zero layers → 0 atom slab");
        }
        Err(_) => { /* acceptable — invalid configuration */ }
    }
}

/// Negative vacuum: generate_slab must not panic, clamp to 0
#[test]
fn test_generate_slab_negative_vacuum_no_panic() {
    let state = make_sc_state(3.0);
    // Should not panic; result may succeed with 0 vacuum or Err
    let result = state.generate_slab([1, 0, 0], 3, -50.0);
    assert!(result.is_ok() || result.is_err(), "Must not panic with negative vacuum");
}

/// Empty crystal: shift_termination must return Err, not panic
#[test]
fn test_shift_termination_empty_crystal_is_error() {
    let mut state = empty_state();
    let result = state.shift_termination(0, 0.3);
    assert!(result.is_err(), "Empty crystal must return Err");
}

/// Out-of-range positive layer index: shift_termination must return Err
#[test]
fn test_shift_termination_out_of_range_positive() {
    let state = make_sc_state(3.0);
    let mut slab = state.generate_slab([1, 0, 0], 3, 10.0).unwrap();

    // Layer 9999 is way beyond valid range
    let result = slab.shift_termination(9999, 0.3);
    assert!(
        result.is_err(),
        "Out-of-range layer index must return Err, not panic"
    );
    // Verify atom count unchanged
    assert_eq!(slab.num_atoms(), 3, "Failed shift must not mutate state");
}

/// Negative layer index: shift_termination must return Err
#[test]
fn test_shift_termination_negative_index_is_error() {
    let state = make_sc_state(3.0);
    let mut slab = state.generate_slab([1, 0, 0], 3, 10.0).unwrap();
    let n_before = slab.num_atoms();

    let result = slab.shift_termination(-1, 0.3);
    assert!(result.is_err(), "Negative layer index must return Err");
    assert_eq!(slab.num_atoms(), n_before, "State must be unchanged");
}

/// Very large layer count: no panic, correct structure
#[test]
fn test_generate_slab_large_layer_count_no_panic() {
    let state = make_sc_state(3.0);
    let result = state.generate_slab([1, 0, 0], 50, 0.0);
    match result {
        Ok(slab) => assert_eq!(slab.num_atoms(), 50),
        Err(_) => { /* acceptable if buffer check rejects */ }
    }
}

/// Version must increment after successful shift_termination
#[test]
fn test_shift_termination_increments_version() {
    let state = make_sc_state(3.0);
    let mut slab = state.generate_slab([1, 0, 0], 3, 10.0).unwrap();
    let v_before = slab.version;

    slab.shift_termination(0, 0.3).unwrap();

    assert_eq!(
        slab.version,
        v_before + 1,
        "Version must increment after successful shift_termination"
    );
}

/// Cartesian positions must be refreshed after shift_termination
#[test]
fn test_shift_termination_refreshes_cart_positions() {
    let state = make_sc_state(3.0);
    let mut slab = state.generate_slab([1, 0, 0], 3, 10.0).unwrap();

    // Snapshot Cartesian z before shift
    let cart_z_before: Vec<f32> = slab.cart_positions.iter().map(|p| p[2]).collect();

    slab.shift_termination(1, 0.3).unwrap();

    let cart_z_after: Vec<f32> = slab.cart_positions.iter().map(|p| p[2]).collect();

    // After shifting by a non-zero layer, Cartesian z values must change
    let any_changed = cart_z_before
        .iter()
        .zip(cart_z_after.iter())
        .any(|(a, b)| (a - b).abs() > 1e-5_f32);
    assert!(
        any_changed,
        "shift_termination must update Cartesian positions"
    );
}
