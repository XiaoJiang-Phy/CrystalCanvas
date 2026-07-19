# CrystalCanvas Algorithms and Implementation Notes

> Baseline: `v0.6.1` | Updated: 2026-07-19

This document describes current visualization and geometry implementations. It is not a validation report for a particular material, calculation, or source file. Numerical or physical claims about a supplied structure require declared conventions and evidence; see [TestingGuide.md](TestingGuide.md).

---

## Conventions

- Lattice matrices at the Rust/C++ boundary are column-major. Their columns are the basis vectors $\mathbf a$, $\mathbf b$, and $\mathbf c$.
- Intrinsic structure data are held in Rust `CrystalState`. Periodic renderer images and Wannier ghosts are presentation-only objects.
- Crystal and geometry computation use `f64`; renderer instance data and WGSL use `f32`.
- A successful committed structural operation is atomic across validation, versioning, undo/redo ownership, and renderer-scene replacement.

---

## Slab construction

**Implementation**: `cpp/src/physics_kernel.cpp`, `src-tauri/src/crystal_state.rs`, and `src-tauri/src/commands/geometry.rs`

The C++ kernel constructs an integer surface basis from a non-zero Miller triplet. For a normalized Miller direction, the first two column vectors of the column-major basis lie in the surface plane; the third is the complementary direction used to build the layered supercell. The Rust caller validates finite lattice parameters, bounded Miller indices, positive layer count, resource limits, and non-negative finite vacuum before calling the FFI routine.

The current slab path:

1. constructs and validates an integer surface-basis expansion;
2. scales the complementary basis direction by the requested layer count;
3. builds the expanded structure and removes coincident Cartesian positions within the kernel's declared duplicate threshold;
4. derives the surface normal from the in-plane output lattice vectors;
5. removes in-plane components from the output $\mathbf c$ direction, adds vacuum along the surface normal, and remaps fractional coordinates; and
6. reconstructs a new intrinsic `CrystalState` through the transaction path.

The current user-facing guard rejects P1 input because the UI interprets Miller indices relative to conventional axes. Preview is non-committing; apply is atomic. Slab regression work must use real declared fixtures and independently assert layers, termination, stoichiometry, shortest distances, and failure atomicity before changing the algorithm.

---

## Brillouin-zone construction

**Implementation**: `src-tauri/src/brillouin_zone.rs`, `src-tauri/src/kpath.rs`, `src-tauri/src/kpath_2d.rs`, and `src-tauri/src/renderer/bz_renderer.rs`

The Brillouin zone is presented as a Wigner-Seitz construction of the reciprocal lattice. The implementation enumerates a bounded reciprocal shell, forms bisecting half-spaces, and clips a convex initial region. It separately supports the application's two-dimensional classification path and its associated high-symmetry-path presentation.

The BZ overlay is a renderer scene derived from the committed lattice. It is not a band calculation, topology diagnosis, or transport calculation. Any claim about a particular material's reciprocal-space convention requires the source calculation's declared basis, periodic axes, and unit convention.

---

## Scalar-field visualization

**Implementation**: `src-tauri/src/volumetric.rs`, `src-tauri/src/renderer/isosurface.rs`, `src-tauri/src/renderer/volume_raycast.rs`, and `src-tauri/shaders/`

Supported scalar-grid importers normalize their grid metadata and lattice information before renderer use. The UI enables data-dependent controls only after receiving finite volumetric metadata with a usable non-zero range.

### Marching Cubes

The compute shader dispatches 4×4×4 workgroups over grid cells, classifies each cell against the chosen isovalue, interpolates edge crossings, and appends generated vertices to a bounded GPU buffer through an atomic counter. Positive, negative, and dual-sign presentation modes are renderer options; they do not modify the imported scalar field.

### Direct volume rendering

The volume path renders a fractional unit cube, transforms it with the field lattice, intersects rays with that cube, samples the scalar field, and composites samples front-to-back. Opaque-scene depth participates in the presentation path. Colormaps, cutoffs, and opacity are visualization parameters; record them with any quantitative figure.

---

## Atom, bond, and measurement presentation

**Implementation**: `src-tauri/src/renderer/instance.rs`, `src-tauri/src/renderer/ray_picking.rs`, `src-tauri/src/commands/analysis.rs`, and `src-tauri/src/commands/viewport.rs`

Atoms are rendered as analytic impostor spheres. Bonds are rendered by a WGSL vertex shader that expands each instance into a 12-segment cylinder; no geometry shader is used. Bond analysis applies the current covalent-radius and threshold settings under the project's periodic-distance conventions, then returns bond, coordination, and summary data to the panel.

Picking converts a screen point into a camera ray and chooses the nearest valid atom hit in the prepared renderer pick scene. That scene retains a source intrinsic-atom index so a boundary image cannot become an independent physical selection.

Distance, angle, and dihedral measurements are stored as overlay definitions referencing intrinsic indices. They are part of the versioned snapshot but are not extra atoms.

---

## Phonon and Wannier presentation

**Implementation**: `src-tauri/src/phonon.rs`, `src-tauri/src/wannier.rs`, and their command/panel modules

Phonon mode selection and phase update renderer presentation coordinates without committing a new structure. The current frontend frame path is retained pending dedicated interaction-animation work.

Wannier hopping networks are read from `wannier90_hr.dat`, filtered by orbital, lattice shell, magnitude, and visibility settings, and drawn as renderer overlay instances. Neighboring-cell endpoints may introduce ghost visuals, but those visuals are not added to `CrystalState` atom arrays or frontend atom tables.

---

## References and change discipline

The implementation draws on established techniques such as Wigner-Seitz reciprocal cells, Marching Cubes, direct volume rendering, periodic minimum-image calculations, and analytic impostor rendering. References should be added or updated only with the algorithm provenance actually used by the code.

Before modifying a physical algorithm, first establish a specific failing structure or data fixture, run the applicable scientific guard, and add an independent regression gate. Do not rewrite an algorithm from general expectations alone.
