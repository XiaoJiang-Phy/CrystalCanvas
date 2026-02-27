//! Criterion benchmarks for CrystalCanvas FFI and parsing performance
//!
//! [Node 1.1] FFI round-trip latency benchmark
//! [Node 1.2] CIF parsing benchmark
//!
//! Run: cargo bench --bench bench_ffi

use criterion::{criterion_group, criterion_main, Criterion, black_box};
use crystal_canvas::crystal_state::CrystalState;
use crystal_canvas::ffi;

// ---------------------------------------------------------------------------
// Helper: resolve test data path
// ---------------------------------------------------------------------------
fn test_data_path(filename: &str) -> String {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    format!("{manifest}/../tests/data/{filename}")
}

// ===========================================================================
// [Node 1.2] CIF Parsing Benchmark
// ===========================================================================

/// Benchmark CIF file parsing + CrystalState construction.
/// Target: < 10ms (for 100~500 atom CIF files)
fn bench_cif_parse_nacl(c: &mut Criterion) {
    let path = test_data_path("nacl.cif");

    c.bench_function("cif_parse_nacl", |b| {
        b.iter(|| {
            let data = ffi::parse_cif_file(black_box(&path)).unwrap();
            let state = CrystalState::from_ffi(data);
            black_box(state.num_atoms());
        });
    });
}

/// Benchmark fractional-to-Cartesian coordinate conversion.
fn bench_fract_to_cart_nacl(c: &mut Criterion) {
    let path = test_data_path("nacl.cif");

    c.bench_function("fract_to_cart_nacl", |b| {
        b.iter(|| {
            let mut state = CrystalState::from_ffi(
                ffi::parse_cif_file(&path).unwrap()
            );
            state.fractional_to_cartesian();
            black_box(state.cart_positions.len());
        });
    });
}

// ===========================================================================
// [Node 1.1] FFI Round-trip Benchmark
// ===========================================================================

/// Benchmark FFI round-trip: 1,000 atoms Rust → C++ → Rust.
/// Target: < 0.5ms (500 µs)
fn bench_ffi_roundtrip_1000_atoms(c: &mut Criterion) {
    let input: Vec<ffi::FfiVec3f> = (0..1000)
        .map(|i| ffi::FfiVec3f { x: i as f32 * 0.1, y: 0.0, z: 0.0 })
        .collect();

    c.bench_function("ffi_roundtrip_1000_atoms", |b| {
        b.iter(|| {
            let output = ffi::translate_positions(black_box(&input), 1.0);
            black_box(output.len());
        });
    });
}

criterion_group!(
    benches,
    bench_cif_parse_nacl,
    bench_fract_to_cart_nacl,
    bench_ffi_roundtrip_1000_atoms,
);
criterion_main!(benches);
