# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
- **⚠️ Slab Cleaving (Cutting Plane) Topologies**: The "Cutting Plane" feature currently functions correctly from a purely coordinate export perspective (underlying data is clean and isolated). However, it occasionally produces mathematically offset or visually incorrect unphysical topologies for low-symmetry crystals. We are aggressively rewriting the internal physics bounding box clipping logic to prepare for `0.1.x` fixes.

### Security
- **Strict Data Pipeline**: Enforced separation between `f64` calculation models and `f32` render projections to avoid floating-point drift accumulation in `.cif` to `.in` workflows.
