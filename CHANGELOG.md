# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] - 2026-04-14

### Added
- **Measurement Tool**: Interactive distance, angle, and dihedral angle measurements.
  - Auto-detects 2/3/4 selected atoms and computes exact distances ($|P_i - P_j|$) or angles.
  - Generates persistent rendering of dashed lines with measurement results placed via coordinate projection.
- **Undo / Redo Stack**: Action historical tracing and state rollback via keyboard (`Cmd+Z` / `Cmd+Shift+Z`).
  - Utilizes a `LightweightState` data structure, storing only crystallographic boundaries and atoms (excluding large volumetric matrices).
  - Configurable boundaries to limit maximum depth (20 steps) and control memory footprint.
- **Fractional Occupancy Visualization**: Fractional atom occupancies interpreted visually.
  - Renders transparency in 3D Impostor spheres utilizing non-linear opacity scaling ($\alpha = \text{occ}^{0.6}$).
  - `Atom Management` table now features an `Occ.` column for precision readout of input files (.cif).

### Changed
- **Architectural Refactor (StateTransaction)**: Command operations rewritten onto a strict transaction pipeline (`with_state_update`) for absolute stability.
  - Resolves lock contention risk during concurrent rendering invocations.
  - Splits the single `commands.rs` monolith into 7 domain-specific operation modules.
- **Modular Frontend UI**: Disassembled the monolithic `RightSidebar` container into separated `lazy()` loading panels to drastically reduce initial parsing overhead.

## [0.5.0] - 2026-04-11

### Added
- **⚡ Tight-Binding (Wannier) Visualizer**: Parse `wannier90_hr.dat` and visualize the hopping Hamiltonian $H = \sum_{\mathbf{R}} t_{ij}(\mathbf{R}) c^\dagger_{i,\mathbf{0}} c_{j,\mathbf{R}}$ as an interactive 3D network overlay.
  - **Hopping Network Rendering**: Instanced cylinder rendering for inter-atomic hoppings with per-orbital color coding (Google Material 500-level palette, 10 colors, mod-cycle).
  - **Magnitude Filtering**: Adjustable $|t|$ threshold slider to suppress noise and isolate dominant couplings.
  - **$\mathbf{R}$-Shell Selection**: Per-shell checkbox toggles for translation vectors $[R_1, R_2, R_3]$.
  - **Orbital Selection**: Per-orbital toggles for multi-orbital systems (e.g., $d$-band transition metals).
  - **On-site Term Control**: Dedicated toggle for on-site ($\mathbf{R}=0$, $m=n$) diagonal terms.
  - **Ghost Atom Rendering**: Semi-transparent neighbor-cell atoms at hopping endpoints (50% radius, 40% opacity, 20% desaturated) for visual context without structural state pollution.
  - **Auto-Bond Management**: Chemical bonds auto-hidden on Wannier load, restored on clear.
- **🎨 Icon Toolbar UI**: Right sidebar redesigned from stacked accordion panels to a compact 44 px icon toolbar with sliding panel.
  - Domain-specific SVG icons: bond diagram, isosurface cloud, sine-wave phonon, hexagonal BZ, hopping arrow, 2×2 cell grid, layered slab, atom badge.
  - Default state: all panels collapsed for maximum viewport area.
  - Tooltip labels on hover for discoverability.

## [0.4.0] - 2026-04-10

### Added
- **🔷 Brillouin Zone Visualization**: Full reciprocal-space analysis pipeline with interactive BZ rendering.
  - **3D Wigner-Seitz Construction**: Voronoi-based BZ generation covering all 14 Bravais lattice types (Setyawan-Curtarolo 2010 convention).
  - **2D BZ Support**: Automated 2D material detection (vacuum gap + c/a ratio heuristics) with Sutherland-Hodgman polygon clipping for 5 wallpaper group types (Hexagonal, Square, Rectangular, Centered-Rectangular, Oblique).
  - **High-Symmetry K-Points**: Automatic identification and labeling of $\Gamma$, $K$, $M$, $X$, $L$, $W$, $U$ etc. based on Bravais type.
  - **Band Path Generator**: One-click k-path export for Quantum ESPRESSO (`K_POINTS {crystal}`) and VASP (`KPOINTS`) with configurable density ($N_k$) and uniform segment spacing.
  - **Orthographic Locked View**: Fixed camera projection ensuring label-wireframe alignment; pan and zoom supported, rotation disabled.
  - **Dedicated Sub-Viewport**: Offscreen wgpu render target with alpha-blended blit compositing onto the main framebuffer.
- **⚛️ Cell Standardization & Reduction**: Complete crystallographic cell transformation toolkit.
  - **Niggli Reduction**: Reduce any lattice to its unique Niggli-reduced form via Spglib.
  - **Primitive Cell**: Transform conventional cell to primitive representation.
  - **Conventional Cell**: Transform primitive cell to conventional representation.
- **🧪 Test Structures**: Reference CIF files for BZ validation — graphene, MoS₂ monolayer, Si diamond cubic, Fe(100) a-axis vacuum slab.

### Fixed
- **2D K-Point Mapping**: Fixed `map_2d_to_3d` coordinate mapping that incorrectly routed k-point components into the vacuum direction for non-standard vacuum axes (a-axis, b-axis).
- **BZ Label Desync**: Disabled camera rotation in BZ view to prevent HTML label overlays from desyncing with GPU-rendered wireframe geometry.

## [0.3.0] - 2026-04-09

### Added
- **📊 Volumetric Rendering Pipeline**: Full-stack volumetric data visualization for charge densities, electrostatic potentials, and orbital wavefunctions.
  - **File Parsers**: VASP CHGCAR/LOCPOT (with $V_{\text{cell}}$ normalization), Gaussian Cube (Bohr→Å, C→F reorder), XSF DATAGRID_3D.
  - **Isosurface Extraction**: CPU Marching Cubes ([Lorensen87]) reference implementation + GPU compute shader pipeline for real-time mesh generation.
  - **Volume Raycasting**: Depth-aware front-to-back compositing with Blinn-Phong shading, supporting both orthographic and perspective projections.
  - **Dual-Color Isosurface**: Positive/negative lobes rendered in distinct colormap-derived colors for signed data (Wannier functions, $\Delta\rho$).
  - **10 Scientific Colormaps**: Viridis, Inferno, Plasma, Magma, Cividis, Turbo, Hot, Grayscale, Coolwarm (diverging), RdYlBu (diverging).
  - **Signed Volume Mapping**: $\sqrt{|v/v_{\max}|}$ perceptual stretch for enhanced positive/negative lobe contrast on sequential colormaps.
  - **Dynamic Step Size**: Nyquist-compliant raymarching ($\Delta t = 0.5 \cdot \min(h_a, h_b, h_c)$) eliminates Moiré banding artifacts.
  - **Render Modes**: Isosurface-only, Volume-only, or Both (with soft-fade clipping at isosurface boundary).
  - **Volume Density Cutoff**: User-adjustable threshold to trim low-density regions; unified with isovalue in Both mode.
  - **Opacity Control**: Per-mode opacity scale slider for fine-tuning transparency.
  - **Drag-and-Drop**: Volumetric files can be loaded via drag-and-drop or File menu.

### Changed
- **Git LFS**: Large test data files (CHGCAR, Cube, XSF; ~54 MB total) migrated to Git LFS for repository hygiene.
- **Light Mode Default**: Application now starts in light mode by default.
- **Code Quality**: Removed all internal development tracking codes from production source files.

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
