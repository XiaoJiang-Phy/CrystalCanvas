# 🔬 CrystalCanvas Wiki (v1.0)

Welcome to the comprehensive guide for **CrystalCanvas**, a high-performance, native desktop application designed for computational materials science and condensed matter physics.

---

## 📋 Table of Contents
1. [Introduction](#-introduction)
2. [Key Features](#-key-features)
3. [Technical Architecture](#-technical-architecture)
4. [Usage Guide](#-usage-guide)
5. [Development Roadmap](#-development-roadmap)

---

## 🌟 Introduction

CrystalCanvas is an open-source structural modeling tool that bridges the gap between high-performance physics kernels and modern user interfaces. Built with **Rust**, **C++**, and **WebGPU**, it offers a "Native-First" experience that handles thousands of atoms with zero lag, while providing an integrated AI assistant to simplify complex modeling workflows.

---

## 🚀 Key Features

### 1. Visualization & Interaction
- **Hardware-Accelerated 3D View**: Render millions of atoms using GPU impostors (Metal/Vulkan/DX12).
- **Pixel-Precise Interaction**: Real-time selection, distance measurement, and property inspection.
- **Dynamic Projection**: Switch between Perspective and Orthographic modes for realistic visualization or technical measurement.

### 2. Advanced Modeling
- **Supercell Generation**: Effortlessly expand unit cells with periodic boundary consistency.
- **Slab Cleaving (hkl)**: Create surface models by defining Miller indices, layer counts, and vacuum thickness.
- **Atomic Operations**: Instant addition, deletion, and element substitution with real-time geometric updates.

### 3. Structural Analysis
- **Symmetry Identification**: Automatic spacegroup detection using the industrial-standard **Spglib** kernel.
- **Coordination Environments**: Automatic recognition of polyhedra (Octahedra, Tetrahedra, etc.).
- **Structural Strain Metrics**: Calculation of **Distortion Index ($\Delta$)** and bond length distribution statistics.

### 4. Simulation Ecology
- **I/O Pipeline**: High-speed parsing for CIF, PDB, and XYZ.
- **Native Exporters**: High-fidelity input generation for **VASP**, **Quantum ESPRESSO**, and **LAMMPS**.

### 5. AI Assistant (Command Bus)
- **Natural Language Control**: Perform batch operations (e.g., "Dope 5% Fe onto the surface") through a validated physics-aware AI agent.

---

## 🏗️ Technical Architecture

CrystalCanvas follows a decoupled **Four-Layer Architecture** to ensure efficiency, safety, and modern UX.

### Layer Diagram
```
┌─────────────────────────────────────────────────────┐
│  L4: Presentation (React + TypeScript + Tailwind)   │
│      Modern UI & Integrated LLM Chat                │
├─────────────────────────────────────────────────────┤
│  L3: Application API (Rust + Tauri 2.0)             │
│      State SSoT, IPC Routing, File I/O Logic        │
├─────────────────────────────────────────────────────┤
│  L2: Graphics Engine (Rust + wgpu)                  │
│      GPU Impostor Shaders (WGSL)                    │
├─────────────────────────────────────────────────────┤
│  L1: Physics Kernel (C++ + Eigen + Spglib)          │
│      Symmetry, MIC Distance, Slab Projection        │
└─────────────────────────────────────────────────────┘
```

### Core Design Decisions
- **Single Source of Truth (SSoT)**: The structural state is maintained in Rust (`CrystalState`) using a SoA (Structure of Arrays) layout for zero-copy efficiency.
- **Type-Safe FFI**: We use the `cxx` crate for Rust-C++ interop, ensuring that memory safety is maintained across the language boundary.
- **Metal 2.0 Capability**: Optimized for macOS hardware acceleration (including Intel Integrated and Apple Silicon).

---

## 📖 Usage Guide

### Getting Started
- **Loading Data**: Simply Drag & Drop your `.cif` or `.xyz` files into the window.
- **Navigation**: 
    - **Left Drag**: Rotate camera.
    - **Right Drag / Middle Click**: Pan viewport.
    - **Scroll**: Zoom.

### Modeling Workflow
1. **Slab Cut**: Open the **Cutting Plane** panel in the Right Sidebar. Enter $(hkl)$, adjust the layers/vacuum sliders, and click "Cut".
2. **Supercell**: Use the **Supercell** panel to replicate your structure across lattice vectors.
3. **Analyze**: Use the **Structural Analysis** panel to identify bonding environments and calculate distortion indexes.

### Animation (Phonons)
- Load a `.mold` or `dynmat.dat` file via the **Phonon Animation** panel.
- Select a frequency and click **Play**. Use the amplitude slider to visualize the atomic displacements.

---

## 🗺️ Development Roadmap

- [x] **Phase 1**: High-speed FFI Bridge & CIF Parsing.
- [x] **Phase 2**: gpu-accelerated Impostor Rendering.
- [x] **Phase 3**: Geometric algorithms (Slab/Supercell) & Symmetry.
- [x] **Phase 4**: Native Simulation Exporters (VASP/QE/LAMMPS).
- [x] **Phase 5**: LLM Command Bus & AI Integration.
- [x] **Phase 6**: Structural Analysis & Phonon Modes (M10).
- [ ] **Phase 7**: Volumetric Rendering (Charge Density/ELF Isosurfaces).
- [ ] **Phase 8**: Trajectory Playback (MD/NEB).

---

## 📄 License
CrystalCanvas is dual-licensed under **MIT** and **Apache 2.0**.
Copyright © 2026 Xiao Jiang and CrystalCanvas Contributors.
