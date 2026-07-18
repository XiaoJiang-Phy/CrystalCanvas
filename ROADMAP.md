# CrystalCanvas Roadmap

> Updated 2026-07-18 | Current Release: v0.6.1 | Development Baseline: macOS (Intel + Apple Silicon)

---

## Release History

| Version | Date | Highlights |
|---|---|---|
| **v0.1.0** | 2026-03-06 | Impostor sphere renderer, CIF/PDB/XYZ import, POSCAR/LAMMPS/QE export, LLM agent, phonon animation |
| **v0.2.0** | 2026-04-05 | Diophantine slab algorithm, c-axis orthogonalization |
| **v0.3.0** | 2026-04-09 | Volumetric pipeline (CHGCAR/Cube/XSF), GPU Marching Cubes, volume raycasting, 10 colormaps |
| **v0.4.0** | 2026-04-10 | 3D/2D Brillouin Zone, k-path generator (14+5 Bravais types), cell standardization (Niggli/Primitive/Conventional) |
| **v0.5.0** | 2026-04-11 | Wannier tight-binding visualizer, icon toolbar UI redesign |
| **v0.6.0** | 2026-04-14 | Measurement tool, Undo/Redo stack, fractional occupancy, modular architecture refactor |
| **v0.6.1** | 2026-07-18 | Verified IPC contracts, atomic structural transactions, physical input gates, single versioned state refresh |

---

## Planned Releases

### v0.6.2 — Interaction & Geometry Hardening

**Current focus:** improve high-frequency interaction and geometry coverage on the v0.6.1 reliability baseline.

- **Atom drag sessions**: Coalesce pointer-rate updates into one validated commit and one undo entry.
- **Backend-driven phonon animation**: Advance animation phase in Rust or WGSL instead of issuing per-frame frontend IPC calls.
- **Unconventional slab regression suite**: Cover large, negative, mixed, and non-coprime Miller indices across cubic, FCC, BCC, hexagonal, and oblique cells.
- **Evidence-based profiling**: Characterize state serialization, scene rebuild, GPU upload, picking, and animation at 500–10,000 intrinsic atoms before introducing delta protocols or dirty ranges.

### v0.7.0 — Condensed Matter Core

- **Charge-density linear combinations**: Compute validated combinations such as $\Delta\rho=\rho_{\mathrm{total}}-\rho_{\mathrm{substrate}}-\rho_{\mathrm{adsorbate}}$ after checking grid, lattice, origin, normalization, and units. Reuse the signed isosurface pipeline.
- **Collinear magnetic moments**: Parse supplied moments from VASP/QE inputs or outputs, render spin-up/down arrows, and validate atom count and magnetic symmetry before classification claims.
- **Publication export quality**: Add capability-checked 4x MSAA with a predictable fallback path.

### v0.8.0 — Reciprocal-Space Physics

- **3D Fermi-surface viewer**: Parse `.bxsf` band grids, keep reciprocal-space units and camera state isolated from real space, extract $E_n(\mathbf{k})=E_F$ surfaces, and clip them to the selected Brillouin Zone.
- **Non-collinear magnetism**: Render supplied $\mathbf{m}=(m_x,m_y,m_z)$ vectors and use MagSpglib-compatible validation before magnetic-symmetry classification.
- **Topological-workflow guardrails**: Do not treat a visualization k-path as sufficient for topological classification; such workflows require all relevant high-symmetry points and, for centrosymmetric systems, parity information at all eight TRIM points.

### v0.9.0+ — Flagship Features & Scalability

- **Moiré superlattice generator**: Commensurate angle search for twisted bilayer systems (graphene, h-BN, TMDs), with residual-strain and atom-count reporting.
- **High-quality rendering engine**: SSAO, soft shadows, tiled rendering for 4K+ export.
- **Symmetry element overlay**: Render rotation axes, mirror planes, and inversion centers.
- **Julia / Python bridge**: Expose a versioned, explicit column-major data contract for external scripts.

Large-scale features remain gated on measured 10,000-atom performance. Spatial indexes, dirty GPU ranges, and delta snapshots will be introduced only if profiling shows that the simpler full-rebuild strategy exceeds its budget.

---

## Platform Support

| Priority | Platform | Status |
|---|---|---|
| P0 | macOS (Intel + Apple Silicon) | Fully supported |
| P1 | Ubuntu 22.04+ | CI tested, may have rendering quirks |
| P2 | Windows 10/11 | Community driven, deferred |
