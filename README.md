<p align="center">
  <img src="logo.svg" width="200" alt="CrystalCanvas Logo">
</p>

# 🔬 CrystalCanvas

**High-performance crystal structure modeling, volumetric visualization, and DFT/MD file preparation — in a native desktop app.**

CrystalCanvas is an open-source desktop GUI application designed for computational materials science, condensed matter physics, and quantum chemistry. It breaks free from the limitations of traditional tools (VESTA, Materials Studio) by combining a native-first architecture with modern AI-powered workflows.

---

## ✨ Key Features

- **🖱️ Pixel-precise manual modeling** — Hardware-accelerated 3D view with real-time atom selection, addition, deletion, and element substitution.
- **🔷 Brillouin Zone visualization** — 3D Wigner-Seitz cell construction (14 Bravais types) and 2D BZ support (5 wallpaper types) with high-symmetry k-point labeling, one-click band path export for QE/VASP.
- **⚡ Tight-Binding (Wannier) visualizer** — Parse `wannier90_hr.dat` hopping Hamiltonians and render as 3D network overlays with per-orbital color coding, R-shell/orbital selection, magnitude filtering, and ghost atom rendering for neighboring cells.
- **📊 Volumetric data visualization** — Real-time isosurface extraction (GPU Marching Cubes) and volume raycasting for CHGCAR, Gaussian Cube, and XSF files. 10 scientific colormaps, dual-color signed isosurfaces, density cutoff control.
- **⚛️ Cell standardization** — Niggli reduction, Primitive/Conventional cell transforms via Spglib.
- **⚙️ Industrial-grade physics kernel** — C++ engine with Spglib (space group analysis), Eigen (matrix transforms), and Gemmi (CIF/PDB parsing).
- **🧠 AI-powered workflow** *(experimental)* — Natural language commands like *"Generate a 3×3×3 silicon supercell and dope 5% phosphorus on the surface"*. Context-aware LLM acts as a semantic parameterizer and command orchestrator with strict physics validation (MIC overlap checks).
- [🔌 **Seamless DFT/MD integration**](CHANGELOG.md) — Native high-fidelity export for VASP (POSCAR), LAMMPS (Data), Quantum ESPRESSO (Input with automatic K-point density and IUPAC 2021 masses).
- **🛡️ Memory-safe architecture** — Rust logic layer eliminates crashes from dangling pointers and buffer overflows.

---

## 🛠️ Known Issues

> **⚠️ Platform Support:**
> Due to rendering engine (`wgpu`) backend differences, **Windows** and **Linux** builds may have rendering issues. Currently **only macOS** is fully tested and supported.

### 🍎 Note for macOS Users (Unverified Developer)

Since releases are not signed with an Apple Developer Certificate, macOS will prevent it from running with a "Developer cannot be verified" warning.

**To run the app:**
1. Move `CrystalCanvas.app` to your `/Applications` folder.
2. **Right-click (or Control-click)** the app icon and select **Open**.
3. Click **Open** again in the dialog box.

Alternatively, run the following command in Terminal:
```bash
sudo xattr -cr /Applications/CrystalCanvas.app
```

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────┐
│  React + TypeScript + TailwindCSS  (UI)             │
├─────────────────────────────────────────────────────┤
│  Rust / Tauri 2.0  (Application Logic / SSoT)       │
├─────────────────────────────────────────────────────┤
│  Rust / wgpu  (Impostor Sphere + Volume Raycast + BZ) │
├─────────────────────────────────────────────────────┤
│  WGSL Compute  (GPU Marching Cubes / Raycasting)    │
├─────────────────────────────────────────────────────┤
│  C++ Physics Kernel  (Spglib / Gemmi / Eigen)       │
└─────────────────────────────────────────────────────┘
```

| Layer | Technology | Role |
|---|---|---|
| **Presentation** | React + TailwindCSS | UI panels, toolbars, chat |
| **Application** | Rust / Tauri 2.0 | State management, IPC, I/O pipeline |
| **Rendering** | Rust / wgpu | GPU-accelerated 3D (Metal / Vulkan / DX12) |
| **Volumetric** | WGSL Compute + Raycast | Isosurface extraction, volume rendering |
| **Compute** | C++ (Spglib, Gemmi, Eigen) | Symmetry, Overlap Detection, MIC |
| **FFI Bridge** | `cxx` (Rust ↔ C++) | Type-safe, zero-copy data transfer |

---

## 🚀 Getting Started

**New to CrystalCanvas? Check out the [User Manual](docs/UserManual.md) for a comprehensive guide on all features.**

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

CrystalCanvas handles C++/Rust/TS full-stack compilation in a unified flow.

#### Activation
Always source the environment script before starting development to ensure `RUSTUP_HOME` and `CARGO_HOME` point to the project-local folders:
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

## 🗺️ Roadmap & Progress

- [x] **M1-M2: Infrastructure & Data Model** — Rust/C++ bridge, CIF parsing.
- [x] **M3: High-Performance Rendering (wgpu)** — Impostor spheres, ray-picking, orbital camera.
- [x] **M4-M6: UI Integration & Geometry Ops** — Hybrid window, slab cleaving, supercells, atomic operations.
- [x] **M7-M8: DFT/MD Ecology & I/O Pipeline** — Overlap detection (MIC), native exporters (VASP, QE, LAMMPS).
- [x] **M8.5: Persistent Settings & UI Polish** — Local JSON caching, global rendering customization.
- [x] **M9: LLM Command Bus** — Context-aware semantic AI agent for macro-scale geometry manipulation.
- [x] **M10: Structural Analysis & Phonons** — Polyhedra identification, defect tracking, and imaginary frequency animation.
- [x] **M10+: Brillouin Zone & Cell Standardization** — 3D/2D Wigner-Seitz BZ, high-symmetry k-path (14+5 Bravais types), Niggli/Primitive/Conventional transforms.
- [x] **M11: Volumetric Rendering** — GPU Marching Cubes isosurfaces, volume raycasting, 10 colormaps, CHGCAR/Cube/XSF parsers.
- [x] **M12: Electronic Overlays** — Wannier tight-binding hopping visualizer with ghost atoms, per-orbital palette, R-shell selection.
- [ ] **M12+: Advanced Topology** — $\mathcal{O}(N)$ Voronoi bonding, MP API agent, tensor strain generator.
- [ ] **M13+: AI4Science Phase Space** — High-throughput MLFF dataset perturbations, NEB playback, and Symmetry Subgroup extraction.

---

## 📁 Project Structure

```text
CrystalCanvas/
├── .github/            # GitHub Actions (CI/CD release workflows)
├── src-tauri/          # Rust backend (Tauri commands, state handling, wgpu orchestration)
│   ├── shaders/        # WGSL shaders (volume_raycast, marching_cubes, isosurface_render)
│   ├── src/
│   │   ├── io/         # File parsers (CIF, POSCAR, CHGCAR, Cube, XSF, QE)
│   │   ├── renderer/   # wgpu pipelines (atoms, bonds, hopping, isosurface, volume raycast)
│   │   └── ...         # State manager, command router, volumetric, wannier module
│   ├── build.rs        # Unified Rust + C++ build script (cmake/cxx bridge)
│   └── Cargo.toml
├── src/                # React frontend (TypeScript + TailwindCSS components)
│   ├── components/     # UI Panel components (RightSidebar: volumetric controls)
│   ├── hooks/          # Custom React hooks (tauri events, file-drop, 3D interaction)
│   └── types/          # Strict TS IPC mappings (e.g., CrystalState, CrystalCommand)
├── cpp/                # C++ physics kernel
│   ├── include/        # Public C-compatible headers (cxx bridge)
│   ├── src/            # Implementation code (Spglib, Gemmi, Eigen integrations)
│   └── CMakeLists.txt
├── docs/               # Public documentation ([User Manual](docs/UserManual.md))
├── tests/              # Integration tests & benchmark data (LFS-tracked volumetric files)
├── dev_env.sh          # Local toolchain environment activation script
├── CHANGELOG.md        # Release history and known issues
└── README.md
```

---

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Notes

- **Primary dev platform**: macOS (Intel & Apple Silicon)
- **Environment toolchains** should be installed locally within the project directory when possible (see `.gitignore` for excluded paths).
- Documentation in `docs/` and `CHANGELOG.md` is tracked for release transparency.

---

## 📄 License

This project is dual-licensed under the **MIT License** and the **Apache License 2.0**. You may choose either license for your use.

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)

For third-party software licenses used in this project, please see [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

---

## 🙏 Acknowledgments

- [Spglib](https://spglib.github.io/spglib/) — Crystal symmetry analysis
- [Gemmi](https://gemmi.readthedocs.io/) — CIF/PDB file parsing
- [Eigen](https://eigen.tuxfamily.org/) — Linear algebra
- [Tauri](https://tauri.app/) — Desktop app framework
- [wgpu](https://wgpu.rs/) — Cross-platform GPU API
