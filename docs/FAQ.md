# CrystalCanvas FAQ & Troubleshooting

> Version: v0.6.0 | Updated: 2026-04-14

---

## Installation & Launch

### macOS: "CrystalCanvas can't be opened because the developer cannot be verified"

CrystalCanvas is not signed with an Apple Developer certificate. macOS Gatekeeper blocks unsigned applications by default.

**Fix**:
1. Open **System Settings** → **Privacy & Security**.
2. Scroll to the **Security** section — you will see *"CrystalCanvas was blocked from use because it is not from an identified developer"*.
3. Click **Open Anyway**.

Alternatively, right-click (or Control-click) the `.app` in Finder and choose **Open** from the context menu.

---

### Linux: Application crashes on launch with `wgpu` or Vulkan errors

CrystalCanvas uses the `wgpu` crate for GPU rendering. On Linux this requires a working Vulkan ICD (Installable Client Driver).

**Fix**:
- **AMD / Intel (Mesa)**:
  ```bash
  sudo apt install mesa-vulkan-drivers
  ```
- **NVIDIA**: Install the proprietary driver (`nvidia-driver-535` or newer). The open-source `nouveau` driver lacks Vulkan support.
- **Wayland compositing issues**: If the window appears but the viewport is blank, try:
  ```bash
  WEBKIT_DISABLE_COMPOSITING_MODE=1 ./crystal-canvas
  ```
- **Verify Vulkan**: Run `vulkaninfo | head -20` — if it prints GPU info, Vulkan is functional.

---

### Windows: Blank white screen on launch

A blank window typically means the Microsoft Edge WebView2 runtime failed to initialize the wgpu surface.

**Fix**:
1. Ensure Windows is fully updated.
2. Install the latest GPU drivers from your vendor (NVIDIA/AMD/Intel).
3. As a last resort, force the OpenGL backend via PowerShell (3D performance will be degraded):
   ```powershell
   $env:WGPU_BACKEND="gl"
   .\CrystalCanvas.exe
   ```

---

## 3D Viewport & Rendering

### Volumetric rendering (CHGCAR / Cube) fails with out-of-memory

High-resolution volumetric grids (e.g., $200 \times 200 \times 200$ ≈ 32 MB of `f32` data) require GPU buffer allocation. If your GPU does not have enough VRAM, the upload will fail.

CrystalCanvas wraps GPU buffer creation in `std::panic::catch_unwind`, so an OOM will return a user-visible error message instead of crashing the application.

**Fix**:
1. Reduce grid resolution in your DFT code before loading (e.g., lower `NGX`/`NGY`/`NGZ` in VASP, or use a coarser `ecutrho` in Quantum ESPRESSO).
2. Close other GPU-intensive applications to free VRAM.

---

### Isosurface color does not match the volume colormap

After changing the colormap (e.g., from Viridis to Coolwarm), the isosurface may still show the old color until you adjust the isovalue slider.

**Fix**: Drag the isovalue slider slightly. This triggers a re-dispatch of the Marching Cubes compute shader, which re-syncs the isosurface color with the active colormap.

---

## Structural Operations

### Slab generation fails: "Requires a conventional unit cell with symmetry"

CrystalCanvas rejects slab cuts on cells with spacegroup P1 (no detected symmetry). Miller indices $(hkl)$ are defined relative to the conventional cell axes, so a P1 cell cannot correctly interpret them.

**Fix**:
1. Open the **Reciprocal Space** tool panel.
2. Under **Standardization**, click **Conventional**.
3. Retry the slab cut.

If your structure was loaded from a file that lost symmetry information (e.g., an XYZ file), reload it from a CIF or POSCAR that includes the spacegroup.

---

### Brillouin Zone shows a 3D polyhedron for a 2D slab

The 2D BZ detector uses heuristics: it checks for a fractional vacancy gap $> 0.35$ along one axis **and** requires that the vacuum axis length exceeds twice the average of the other two axes ($c / \bar{a} > 2.0$). If your slab has a small vacuum layer, it may be classified as 3D.

**Fix**: Regenerate the slab with a larger vacuum gap. As a rule of thumb, use at least 15 Å of vacuum to ensure reliable 2D detection.

---

### Phonon animation fails: "Atom count mismatch"

The phonon eigenvector file must contain displacement vectors for exactly the same number of atoms as the currently loaded crystal structure.

**Fix**: Ensure that you loaded the correct structure file **before** loading the phonon data. If the structure was modified (e.g., by building a supercell), restore the original unit cell first via the **Supercell** panel → **Restore Original Cell**.

Alternatively, use the **Load Phonon (Interactive)** button, which loads both the structure and phonon data from separate files in a single step.

---

### Wannier hopping visualization is empty

The `wannier90_hr.dat` file requires `num_wann ≤ num_atoms` because orbital centers are mapped to atomic positions. If your Wannier model has more orbitals than atoms, the mapping will fail.

**Fix**: Ensure the loaded crystal has at least as many atoms as the number of Wannier functions. For multi-orbital systems, build a supercell or load the full unit cell.

---

## Development & Build

### `cargo build` fails with C++ compiler or `cxxbridge` errors

The C++ physics kernel (Spglib, Gemmi, slab/supercell builders) is compiled automatically by `build.rs` via the `cxx` crate. You need a C++17-compatible toolchain.

**Fix**:
- **macOS**: `xcode-select --install`
- **Linux**: `sudo apt install build-essential cmake`
- **Windows**: Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload.

---

### UI changes are not reflected after editing React code

The development server must be running for Hot Module Replacement (HMR) to work.

**Fix**:
```bash
npx tauri dev
```
This starts the Vite dev server alongside the Rust backend. Edits to `.tsx` files will hot-reload instantly. If you ran `npm run build` + `npx tauri dev`, the app may be serving stale assets from `dist/` — delete the `dist/` folder and restart.

---

### Spglib detection returns wrong spacegroup

Spglib uses a symmetry precision parameter (default `symprec = 1e-5`). If your structure has slight distortions (e.g., from a geometry optimization), it may be classified incorrectly.

**Fix**: This is expected behavior for near-threshold structures. If needed, manually round your coordinates to higher precision before loading.

---

*Cross-references: [UserManual.md](./UserManual.md) · [DeveloperGuide.md](./DeveloperGuide.md) · [Algorithms.md](./Algorithms.md)*
