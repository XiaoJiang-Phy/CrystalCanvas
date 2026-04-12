<p align="center">
  <img src="logo.svg" width="200" alt="CrystalCanvas Logo">
</p>

# CrystalCanvas

**A next-generation, open-source crystal structure modeler — built for the GPU era.**

Legacy tools like VESTA and XCrySDen were pioneering in their time, but remain bound to single-threaded CPU rendering, decades-old GUI toolkits, and closed or stagnant codebases. CrystalCanvas is designed from scratch to close that gap: a **Rust + wgpu + C++** native stack delivers real-time GPU-accelerated isosurface extraction and volume raycasting, a modern React/Tauri interface replaces 2000s-era widget UIs, and an AI-assisted command bus lets you manipulate structures with natural language — capabilities no existing crystallographic tool offers. From interactive 3D modeling and publication-quality volumetric rendering to Brillouin Zone visualization, Wannier tight-binding overlays, and one-click DFT/MD file export, CrystalCanvas unifies the entire pre-computation workflow in a single, memory-safe application.

> **Current Release**: `v0.5.0` · Rust 15.5k LOC · TypeScript 3.9k LOC · C++ 737 LOC · 7 WGSL shaders

---

## 📥 Download & Installation

[![Download for macOS](https://img.shields.io/badge/Download_v0.5.0-macOS_(Intel_%26_Apple_Silicon)-007AFF?style=for-the-badge&logo=apple)](https://github.com/XiaoJiang-Phy/CrystalCanvas/releases/latest)

> [!WARNING]
> **Important Note for macOS Users (Unverified Developer)**
> 
> Because this is an open-source project and currently not signed with a paid Apple Developer Certificate, macOS will show a "Developer cannot be verified" warning and prevent the app from launching normally.
> 
> **To run the app:**
> 1. Move `CrystalCanvas.app` to your `/Applications` folder.
> 2. **Right-click (or Control-click)** the app icon and select **Open**.
> 3. Click **Open** again in the dialog box.
> 
> *Alternatively, run the following command in Terminal to clear the quarantine attribute:*
> ```bash
> sudo xattr -cr /Applications/CrystalCanvas.app
> ```

---

## Key Features

### Crystal Structure Modeling
- **Pixel-precise manual modeling** — Hardware-accelerated 3D viewport with real-time atom selection, addition, deletion, element substitution, and multi-atom drag translation.
- **Cell standardization** — Niggli reduction, Delaunay reduction, Primitive ↔ Conventional cell transforms via Spglib.
- **Slab cleaving** — Rigorous $(h,k,l)$ surface cutting via Extended Euclidean Algorithm (Diophantine solver), not heuristic templates. Adjustable layer count, vacuum thickness, and termination selection.
- **Supercell generator** — Arbitrary $3\times3$ integer transformation matrices with automatic coordinate remapping and boundary deduplication.

### Reciprocal Space & Electronic Structure
- **Brillouin Zone visualization** — 3D Wigner-Seitz cell construction (all 14 Bravais lattice types) and 2D BZ support (5 wallpaper group types) with high-symmetry $\mathbf{k}$-point labeling. One-click band path export for Quantum ESPRESSO and VASP.
- **Tight-Binding (Wannier) visualizer** — Parse `wannier90_hr.dat` hopping Hamiltonians $H = \sum_{\mathbf{R}} t_{ij}(\mathbf{R}) c^\dagger_{i,\mathbf{0}} c_{j,\mathbf{R}}$ and render as 3D network overlays with per-orbital color coding (10-color Material palette), $\mathbf{R}$-shell/orbital selection, magnitude filtering, and ghost atom rendering for neighboring cells.

### Volumetric Data Visualization
- **GPU isosurface extraction** — Real-time Marching Cubes (GPU compute shader) for CHGCAR, Gaussian Cube, and XSF files.
- **Volume raycasting** — Depth-aware front-to-back compositing with Blinn-Phong shading. Nyquist-compliant step size eliminates Moiré banding.
- **Dual-color signed isosurfaces** — Positive/negative lobes in distinct colormap-derived colors for Wannier functions and $\Delta\rho$. 10 scientific colormaps (Viridis, Coolwarm, RdYlBu, etc.).

### DFT/MD Integration & AI
- **Seamless DFT/MD export** — Native high-fidelity export for VASP (POSCAR), LAMMPS (Data), Quantum ESPRESSO (Input with automatic K-point density and IUPAC 2021 masses).
- **AI-powered workflow** *(experimental)* — Natural language commands like *"Generate a 3×3×3 silicon supercell and dope 5% phosphorus on the surface"*. Context-aware LLM agent with strict physics validation (MIC overlap checks).
- **Memory-safe architecture** — Rust logic layer eliminates crashes from dangling pointers and buffer overflows. All crystal state managed via SSoT (Single Source of Truth) with `f64` physics / `f32` GPU precision separation.

---

## Roadmap

### Completed

| Version | Highlights |
|---|---|
| **v0.1.0** | Hybrid window (wgpu + WebView), impostor sphere rendering, CIF parsing |
| **v0.2.0** | Slab cleaving (Diophantine), supercell, atom editing, DFT exporters |
| **v0.3.0** | Volumetric rendering (GPU Marching Cubes, volume raycasting, 10 colormaps) |
| **v0.4.0** | 3D/2D Brillouin Zone, cell standardization (Niggli/Primitive/Conventional) |
| **v0.5.0** | Wannier tight-binding visualizer, icon toolbar UI redesign |

### Planned

| Version | Target | Key Features |
|---|---|---|
| **v0.6.0** | UX Foundation | Distance/angle measurement, undo/redo stack, partial occupancy visualization, `commands.rs` refactor |
| **v0.7.0** | CMP Core | In-GUI charge density difference ($\Delta\rho$), collinear magnetic moments ($m_z$), MSAA anti-aliasing |
| **v0.8.0** | Reciprocal Physics | 3D Fermi surface viewer (`.bxsf`), non-collinear magnetism |
| **v0.9.0+** | Flagship | Moiré superlattice generator (twistronics), high-quality rendering engine (SSAO), symmetry element overlay |

> For the full roadmap, see [ROADMAP.md](ROADMAP.md).

---

## Known Issues

> **Platform Support:**
> Due to rendering engine (`wgpu`) backend differences, **Windows** and **Linux** builds may have rendering issues. Currently **only macOS** is fully tested and supported.

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  L4: React + TypeScript + TailwindCSS  (Presentation)   │
│      WebView / Tauri IPC (invoke / events)              │
├─────────────────────────────────────────────────────────┤
│  L3: Rust / Tauri 2.0  (Application Logic / SSoT)       │
│      State Manager • Command Router • Undo Stack        │
├─────────────────────────────────────────────────────────┤
│  L2: Rust / wgpu  (Rendering Engine)                    │
│      Impostor Spheres • Bond Cylinders • BZ Wireframe   │
│      Volume Raycast • GPU Marching Cubes • Wannier Net  │
├─────────────────────────────────────────────────────────┤
│  L1: C++ Physics Kernel  (Spglib / Gemmi / Eigen)       │
│      Symmetry • Slab • Supercell • Overlap Detection    │
└─────────────────────────────────────────────────────────┘
```

| Layer | Technology | Role |
|---|---|---|
| **L4 Presentation** | React + TailwindCSS | UI panels, icon toolbar, chat, measurement overlays |
| **L3 Application** | Rust / Tauri 2.0 | SSoT state management, IPC, I/O pipeline, undo stack |
| **L2 Rendering** | Rust / wgpu (WGSL) | GPU-accelerated 3D (Metal / Vulkan / DX12), isosurface, BZ |
| **L1 Compute** | C++ (Spglib, Gemmi, Eigen) | Symmetry analysis, slab geometry, bonding (MIC) |
| **FFI Bridge** | `cxx` (Rust ↔ C++) | Type-safe, zero-copy data transfer, exception isolation |

**Key Design Decisions:**
- **Dual-precision**: `f64` for crystallographic calculations, `f32` for GPU rendering
- **ColMajor enforcement**: All lattice matrices follow Fortran column-major order throughout the stack
- **Full GPU reconstruction**: Instance buffer rebuilt on every state change (~16 KB for 500 atoms, < 0.1 ms)
- **Three-layer LLM safety**: Schema validation → physics sandbox → undo snapshot before every AI-generated command

---

## Getting Started

**New to CrystalCanvas? Check out the [User Manual](docs/UserManual.md) for a comprehensive guide.**

For more in-depth documentation, see the [Documentation](#documentation) section below.

CrystalCanvas utilizes a **Zero-Global-Pollution** strategy. All toolchains (Rust, Node) and dependencies are isolated within the project directory.

### 1. Prerequisites (macOS)
- **Xcode Command Line Tools**: `xcode-select --install`
- **pnpm**: `npm install -g pnpm` (the only global dependency required)

### 2. Initial Setup
Clone the repository and initialize the local toolchains:

```bash
git clone https://github.com/XiaoJiang-Phy/CrystalCanvas.git
cd CrystalCanvas

# Initialize local Rustup and Cargo home
mkdir -p .rustup .cargo
source dev_env.sh

# Install Rust stable locally (if not present)
rustup toolchain install stable

# Install Node dependencies
pnpm install
```

### 3. Build & Run

#### Activation
Always source the environment script before starting development:
```bash
source dev_env.sh
```

#### Run in Development Mode
```bash
# This starts the Vite dev server and the Tauri native window
pnpm run tauri dev
```

#### Run Standalone Rendering Demo
To verify GPU/wgpu compatibility without the full React UI:
```bash
cd src-tauri
RUST_LOG=info cargo run --bin render_demo
```
*Controls: Left-click drag to rotate, scroll to zoom.*

> **Note**: The C++ kernel (Spglib, Gemmi, Eigen) is compiled automatically via the Rust `build.rs` script using `cxx-build`. No manual CMake interaction is required.

---

## Project Structure

```text
CrystalCanvas/
├── .github/            # GitHub Actions (CI/CD release workflows)
├── src-tauri/          # Rust backend (Tauri commands, state handling, wgpu orchestration)
│   ├── shaders/        # WGSL shaders (volume_raycast, marching_cubes, isosurface_render)
│   ├── src/
│   │   ├── io/         # File parsers (CIF, POSCAR, CHGCAR, Cube, XSF, QE, wannier90_hr)
│   │   ├── renderer/   # wgpu pipelines (atoms, bonds, hopping, isosurface, volume, BZ)
│   │   └── ...         # State manager, command router, volumetric, wannier, BZ modules
│   ├── build.rs        # Unified Rust + C++ build script (cmake/cxx bridge)
│   └── Cargo.toml
├── src/                # React frontend (TypeScript + TailwindCSS components)
│   ├── components/     # UI components (icon toolbar, panels, chat)
│   ├── hooks/          # Custom React hooks (tauri events, file-drop, 3D interaction)
│   └── types/          # Strict TS IPC type mappings
├── cpp/                # C++ physics kernel
│   ├── include/        # Public C-compatible headers (cxx bridge)
│   ├── src/            # Spglib, Gemmi, Eigen integrations
│   └── CMakeLists.txt
├── doc/                # Internal technical docs (TDD, Roadmap, Feature Assessment)
├── docs/               # Public documentation
│   ├── UserManual.md       # End-user guide
│   ├── DeveloperGuide.md   # Architecture & contribution guide
│   ├── Algorithms.md       # Core algorithm specifications
│   ├── IPC_Commands.md     # Complete Tauri IPC command reference
│   ├── Shader_Reference.md # WGSL shader bind groups & pipelines
│   ├── TestingGuide.md     # Node TDD process & test inventory
│   └── FAQ.md              # Troubleshooting & common issues
├── tests/              # Integration tests & benchmark data (LFS-tracked volumetric files)
├── dev_env.sh          # Local toolchain environment activation script
├── CHANGELOG.md        # Release history
└── README.md
```

---

## Documentation

| Document | Audience | Description |
|---|---|---|
| [User Manual](docs/UserManual.md) | End users | Feature walkthrough, import/export, UI guide |
| [Developer Guide](docs/DeveloperGuide.md) | Contributors | Architecture, build system, coding conventions |
| [Algorithms](docs/Algorithms.md) | Developers / Researchers | Mathematical formulations (Slab, BZ, Marching Cubes, Ray-Picking, etc.) |
| [IPC Commands](docs/IPC_Commands.md) | Frontend developers | All 55 Tauri `invoke()` signatures with types and side effects |
| [Shader Reference](docs/Shader_Reference.md) | GPU developers | Bind group layouts, vertex formats, lighting parameters for all 7 WGSL shaders |
| [Testing Guide](docs/TestingGuide.md) | Contributors | Node TDD process, test inventory (12 Rust + 6 C++), tolerances |
| [FAQ](docs/FAQ.md) | All | Installation troubleshooting, rendering issues, common errors |

---

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Notes

- **Primary dev platform**: macOS (Intel & Apple Silicon)
- **Environment**: Always `source dev_env.sh` before building. API keys stored in OS Keychain — never in code or logs.
- **Code conventions**: `snake_case` for variables/functions, `PascalCase` for types, physics symbol fidelity preserved (e.g., `sigma_k` ≠ `Sigma_K`). See [CONTRIBUTING.md](CONTRIBUTING.md) for full guidelines.
- **Documentation**: Internal docs in `doc/`, public docs in `docs/`. Changes tracked in `CHANGELOG.md`.

---

## License

This project is dual-licensed under the **MIT License** and the **Apache License 2.0**. You may choose either license for your use.

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)

For third-party software licenses used in this project, please see [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

---

## Acknowledgments

- [Spglib](https://spglib.github.io/spglib/) — Crystal symmetry analysis
- [Gemmi](https://gemmi.readthedocs.io/) — CIF/PDB file parsing
- [Eigen](https://eigen.tuxfamily.org/) — Linear algebra
- [Tauri](https://tauri.app/) — Desktop app framework
- [wgpu](https://wgpu.rs/) — Cross-platform GPU API
