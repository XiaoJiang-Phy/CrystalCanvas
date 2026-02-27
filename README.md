# 🔬 CrystalCanvas

**High-performance crystal structure modeling, slab cleaving, and DFT/MD file preparation — in a native desktop app.**

CrystalCanvas is an open-source desktop GUI application designed for computational materials science, condensed matter physics, and quantum chemistry. It breaks free from the limitations of traditional tools (VESTA, Materials Studio) by combining a native-first architecture with modern AI-powered workflows.

---

## ✨ Key Features

- **🖱️ Pixel-precise manual modeling** — Hardware-accelerated 3D view with real-time atom selection, addition, deletion, and element substitution.
- **⚙️ Industrial-grade physics kernel** — C++ engine with Spglib (space group analysis), Eigen (matrix transforms), and Gemmi (CIF/PDB parsing).
- **🧠 AI-powered workflow** *(experimental)* — Natural language commands like *"Generate a 3×3×3 silicon supercell and dope 5% phosphorus on the surface"*.
- **🔌 Seamless DFT/MD integration** — Native import/export for VASP (POSCAR), LAMMPS, Quantum ESPRESSO, CIF, XYZ, PDB formats.
- **🛡️ Memory-safe architecture** — Rust logic layer eliminates crashes from dangling pointers and buffer overflows.

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────┐
│  React + TypeScript + TailwindCSS  (UI)             │
├─────────────────────────────────────────────────────┤
│  Rust / Tauri 2.0  (Application Logic / SSoT)       │
├─────────────────────────────────────────────────────┤
│  Rust / wgpu  (Rendering: Impostor Sphere)          │
├─────────────────────────────────────────────────────┤
│  C++ Kernel  (Spglib / Gemmi / Eigen)               │
└─────────────────────────────────────────────────────┘
```

| Layer | Technology | Role |
|---|---|---|
| **Presentation** | React + TailwindCSS | UI panels, toolbars, chat |
| **Application** | Rust / Tauri 2.0 | State management, IPC, file I/O |
| **Rendering** | Rust / wgpu | GPU-accelerated 3D (Metal / Vulkan / DX12) |
| **Compute** | C++ (Spglib, Gemmi, Eigen) | Symmetry, slab cleaving, parsing |
| **FFI Bridge** | `cxx` (Rust ↔ C++) | Type-safe, zero-copy data transfer |

---

## 🚀 Getting Started

### Build & Run

CrystalCanvas is currently in active development.

#### Run Rendering Demo (Standalone)
We have just completed the **M3: High-Performance Rendering Engine** phase. You can run the standalone GPU-accelerated demo now:

```bash
# 1. Setup local environment (if not already done)
# source dev_env.sh (if you use our local toolchain setup)

# 2. Run the demo
cd src-tauri
RUST_LOG=info cargo run --bin render_demo
```
*Controls: Left-click drag to rotate, scroll to zoom.*

#### Full App Development (M4+)
```bash
# Install Node dependencies
npm install

# Run in development mode
npm run tauri dev
```

> **Note**: The C++ kernel (Spglib, Gemmi) is compiled automatically via `build.rs` — no manual CMake step required.

---

## 🗺️ Roadmap & Progress

- [x] **M1-M2: Infrastructure & Data Model** — Rust/C++ bridge, CIF parsing.
- [x] **M3: High-Performance Rendering (wgpu)** — Impostor spheres, ray-picking, orbital camera.
- [ ] **M4: UI Integration (Tauri + React)** — (In Progress) Hybrid window, sidebars, file loading.
- [ ] **M5-M6: Geometry Algorithms** — Slab cleaving, supercells.
- [ ] **M9+: AI Agent Integration** — Natural language modeling commands.

---

## 📁 Project Structure

```
CrystalCanvas/
├── src-tauri/          # Rust backend (Tauri commands, state, wgpu)
│   ├── src/
│   ├── build.rs        # Unified Rust + C++ build
│   └── Cargo.toml
├── src/                # React frontend (TypeScript + TailwindCSS)
├── cpp/                # C++ physics kernel
│   ├── include/        # Public headers (thin C wrappers)
│   ├── src/            # Implementation (Spglib, Gemmi, Eigen)
│   └── CMakeLists.txt
├── shaders/            # WGSL shader sources
├── README.md
└── .gitignore
```

---

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Notes

- **Primary dev platform**: macOS (Intel & Apple Silicon)
- **Environment toolchains** should be installed locally within the project directory when possible (see `.gitignore` for excluded paths).
- Internal docs (`roadmap.md`, `docs/`) are **not tracked in git** — they are local planning documents.

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
