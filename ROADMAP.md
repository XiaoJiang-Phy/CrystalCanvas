# CrystalCanvas Roadmap

> Updated 2026-07-18 | Latest release: `v0.6.1` | Primary target: macOS / Metal

## Product Direction

CrystalCanvas is a visualization-first desktop application for crystal structures and three-dimensional scientific data. Its long-term value is publication-quality rendering of structure-aware real-space and reciprocal-space scenes, not breadth for its own sake.

The project prioritizes:

- crystal structures, bonds, cells, measurements, and structural overlays;
- volumetric fields, isosurfaces, slices, contours, and transparent multi-layer scenes;
- phonon eigenvectors and other spatial or modal vector data;
- Brillouin zones, Wannier networks, Fermi surfaces, and reciprocal-space scalar overlays;
- reproducible high-quality image export.

The project does not plan to become a DFT, EPC, transport, superconductivity, TCI, DMFT, or general workflow solver. Such programs may produce data that CrystalCanvas visualizes, but their numerical methods and research interpretation remain outside this application.

## Release History

| Version | Date | Highlights |
|---|---|---|
| `v0.1.0` | 2026-03-06 | GPU atom renderer, structure I/O, initial phonon and command surfaces |
| `v0.2.0` | 2026-04-05 | slab and supercell geometry |
| `v0.3.0` | 2026-04-09 | CHGCAR/Cube/XSF, GPU Marching Cubes, volume raycasting |
| `v0.4.0` | 2026-04-10 | 3D/2D Brillouin zones and cell standardization |
| `v0.5.0` | 2026-04-11 | Wannier hopping visualization and toolbar redesign |
| `v0.6.0` | 2026-04-14 | measurement, undo/redo, partial occupancy, modular commands and panels |
| `v0.6.1` | 2026-07-18 | verified IPC contracts, intrinsic-only state, atomic transactions, physical input gates, single versioned snapshot refresh |

## Planned Releases

### `v0.6.2` — Scientific Workbench Hardening

- finish the compact opaque desktop workbench and remaining density corrections;
- make the experimental Assistant closed by default and freeze further product expansion;
- coalesce atom dragging into renderer previews plus one committed mutation;
- move phonon phase updates out of per-frame React-to-Tauri IPC;
- close native event and browser-mock lifecycle gaps;
- measure 500–10,000 atom interaction and rendering behavior before introducing complex performance protocols.

This version does not add new scientific domains. It prepares a stable visualization platform for later rendering work.

### `v0.7.0` — Publication Rendering Core

- separate the low-cost interactive viewport from a high-fidelity offscreen export path;
- capability-checked MSAA/SSAA and deterministic fallback;
- SSAO, soft shadows, and contact shadows for geometric depth;
- transparent background and explicit color-space handling;
- orthographic and perspective camera presets;
- reproducible materials, lighting, atom radii, bond radii, and scene settings;
- tiled 4K/8K export without requiring continuous high-cost rendering in the viewport.

### `v0.8.0` — Advanced Volumetric and Field Figures

- multiple scalar-field layers in one structure-aware scene;
- signed isosurfaces, slices, contours, clipping planes, and transfer functions;
- correct transparent-surface composition;
- quantitative colorbars, units, ranges, and unlit scientific-color profiles;
- visualization-oriented linear combinations such as difference density when source grids are compatible;
- publication export for charge density, orbitals, ELF, electrostatic potential, spin density, and other imported scalar fields.

### `v0.9.0` — Reciprocal Space and Fermi Surfaces

- `.bxsf` and explicitly reviewed reciprocal-space sources;
- Fermi-surface extraction, band/sheet selection, and Brillouin-zone clipping;
- reciprocal-space camera and unit isolation from real-space scenes;
- imported scalar overlays such as velocity, lifetime, EPC strength, spectral weight, or superconducting gap;
- cutting planes, isoenergy surfaces, legends, and publication-quality export.

CrystalCanvas visualizes supplied quantities; it does not calculate transport coefficients, EPC self-energies, or superconducting solutions.

### `v1.0.0` — Reproducible Scientific Figure Workspace

- composable structure, field, vector, and reciprocal-space layers;
- saved cameras, visibility, clipping, transfer functions, materials, and export profiles;
- axes, legends, colorbars, labels, scale bars, and annotations;
- multi-view figure layouts where they add value beyond ordinary 2D plotting tools;
- reproducible project files and optional batch rendering of approved scenes.

## Data-Format Policy

Existing stable public formats may have built-in parsers. Future private or self-developed formats receive no speculative implementation.

The architecture reserves only this boundary:

```text
source-specific importer or converter
    → normalized visualization data
    → renderer and export pipeline
```

No custom CrystalCanvas container, plugin runtime, dynamic script loader, or converter framework will be designed until a real dataset, coordinate convention, scale, and target visualization are known.

## Deferred or Removed Directions

- general LLM chat, memory, RAG, multi-agent, or autonomous workflows;
- embedded DFT, EPC, transport, superconductivity, or many-body solvers;
- generic material-database browsing;
- moiré generators without a concrete visualization requirement;
- magnetic or topological analysis products without an active research visualization need;
- decorative rendering features that do not improve scientific depth perception or publication output;
- speculative delta snapshots, dirty GPU ranges, spatial indexes, or zero-copy protocols without benchmark evidence;
- broad Windows support until it is required by the maintainer's actual workflow.

## Platform Support

| Priority | Platform | Policy |
|---|---|---|
| P0 | macOS Intel / Metal 2.0 | baseline development, compatibility, and performance target |
| C1 | macOS Apple Silicon / Metal | continuous secondary verification |
| P2 | Ubuntu / Vulkan | build, shader, and selected rendering verification |
| P3 | Windows | deferred |
