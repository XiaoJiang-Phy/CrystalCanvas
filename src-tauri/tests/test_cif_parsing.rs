//! [Node 1.2] CIF file parsing and CrystalState construction tests
//!
//! Acceptance Criteria:
//! - Parsing a standard CIF file and loading it into `CrystalState` must take < 10ms
//! - Lattice parameters, space group, and atom count must match exactly
//! - Invalid file paths must return an Err

use crystal_canvas::crystal_state::CrystalState;
use crystal_canvas::ffi;

// ---------------------------------------------------------------------------
// Helper: resolve test data path relative to workspace root
// ---------------------------------------------------------------------------
fn test_data_path(filename: &str) -> String {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    format!("{manifest}/../tests/data/{filename}")
}

// ===========================================================================
// Accuracy Tests
// ===========================================================================

/// Parse NaCl CIF and verify lattice parameters, space group, and atom count.
#[test]
fn test_parse_nacl_cif_correctness() {
    let path = test_data_path("nacl.cif");
    let data = ffi::parse_cif_file(&path).expect("Failed to parse nacl.cif");
    let state = CrystalState::from_ffi(data);

    // NaCl: Fm-3m (225), cubic a=b=c=5.64 Å, α=β=γ=90°
    assert_eq!(
        state.spacegroup_number, 225,
        "Space group should be 225 (Fm-3m)"
    );
    assert!(
        (state.cell_a - 5.64).abs() < 0.01,
        "cell_a: {}",
        state.cell_a
    );
    assert!(
        (state.cell_b - 5.64).abs() < 0.01,
        "cell_b: {}",
        state.cell_b
    );
    assert!(
        (state.cell_c - 5.64).abs() < 0.01,
        "cell_c: {}",
        state.cell_c
    );
    assert!((state.cell_alpha - 90.0).abs() < 0.01);
    assert!((state.cell_beta - 90.0).abs() < 0.01);
    assert!((state.cell_gamma - 90.0).abs() < 0.01);

    // NaCl CIF has 27 sites after symmetry expansion and boundary mirroring
    assert_eq!(state.num_atoms(), 27, "NaCl CIF should have 27 sites after expansion");

    // Verify element symbols
    assert!(state.elements.contains(&"Na".to_string()));
    assert!(state.elements.contains(&"Cl".to_string()));

    // Verify fractional coordinates are in [0, 1]
    for i in 0..state.num_atoms() {
        assert!(
            state.fract_x[i] >= 0.0 && state.fract_x[i] <= 1.0,
            "fract_x[{}] = {} out of range",
            i,
            state.fract_x[i]
        );
        assert!(
            state.fract_y[i] >= 0.0 && state.fract_y[i] <= 1.0,
            "fract_y[{}] = {} out of range",
            i,
            state.fract_y[i]
        );
        assert!(
            state.fract_z[i] >= 0.0 && state.fract_z[i] <= 1.0,
            "fract_z[{}] = {} out of range",
            i,
            state.fract_z[i]
        );
    }
}

/// Verify Cartesian coordinate conversion produces valid results.
#[test]
fn test_fractional_to_cartesian_nacl() {
    let path = test_data_path("nacl.cif");
    let data = ffi::parse_cif_file(&path).unwrap();
    let mut state = CrystalState::from_ffi(data);

    state.fractional_to_cartesian();

    // cart_positions should be populated
    assert_eq!(state.cart_positions.len(), state.num_atoms());

    // Find the Na at origin (or closest to it)
    let na_idx = state.elements.iter().position(|e| e == "Na").unwrap();
    let na_cart = state.cart_positions[na_idx];
    assert!((na_cart[0]).abs() < 0.01, "Na cart_x should be ~0");
    assert!((na_cart[1]).abs() < 0.01, "Na cart_y should be ~0");
    assert!((na_cart[2]).abs() < 0.01, "Na cart_z should be ~0");

    // Find a Cl atom. One of the Cl atoms should be at (0.5, 0.5, 0.5)
    let cl_idx = state.elements.iter().position(|e| e == "Cl").unwrap();
    // Use the first Cl; or find the Cl specifically at 0.5, 0.5, 0.5
    let cl_idx = state.elements.iter().enumerate().find(|(i, e)| {
        *e == "Cl" && (state.fract_x[*i] - 0.5).abs() < 0.01
    }).unwrap().0;
    
    let cl_cart = state.cart_positions[cl_idx];
    let expected = 5.64 * 0.5;
    assert!(
        (cl_cart[0] - expected as f32).abs() < 0.1,
        "Cl cart_x: {}",
        cl_cart[0]
    );
    assert!(
        (cl_cart[1] - expected as f32).abs() < 0.1,
        "Cl cart_y: {}",
        cl_cart[1]
    );
    assert!(
        (cl_cart[2] - expected as f32).abs() < 0.1,
        "Cl cart_z: {}",
        cl_cart[2]
    );
}

// ===========================================================================
// Performance Tests
// ===========================================================================

/// Parse timing must be < 10ms for standard CIF files.
#[test]
fn test_parse_nacl_timing() {
    let path = test_data_path("nacl.cif");

    // Warm up (filesystem cache)
    let _ = ffi::parse_cif_file(&path);

    let start = std::time::Instant::now();
    let data = ffi::parse_cif_file(&path).expect("Parse failed");
    let _state = CrystalState::from_ffi(data);
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 10,
        "CIF parse + CrystalState build took {:?} (must be < 10ms)",
        elapsed
    );
}

// ===========================================================================
// Error Handling Tests
// ===========================================================================

/// Invalid file path should return Err, not panic.
#[test]
fn test_invalid_file_returns_error() {
    let result = ffi::parse_cif_file("/nonexistent/path/fake.cif");
    assert!(
        result.is_err(),
        "Parsing nonexistent file should return Err"
    );
}

/// Empty string path should return Err.
#[test]
fn test_empty_path_returns_error() {
    let result = ffi::parse_cif_file("");
    assert!(result.is_err(), "Empty path should return Err");
}
