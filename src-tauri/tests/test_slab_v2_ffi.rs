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

/// FCC Al conventional cell: a = 4.05 Å, Fm-3m (#225), 4 atoms
fn make_fcc_al_state() -> CrystalState {
    let a0 = 4.05_f64;
    let mut state = CrystalState::default();
    state.name = "Al_fcc_conv".to_string();
    state.cell_a = a0;
    state.cell_b = a0;
    state.cell_c = a0;
    state.spacegroup_hm = "Fm-3m".to_string();
    state.spacegroup_number = 225;
    state.labels = vec!["Al1".into(), "Al2".into(), "Al3".into(), "Al4".into()];
    state.elements = vec!["Al".into(), "Al".into(), "Al".into(), "Al".into()];
    state.fract_x = vec![0.0, 0.5, 0.5, 0.0];
    state.fract_y = vec![0.0, 0.5, 0.0, 0.5];
    state.fract_z = vec![0.0, 0.0, 0.5, 0.5];
    state.occupancies = vec![1.0; 4];
    state.atomic_numbers = vec![13; 4];
    state.cart_positions = vec![[0.0; 3]; 4];
    state.intrinsic_sites = 4;
    state.version = 1;
    state
}

/// Simple cubic with 1 atom, Pm-3m (#221)
fn make_sc_state(a: f64) -> CrystalState {
    let mut state = CrystalState::default();
    state.name = "SC".to_string();
    state.cell_a = a;
    state.cell_b = a;
    state.cell_c = a;
    state.spacegroup_hm = "Pm-3m".to_string();
    state.spacegroup_number = 221;
    state.labels = vec!["X1".to_string()];
    state.elements = vec!["X".to_string()];
    state.fract_x = vec![0.0];
    state.fract_y = vec![0.0];
    state.fract_z = vec![0.0];
    state.occupancies = vec![1.0];
    state.atomic_numbers = vec![1];
    state.cart_positions = vec![[0.0_f32; 3]];
    state.intrinsic_sites = 1;
    state.version = 1;
    state
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

    assert!(
        result.is_ok(),
        "generate_slab should succeed: {:?}",
        result.err()
    );
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
            i,
            fz
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
    assert_eq!(
        slab.num_atoms(),
        4,
        "SC (1,0,0) 4L must have exactly 4 atoms"
    );
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
            i,
            fz
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

/// Negative vacuum must be rejected without a panic.
#[test]
fn test_generate_slab_negative_vacuum_no_panic() {
    let state = make_sc_state(3.0);
    let result = state.generate_slab([1, 0, 0], 3, -50.0);
    assert!(result.is_err(), "Negative vacuum must be rejected");
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

/// Low-level termination shifts preserve the version until transaction commit.
#[test]
fn test_shift_termination_preserves_version() {
    let state = make_sc_state(3.0);
    let mut slab = state.generate_slab([1, 0, 0], 3, 10.0).unwrap();
    let v_before = slab.version;

    slab.shift_termination(0, 0.3).unwrap();

    assert_eq!(
        slab.version, v_before,
        "Low-level mutation must not commit the state version"
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

/// NaCl conventional cell (Fm-3m, 8 atoms)
fn make_nacl_state() -> CrystalState {
    let a = 5.64;
    let mut state = CrystalState::default();
    state.name = "NaCl".to_string();
    state.cell_a = a;
    state.cell_b = a;
    state.cell_c = a;
    state.spacegroup_hm = "Fm-3m".to_string();
    state.spacegroup_number = 225;
    state.labels = vec![
        "Na1".into(),
        "Na2".into(),
        "Na3".into(),
        "Na4".into(),
        "Cl1".into(),
        "Cl2".into(),
        "Cl3".into(),
        "Cl4".into(),
    ];
    state.elements = vec![
        "Na".into(),
        "Na".into(),
        "Na".into(),
        "Na".into(),
        "Cl".into(),
        "Cl".into(),
        "Cl".into(),
        "Cl".into(),
    ];
    state.fract_x = vec![0.0, 0.5, 0.5, 0.0, 0.5, 0.0, 0.0, 0.5];
    state.fract_y = vec![0.0, 0.5, 0.0, 0.5, 0.5, 0.0, 0.5, 0.0];
    state.fract_z = vec![0.0, 0.0, 0.5, 0.5, 0.5, 0.5, 0.0, 0.0];
    state.occupancies = vec![1.0; 8];
    state.atomic_numbers = vec![11, 11, 11, 11, 17, 17, 17, 17];
    state.cart_positions = vec![[0.0; 3]; 8];
    state.intrinsic_sites = 8;
    state.version = 1;
    state
}

/// KILLER TEST: NaCl (110) 3-layer slab must have atoms at DISTINCT z-coords.
/// This is the exact failure mode: all atoms collapse to the same fract_z.
#[test]
fn test_nacl_110_slab_has_distinct_z_layers() {
    let state = make_nacl_state();
    let slab = state
        .generate_slab([1, 1, 0], 3, 10.0)
        .expect("NaCl (110) 3L slab generation must succeed");

    eprintln!("[TEST] NaCl (110) 3L: {} atoms", slab.num_atoms());
    eprintln!(
        "[TEST] cell: a={:.3} b={:.3} c={:.3} α={:.1} β={:.1} γ={:.1}",
        slab.cell_a, slab.cell_b, slab.cell_c, slab.cell_alpha, slab.cell_beta, slab.cell_gamma
    );

    // Print first 10 atoms' fractional coords
    for i in 0..slab.num_atoms().min(10) {
        eprintln!(
            "[TEST]   atom {} frac=({:.6}, {:.6}, {:.6})",
            i, slab.fract_x[i], slab.fract_y[i], slab.fract_z[i]
        );
    }

    // Collect unique z values (tolerance 0.01)
    let mut unique_z: Vec<f64> = Vec::new();
    for &fz in &slab.fract_z {
        if unique_z.iter().all(|&uz| (uz - fz).abs() > 0.01) {
            unique_z.push(fz);
        }
    }
    unique_z.sort_by(|a, b| a.partial_cmp(b).unwrap());

    eprintln!("[TEST] distinct fract_z values: {:?}", unique_z);

    // With 3 layers, we MUST have at least 3 distinct z-values
    assert!(
        unique_z.len() >= 3,
        "NaCl (110) 3-layer slab must have ≥3 distinct fract_z values, \
         but found only {}: {:?}. This is the z-collapse bug!",
        unique_z.len(),
        unique_z
    );
}

/// KILLER TEST 2: User workflow — supercell 2×2×1 THEN cut (110).
/// This is the exact path that fails in the GUI.
#[test]
fn test_nacl_supercell_then_110_slab() {
    let state = make_nacl_state();

    // Step 1: 2x2x1 supercell
    let sc = state
        .generate_supercell(&[2, 0, 0, 0, 2, 0, 0, 0, 1])
        .expect("2x2x1 supercell must succeed");
    eprintln!(
        "[TEST] Supercell: {} atoms, a={:.3} b={:.3} c={:.3}",
        sc.num_atoms(),
        sc.cell_a,
        sc.cell_b,
        sc.cell_c
    );

    // Step 2: Cut (110) from the supercell
    let slab = sc
        .generate_slab([1, 1, 0], 3, 10.0)
        .expect("(110) slab from supercell must succeed");

    eprintln!("[TEST] Slab from SC: {} atoms", slab.num_atoms());
    eprintln!(
        "[TEST] cell: a={:.3} b={:.3} c={:.3} α={:.1} β={:.1} γ={:.1}",
        slab.cell_a, slab.cell_b, slab.cell_c, slab.cell_alpha, slab.cell_beta, slab.cell_gamma
    );

    for i in 0..slab.num_atoms().min(10) {
        eprintln!(
            "[TEST]   atom {} frac=({:.6}, {:.6}, {:.6})",
            i, slab.fract_x[i], slab.fract_y[i], slab.fract_z[i]
        );
    }

    let mut unique_z: Vec<f64> = Vec::new();
    for &fz in &slab.fract_z {
        if unique_z.iter().all(|&uz| (uz - fz).abs() > 0.01) {
            unique_z.push(fz);
        }
    }
    unique_z.sort_by(|a, b| a.partial_cmp(b).unwrap());
    eprintln!("[TEST] distinct fract_z: {:?}", unique_z);

    assert!(
        unique_z.len() >= 3,
        "NaCl supercell→(110) 3L slab must have ≥3 distinct fract_z, \
         but found only {}: {:?}",
        unique_z.len(),
        unique_z
    );
}

/// Software regression: source metadata survives coincident and same-element sites.
#[test]
fn source_sites_survive_supercell_then_slab() {
    let mut state = make_fcc_al_state();
    state.spacegroup_number = 1;
    state.spacegroup_hm = "P1".into();
    state.fract_x[1] = state.fract_x[0];
    state.fract_y[1] = state.fract_y[0];
    state.fract_z[1] = state.fract_z[0];
    state.occupancies = vec![0.25, 0.75, 0.5, 1.0];
    // Distinct species at the same position must also survive.
    state.elements[1] = "Si".into();
    state.atomic_numbers[1] = 14;
    let expanded = state
        .generate_supercell(&[2, 0, 0, 0, 1, 0, 0, 0, 1])
        .unwrap();
    let slab = expanded.generate_slab([1, 1, 1], 3, 12.0).unwrap();
    assert_eq!(slab.num_atoms(), 24);
    for source in 0..4 {
        let replicas: Vec<_> = slab
            .labels
            .iter()
            .enumerate()
            .filter(|(_, label)| **label == state.labels[source])
            .map(|(index, _)| index)
            .collect();
        assert_eq!(replicas.len(), 6);
        for index in replicas {
            assert_eq!(slab.occupancies[index], state.occupancies[source]);
            assert_eq!(slab.elements[index], state.elements[source]);
            assert_eq!(slab.atomic_numbers[index], state.atomic_numbers[source]);
        }
    }
}

#[test]
fn valid_p1_and_periodic_input_images_are_accepted() {
    let mut state = make_sc_state(3.0);
    state.spacegroup_number = 1;
    state.spacegroup_hm = "P1".into();
    let reference = state.generate_slab([2, 1, -1], 3, 10.0).unwrap();
    state.fract_x[0] = 7.0;
    state.fract_y[0] = -4.0;
    state.fract_z[0] = 2.0;
    let shifted = state.generate_slab([2, 1, -1], 3, 10.0).unwrap();
    assert_eq!(shifted.fract_x, reference.fract_x);
    assert_eq!(shifted.fract_y, reference.fract_y);
    assert_eq!(shifted.fract_z, reference.fract_z);
}

#[test]
fn reposition_more_than_128_layers() {
    let mut state = make_sc_state(200.0);
    let count = 129;
    state.labels = (0..count).map(|i| format!("X{i}")).collect();
    state.elements = vec!["H".into(); count];
    state.atomic_numbers = vec![1; count];
    state.occupancies = vec![1.0; count];
    state.fract_x = vec![0.0; count];
    state.fract_y = vec![0.0; count];
    state.fract_z = (0..count).map(|i| i as f64 / count as f64).collect();
    state.cart_positions = vec![[0.0; 3]; count];
    state.intrinsic_sites = count;
    assert_eq!(state.shift_termination(128, 0.1).unwrap(), 129);
    assert!(state.fract_z[128].abs() < 1e-12);
    let previous = state.fract_z.clone();
    assert!(state.shift_termination(129, 0.1).is_err());
    assert_eq!(state.fract_z, previous);
}

#[test]
fn invalid_reposition_inputs_leave_state_unchanged() {
    let mut state = make_sc_state(3.0)
        .generate_slab([0, 0, 1], 3, 10.0)
        .unwrap();
    let before = state.fract_z.clone();
    for tolerance in [0.0, -0.1, f64::NAN, f64::INFINITY] {
        assert!(state.shift_termination(0, tolerance).is_err());
        assert_eq!(state.fract_z, before);
    }
    state.cell_beta = 75.0;
    assert!(state.shift_termination(0, 0.1).is_err());
    assert_eq!(state.fract_z, before);
}

#[test]
fn malformed_site_arrays_return_errors_without_panicking() {
    let mut state = make_fcc_al_state();
    state.occupancies.pop();
    assert!(state.generate_slab([1, 0, 0], 2, 10.0).is_err());
    assert!(
        state
            .generate_supercell(&[2, 0, 0, 0, 1, 0, 0, 0, 1])
            .is_err()
    );
    assert!(state.shift_termination(0, 0.1).is_err());
}

#[test]
fn reposition_cartesian_overflow_does_not_commit() {
    let mut state = make_fcc_al_state();
    state.cell_c = 1e40;
    let before_z = state.fract_z.clone();
    let before_cart = state.cart_positions.clone();
    assert!(state.shift_termination(0, 0.1).is_err());
    assert_eq!(state.fract_z, before_z);
    assert_eq!(state.cart_positions, before_cart);
}
