# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-04-05

### Fixed
- **🔧 Slab Cleaving Algorithm**: Complete rewrite of the slab generation pipeline, resolving the critical topology bug reported in v0.1.0-alpha.
  - **Root Cause 1 — Eigen Memory Layout**: `EIGEN_DEFAULT_TO_ROW_MAJOR=0` in CMakeLists.txt paradoxically enabled RowMajor storage due to Eigen's `#ifdef` (existence check) vs `#if` (value check) preprocessor semantics. All non-diagonal C++↔Rust matrix transfers were silently transposed. Removed the erroneous define.
  - **Root Cause 2 — Surface Basis Collapse**: The brute-force shortest-vector search produced transformation matrices with `det(P) > 1`, causing fractional z-coordinates to collapse to identical values after modular wrapping. Replaced with a Diophantine equation solver guaranteeing `det(P) = 1` (unimodular).
  - **Root Cause 3 — Tilted C-axis**: The out-of-plane vector v₃ was not perpendicular to the surface, producing non-physical angles (e.g., α = 45° instead of 90°). Added Gram-matrix decomposition to absorb the in-plane tilt component into fractional (x, y) coordinates and force the c-axis along the surface normal.

### Added
- **Diophantine Surface Basis** (`get_surface_basis`): Pure-integer construction via Extended Euclidean Algorithm. Properties: `v₁·G = v₂·G = 0` (in-plane), `v₃·G = 1` (unit interplanar step), `det(P) = 1`. O(1) complexity vs previous O(N³) brute-force search.
- **2D Gauss Lattice Reduction**: Post-processes in-plane basis vectors toward orthogonality using the lattice metric tensor, improving slab cell shape without changing surface area.
- **C-axis Orthogonalization**: Decomposes `c_tilted = α·a + β·b + h·n̂` via Gram system, absorbs (α, β) into fractional coordinates, and sets c = (h + vacuum)·n̂. Guarantees α = β = 90° for all Miller indices.
- **QR Standardization**: Householder QR decomposition rotates the final lattice to PDB convention (a‖X, b in XY plane) without altering fractional coordinates.
- **P1 Guard**: Slab generation now rejects primitive cells (`spacegroup = P1`) with a clear error message, since Miller indices are defined relative to conventional axes.
- **Killer Tests**: Added NaCl (110) regression tests that verify distinct z-layer separation for both primitive and supercell inputs — the exact failure mode of the original bug.

### Changed
- **Rust FFI Layer**: Removed the fragile Rust-side `rebase_matrix` coordinate transformation. The C++ kernel now outputs lattice matrices directly in PDB-standard orientation.
- **Test Infrastructure**: All slab test helpers updated to use conventional unit cells (FCC Al → Fm-3m #225, SC → Pm-3m #221) instead of P1 primitive cells.


## [0.1.0-alpha] - 2026-03-06

### Added
- **Alpha Release**: Initial MVP build.
- **High-Performance Renderer**: `wgpu` based orbital camera with Impostor Sphere and Ray-Picking for ~1000 atomic structures at 60 FPS.
- **Physics Kernel**: C++ engine with `Spglib` Space Group Analysis and `Gemmi` high-speed CIF parser integration via Rust FFI (`cxx`).
- **Interactive Modeling**: Real-time addition, deletion, element replacement, and supercell generation.
- **DFT/MD Native IO**: Built-in VASP (POSCAR), LAMMPS (Data), and Quantum ESPRESSO geometry exporters.
- **LLM Command Bus**: Context-aware natural language interface via OpenAI/DeepSeek API integration (Secure OS Keychain storage).
- **Phonon Analysis**: Import `.mold`/`.dat` and QE `modes` to visualize animated imaginary frequencies and atomic displacements.
- **Event-Driven UI**: Deeply decoupled Tauri Command bindings with React custom Hooks for seamless state propagation.

### Known Issues
- **⚠️ Platform Support**: Due to rendering engine (`wgpu`) backend compatibility issues, **Windows** and **Ubuntu (Linux)** versions are temporarily broken (Blank/Transparent screen or extreme lag). For this `v0.1.0` alpha, **only macOS is officially supported and available for download**. We plan to fix the Vulkan/DX12 rendering pipelines in a future release.
- **⚠️ Slab Cleaving (Cutting Plane) Topologies**: ~~The "Cutting Plane" feature currently functions correctly from a purely coordinate export perspective (underlying data is clean and isolated). However, it occasionally produces mathematically offset or visually incorrect unphysical topologies for low-symmetry crystals.~~ **Resolved in v0.2.0** — complete algorithm rewrite using Diophantine surface basis and c-axis orthogonalization.

### Security
- **Strict Data Pipeline**: Enforced separation between `f64` calculation models and `f32` render projections to avoid floating-point drift accumulation in `.cif` to `.in` workflows.
