# CrystalCanvas User Manual (v0.4)

Welcome to **CrystalCanvas**, an open-source, high-performance desktop application for computational materials science and condensed matter physics. CrystalCanvas provides a fluid native experience for building, transforming, analyzing, and exporting complex crystalline geometries.

---

## 🚀 1. Installation

### Pre-compiled Binaries
Download from the **[GitHub Releases](https://github.com/XiaoJiang-Phy/CrystalCanvas/releases)** page.
- **macOS**: Download the `.dmg` file. Open and drag to `Applications`. *(Supports Apple Silicon and Intel Macs.)*
- **Windows / Linux**: Experimental — see release notes for platform availability.

> **macOS Gatekeeper Note**: Since releases are not code-signed, right-click the app and select **Open**, or run `sudo xattr -cr /Applications/CrystalCanvas.app`.

### Building from Source
```bash
git clone https://github.com/XiaoJiang-Phy/CrystalCanvas.git
cd CrystalCanvas
source dev_env.sh          # Project-local Rust/Cargo toolchain
rustup toolchain install stable
pnpm install
pnpm run tauri dev         # Starts Vite dev server + native window
```

---

## 🖥 2. User Interface Overview

CrystalCanvas uses a Hybrid Window design — a React/TS UI floating over a native wgpu (Metal/Vulkan) 3D viewport.

### 2.1 The 3D Viewport (Center)
Atoms are rendered as GPU Impostor Spheres at 60 FPS for up to ~1000 atoms.

- **Orbit / Rotate**: Left-click and drag in empty space.
- **Pan**: Right-click (or `Ctrl`+Left-click) and drag.
- **Zoom**: Scroll wheel (or trackpad pinch).
- **Select Atom(s)**: Left-click on an atom. Hold `Shift` for multi-select.

### 2.2 Top Navigation Bar
- **Interaction Modes**: Pick, Move, Rotate cursors.
- **View Axes**: Snap camera to lattice basis (`a`, `b`, `c`) or reciprocal vectors (`a*`, `b*`, `c*`).
- **Labels Toggle**: Show/hide element labels on atoms.
- **Light/Dark Mode**: Toggle application theme.
- **LLM Assistant (Bot Icon)**: Open AI chat panel.

### 2.3 Left Toolbar (I/O)
- **Import**: Drag-and-drop or menu. Supported: `.cif`, `.pdb`, `.xyz`, `.POSCAR`, `.scf.in`.
- **Export**: Native high-fidelity exporters:
  - **VASP**: POSCAR
  - **LAMMPS**: Data file
  - **Quantum ESPRESSO**: `.in` (auto K-point density, IUPAC 2021 masses)
- **Settings**: Application preferences and rendering defaults.

### 2.4 Right Sidebar (Analysis & Transformations)
The right sidebar hosts all physics operations, organized into collapsible accordion panels.

---

## ⚗️ 3. Structural Analysis

### Bond Analysis
Click **Analyze** in the Structural Analysis panel to compute:
- **Bond lengths** and their statistical distribution.
- **Coordination polyhedra** (octahedra, tetrahedra, etc.) for transition metal sites.
- **Distortion Index** ($\Delta$) quantifying deviation from ideal geometry.

### Cell Standardization & Reduction
Three one-click transforms in the **Reciprocal Space** panel:
- **Niggli Reduce**: Transform to the unique Niggli-reduced cell.
- **Primitive**: Convert conventional cell → primitive cell.
- **Conventional**: Convert primitive cell → conventional cell.

---

## 🔷 4. Brillouin Zone & K-Path Generator

### Computing the Brillouin Zone
1. Load a crystal structure.
2. Open the **Reciprocal Space** accordion in the Right Sidebar.
3. Click **Compute Brillouin Zone**.

CrystalCanvas automatically:
- Identifies the **Bravais lattice type** from the space group (14 types for 3D, 5 for 2D).
- Constructs the **Wigner-Seitz cell** in reciprocal space.
- Labels all **high-symmetry k-points** ($\Gamma$, $K$, $M$, $X$, $L$, $W$, $U$, etc.).
- Displays the BZ wireframe in an orthographic locked view.

### 2D Material Support
For slab/monolayer structures (large vacuum gap along one axis), the system automatically:
- Detects the vacuum axis and activates 2D mode.
- Projects the reciprocal lattice onto the in-plane 2D subspace.
- Classifies the 2D Bravais type (Hexagonal, Square, Rectangular, Centered-Rectangular, Oblique).
- Shows the 2D BZ as a polygon with appropriate k-point labels.

### Generating Band Paths
1. Set $N_k$ (points per segment, default 20).
2. Choose output format: **QE (crystal)** or **VASP**.
3. Click **💾 Generate & Save K-Path**.
4. A save dialog exports the k-path file ready for DFT band structure calculations.

---

## 🔬 5. Slab Cleaving (Cutting Plane)

Create surface models by specifying Miller indices.

1. Open **Cutting Plane** in the Right Sidebar.
2. Enter Miller indices $(h, k, l)$, number of layers, and vacuum thickness (Å).
3. Click **Cut**.

The algorithm uses a **Diophantine surface basis** with c-axis orthogonalization, guaranteeing $\alpha = \beta = 90°$ for all Miller indices. Layer termination can be shifted via the termination selector.

> **Note**: Input must be a conventional cell (not P1). The UI will warn if a P1 cell is detected.

---

## 🧊 6. Supercell Construction

Expand the unit cell periodically:
1. Open **Supercell Construction** in the Right Sidebar.
2. Enter multipliers ($N_a$, $N_b$, $N_c$).
3. Click **Build**. The expanded structure replaces the current state.

---

## 📊 7. Volumetric Data Visualization

CrystalCanvas provides real-time volumetric rendering for charge densities, wavefunctions, and electrostatic potentials.

### Supported Formats
- **VASP**: CHGCAR, LOCPOT (with $V_\text{cell}$ normalization)
- **Gaussian**: Cube files (Bohr→Å auto-conversion)
- **XSF**: DATAGRID_3D

### Loading Data
Drag-and-drop a volumetric file onto the window, or use the **Volumetric Data** panel.

### Render Modes
- **Isosurface**: GPU Marching Cubes mesh extraction at a user-defined isovalue.
- **Volume**: Front-to-back raycasting with depth-aware compositing.
- **Both**: Combined view with soft-fade clipping at the isosurface boundary.

### Controls
- **Isovalue Slider**: Adjust the isosurface threshold.
- **Sign Mode**: Display positive lobe only, negative only, or both (dual-color).
- **Colormap**: 10 scientific colormaps — Viridis, Inferno, Plasma, Magma, Cividis, Turbo, Hot, Grayscale, Coolwarm, RdYlBu.
- **Opacity / Density Cutoff**: Fine-tune transparency and trim low-density regions.

---

## 🌊 8. Phonon Visualizer

Animate phonon eigenvectors directly on your crystal structure.

1. Load a base crystal (CIF or SCF input).
2. Open **Phonon Animation** in the Right Sidebar.
3. Click **Load Phonon Data** and select your file:
   - Quantum ESPRESSO `modes` / `dynmat.dat`
   - Molden `.mold` format
   - AXSF animated format
4. Select a q-point frequency mode from the dropdown (imaginary modes marked with `(i)`).
5. Adjust the **Amplitude** slider and click **▶ Play Animation**.

---

## 🤖 9. LLM Command Bus (Experimental)

A context-aware AI assistant translates natural language into validated physics operations.

### Setup
1. Click the **Bot Icon** in the Top Navigation bar.
2. Select your provider (OpenAI, DeepSeek, or Local Ollama).
3. Enter your API key and click **Save**.
   - Keys are stored in your OS Keychain — never in plain text.

### Usage
The LLM acts as a "Semantic Parameterizer" — it does not write code, but maps your intent to rigorous tool calls.

**Example prompts:**
- *"Replace all sodium atoms with potassium."*
- *"Generate a 2×2×2 supercell."*
- *"Cut a (110) surface with 3 layers and 15 Å vacuum."*

The LLM presents a JSON command card. Click **Execute** to approve the operation.

---

## 📁 Supported File Formats

| Format | Import | Export | Notes |
|--------|--------|--------|-------|
| CIF | ✅ | — | Via Gemmi (C++) |
| PDB | ✅ | — | Via Gemmi |
| XYZ | ✅ | — | |
| POSCAR | ✅ | ✅ | VASP 5 format |
| LAMMPS Data | — | ✅ | |
| QE Input | ✅ | ✅ | Auto K-point density, IUPAC masses |
| CHGCAR/LOCPOT | ✅ | — | Volumetric |
| Gaussian Cube | ✅ | — | Volumetric |
| XSF | ✅ | — | Volumetric (DATAGRID_3D) |

---

*CrystalCanvas v0.4.0 — Copyright © 2026 Xiao Jiang and CrystalCanvas Contributors. Dual-licensed under MIT and Apache-2.0.*
