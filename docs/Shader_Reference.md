# GPU Shader Reference

> Version: v0.6.0 | Updated: 2026-04-14

CrystalCanvas uses a pure WebGPU (`wgpu`) rendering pipeline with all shaders written in WGSL and validated through `naga`. This document catalogs every shader file, its pipeline stage, bind group layout, and rendering behavior.

---

## Shader Inventory

| File | Location | Stage | Purpose |
|---|---|---|---|
| `marching_cubes.wgsl` | `shaders/` | Compute | GPU isosurface extraction |
| `isosurface_render.wgsl` | `shaders/` | Vertex + Fragment | Isosurface shading (dual-color, two-sided) |
| `volume_raycast.wgsl` | `shaders/` | Vertex + Fragment | Direct volume rendering (ray marching) |
| `impostor_sphere.wgsl` | `shaders/` | Vertex + Fragment | Atom visualization (ray-sphere impostor) |
| `bond_cylinder.wgsl` | `shaders/` | Vertex + Fragment | Bond visualization (procedural cylinder) |
| `bz_blit.wgsl` | `renderer/shaders/` | Vertex + Fragment | BZ texture blit (picture-in-picture) |
| `line.wgsl` | `renderer/shaders/` | Vertex + Fragment | Wireframe edges (unit cell, BZ, Wannier hoppings) |

---

## 1. `marching_cubes.wgsl` — Compute Shader

**Entry point**: `fn main(@builtin(global_invocation_id) global_id: vec3<u32>)`  
**Workgroup size**: `@workgroup_size(4, 4, 4)`

### Bind Group 0

| Binding | Type | Content |
|---|---|---|
| 0 | `uniform` | `MCParams` (grid dims, lattice vectors, origin, threshold, sign_mode) |
| 1 | `storage<read>` | `scalar_field: array<f32>` — flat 3D volumetric data |
| 2 | `storage<read>` | `edge_table: array<i32>` — 256-entry edge intersection bitmask |
| 3 | `storage<read>` | `tri_table: array<i32>` — 256×16 triangle vertex table |
| 4 | `storage<read_write>` | `vertices: array<IsoVertex>` — output vertex buffer |
| 5 | `storage<read_write>` | `counter: atomic<u32>` — vertex append counter |

### `IsoVertex` Layout (32 bytes)

| Field | Type | Description |
|---|---|---|
| `pos_x/y/z` | `f32` × 3 | Cartesian position (Å) |
| `norm_x/y/z` | `f32` × 3 | Interpolated gradient normal |
| `sign_flag` | `f32` | +1.0 (positive lobe) or −1.0 (negative lobe) |
| `_pad` | `f32` | Alignment padding |

### Sign Mode Logic

| `sign_mode` | Classification | Effective threshold |
|---|---|---|
| 0 (positive) | `val >= threshold` | `+threshold` |
| 1 (negative) | `val <= -threshold` | `-threshold` |
| 2 (both) | `abs(val) >= threshold` | Sign determined per-edge by `avg(fa, fb) < 0.0` |

### Key Functions

- `interp_t(f0, f1, threshold)`: Linear interpolation parameter, clamped to 0.5 when $|f_1 - f_0| < 10^{-7}$
- `frac_to_cart(u, v, w)`: Fractional → Cartesian via `origin + u·a + v·b + w·c`
- `grad(x, y, z)`: Central-difference gradient in grid space
- `gradient_at(ix, iy, iz, t, edge_axis)`: Per-edge gradient interpolation with 12-case switch, normalized to $-\hat{\nabla}V$ (outward-pointing normals)
- **Atomic append**: `atomicAdd(&counter, 3u)` with overflow guard (`start_idx + 2 >= arrayLength(&vertices)`)

---

## 2. `isosurface_render.wgsl` — Vertex + Fragment

**Entry points**: `vs_main`, `fs_main`

### Bind Groups

| Group | Binding | Type | Content |
|---|---|---|---|
| 0 | 0 | `uniform` | `CameraUniforms` (view_proj, view, proj) |
| 1 | 0 | `uniform` | `IsosurfaceUniforms` (color, color_negative) |

### Vertex Input

Reads from the `IsoVertex` buffer: `position` (location 0), `normal` (location 1), `sign_flag` (location 2).

### Fragment Shader — Two-Sided Blinn-Phong

The shader detects front vs. back faces via `normal.z > 0.0` (view-space z convention) and applies distinct material parameters:

| Parameter | Front face | Back face |
|---|---|---|
| Ambient | 0.18 | 0.22 |
| Diffuse weight | 0.72 | 0.58 |
| Specular weight | 0.35 | 0.15 |
| Fresnel rim | `0.25 × (1 - N·V)³` | 0.0 |

**Dual coloring**: `sign_flag < 0.0` selects `color_negative`; otherwise `color`. Alpha is passed through from the uniform.

---

## 3. `volume_raycast.wgsl` — Vertex + Fragment (474 lines)

**Entry points**: `vs_main`, `fs_main`

### Bind Groups

| Group | Binding | Type | Content |
|---|---|---|---|
| 0 | 0 | `uniform` | `CameraUniforms` |
| 1 | 0 | `uniform` | `VolumeRaycastUniforms` (lattice, inv_lattice, eye_pos, origin, grid_dims, transfer params, colormap_mode, etc.) |
| 1 | 1 | `storage<read>` | `scalar_field: array<f32>` |
| 1 | 2 | `texture_depth_2d` | `depth_tex` — opaque geometry depth buffer for early-out |

### Vertex Shader

Transforms unit-cube vertices ($[0,1]^3$) to world space via lattice vectors: `origin + x·a + y·b + z·c`. Passes both `world_pos` and `frac_pos` to fragment stage.

### Fragment Shader — Ray Marching Pipeline

1. **Ray construction**: Perspective (eye → fragment) or orthographic (`camera_forward`), detected via `is_orthographic` flag.
2. **Fractional transform**: Ray origin/direction transformed via `inv_lattice_{a,b,c}` dot products.
3. **Unit-cube intersection**: AABB slab test yields `[t_min, t_max]`.
4. **Trilinear sampling**: `sample_field_frac` — 8-corner interpolation with boundary clamping.
5. **Transfer function**: `apply_transfer_function(val)` — maps scalar to RGBA via:
   - Unsigned: $t = |V| / V_{\max}$, smoothstep opacity $[0.05, 0.3]$
   - Signed: sqrt-stretch $t = \sqrt{|V/V_{\max}|}$ with sign-dependent colormap offset
   - Near-zero transparency: $t < 0.05 \Rightarrow \alpha = 0$
6. **Per-sample Blinn-Phong**: Gradient computed in fractional space, transformed to world space. Light direction $(0.3, 0.6, 0.8)$, ambient 0.2, diffuse 0.6, specular 0.2, shininess 32.
7. **Depth early-out**: Precomputed linear clip-space evolution $\mathbf{c}(i) = \mathbf{c}_0 + i \cdot \Delta\mathbf{c}$. Terminates if `step_depth >= opaque_depth`.
8. **Front-to-back compositing**: Pre-multiplied alpha; early termination at $\alpha \ge 0.95$.

### Colormaps (10 total, piecewise smoothstep)

| `colormap_mode` | Name | Type |
|---|---|---|
| 0 | Viridis | Sequential |
| 1 | Grayscale | Sequential |
| 2 | Inferno | Sequential |
| 3 | Plasma | Sequential |
| 4 | Coolwarm | Diverging (blue → white → red) |
| 5 | Hot | Sequential |
| 6 | Magma | Sequential |
| 7 | Cividis | Sequential |
| 8 | Turbo | Rainbow |
| 9 | RdYlBu | Diverging (red → yellow → blue) |

---

## 4. `impostor_sphere.wgsl` — Vertex + Fragment

**Entry points**: `vs_main`, `fs_main_impl`  
**Note**: `fs_main` is a dead stub; the actual fragment entry point is `fs_main_impl`, which returns `FragOutput { color, depth }`.

### Bind Group 0

| Binding | Type | Content |
|---|---|---|
| 0 | `uniform` | `CameraUniforms` |

### Vertex Input (per-instance)

| Location | Field | Type |
|---|---|---|
| 0 | `atom_position` | `vec3<f32>` |
| 1 | `atom_radius` | `f32` |
| 2 | `atom_color` | `vec4<f32>` |

### Vertex Shader

- 6 vertices per atom via `INDEX_MAP = [0,1,2, 2,1,3]` → 4 quad corners.
- Atom center transformed to view space.
- Billboard expanded by `radius × 1.2` in view-space XY.

### Fragment Shader

1. **Ray-sphere intersection**: Ray origin = billboard position in view space, direction = $(0, 0, -1)$.
2. **Quadratic**: $\Delta = b^2 - 4ac$, where $\mathbf{oc} = \mathbf{ray\_pos} - \mathbf{center}$.
3. **Discard** if $\Delta < 0$.
4. **Hit point**: $t = (-b - \sqrt{\Delta}) / 2a$.
5. **Depth write**: `@builtin(frag_depth) = clip.z / clip.w` via `camera.proj * hit_view`.
6. **Blinn-Phong**: Light $(0.3, 0.6, 0.8)$, ambient 0.15, diffuse 0.7, specular 0.4, shininess 32.

---

## 5. `bond_cylinder.wgsl` — Vertex + Fragment

**Entry points**: `vs_main`, `fs_main`

### Bind Group 0

| Binding | Type | Content |
|---|---|---|
| 0 | `uniform` | `CameraUniforms` |

### Vertex Input (per-instance)

| Location | Field | Type |
|---|---|---|
| 0 | `start` | `vec3<f32>` — bond start position |
| 1 | `radius_len` | `f32` — cylinder radius |
| 2 | `end` | `vec3<f32>` — bond end position |
| 3 | `color` | `vec4<f32>` |

### Vertex Shader — Procedural Cylinder

- 12-segment cylinder: 72 vertices (12 faces × 6 vertices/face, no index buffer).
- Orthonormal basis `(right, fw, up)` constructed from the bond axis with singularity avoidance (switches reference vector when `|up.x| > 0.99`).
- Each vertex positioned at `mix(start, end, is_end) + right × cos(θ) × r + fw × sin(θ) × r`.
- Normal = `normalize(local_p)` — radial outward.

### Fragment Shader — Blinn-Phong

Light $(0.3, 0.6, 0.8)$, ambient 0.15, diffuse 0.7, specular 0.3, shininess 16.

---

## 6. `line.wgsl` — Vertex + Fragment

**Entry points**: `vs_main`, `fs_main`

### Bind Group 0

| Binding | Type | Content |
|---|---|---|
| 0 | `uniform` | `CameraUniform` |

### Vertex Input

| Location | Field | Type |
|---|---|---|
| 0 | `position` | `vec3<f32>` |
| 1 | `color` | `vec4<f32>` |

### Behavior

Minimal passthrough shader. Transforms position via `view_proj`, forwards color to fragment. No lighting — used for unit cell wireframes, BZ edges, Wannier hopping lines, and k-path segments.

---

## 7. `bz_blit.wgsl` — Fullscreen Quad Blit

**Entry points**: `vs_main`, `fs_main`

### Bind Group 0

| Binding | Type | Content |
|---|---|---|
| 0 | `texture_2d<f32>` | `bz_texture` — offscreen BZ render target |
| 1 | `sampler` | `bz_sampler` |

### Behavior

- Generates a fullscreen quad from 6 hardcoded vertex positions (no vertex buffer).
- UV mapping: `(x * 0.5 + 0.5, 0.5 - y * 0.5)` (Y-flip for texture convention).
- **Border**: If edge distance < 0.015, draws a gray border `(0.6, 0.6, 0.6, 1.0)`.
- **Background fill**: If `color.a < 0.1`, fills with dark translucent pad `(0.15, 0.15, 0.15, 0.85)`.
- Otherwise passes through the sampled BZ texture color.

---

## Shared Conventions

All shaders share these conventions:

| Convention | Value |
|---|---|
| **Camera uniform struct** | `{ view_proj, view, proj }` — 3 × `mat4x4<f32>` |
| **Light direction** | Normalized $(0.3, 0.6, 0.8)$ — upper-right-front |
| **Lighting model** | Blinn-Phong (ambient + diffuse + specular with half-vector) |
| **Coordinate system** | Right-handed, camera looks down $-z$ in view space |
| **Depth convention** | `clip.z / clip.w` — normalized device depth for `@builtin(frag_depth)` |

---

*Cross-references: [Algorithms.md](./Algorithms.md) · [IPC_Commands.md](./IPC_Commands.md) · [DeveloperGuide.md](./DeveloperGuide.md)*
