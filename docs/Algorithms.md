# CrystalCanvas Algorithms and Implementation Notes

> Baseline: `v0.6.1` | Updated: 2026-07-20

This document connects the current visualization and geometry algorithms to their implementation and regression gates. It guides contributors who change existing behavior. It is not a validation report for a material, calculation, or source file.

CrystalCanvas does not infer scientific meaning that is absent from an input file. Before changing a structure, Brillouin-zone, slab, phonon, or other physical path, declare the input convention and establish a reproducible fixture. See [TestingGuide.md](TestingGuide.md).

---

## Shared numerical and ownership conventions

### Coordinate transforms

At the Rust/C++ boundary, lattice matrices are flat column-major arrays whose columns are the basis vectors $\mathbf a$, $\mathbf b$, and $\mathbf c$:

$$
L = \begin{bmatrix} \mathbf a & \mathbf b & \mathbf c \end{bmatrix},
\qquad
\mathbf r = L\mathbf f
= f_a\mathbf a + f_b\mathbf b + f_c\mathbf c.
$$

Structure and geometry computations use `f64`. Prepared renderer instances and WGSL buffers use `f32`. Convert to renderer precision only after parsing, validation, and structural computation succeed.

Scalar grids use x-fastest flat indexing:

$$
i = i_x + n_x i_y + n_x n_y i_z.
$$

The corresponding implementation is visible in `src-tauri/src/renderer/isosurface.rs` and the volumetric parsers.

### Physical state versus presentation data

Rust `CrystalState` owns intrinsic atoms, the committed lattice, versioned selections and measurements, and attached data accepted for the current structure. Renderer-only periodic images, phonon-displaced coordinates, and Wannier endpoint ghosts are derived presentation data and must never be copied into intrinsic atom arrays.

A committed structural operation must be atomic across:

1. input validation and resource checks;
2. preparation of the candidate structure and renderer scene;
3. undo/version ownership; and
4. replacement of committed state and renderer buffers.

Preview commands may construct a candidate but do not create a version or undo entry. A failed preview or commit must leave the accepted state unchanged.

---

## Supercell generation

**Implementation**: `src-tauri/src/crystal_state.rs`, `src-tauri/src/commands/geometry.rs`, and `cpp/src/physics_kernel.cpp`

The low-level supercell expansion is a 3×3 integer matrix supplied to `CrystalState::generate_supercell` as a flat column-major array. The determinant determines the multiplicity, so the expected output atom count is

$$
N_{\mathrm{out}} = N_{\mathrm{in}}\,|\det S|.
$$

The Rust preflight rejects an empty structure, a singular or resource-exceeding expansion, invalid lattice parameters, and arithmetic or capacity overflow. It performs these checks before it allocates output buffers. The C++ kernel returns the transformed lattice, fractional positions, and atom types. Rust then reconstructs the candidate `CrystalState` and validates it again before commit.

The IPC shapes intentionally differ:

- `preview_supercell` receives the checked flat nine-integer `expansion` contract and returns a non-committed snapshot;
- `apply_supercell` receives a nested 3×3 `matrix`, adapts it at the command boundary for the kernel call, redetects the space group, and commits through the prepared-state transaction helper.

Do not change either public shape as a side effect of an algorithm edit. Add a contract migration and inventory update if the API itself must change.

---

## Slab construction

**Implementation**: `cpp/src/physics_kernel.cpp`, `src-tauri/src/crystal_state.rs`, and `src-tauri/src/commands/geometry.rs`

The slab path accepts a non-zero Miller triplet $(hkl)$, a positive layer count, and a finite non-negative vacuum thickness. Rust also applies bounded Miller-index, atom-count, determinant, and allocation checks before entering the C++ kernel.

The current construction proceeds as follows:

1. Reduce the Miller direction and construct a non-singular integer surface basis $S$.
2. Require the first two columns of $S$ to satisfy the surface-plane condition; the third column is a complementary lattice direction.
3. Multiply the complementary column by the requested layer count. The planned determinant magnitude therefore scales with the number of layers.
4. Generate the expanded structure and remove coincident Cartesian positions using the kernel's explicit duplicate tolerance.
5. Form the output surface normal from the in-plane lattice vectors,

   $$
   \hat{\mathbf n} =
   \frac{\mathbf a'\times\mathbf b'}
        {|\mathbf a'\times\mathbf b'|}.
   $$

6. Project the transformed $\mathbf c'$ direction onto $\hat{\mathbf n}$ to obtain the occupied height, then construct the final out-of-plane vector with the requested vacuum along that normal.
7. Remap atom positions into the final cell and reconstruct a validated intrinsic `CrystalState`.

The current UI rejects P1 input because its Miller indices are interpreted relative to conventional axes and no trustworthy conventional orientation can be inferred from P1 alone. `preview_slab` is non-committing; `apply_slab` redetects symmetry and commits once.

Slab changes require a real, declared regression matrix. At minimum, independently assert layer count, termination behavior, stoichiometry, shortest Cartesian separation, primitive/conventional equivalence where expected, and failure atomicity. A surprising structure is evidence to investigate, not permission to replace the algorithm from intuition alone.

---

## Brillouin-zone construction

**Implementation**: `src-tauri/src/brillouin_zone.rs`, `src-tauri/src/kpath.rs`, `src-tauri/src/kpath_2d.rs`, and `src-tauri/src/renderer/bz_renderer.rs`

For a real-space cell, the reciprocal basis follows

$$
\mathbf b_1 = 2\pi\frac{\mathbf a_2\times\mathbf a_3}
{\mathbf a_1\cdot(\mathbf a_2\times\mathbf a_3)},
$$

with cyclic permutations for $\mathbf b_2$ and $\mathbf b_3$. The three-dimensional Brillouin zone is built as the reciprocal-lattice Wigner–Seitz cell: a bounded neighbor shell supplies reciprocal points, each point defines a perpendicular bisecting half-space, and the implementation clips an initial convex region by those planes.

The two-dimensional path accepts two explicitly chosen in-plane real-space vectors. With $\mathbf n=\mathbf a_1\times\mathbf a_2$, it constructs

$$
\mathbf b_1 = 2\pi\frac{\mathbf a_2\times\mathbf n}{|\mathbf n|^2},
\qquad
\mathbf b_2 = 2\pi\frac{\mathbf n\times\mathbf a_1}{|\mathbf n|^2},
$$

clips a 2D Wigner–Seitz polygon, and embeds the polygon back into three dimensions. A degenerate in-plane area returns an empty `Unknown` result rather than fabricated geometry.

The BZ overlay and labeled k path are derived from the committed lattice. They are not a band calculation, topology diagnosis, transport result, or proof that an external calculation used the same reciprocal convention. When you add an importer, retain enough metadata to compare its basis, periodic axes, and $2\pi$ convention explicitly.

---

## Scalar-field visualization

**Implementation**: `src-tauri/src/volumetric.rs`, `src-tauri/src/renderer/isosurface.rs`, `src-tauri/src/renderer/volume_raycast.rs`, and `src-tauri/shaders/`

Volumetric importers produce grid dimensions, x-fastest scalar samples, origin, lattice, and finite minimum/maximum metadata. UI controls are data-dependent and must remain unavailable until a usable grid and non-zero finite range have been accepted.

### Marching Cubes

The GPU path dispatches 4×4×4 workgroups over grid cells. Each cell compares its eight samples with the selected threshold, reads the edge and triangle lookup tables, and interpolates an edge crossing with

$$
t = \frac{\rho_0-f_0}{f_1-f_0},
\qquad
\mathbf p = \mathbf p_0+t(\mathbf p_1-\mathbf p_0),
$$

using the midpoint when the denominator is numerically too small. Positive, negative, and dual-sign modes choose the classification threshold and populate the `IsoVertex.sign_flag`; they do not alter the stored scalar samples.

Generated triangle vertices are appended through an atomic counter into a preallocated bounded buffer. Both the compute shader and Rust-side capacity planning must preserve the bound. `src-tauri/src/renderer/isosurface.rs` also contains a CPU reference/fallback and topology, finiteness, lattice-transform, normal, and GPU-dispatch tests.

### Direct volume rendering

The raycaster renders the field's fractional unit cube after transforming it by the supplied lattice. The fragment path intersects the camera ray with the cube, converts sampling positions to grid coordinates, performs trilinear interpolation, applies the selected transfer function, and composites front-to-back. For premultiplied sample color $\mathbf c_s$ and opacity $\alpha_s$:

$$
\mathbf C \leftarrow \mathbf C + (1-A)\mathbf c_s\alpha_s,
\qquad
A \leftarrow A + (1-A)\alpha_s.
$$

Opaque-scene depth limits the ray segment so volume fragments behind accepted opaque geometry do not dominate the presentation. Density range, cutoff, colormap, opacity scale, step size, and sign mode are visualization parameters. They must be recorded alongside any quantitative figure and must not silently redefine imported units.

---

## Atom, bond, picking, and measurement presentation

**Implementation**: `src-tauri/src/renderer/instance.rs`, `src-tauri/src/renderer/ray_picking.rs`, `src-tauri/src/commands/analysis.rs`, and `src-tauri/src/commands/viewport.rs`

Atoms are camera-facing quads whose fragment shader analytically intersects a sphere and writes the corrected hit depth. Bonds are `BondInstance` segments expanded by the vertex shader into open 12-segment cylinders. Neither path adds tessellated sphere meshes or a geometry-shader stage.

Bond candidates use the current empirical covalent radii and user threshold factor. The public analysis command requires the factor to be finite and positive, defaults it to 1.2, and returns bonds, coordination summaries, length statistics, and distortion indices. Treat these as visualization/inspection aids; a chemistry-specific bonding interpretation requires its own declared model and tests.

Picking converts the screen coordinate into a camera ray and performs a CPU linear scan over the prepared pick scene. For a normalized ray $\mathbf r(t)=\mathbf o+t\mathbf d$ and atom center $\mathbf c$, it solves

$$
|\mathbf o+t\mathbf d-\mathbf c|^2=R^2
$$

and selects the closest positive intersection. Each pick entry retains its source intrinsic index, so a boundary image selects its physical source atom instead of becoming a new atom.

Distance, angle, and dihedral overlays reference two, three, or four intrinsic indices. Their definitions are versioned snapshot data; projected label positions are renderer-derived screen data.

---

## Phonon and Wannier presentation

**Implementation**: `src-tauri/src/phonon.rs`, `src-tauri/src/wannier.rs`, and their command/panel modules

Phonon loaders verify that the supplied mode atom count matches the accepted intrinsic structure. Selecting a mode and changing its phase update renderer presentation coordinates; they do not replace the baseline Cartesian coordinates, create a structural version, or add an undo record. The current frontend per-frame IPC path remains until the dedicated animation node replaces it.

Wannier hopping networks are parsed from `wannier90_hr.dat`, validated for finite values and valid orbital/shell mappings, then filtered by orbital, lattice shell, magnitude, onsite visibility, and master visibility. Neighbor-cell endpoints may require renderer ghost atoms or lines. Those ghosts must remain outside `CrystalState`, snapshots, atom tables, selection counts, and exported structures.

---

## Where to change and where to test

| Behavior | Primary implementation | Existing focused evidence |
|---|---|---|
| supercell/slab kernel | `cpp/src/physics_kernel.cpp` | `cpp/tests/test_supercell_eigen.cpp`, slab test executables under `cpp/tests/` |
| Rust structure validation/commit | `src-tauri/src/crystal_state.rs`, `transaction.rs`, `commands/geometry.rs` | module tests plus Rust transaction/structure tests |
| 3D/2D BZ and paths | `brillouin_zone.rs`, `kpath.rs`, `kpath_2d.rs` | colocated Rust test modules |
| volumetric metadata/import | `volumetric.rs` and format loaders | colocated volumetric/parser tests |
| CPU/GPU isosurface | `renderer/isosurface.rs`, `marching_cubes.wgsl` | CPU invariants and GPU dispatch test |
| volume transfer/rendering | `renderer/volume_raycast.rs`, `volume_raycast.wgsl` | renderer tests plus desktop GPU smoke test |
| atom/bond/picking | `renderer/instance.rs`, `ray_picking.rs` | instance/picking tests plus desktop selection check |
| Wannier overlay | `wannier.rs`, commands and panel | filter, mapping, failure-preservation tests |

Before changing an algorithm:

1. record the input format, coordinate convention, units, and expected invariant;
2. let Breaker establish a failing independent regression;
3. make the smallest implementation change that satisfies that gate;
4. run the focused C++/Rust/GPU gate and the standard repository gates; and
5. have Auditor check physical assumptions, failure atomicity, ownership, and unintended contract changes.

There is no repository-wide physical Manifest that declares one material or simulation setup. Passing implementation tests therefore establishes software behavior only. It does not validate an unpublished scientific interpretation.
