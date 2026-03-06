# CrystalCanvas User Manual (v0.1 Alpha)

Welcome to **CrystalCanvas**, an open-source, high-performance desktop application for computational materials science and condensed matter physics. CrystalCanvas provides a fluid native experience for building, transforming, and exporting complex crystalline geometries.

---

## 🚀 1. Installation 

### Pre-compiled Binaries
You can download the pre-compiled standalone application directly from the **[GitHub Releases](https://github.com/XiaoJiang-Phy/CrystalCanvas/releases)** page.
- **macOS**: Download the `.dmg` file. Open the disk image and drag the application to `Applications`. *(Supports both Apple Silicon and Intel Macs).*
- **Windows**: Download the `.exe` or `.msi` installers.
- **Linux**: Download the `.AppImage` (make executable via `chmod +x`) or the `.deb` package.

### Building from Source (Developers)
CrystalCanvas enforces a **Zero-Global-Pollution** build strategy. The core physics kernel is written in C++ (Spglib, Eigen, Gemmi) but wrapped safely by Rust.
```bash
git clone https://github.com/XiaoJiang-Phy/CrystalCanvas.git
cd CrystalCanvas
# Sourcing this script installs Rust toolchains and Cargo dependencies LOCALLY in the current directory, keeping your OS clean.
source dev_env.sh
rustup toolchain install stable
pnpm install
# Run the application
pnpm run tauri dev
```

---

## 🖥 2. User Interface Overview

CrystalCanvas features a unified Hybrid Window (Web UI floating over a high-performance native Metal/Vulkan 3D viewport).

### 2.1 The 3D Viewport (Center)
The central canvas displays atoms as mathematically perfect "Impostor Spheres" via `wgpu`, allowing for up to ~1000 atoms to be displayed at a buttery smooth 60 FPS.

- **Orbit / Rotate**: Left-Click and drag in empty space.
- **Pan**: Right-Click (or `Ctrl`+Left-Click) and drag.
- **Zoom**: Mouse Scroll Wheel (or Pinch on trackpad).
- **Select Atom(s)**: Left-Click on an atom. Hold `Shift` to add to the current selection.

### 2.2 Top Navigation Bar
- **Interaction Modes**: Select between Pick cursor, Move cursor, and Rotation interactions.
- **View Axes**: Quickly snap the camera to look down the lattice basis vectors (`a`, `b`, `c`) or reciprocal vectors (`a*`, `b*`, `c*`).
- **Toggles**: Enable/disable atomic labels or toggle between Light/Dark mode.
- **LLM Assistant (Bot Icon)**: Open the slide-out contextual AI chat window.

### 2.3 Left Toolbar (I/O & Structural Analysis)
- **Import File (+)**: Load geometry via typical drag-and-drop or explicit selection. Supports `.cif`, `.pdb`, and `.xyz`.
- **Export Data (Download Arrow)**: Instantly export the current geometry. The exporter relies strictly on underlying core physics arrays and is completely immune to any UI rendering glitches.
  - **VASP**: `.POSCAR`
  - **LAMMPS**: `.data`
  - **Quantum ESPRESSO**: `.in` (Auto-determines intelligent K-Point grids based on lattice volume).
- **Settings (Gear)**: Configure Application appearance and default output parameters.

### 2.4 Right Sidebar (Transformations)
The right sidebar controls physics transformations.

- **Supercell Construction**: Input multipliers (Nx, Ny, Nz) to expand the unit cell periodically.
- **Cutting Plane (Slab)**: *(⚠️ v0.1 Alpha Known Issue)* Enter Miller Indices (h,k,l) and layer count. **Note**: The visual clipping may occasionally slice geometry unpredictably for low-symmetry crystals, but the underlying Cartesian coordinate export remains completely mathematically isolated and functionally safe.
- **Atom Operations**: Select one or more atoms to explicitly Replace their Element or Delete them from the geometry.
- **Structural Analysis**: Calculate Polyhedral coordination numbers, identify bonding lengths, and compute Distortion Indexes (Δ) for transition metal oxides based on nearest-neighbor proximity.

---

## 🤖 3. The LLM Command Bus

CrystalCanvas ships with an experimental, highly context-aware Semantic AI built directly into the Desktop Environment.

### Setup API Key
1. Click the **Bot Icon** in the Top Navigation bar.
2. At the top of the LLM Panel, select your provider (e.g., DeepSeek, OpenAI).
3. Insert your API Key and click **Save**.
   - *Security Note*: CrystalCanvas does **not** save your key in plain text. It pushes the API Key securely into your operating system's native OS Keychain / Credential Manager.

### How it Works
The LLM does **not** write code. Instead, it acts as a "Semantic Parameterizer" mapping your fuzzy human language into rigorous C++/Rust tool calls.

**Try asking it:**
- *"Turn all Oxygen atoms on the top surface into Phosphorus."*
- *"Expand this primitive cell into a 2x2x2 supercell and give me the new atomic count."*
- *"I need to export this to LAMMPS format but I want you to replace the center iron atom with cobalt first."*

When the LLM formulates a command plan, it will present a JSON card. You must explicitly click **Execute** to approve the operation.

---

## 🌊 4. Phonon Visualizer

CrystalCanvas can map imaginary phonon modes into 3D animations directly on your crystal.

1. Create or Import a base crystal structure (e.g., via CIF or SCF input).
2. Open the **Phonon Animation** accordion in the Right Sidebar.
3. Click **Load Phonon Data (.mold/.dat)**.
4. Select your Quantum ESPRESSO `modes` file, `dynmat.dat`, or Molden format file.
5. In the dropdown, select a specific q-point frequency mode (Imaginary frequencies are marked with `(i)`).
6. Adjust the physical **Amplitude** slider, and click **▶ Play Animation** to watch the crystal vibrate based on the eigenvector perturbation!
