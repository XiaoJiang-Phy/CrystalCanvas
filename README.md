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

### Prerequisites

- **macOS** (primary): Xcode Command Line Tools
  ```bash
  xcode-select --install
  ```
- **Ubuntu** (secondary): GCC 12+, CMake 3.20+, libgtk-3-dev, libwebkit2gtk-4.1-dev

### Build

```bash
# Clone the repository
git clone https://github.com/XiaoJiang-Phy/CrystalCanvas.git
cd CrystalCanvas

# Build the project (Rust + C++ compiled together via cargo)
cargo build

# Run in development mode (with Tauri)
npm install
npm run tauri dev
```

> **Note**: The C++ kernel (Spglib, Gemmi) is compiled automatically via `build.rs` — no manual CMake step required.

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
