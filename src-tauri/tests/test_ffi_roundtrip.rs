//! [Node 1.1] FFI round-trip test: Rust ↔ C++ coordinate translation via translate_positions
//!
//! Acceptance criteria:
//! - 1,000 atom coordinates (f32×3) Rust→C++→Rust round-trip latency < 0.5ms
//! - ASan memory leak detection passes
//!
//! Status: ACTIVE — translate_positions FFI implemented in M2

use crystal_canvas::ffi;

/// Helper: create FfiVec3f from components
fn vec3f(x: f32, y: f32, z: f32) -> ffi::FfiVec3f {
    ffi::FfiVec3f { x, y, z }
}

// ===========================================================================
// Correctness tests
// ===========================================================================

/// 1,000 atoms coordinate round-trip: Rust → C++ (translate +1.0) → Rust.
/// Verifies every coordinate is correctly shifted.
#[test]
fn test_roundtrip_1000_atoms_correctness() {
    let n_atoms = 1000;
    let input: Vec<ffi::FfiVec3f> = (0..n_atoms)
        .map(|i| {
            let f = i as f32;
            vec3f(f * 0.1, f * 0.2 - 50.0, f * 0.3 + 100.0)
        })
        .collect();

    let output = ffi::translate_positions(&input, 1.0);

    assert_eq!(output.len(), n_atoms, "Output atom count mismatch");
    // Use 1e-5 tolerance: f32 has ~7 significant digits, so at magnitude ~100+
    // the ULP can reach ~1.5e-5. This tolerance is still strict but realistic.
    let _tol = 1e-5_f32;
    for (i, (inp, out)) in input.iter().zip(output.iter()).enumerate() {
        let ex = inp.x + 1.0;
        let ey = inp.y + 1.0;
        let ez = inp.z + 1.0;
        // Relative tolerance: allow up to ~2 ULP of f32 at the expected magnitude
        let tol_x = ex.abs().max(1.0) * 1e-6;
        let tol_y = ey.abs().max(1.0) * 1e-6;
        let tol_z = ez.abs().max(1.0) * 1e-6;
        assert!(
            (out.x - ex).abs() < tol_x
            && (out.y - ey).abs() < tol_y
            && (out.z - ez).abs() < tol_z,
            "Atom {i}: expected [{ex}, {ey}, {ez}], got [{}, {}, {}]",
            out.x, out.y, out.z
        );
    }
}

// ===========================================================================
// Performance tests
// ===========================================================================

/// Round-trip latency for 1,000 atoms must be < 0.5ms (500 µs).
#[test]
fn test_roundtrip_1000_atoms_latency() {
    let n_atoms = 1000;
    let input: Vec<ffi::FfiVec3f> = (0..n_atoms)
        .map(|i| vec3f(i as f32, 0.0, 0.0))
        .collect();

    // Warm up
    let _ = ffi::translate_positions(&input, 0.0);

    let start = std::time::Instant::now();
    let _output = ffi::translate_positions(&input, 1.0);
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_micros() < 500,
        "FFI round-trip took {:?} (must be < 0.5ms / 500µs)",
        elapsed
    );
}

// ===========================================================================
// Boundary condition tests
// ===========================================================================

/// Empty input should return empty output without error.
#[test]
fn test_roundtrip_empty_array() {
    let input: Vec<ffi::FfiVec3f> = vec![];
    let output = ffi::translate_positions(&input, 1.0);
    assert_eq!(output.len(), 0);
}

/// Single atom round-trip.
#[test]
fn test_roundtrip_single_atom() {
    let input = vec![vec3f(1.0, 2.0, 3.0)];
    let output = ffi::translate_positions(&input, 5.0);
    assert!((output[0].x - 6.0).abs() < f32::EPSILON);
    assert!((output[0].y - 7.0).abs() < f32::EPSILON);
    assert!((output[0].z - 8.0).abs() < f32::EPSILON);
}

/// Negative translation values should work correctly.
#[test]
fn test_roundtrip_negative_translation() {
    let input = vec![vec3f(10.0, 20.0, 30.0)];
    let output = ffi::translate_positions(&input, -5.0);
    assert!((output[0].x - 5.0).abs() < f32::EPSILON);
    assert!((output[0].y - 15.0).abs() < f32::EPSILON);
    assert!((output[0].z - 25.0).abs() < f32::EPSILON);
}
