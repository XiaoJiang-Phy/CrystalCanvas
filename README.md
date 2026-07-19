<p align="center">
  <img src="logo.svg" width="200" alt="CrystalCanvas Logo">
</p>

# CrystalCanvas

**GPU-accelerated desktop visualization for crystal structures, volumetric fields, phonons, Wannier models, and reciprocal space.**

CrystalCanvas is a research-oriented visualization application built with Rust, wgpu/WGSL, C++, React, and Tauri. Its purpose is deliberately narrow: turn structural, real-space, and reciprocal-space data into clear interactive scenes and reproducible publication figures.

The project does not aim to become a general electronic-structure solver, workflow manager, database browser, or AI research platform. DFT, DFPT, Wannier, EPC, transport, superconductivity, and many-body calculations remain in specialized external codes. CrystalCanvas focuses on the part ordinary two-dimensional plotting tools handle poorly: structure-aware three-dimensional scientific visualization.

> **Latest release**: `v0.6.1`
>
> **Current development line**: `v0.6.2` scientific-workbench and interaction hardening
>
> **Primary platform**: macOS Intel and Apple Silicon

---

## Download

[![Download for macOS](https://img.shields.io/badge/Download_v0.6.1-macOS_(Intel_%26_Apple_Silicon)-007AFF?style=for-the-badge&logo=apple)](https://github.com/XiaoJiang-Phy/CrystalCanvas/releases/latest)

> [!WARNING]
> The application is not signed with a paid Apple Developer certificate. If macOS blocks the first launch, move `CrystalCanvas.app` to `/Applications`, right-click it, choose **Open**, and confirm once more.

If necessary, the quarantine attribute can be cleared manually:

```bash
sudo xattr -cr /Applications/CrystalCanvas.app
```

---

## Current Capabilities

### Crystal structures

- CIF, POSCAR, PDB, XYZ, and Quantum ESPRESSO structure input.
- GPU impostor-sphere atoms, bonds, unit-cell boundaries, labels, selection, and measurement overlays.
- Intrinsic-atom editing with undo/redo, partial occupancy, supercells, slab construction, and cell standardization.
- Niggli, primitive, and conventional transformations through Spglib-backed kernels.
- VASP, Quantum ESPRESSO, and LAMMPS export.

### Volumetric fields

- CHGCAR, Gaussian Cube, and XSF scalar grids.
- GPU Marching Cubes isosurfaces and volume raycasting.
- Signed positive/negative isosurfaces for orbitals and difference-density style data.
- Scientific colormaps, controllable isovalues, render modes, and structure/field co-visualization.

### Phonons and reciprocal space

- Phonon mode import and eigenvector animation.
- Three-dimensional and two-dimensional Brillouin-zone visualization.
- High-symmetry point and path presentation.
- Wannier90 `wannier90_hr.dat` hopping-network visualization with orbital, lattice-shell, and magnitude filters.

### Reliability baseline

- Rust `CrystalState` is the only committed physical-state authority.
- Renderer-only periodic images and Wannier ghosts never enter physical arrays.
- Structural mutations are atomic across validation, undo, version, state, and renderer resources.
- Typed and inventoried Tauri IPC contracts connect Rust and TypeScript.
- One versioned `state_changed` path owns complete frontend snapshot refresh.

---

## Visualization-First Product Policy

Every new feature must directly improve one of four outcomes:

1. interactive understanding of a structure or three-dimensional scientific field;
2. faithful co-visualization of structure, real-space, or reciprocal-space data;
3. publication-quality image production;
4. reproducible scene and export configuration.

The renderer has two intentionally different budgets:

| Mode | Priority | Policy |
|---|---|---|
| Interactive viewport | responsiveness and low idle cost | compact opaque UI, no persistent decorative effects, simple full rebuilds until profiling proves otherwise |
| Publication export | image fidelity | capability-checked high sampling, advanced lighting, tiled high-resolution rendering, accurate legends and reproducible settings |

SSAO, soft shadows, contact shadows, MSAA/SSAA, and 4K/8K tiled rendering belong to the publication path. They are not required to consume continuous resources in the interactive viewport.

Quantitative scalar colors must remain interpretable. Lighting and ambient occlusion may enhance geometric depth, but colorbars, scalar ranges, sign conventions, and unlit scientific-color modes must remain available and must not be silently altered by presentation effects.

---

## Roadmap

| Version | Theme | Scope |
|---|---|---|
| `v0.6.2` | Scientific Workbench Hardening | finish the compact desktop workbench, coalesced atom dragging, renderer-driven phonon animation, event lifecycle gates, and evidence-based performance baselines |
| `v0.7.0` | Publication Rendering Core | add a separate high-fidelity export path with reproducible cameras and materials, advanced lighting, transparent backgrounds, antialiasing, and tiled 4K/8K output |
| `v0.8.0` | Advanced Volumetric and Field Figures | compose multiple scalar-field layers with signed isosurfaces, slices, contours, clipping, transfer functions, correct transparency, quantitative colorbars, and units |
| `v0.9.0` | Reciprocal Space and Fermi Surfaces | visualize Fermi-surface sheets, Brillouin-zone clipping, cutting planes, isoenergy surfaces, and imported reciprocal-space quantities such as velocity, lifetime, EPC strength, spectral weight, or superconducting gap |
| `v1.0.0` | Reproducible Scientific Figure Workspace | combine structure, field, vector, and reciprocal-space layers with saved cameras, annotations, legends, export profiles, project files, and optional batch rendering |

CrystalCanvas may visualize outputs from DFT, DFPT, Wannier, EPC, transport, superconductivity, TCI, or many-body codes, but it will not embed their numerical solvers or research-assessment workflows.

Future private or self-developed data formats are not designed in advance. The architecture reserves a source-adapter boundary, but no plugin system, custom container, or conversion framework will be added before a real dataset and visualization requirement exist.

See [ROADMAP.md](ROADMAP.md) for the public release plan.

---

## Architecture

```text
┌──────────────────────────────────────────────────────────┐
│ L4  React + TypeScript + TailwindCSS                    │
│     Desktop workbench, panels, dialogs, typed IPC       │
├──────────────────────────────────────────────────────────┤
│ L3  Rust + Tauri                                        │
│     CrystalState SSoT, transactions, validation, I/O    │
├──────────────────────────────────────────────────────────┤
│ L2  Rust + wgpu + WGSL                                  │
│     Interactive renderer and publication export         │
├──────────────────────────────────────────────────────────┤
│ L1  C++ + Eigen + Spglib + Gemmi                        │
│     Stateless crystallographic and geometry kernels      │
└──────────────────────────────────────────────────────────┘
```

Core constraints:

- `f64` for physical and crystallographic computation; `f32` for GPU presentation.
- Explicit column-major lattice layout across Rust, C++, and any future external adapter.
- Thin synchronous C++ wrappers; exceptions never cross the FFI boundary.
- WGSL is the only shader language and must pass the project wgpu/naga validation path.
- React snapshots are read-only projections, not a second physical-state store.
- Importers normalize supported source files before renderer consumption; the renderer does not branch on producer-specific formats.

---

## Development Setup

### Prerequisites

- macOS with Xcode Command Line Tools
- `pnpm`

The repository keeps Rust and project dependencies local rather than polluting a global development environment.

```bash
git clone https://github.com/XiaoJiang-Phy/CrystalCanvas.git
cd CrystalCanvas
source dev_env.sh
pnpm install --frozen-lockfile
pnpm run tauri dev
```

The Rust build integrates the C++ kernel through `build.rs` and the thin C++ bridge.

### Standard verification

```bash
source dev_env.sh && cargo check --manifest-path src-tauri/Cargo.toml
source dev_env.sh && cargo test --no-fail-fast --manifest-path src-tauri/Cargo.toml
cmake --build cpp/tests/build
ctest --test-dir cpp/tests/build --output-on-failure
pnpm install --frozen-lockfile
npm run ipc:inventory
npm run check:ipc
npm run test:ipc
./node_modules/.bin/tsc --noEmit
pnpm run build
git diff --check
```

---

## Repository Layout

```text
CrystalCanvas/
├── src/                    # React/TypeScript desktop workbench
├── src-tauri/              # Rust application, I/O, renderer, commands, WGSL
├── cpp/                    # Stateless C++ crystallographic kernels and tests
├── ipc/                    # Reviewed command/event inventory
├── scripts/                # Contract and UI gates
├── tests/                  # Test fixtures
├── docs/                   # Public user/developer documentation
├── doc/                    # Internal architecture and execution plans
├── ROADMAP.md              # Public release direction
└── README.md
```

---

## Documentation

- [User Manual](docs/UserManual.md)
- [Developer Guide](docs/DeveloperGuide.md)
- [Algorithms](docs/Algorithms.md)
- [IPC Commands](docs/IPC_Commands.md)
- [Shader Reference](docs/Shader_Reference.md)
- [Testing Guide](docs/TestingGuide.md)
- [Internal Technical Design](doc/TDD_CrystalCanvas_v1.md)

---

## Platform Policy

| Priority | Platform | Policy |
|---|---|---|
| P0 | macOS Intel / Metal 2.0 | baseline compatibility and performance target |
| C1 | macOS Apple Silicon / Metal | continuous secondary verification |
| P2 | Ubuntu / Vulkan | build, shader, and selected rendering verification |
| P3 | Windows | deferred until required by the maintainer's workflow |

---

## Contributing

Focused bug reports and contributions are welcome. New features are accepted according to the visualization-first scope above; broad platform or workflow expansion is not assumed to be a project goal.

See [CONTRIBUTING.md](CONTRIBUTING.md) for development conventions.

---

## License

CrystalCanvas is dual-licensed under the MIT License and the Apache License 2.0.

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)

Third-party license information is recorded in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

---

## Acknowledgments

- [Spglib](https://spglib.github.io/spglib/) — crystal symmetry
- [Gemmi](https://gemmi.readthedocs.io/) — crystallographic file handling
- [Eigen](https://eigen.tuxfamily.org/) — linear algebra
- [Tauri](https://tauri.app/) — desktop application framework
- [wgpu](https://wgpu.rs/) — cross-platform GPU API
