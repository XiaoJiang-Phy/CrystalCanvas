# CrystalCanvas Testing & TDD Guide

> Version: v0.6.0 | Updated: 2026-04-14

---

## 1. Node TDD Process

All CrystalCanvas development follows **Node-based TDD** — every feature is decomposed into atomic, sequentially gated Nodes. A Node must pass its full test suite before the next Node begins.

| Phase | Action |
|---|---|
| **Atomize** | Break feature into Nodes (e.g., `Node S1: Surface Basis Solver`, `Node S2: Slab Builder`, `Node S3: Layer Clustering`) |
| **Test First** | Write the test asserting exact expected behavior (tolerances, geometry, error paths) |
| **Implement** | Write minimal code to pass the test |
| **Gate** | `cargo test` / `ctest` must be fully green before proceeding |
| **Audit** | Stress-test edge cases, pathological inputs, and numerical stability |

Each test file is annotated with its Node ID and acceptance criteria. Example header:

```rust
// [PROJECT RULE L0] - Do not modify assert tolerance, time values, or thresholds in this file.
//! [Node S4] FFI Bridge — build_slab_v2 / cluster_slab_layers / shift_slab_termination
//!
//! Acceptance criteria (Plan):
//! - cargo build: zero error, zero warning
//! - test_ffi_roundtrip: still passes (no regression)
//! - S4 new tests: generate_slab via CrystalState using v2 API
//! - shift_termination invalidated by empty crystal, out-of-range index
```

---

## 2. Project-Wide Testing Rules

The following constraints apply **unconditionally** to all test code:

### 2.1 Numerical Tolerances (non-negotiable)

| Domain | Tolerance | Rationale |
|---|---|---|
| Fractional/Cartesian coordinates | $10^{-5}$ Å | Sub-pm precision eliminates rounding ambiguities across parsers |
| Physics properties (energy, density, frequency) | $10^{-6}$ | Avoids false negatives from parser/FFI float truncation |
| Marching Cubes vertex interpolation | $10^{-3}$ Å | GPU `f32` precision limit on voxel edge positions |

### 2.2 ColMajor Enforcement
All matrices crossing the C++/Rust FFI boundary must be **Column-Major** (`Eigen::Matrix<double, 3, 3, Eigen::ColMajor>`). Tests must verify that lattice vectors returned from FFI are correctly ordered (column 0 = $\mathbf{a}$, column 1 = $\mathbf{b}$, column 2 = $\mathbf{c}$).

### 2.3 State Isolation
Every test must instantiate its **own** `CrystalState`. Global state pollution across tests is strictly prohibited. Helper functions (`make_fcc_al_state()`, `make_nacl_state()`, etc.) construct fresh instances per test.

### 2.4 Tolerance Lock
Test files carry the `[PROJECT RULE L0]` header comment: **Do not modify assert tolerances, time values, or thresholds.** Changing a tolerance requires explicit justification and reviewer approval.

---

## 3. Testing Stack

### Layer 1: C++ Physics Kernel — GoogleTest

**Location**: `cpp/tests/`  
**Build system**: CMake + GoogleTest v1.14.0 (via `FetchContent`)  
**Dependencies**: Eigen (header-only), Spglib (static)

**Build & Run**:
```bash
cd cpp/tests
cmake -B build -DCMAKE_BUILD_TYPE=Debug
cmake --build build
cd build && ctest --output-on-failure
```

**Existing test suites**:

| File | Node | Tests |
|---|---|---|
| `test_spglib_robustness.cpp` | 3.1 | Spglib identification edge cases (P1 fallback, high-symmetry, distorted cells) |
| `test_supercell_eigen.cpp` | 3.2 | Supercell expansion determinant, atom count, lattice vector scaling |
| `test_slab_eigen.cpp` | 3.3 | Slab lattice orthogonality ($\alpha = \beta = 90°$), unimodular $\det P = 1$ |
| `test_slab_surface_basis.cpp` | S1 | Diophantine solver correctness for all Miller index families |
| `test_slab_builder.cpp` | S2 | `build_slab_v2` end-to-end: atom deduplication, vacuum injection, density preservation |
| `test_slab_layers.cpp` | S3 | `cluster_slab_layers` / `shift_slab_termination`: layer detection tolerance, out-of-range rejection |

**Key pattern — C++ tests enforce ColMajor explicitly**:
```cpp
// ColMajor storage: columns are basis vectors
Eigen::Matrix3d make_sc(double a) {
    Eigen::Matrix3d L = Eigen::Matrix3d::Identity() * a;
    return L;  // ColMajor by default in Eigen
}
```

### Layer 2: Rust / Tauri Backend — `cargo test`

**Location**: `src-tauri/tests/`  
**Run all**:
```bash
cd src-tauri && cargo test -- --nocapture
```

**Run specific test**:
```bash
cargo test --test test_slab_v2_ffi -- --nocapture
```

**Existing test suites**:

| File | Node | Tests |
|---|---|---|
| `test_cif_parsing.rs` | 1.x | CIF/PDB parser via Gemmi FFI: lattice params, atom sites, spacegroup |
| `test_ffi_roundtrip.rs` | 2.x | Rust↔C++ data roundtrip: `parse_cif_file`, `get_spacegroup`, position translation |
| `test_importers.rs` | 4.x | POSCAR, QE, XYZ, XSF, Cube file parser correctness |
| `test_exporters.rs` | 4.x | File export format verification |
| `test_fe2o3_spacegroup.rs` | 3.1 | Spacegroup detection for Fe₂O₃ (R-3c, #167) |
| `test_slab_v2_ffi.rs` | S4 | `generate_slab` via `CrystalState`: orthogonality, density, termination shifts |
| `test_overlap_detection.rs` | S5 | MIC-based atom collision detection (`check_overlap_mic`) |
| `test_ray_picking.rs` | 7.x | CPU ray-sphere intersection for atom selection |
| `test_rutile_polyhedra.rs` | 6.x | Bond analysis and coordination shell detection for TiO₂ |
| `test_mc_breaker.rs` | 11.4a | Marching Cubes stress tests: pathological inputs (zero matrices, empty grids, Euler characteristic validation) |
| `test_command_schema.rs` | 5.1 | LLM command security: malicious JSON injection rejection, `deny_unknown_fields`, negative index handling |
| `test_deepseek_live.rs` | 5.x | Live LLM API integration (requires API key, disabled in CI) |

### Layer 3: Frontend (React/Vite)

No automated test runner is currently configured for the frontend. Manual testing focuses on:
- IPC payload mapping: camelCase (TypeScript) ↔ snake_case (Rust) via Tauri's automatic renaming
- `state_changed` event propagation: UI panels re-fetch `CrystalState` after backend mutations
- `version` counter monotonicity: each mutation increments the version, preventing stale renders

**Note**: Adding Vitest or Playwright is on the backlog (see roadmap P3).

---

## 4. Test Data

Reference structures are stored in `tests/data/` and used by both C++ and Rust test suites:

| File | Structure | Used By |
|---|---|---|
| `nacl.cif` | NaCl (Fm-3m, #225) | Parser, supercell, slab tests |
| `si_bulk.cif` | Silicon (Fd-3m, #227) | Spacegroup, Niggli reduction |
| `rutile.cif` | TiO₂ rutile (P4₂/mnm, #136) | Bond analysis, coordination |
| `graphene.cif` | Graphene monolayer | 2D detection, 2D BZ |
| `mos2_monolayer.cif` | MoS₂ monolayer | 2D BZ, k-path |
| `diamond.cif` | Diamond (Fd-3m) | Supercell, slab |
| `slab_a_vacuum.cif` | Pre-built slab with vacuum | Vacuum detection |
| `Fe2O3/` | Fe₂O₃ (R-3c, #167) | Spacegroup edge case |
| `POSCAR/` | VASP POSCAR examples | POSCAR parser |
| `scf.in` | Quantum ESPRESSO input | QE parser |

---

## 5. Writing a Physical Test

A good test validates **geometry**, not just counting. Use the following template:

```rust
#[test]
fn test_slab_diophantine_geometry() {
    let cs = make_fcc_al_state();    // Fresh isolated state
    let slab = cs.generate_slab([1, 1, 1], 3, 10.0).unwrap();

    // 1. Density preservation:
    //    nᵢₙ × layers = nₒᵤₜ
    assert_eq!(slab.intrinsic_sites, cs.intrinsic_sites * 3);

    // 2. Surface orthogonality:
    //    α = β = 90° (c-axis perpendicular to the surface plane)
    assert!((slab.cell_alpha - 90.0).abs() < 1e-5,
            "α = {}, expected 90°", slab.cell_alpha);
    assert!((slab.cell_beta  - 90.0).abs() < 1e-5,
            "β = {}, expected 90°", slab.cell_beta);

    // 3. Symmetry detection must not fail:
    assert_ne!(slab.spacegroup_number, 0,
               "Spglib should identify the slab symmetry");

    // 4. Vacuum present:
    assert!(slab.cell_c > cs.cell_c * 2.0,
            "Slab c-axis should include 10 Å vacuum");
}
```

**Key anti-patterns to avoid**:
- ❌ Testing only atom count without geometric assertions
- ❌ Reusing a `CrystalState` across multiple `#[test]` functions
- ❌ Hardcoding expected atom positions (brittle to parser updates) — use relative checks instead
- ❌ Modifying tolerance constants without explicit justification in the PR description

---

## 6. Stress & Security Tests

Stress tests intentionally feed pathological inputs to verify graceful failure:

### Physics Stress Tests (`test_mc_breaker.rs`)
- Zero-volume lattice matrices
- Empty scalar fields (all zeros)
- Extreme isovalue thresholds ($\tau = 0$, $\tau = \pm\infty$)
- Euler characteristic validation: $\chi = V - E + F = 2$ for closed surfaces

### LLM Security Tests (`test_command_schema.rs`)
- Negative atom indices → `serde` type rejection (u32)
- Unknown fields in JSON → `#[serde(deny_unknown_fields)]` rejection
- Missing required fields → deserialization error
- Buffer overflow attempts via extremely large index arrays
- Physically impossible parameters (negative cell volume, zero-length bonds)

### Pattern:
```rust
#[test]
fn test_negative_index_rejected() {
    let json = r#"{"action": "delete_atoms", "params": {"indices": [-1]}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Negative index should be rejected by u32 type");
}
```

---

## 7. CI/CD Integration (Future)

Currently, all tests are run locally. The planned CI pipeline:

```
┌─────────────┐    ┌──────────────┐    ┌─────────────┐
│  C++ GTest  │───▶│  cargo test  │───▶│  cargo build │
│  cpp/tests/ │    │  src-tauri/  │    │  (release)   │
└─────────────┘    └──────────────┘    └─────────────┘
        │                  │                   │
        ▼                  ▼                   ▼
   ctest --output     --nocapture        tauri build
   -on-failure                           (macOS only)
```

**Target platforms** (priority order):
1. **P0**: macOS Intel (Metal 2.0)
2. **C1**: macOS Apple Silicon
3. **P2**: Ubuntu (Vulkan)
4. **P3**: Windows

GPU-dependent tests (`test_mc_breaker.rs` uses CPU fallback; the actual GPU compute path requires a Metal/Vulkan device) are gated by `#[cfg(feature = "gpu-tests")]`.

---

*Cross-references: [Algorithms.md](./Algorithms.md) · [IPC_Commands.md](./IPC_Commands.md) · [DeveloperGuide.md](./DeveloperGuide.md)*
