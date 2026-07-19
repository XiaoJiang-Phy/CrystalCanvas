# CrystalCanvas GPU Shader Reference

> Baseline: `v0.6.1` | Updated: 2026-07-19

CrystalCanvas uses WGSL exclusively. Shader changes must pass the project's wgpu/naga validation path and preserve the Rust-side buffer layouts in `src-tauri/src/renderer/`.

---

## Current inventory

| Shader | Location | Stage | Purpose |
|---|---|---|---|
| `marching_cubes.wgsl` | `src-tauri/shaders/` | Compute | bounded GPU isosurface extraction |
| `isosurface_render.wgsl` | `src-tauri/shaders/` | Vertex + fragment | signed isosurface shading |
| `volume_raycast.wgsl` | `src-tauri/shaders/` | Vertex + fragment | direct scalar-field rendering |
| `impostor_sphere.wgsl` | `src-tauri/shaders/` | Vertex + fragment | analytic atom spheres |
| `bond_cylinder.wgsl` | `src-tauri/shaders/` | Vertex + fragment | instanced 12-segment bond cylinders |
| `line.wgsl` | `src-tauri/src/renderer/shaders/` | Vertex + fragment | unit-cell, measurement, and overlay lines |
| `bz_blit.wgsl` | `src-tauri/src/renderer/shaders/` | Vertex + fragment | Brillouin-zone offscreen texture blit |

---

## Data and binding layout

| Path | Bind groups | Main input/output |
|---|---|---|
| Marching Cubes | Group 0 bindings 0–5 | parameters, scalar field, lookup tables, bounded vertex buffer, atomic counter |
| Isosurface render | group 0 camera; group 1 material | `IsoVertex` position, normal, sign flag |
| Volume raycast | group 0 camera; group 1 bindings 0–2 | volume parameters, scalar storage buffer, opaque depth texture |
| Atom impostor | group 0 camera | per-instance position, radius, RGBA color |
| Bond cylinder | group 0 camera | per-instance start, radius, end, RGBA color |
| Line | group 0 camera | per-vertex position and RGBA color |
| BZ blit | group 0 texture and sampler | offscreen BZ texture |

Rust owns the corresponding layouts. In particular, `AtomInstance` is 32 bytes (`vec3<f32>` position, `f32` radius, `vec4<f32>` color); `BondInstance` carries start/radius/end/padding/color; and `LineVertex` carries position/color. Change Rust and WGSL together, then validate all locations, strides, formats, and bind-group indices.

---

## Rendering behavior

### Atoms and bonds

Atom impostors solve the sphere surface in the fragment shader and correct depth for the analytic hit. Bond cylinders are expanded in the vertex shader from instanced line endpoints; the shader emits 12 radial segments. Both paths use the shared camera matrices and shaded RGBA output.

### Isosurfaces and volumes

Marching Cubes uses a 4×4×4 workgroup. It reads a normalized scalar grid and writes a bounded triangle-vertex stream through an atomic counter. The isosurface render path uses the generated sign flag to choose the configured signed material color.

The volume path transforms a fractional cube by the supplied lattice, samples the scalar field along camera rays, and composites samples against the opaque depth path. It is a visualization algorithm; it must not silently redefine imported data units or scalar ranges.

### Lines and Brillouin-zone overlay

The line shader is an unlit position/color pass. `bz_blit.wgsl` creates a six-vertex full-screen quad, samples the offscreen BZ texture, applies its border/pad behavior, and composites the overlay into the main presentation.

---

## Shader-change checklist

1. Keep WGSL as the only shader language.
2. Update Rust buffer layout, shader locations, and bind groups together.
3. Preserve renderer ownership: shaders consume prepared scene data and never define physical state.
4. Add a focused gate before behavior changes, then run the relevant Rust, IPC/UI, and build checks.
5. Do not add SSAO, shadows, MSAA/SSAA, or export-only render protocols to the interactive path without a measured requirement and an approved publication-rendering design.

High-fidelity lighting and tiled export are future publication-rendering work. They must remain separable from responsive interactive rendering.

See [Algorithms.md](Algorithms.md), [DeveloperGuide.md](DeveloperGuide.md), and [TestingGuide.md](TestingGuide.md) for the connected implementation, ownership, and verification rules.
