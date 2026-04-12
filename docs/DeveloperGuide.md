# CrystalCanvas Developer Guide

> Version: v0.5.0 | Updated: 2026-04-12

This guide provides technical details for developers who want to understand, modify, or extend CrystalCanvas. For end-user instructions, see [UserManual.md](UserManual.md).

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [State Management](#2-state-management)
3. [Rendering Pipeline](#3-rendering-pipeline)
4. [File I/O Pipeline](#4-file-io-pipeline)
5. [Brillouin Zone & K-Path](#5-brillouin-zone--k-path)
6. [Volumetric Rendering](#6-volumetric-rendering)
7. [Wannier Tight-Binding Visualizer](#7-wannier-tight-binding-visualizer)
8. [LLM Command Bus](#8-llm-command-bus)
9. [C++ Physics Kernel & FFI](#9-c-physics-kernel--ffi)
10. [IPC Protocol](#10-ipc-protocol)

---

## 1. Architecture Overview

CrystalCanvas is a four-layer hybrid desktop application:

```
L4: React + TypeScript + TailwindCSS    (Presentation)
    └── WebView / Tauri IPC (invoke / events)
L3: Rust / Tauri 2.0                    (Application Logic / SSoT)
    └── State Manager · Command Router · File I/O · LLM Bridge
L2: Rust / wgpu                         (Rendering Engine)
    └── Impostor Spheres · Bond Cylinders · Volume Raycast · Marching Cubes · BZ Wireframe
L1: C++ Kernel                          (Physics & Math Engine)
    └── Spglib · Gemmi · Eigen · Slab/Supercell Algorithms
```

**Key design principles:**

- **Single Source of Truth (SSoT)**: All crystal state lives in `CrystalState` (Rust, L3). The frontend (L4) is a pure presentation layer with no physics state caching.
- **Dual-precision**: `f64` for crystallographic calculations (fractional coordinates, lattice parameters), `f32` for GPU rendering (Cartesian positions, instance buffers).
- **ColMajor enforcement**: All lattice matrices use Fortran column-major order throughout the entire stack. The 3x3 lattice is stored as `[f64; 9]` in order `[a_x, a_y, a_z, b_x, b_y, b_z, c_x, c_y, c_z]`.
- **Full instance buffer reconstruction**: On every state change, the GPU instance buffer is rebuilt from scratch. At the current scale (~500 atoms, ~16 KB), this takes < 0.1 ms and avoids the complexity of incremental updates.

### Source Tree

```
src-tauri/src/
├── main.rs              # Tauri entry point, native menu, render loop
├── commands.rs          # All Tauri IPC command handlers (~2500 LOC)
├── crystal_state.rs     # CrystalState struct (SSoT) and SoA layout
├── settings.rs          # AppSettings (atom_scale, bond_tolerance, colors)
├── volumetric.rs        # VolumetricData struct (3D scalar fields)
├── brillouin_zone.rs    # Wigner-Seitz BZ construction (3D + 2D)
├── kpath.rs             # High-symmetry k-path generation (14 Bravais types)
├── kpath_2d.rs          # 2D k-path for wallpaper groups (5 types)
├── phonon.rs            # Phonon eigenvector parser and animation state
├── wannier.rs           # WannierOverlay, hopping filtering, ghost atoms
├── ffi/
│   ├── mod.rs           # Module declaration
│   └── bridge.rs        # cxx FFI bridge to C++ kernel
├── io/
│   ├── import.rs        # Unified file loader (dispatch by extension)
│   ├── export.rs        # POSCAR, QE, LAMMPS exporters
│   ├── chgcar_parser.rs # VASP CHGCAR/LOCPOT parser
│   ├── cube_parser.rs   # Gaussian Cube parser (Bohr→Å, C→F reorder)
│   ├── xsf_volumetric_parser.rs  # XSF DATAGRID_3D parser
│   ├── poscar_parser.rs # VASP POSCAR parser
│   ├── qe_parser.rs     # Quantum ESPRESSO input parser
│   ├── axsf_parser.rs   # Animated XSF (phonon) parser
│   └── wannier_hr_parser.rs  # wannier90_hr.dat parser
├── llm/
│   ├── provider.rs      # OpenAI / DeepSeek / Ollama API clients
│   ├── prompt.rs        # System prompt construction
│   ├── context.rs       # Crystal state context serialization for LLM
│   ├── command.rs       # CrystalCommand schema definition
│   ├── router.rs        # JSON command → Rust function dispatch
│   └── sandbox.rs       # Physics validation (MIC overlap, collision checks)
└── renderer/
    ├── renderer.rs      # Top-level Renderer struct, frame loop, buffer management
    ├── gpu_context.rs   # wgpu Device, Queue, Surface, Config initialization
    ├── camera.rs        # Orbital camera (perspective + orthographic)
    ├── pipeline.rs      # Render pipeline creation (impostor sphere, line, bond)
    ├── instance.rs      # AtomInstance, BondInstance, LineVertex structs + builders
    ├── ray_picking.rs   # CPU-side ray-sphere intersection for atom selection
    ├── isosurface.rs    # GPU Marching Cubes + isosurface render pipeline
    ├── volume_raycast.rs # Volume rendering (front-to-back raycasting)
    ├── bz_renderer.rs   # BZ sub-viewport (offscreen render + alpha blit)
    ├── mc_lut.rs        # Marching Cubes lookup tables (256 cases)
    └── render_config.rs # Render state flags and constants
```

---

## 2. State Management

### CrystalState (SSoT)

All crystal data is held in a single `CrystalState` struct using **Structure of Arrays (SoA)** layout:

```rust
pub struct CrystalState {
    // Unit cell parameters (f64, Å and degrees)
    pub cell_a: f64, pub cell_b: f64, pub cell_c: f64,
    pub cell_alpha: f64, pub cell_beta: f64, pub cell_gamma: f64,

    // Space group
    pub spacegroup_hm: String,
    pub spacegroup_number: i32,

    // SoA — f64 for physics
    pub labels: Vec<String>,
    pub elements: Vec<String>,
    pub fract_x: Vec<f64>, pub fract_y: Vec<f64>, pub fract_z: Vec<f64>,
    pub occupancies: Vec<f64>,
    pub atomic_numbers: Vec<u8>,

    // f32 for GPU rendering (populated on demand via fractional_to_cartesian())
    pub cart_positions: Vec<[f32; 3]>,

    // Reactivity trigger
    pub version: u32,

    // Attached heavy data (skipped in serialization)
    pub volumetric_data: Option<VolumetricData>,  // #[serde(skip)]
    pub bz_cache: Option<(BrillouinZone, KPath)>, // #[serde(skip)]
    pub wannier_overlay: Option<WannierOverlay>,   // #[serde(skip)]
    // ...
}
```

### Threading Model

All shared state is wrapped in `std::sync::Mutex` and managed by Tauri:

```rust
app.manage(Mutex::new(CrystalState::default()));
app.manage(Mutex::new(Renderer::new(...)));
app.manage(Mutex::new(AppSettings::load(...)));
```

**Lock ordering** (must always be acquired in this order to prevent deadlocks):

```
crystal_state → settings → renderer
```

### State Mutation Pattern

Every `#[tauri::command]` that mutates `CrystalState` follows this pattern:

1. Lock `crystal_state`
2. Mutate the state
3. Increment `version`
4. Lock `settings`, build GPU instances via `build_instance_data()`
5. Lock `renderer`, upload instances and update camera
6. Emit `state_changed` event to L4

---

## 3. Rendering Pipeline

### GPU Instance Types

All rendering uses **instanced drawing** — each atom/bond/hopping is one instance in a GPU buffer.

| Struct | Size | Usage | Shader |
|---|---|---|---|
| `AtomInstance` | 32 B | Atoms (impostor spheres) | `impostor_sphere.wgsl` |
| `BondInstance` | 48 B | Chemical bonds, hoppings, arrows | `bond_cylinder.wgsl` |
| `LineVertex` | 28 B | Unit cell edges | Line pipeline |

```rust
#[repr(C)]
pub struct AtomInstance {
    pub position: [f32; 3],  // 12 B — Cartesian world coords (Å)
    pub radius: f32,         //  4 B — display radius
    pub color: [f32; 4],     // 16 B — RGBA
}
// Total: 32 bytes per atom

#[repr(C)]
pub struct BondInstance {
    pub start: [f32; 3],     // 12 B
    pub radius: f32,         //  4 B
    pub end: [f32; 3],       // 12 B
    pub _pad: f32,           //  4 B — alignment padding
    pub color: [f32; 4],     // 16 B
}
// Total: 48 bytes per bond
```

### Impostor Sphere Rendering

Atoms are **not** rendered as tessellated meshes. Instead, each atom is a screen-aligned billboard quad. The fragment shader performs **analytical ray-sphere intersection** to compute per-pixel depth and normals, producing pixel-perfect spheres at any zoom level with zero tessellation artifacts.

Key shader logic (`impostor_sphere.wgsl`):

1. Vertex shader expands each instance to a 2-triangle quad oriented toward the camera.
2. Fragment shader casts a ray from the camera through each pixel.
3. Ray-sphere intersection gives the exact hit point on the sphere surface.
4. Normal is computed analytically → Blinn-Phong shading applied.
5. Fragment depth is corrected to enable proper z-ordering with bonds and volumes.

### Dual-Pass Depth Architecture

The render loop uses two separate depth buffers:

```
Pass 1 (Opaque):     Atoms + Bonds + Unit Cell → opaque_depth_texture
    ↓ depth copy
Pass 2 (Transparent): Isosurface / Volume Raycast → transparent_depth_texture
    (reads opaque depth for correct compositing)
```

This ensures volumetric rendering correctly occludes behind atoms.

### Camera System

Orbital camera in `camera.rs`:

- **Coordinate system**: Right-handed, Y-up
- **Depth range**: `[0, 1]` (wgpu convention, not OpenGL's `[-1, 1]`)
- **Projection**: Perspective (`Mat4::perspective_rh`) or Orthographic (`Mat4::orthographic_rh`)
- **GPU uniform**: `CameraUniform` contains `view`, `proj`, and `view_proj` matrices — all column-major `[[f32; 4]; 4]`

### WGSL Shaders

| Shader | File | Purpose |
|---|---|---|
| Impostor Sphere | `impostor_sphere.wgsl` | Atom rendering via billboard ray-sphere intersection |
| Bond Cylinder | `bond_cylinder.wgsl` | Chemical bonds and Wannier hopping lines |
| Isosurface Render | `isosurface_render.wgsl` | Render Marching Cubes mesh (triangle list) |
| Marching Cubes | `marching_cubes.wgsl` | GPU compute shader for isosurface extraction |
| Volume Raycast | `volume_raycast.wgsl` | Front-to-back raycasting with depth-aware compositing |

---

## 4. File I/O Pipeline

### Import

All imports route through `io::import::load_file(path: &str) -> Result<CrystalState, String>`, which dispatches by file extension:

| Extension | Parser | Notes |
|---|---|---|
| `.cif`, `.pdb` | C++ Gemmi (via FFI) | Full symmetry expansion |
| `.xyz` | C++ Gemmi | No cell parameters |
| `.poscar`, `.contcar`, `.vasp` | `poscar_parser.rs` | VASP 5 format, Selective Dynamics |
| `.in`, `.pwi` | `qe_parser.rs` | QE `pw.x` input, `ibrav` support |
| `.chgcar`, `.locpot` | `chgcar_parser.rs` | Atoms + Volumetric data |
| `.cube` | `cube_parser.rs` | Bohr→Å conversion, C→F grid reorder |
| `.xsf` | `xsf_volumetric_parser.rs` | DATAGRID_3D blocks |
| `.dat` (wannier90_hr) | `wannier_hr_parser.rs` | Hopping Hamiltonian only (no atoms) |

### Export

Exporters in `io::export.rs`:

| Format | Function | Key Features |
|---|---|---|
| VASP POSCAR | `export_poscar()` | VASP 5 format with element line |
| Quantum ESPRESSO | `export_qe_input()` | Automatic K-point density, IUPAC 2021 atomic masses |
| LAMMPS Data | `export_lammps_data()` | Orthogonal box, atom type mapping |

### Volumetric Data Normalization

- **CHGCAR**: Raw values are divided by cell volume $V_\text{cell}$ to convert to $e/\text{Å}^3$.
- **Gaussian Cube**: Bohr → Å conversion (`1 Bohr = 0.529177 Å`). Grid voxels are reordered from C row-major to Fortran column-major.
- **XSF**: Values used as-is (typically already in physical units).

---

## 5. Brillouin Zone & K-Path

### BZ Construction (`brillouin_zone.rs`)

The Brillouin Zone is constructed as a 3D Wigner-Seitz cell in reciprocal space:

1. **Reciprocal lattice**: Compute $\mathbf{b}_i = 2\pi \frac{\mathbf{a}_j \times \mathbf{a}_k}{\mathbf{a}_i \cdot (\mathbf{a}_j \times \mathbf{a}_k)}$
2. **Bisecting planes**: For each reciprocal lattice point $\mathbf{G}$ (up to 3rd shell, 125 points), generate the perpendicular bisector plane with normal $\hat{\mathbf{G}}$ at distance $|\mathbf{G}|/2$.
3. **Convex hull intersection**: Starting from a large cube, iteratively clip against each bisecting plane using the Sutherland-Hodgman algorithm.
4. **Face extraction**: Group coplanar vertices into polygon faces, sorted by winding order.

```rust
pub struct BrillouinZone {
    pub recip_lattice: [[f64; 3]; 3],
    pub vertices: Vec<[f64; 3]>,     // BZ corner positions (Å⁻¹)
    pub edges: Vec<[usize; 2]>,      // vertex index pairs
    pub faces: Vec<Vec<usize>>,      // ordered vertex indices per face
    pub bravais_type: BravaisType,   // one of 14 types
    pub is_2d: bool,
}
```

### 2D BZ Support

For 2D materials (detected via vacuum gap heuristic: $c/a > 3$ or vacuum > 10 Å):

1. Project reciprocal lattice onto the 2D in-plane subspace.
2. Classify the 2D Bravais type (5 wallpaper groups: Hexagonal, Square, Rectangular, Centered-Rectangular, Oblique).
3. Construct the 2D BZ as a polygon via 2D Sutherland-Hodgman clipping.

### K-Path Generation (`kpath.rs`, `kpath_2d.rs`)

- **14 Bravais types** (3D): High-symmetry points and paths follow the Setyawan-Curtarolo (2010) convention.
- **5 wallpaper types** (2D): $\Gamma$-$M$-$K$-$\Gamma$ for hexagonal, $\Gamma$-$X$-$M$-$\Gamma$ for square, etc.
- **Output formats**: Quantum ESPRESSO `K_POINTS {crystal}` and VASP `KPOINTS` (with configurable $N_k$ density).

### BZ Rendering (`bz_renderer.rs`)

The BZ is rendered in an **isolated sub-viewport** to avoid coordinate system conflicts with real-space atoms:

1. Offscreen `wgpu::Texture` with independent camera (orthographic, rotation-locked).
2. BZ wireframe rendered as `LineVertex` segments.
3. K-point labels rendered as HTML overlays, positioned via `world_to_ndc → ndc_to_screen` projection.
4. Final image alpha-blended onto the main framebuffer.

---

## 6. Volumetric Rendering

### Data Model

```rust
pub struct VolumetricData {
    pub grid_dims: [usize; 3],      // (Nx, Ny, Nz)
    pub lattice: [f64; 9],          // ColMajor 3x3 lattice matrix (Å)
    pub data: Vec<f32>,             // flattened scalar field, x-fastest
    pub data_min: f32,              // global min (for UI slider)
    pub data_max: f32,              // global max
    pub source_format: VolumetricFormat,
    pub origin: [f64; 3],           // grid origin offset
}
```

**Indexing convention**: `data[ix + iy * Nx + iz * Nx * Ny]` (Fortran column-major, x-fastest).

### GPU Marching Cubes (`isosurface.rs` + `marching_cubes.wgsl`)

1. Volumetric data uploaded as a `wgpu::Buffer` (storage buffer).
2. A compute shader dispatches one workgroup per voxel, performing the Marching Cubes algorithm using the 256-case lookup table (`mc_lut.rs`).
3. Output: triangle vertices with normals and sign flags written to a vertex buffer.
4. The `isosurface_render.wgsl` shader renders this mesh with Blinn-Phong shading.

**Dual-color signed rendering**: Each triangle vertex carries a `sign_flag` (+1.0 or -1.0). The fragment shader selects between `color` and `color_negative` uniforms based on this flag, enabling red/blue visualization of positive/negative lobes.

### Volume Raycasting (`volume_raycast.rs` + `volume_raycast.wgsl`)

- **Algorithm**: Front-to-back compositing along rays through the volumetric grid.
- **Step size**: Nyquist-compliant: $\Delta t = 0.5 \cdot \min(h_a, h_b, h_c)$ where $h_i = |\mathbf{a}_i| / N_i$. This eliminates Moiré banding artifacts.
- **Depth awareness**: The shader reads the opaque depth texture to terminate rays at solid geometry (atoms/bonds).
- **10 colormaps**: Viridis, Inferno, Plasma, Magma, Cividis, Turbo, Hot, Grayscale, Coolwarm, RdYlBu — implemented as hardcoded 256-sample LUTs in WGSL.
- **Signed volume mapping**: For signed data, values are mapped via $\sqrt{|v/v_\max|}$ to enhance positive/negative lobe contrast on sequential colormaps.

---

## 7. Wannier Tight-Binding Visualizer

### Data Flow

```
wannier90_hr.dat → WannierHrData → WannierOverlay → VisibleHopping[] → BondInstance[] → GPU
```

### Parser (`io/wannier_hr_parser.rs`)

Parses the standard Wannier90 `_hr.dat` format:

```
<header comment>
<num_wann>
<num_r_shells>
<degeneracy weights (multi-line)>
R1 R2 R3  m  n  Re(t)  Im(t)    ← repeated for all hoppings
```

Output struct:
```rust
pub struct WannierHrData {
    pub num_wann: usize,
    pub r_shells: Vec<[i32; 3]>,         // unique R vectors
    pub degeneracy: Vec<usize>,          // weight per R shell
    pub hoppings: Vec<WannierHopping>,   // all t_mn(R) entries
}
```

### Filtering Engine (`wannier.rs`)

`WannierOverlay::filter_and_rebuild()` recomputes visible hoppings based on:

- **Magnitude threshold**: $|t_{mn}(\mathbf{R})| > t_\min$
- **R-shell selection**: Per-shell boolean toggles
- **Orbital selection**: Per-orbital boolean toggles
- **On-site toggle**: Include/exclude $\mathbf{R} = 0, m = n$ terms

Each visible hopping is converted to Cartesian coordinates:

$$\mathbf{r}_\text{end} = \mathbf{r}_n + R_1 \mathbf{a}_1 + R_2 \mathbf{a}_2 + R_3 \mathbf{a}_3$$

### Ghost Atom Rendering

For hoppings that terminate in neighboring cells ($\mathbf{R} \neq 0$), semi-transparent "ghost atoms" are rendered at the endpoint positions:

- 50% of normal radius
- 40% opacity
- 20% desaturation (mixed toward gray)

Ghost atoms are injected into the atom instance buffer by `build_atoms_with_ghosts()`, appended after the real atoms.

### Visual Encoding

- **Hopping color**: Per-orbital index, using a 10-color Google Material 500-level palette (mod-cycled for > 10 orbitals).
- **Hopping radius**: Linearly scaled by $|t| / t_\max$, range `[0.02, 0.08]` Å.

---

## 8. LLM Command Bus

### Architecture

```
User prompt → LLM Provider → JSON CrystalCommand → Router → Sandbox → Execution
```

### Module Structure

| File | Role |
|---|---|
| `provider.rs` | HTTP client for OpenAI, DeepSeek, local Ollama |
| `prompt.rs` | System prompt with crystal context injection |
| `context.rs` | Serialize current `CrystalState` into compact LLM context |
| `command.rs` | `CrystalCommand` enum definition (the LLM's "tool" schema) |
| `router.rs` | Deserialize JSON → dispatch to the correct Rust function |
| `sandbox.rs` | Pre-execution physics validation (collision detection, MIC overlap) |

### Safety Pipeline

1. **Schema validation**: The LLM's JSON output is deserialized via `serde`. Any field mismatch, unknown variant, or type error fails fast at the parser — the LLM cannot inject arbitrary operations.
2. **Physics sandbox**: Before execution, the sandbox validates physical constraints (e.g., new atom not within 0.5 Å of existing atoms via MIC distance check).
3. **Approval gate**: The frontend displays a command card summarizing the proposed operation. The user must click "Execute" to approve.

### API Key Security

- Keys are stored in the OS Keychain via Tauri's secure storage plugin.
- Keys are **never** logged, serialized, or persisted in plain text.
- The `provider.rs` module reads keys from memory only at request time.

---

## 9. C++ Physics Kernel & FFI

### Kernel Components

Located in `cpp/`:

| Library | Purpose |
|---|---|
| **Spglib** | Space group detection, symmetry operations, Niggli/Delaunay reduction |
| **Gemmi** | CIF and PDB file parsing with full crystallographic symmetry expansion |
| **Eigen** | Matrix operations for lattice transforms (slab, supercell) |

### FFI Bridge (`ffi/bridge.rs`)

The Rust ↔ C++ boundary uses the `cxx` crate for type-safe, zero-copy interop:

```rust
#[cxx::bridge]
mod ffi {
    struct FfiSiteData {
        label: String,
        element_symbol: String,
        fract_x: f64,
        fract_y: f64,
        fract_z: f64,
        occ: f64,
        atomic_number: u8,
    }

    struct FfiCrystalData {
        name: String,
        a: f64, b: f64, c: f64,
        alpha: f64, beta: f64, gamma: f64,
        spacegroup_hm: String,
        spacegroup_number: i32,
        sites: Vec<FfiSiteData>,
    }

    extern "C++" {
        fn parse_cif_file(path: &str) -> Result<FfiCrystalData>;
        fn detect_spacegroup(...) -> Result<SpgResult>;
        fn niggli_reduce(...) -> Result<...>;
        // ...
    }
}
```

**Exception isolation**: All C++ functions are wrapped in `try/catch` blocks. C++ exceptions are converted to `Result::Err` on the Rust side — they never cross the FFI boundary as unwinding.

### Build System

The `build.rs` script:
1. Compiles C++ sources via `cxx-build` with CMake-like flag handling.
2. Links Spglib, Gemmi, and Eigen (header-only) automatically.
3. No manual CMake invocation required — `cargo build` handles everything.

---

## 10. IPC Protocol

### Frontend → Backend (Tauri `invoke`)

All communication from React to Rust uses Tauri's `invoke` mechanism. Each function is registered in `main.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    commands::load_cif_file,
    commands::apply_supercell,
    commands::set_isovalue,
    // ... ~60 commands total
])
```

### Backend → Frontend (Tauri `emit`)

State changes and asynchronous results are communicated back via events:

| Event | Payload | Purpose |
|---|---|---|
| `state_changed` | `()` | Trigger frontend to re-fetch `CrystalState` |
| `view_projection_changed` | `{is_perspective: bool}` | Sync UI projection toggle |
| `menu-action` | `String` | Route native menu clicks to React handlers |
| `volumetric_loaded` | `{min, max, is_signed}` | Update volume control sliders |

### Data Flow Example: Loading a CIF File

```
1. User drops file → React calls invoke("load_cif_file", {path})
2. Rust: io::import::load_file(path) → CrystalState
3. Rust: crystal_state.lock() → replace state → version++
4. Rust: build_instance_data() → AtomInstance[]
5. Rust: renderer.lock() → update_atoms(), update_lines(), update_camera()
6. Rust: emit("state_changed")
7. React: listener fires → invoke("get_crystal_state") → re-render UI panels
```

---

*CrystalCanvas v0.5.0 — Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors. Dual-licensed under MIT and Apache-2.0.*
