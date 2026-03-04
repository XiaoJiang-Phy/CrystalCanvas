// [Overview: Verify that Fe2O3 (R-3c, #167) is parsed and identified correctly.]
//! Regression test for hexagonal/trigonal spacegroup detection.
//! Fe2O3 (alpha-hematite) has space group R-3c (#167) with hexagonal cell
//! parameters: a=b=5.105 Å, c=13.913 Å, α=β=90°, γ=120°.
//!
//! This test ensures:
//! - Gemmi symmetry expansion produces 30 atoms (12 Fe + 18 O)
//! - Spglib correctly identifies space group #167 (not #2 due to lattice transpose bug)
//! - Lattice parameters match the CIF values

use crystal_canvas::crystal_state::CrystalState;

fn test_data_path(filename: &str) -> String {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    format!("{manifest}/../tests/data/{filename}")
}

#[test]
fn test_fe2o3_atom_count_and_spacegroup() {
    let path = test_data_path("Fe2O3/Fe2O3_mp-19770_symmetrized.cif");
    let state = CrystalState::from_cif(&path).expect("Failed to parse Fe2O3 CIF");

    // Gemmi should expand 2 Wyckoff sites (Fe 12c, O 18e) into 30 atoms
    // in the conventional hexagonal cell.
    assert_eq!(
        state.intrinsic_sites, 30,
        "Fe2O3 conventional cell should have 30 intrinsic atoms (12 Fe + 18 O), got {}",
        state.intrinsic_sites
    );

    // Verify element counts
    let fe_count = state
        .elements
        .iter()
        .take(state.intrinsic_sites)
        .filter(|e| *e == "Fe")
        .count();
    let o_count = state
        .elements
        .iter()
        .take(state.intrinsic_sites)
        .filter(|e| *e == "O")
        .count();
    assert_eq!(fe_count, 12, "Should have 12 Fe atoms, got {}", fe_count);
    assert_eq!(o_count, 18, "Should have 18 O atoms, got {}", o_count);

    // Spacegroup must be #167 (R-3c), NOT #2
    assert_eq!(
        state.spacegroup_number, 167,
        "Fe2O3 spacegroup should be #167 (R-3c), got #{}",
        state.spacegroup_number
    );

    // Verify lattice parameters (hexagonal setting)
    let eps = 0.01;
    assert!(
        (state.cell_a - 5.105).abs() < eps,
        "cell_a should be ~5.105, got {}",
        state.cell_a
    );
    assert!(
        (state.cell_b - 5.105).abs() < eps,
        "cell_b should be ~5.105, got {}",
        state.cell_b
    );
    assert!(
        (state.cell_c - 13.913).abs() < eps,
        "cell_c should be ~13.913, got {}",
        state.cell_c
    );
    assert!(
        (state.cell_alpha - 90.0).abs() < eps,
        "alpha should be 90°, got {}",
        state.cell_alpha
    );
    assert!(
        (state.cell_beta - 90.0).abs() < eps,
        "beta should be 90°, got {}",
        state.cell_beta
    );
    assert!(
        (state.cell_gamma - 120.0).abs() < eps,
        "gamma should be 120°, got {}",
        state.cell_gamma
    );
}
