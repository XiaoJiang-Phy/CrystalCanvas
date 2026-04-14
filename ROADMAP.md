# CrystalCanvas Roadmap

> Updated 2026-04-14 | Development Baseline: macOS (Intel + Apple Silicon)

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

---

## Planned Releases

### v0.7.0 — Condensed Matter Core

- **Charge density difference**: Load two CHGCAR files and compute element-wise subtraction in-app. Result rendered via the existing signed isosurface pipeline (red/blue dual-color).
- **Collinear magnetic moments**: Parse VASP `MAGMOM` tags and render spin-up/down arrows on atomic sites.
- **MSAA anti-aliasing**: 4x MSAA for improved screenshot quality.

### v0.8.0 — Reciprocal-Space Physics

- **3D Fermi surface viewer**: Parse Wannier90 `.bxsf` files, extract isosurfaces at the Fermi level via GPU Marching Cubes, render inside the Brillouin Zone wireframe.
- **Non-collinear magnetism**: Full 3D magnetic moment arrows with arbitrary orientation.

### v0.9.0+ — Flagship Features & Scalability

- **Moire superlattice generator**: Commensurate angle search for twisted bilayer systems (graphene, h-BN, TMDs) based on coincidence site lattice theory.
- **High-quality rendering engine**: SSAO, soft shadows, tiled rendering for 4K+ export.
- **Symmetry element overlay**: Render rotation axes, mirror planes, and inversion centers.
- **Julia / Python IPC bridge**: Shared memory interface for external scripts to drive the 3D viewport.

---

## Platform Support

| Priority | Platform | Status |
|---|---|---|
| P0 | macOS (Intel + Apple Silicon) | Fully supported |
| P1 | Ubuntu 22.04+ | CI tested, may have rendering quirks |
| P2 | Windows 10/11 | Community driven, deferred |
