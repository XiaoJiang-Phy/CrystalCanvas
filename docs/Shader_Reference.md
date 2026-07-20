# CrystalCanvas GPU Shader Reference

> Baseline: `v0.6.1` | Updated: 2026-07-20

CrystalCanvas uses WGSL exclusively. This reference maps each current shader to its Rust pipeline, buffers, entry points, and validation responsibilities. Shader code consumes prepared presentation data. It never owns crystal state or defines the scientific meaning of an imported field.

---

## Shader inventory

| Shader | Location | Stage | Rust owner | Purpose |
|---|---|---|---|---|
| `marching_cubes.wgsl` | `src-tauri/shaders/` | compute | `renderer/isosurface.rs` | bounded GPU isosurface extraction |
| `isosurface_render.wgsl` | `src-tauri/shaders/` | vertex + fragment | `renderer/isosurface.rs` | signed isosurface shading |
| `volume_raycast.wgsl` | `src-tauri/shaders/` | vertex + fragment | `renderer/volume_raycast.rs` | direct scalar-field rendering |
| `impostor_sphere.wgsl` | `src-tauri/shaders/` | vertex + fragment | `renderer/pipeline.rs` | analytic atom spheres |
| `bond_cylinder.wgsl` | `src-tauri/shaders/` | vertex + fragment | `renderer/pipeline.rs` | instanced 12-segment bond cylinders |
| `line.wgsl` | `src-tauri/src/renderer/shaders/` | vertex + fragment | `renderer/pipeline.rs` | unit-cell, measurement, and overlay lines |
| `bz_blit.wgsl` | `src-tauri/src/renderer/shaders/` | vertex + fragment | `renderer/bz_renderer.rs` | Brillouin-zone offscreen texture blit |

Shaders under `src-tauri/shaders/` are included by the main renderer modules. The two shaders under `renderer/shaders/` are local to their renderer paths. Search the Rust owner before changing an entry-point name, bind group, output target, or depth/blend state.

---

## Shared camera uniforms

Atom, bond, line, isosurface, and volume pipelines use a camera uniform at group 0, binding 0. The common logical fields are:

```wgsl
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
};
```

Not every shader gives the struct the same name. The Rust buffer and WGSL memory layout must still agree exactly. WGSL matrices are column-major, matching the project convention. Do not reorder fields or add a member in only one shader. The same Rust camera buffer is bound to multiple pipelines.

---

## CPU/GPU vertex layouts

The authoritative Rust layouts are in `src-tauri/src/renderer/instance.rs` and `renderer/isosurface.rs`.

### `AtomInstance`

| Location | WGSL type | Rust field | Byte offset |
|---|---|---|---:|
| 0 | `vec3<f32>` | `position` | 0 |
| 1 | `f32` | `radius` | 12 |
| 2 | `vec4<f32>` | `color` | 16 |

The stride is 32 bytes and the step mode is `Instance`.

### `BondInstance`

| Location | WGSL type | Rust field | Byte offset |
|---|---|---|---:|
| 0 | `vec3<f32>` | `start` | 0 |
| 1 | `f32` | `radius` | 12 |
| 2 | `vec3<f32>` | `end` | 16 |
| 3 | `vec4<f32>` | `color` | 32 |

Rust includes a four-byte padding field after `end`; the stride is 48 bytes and the step mode is `Instance`.

### `LineVertex` and `IsoVertex`

`LineVertex` stores a `vec3<f32>` position at location 0 and `vec4<f32>` color at location 1. Its C-compatible structure includes the alignment implied by the declared Rust layout.

`IsoVertex` is 32 bytes: position at location 0, normal at location 1, and sign flag at location 2, followed by explicit padding. The Marching Cubes storage-buffer declaration and isosurface vertex-buffer declaration must remain byte-for-byte compatible.

When changing a layout, update the Rust `#[repr(C)]` structure, bytemuck safety assumptions, `VertexBufferLayout`, WGSL input/storage structure, allocation calculation, and every pipeline that reads it in the same node.

---

## Atom impostor shader

**File**: `src-tauri/shaders/impostor_sphere.wgsl`

**Entry points**: `vs_main`, `fs_main_impl`

**Bindings**: group 0 binding 0 camera uniform

The vertex stage uses six generated vertices per atom instance to form a camera-facing quad. It expands the quad slightly beyond the projected radius to avoid clipping the analytic sphere edge.

The fragment stage:

1. constructs a view-space ray through the billboard fragment;
2. solves the ray–sphere quadratic;
3. discards fragments outside the sphere;
4. evaluates the view-space normal and lighting at the nearest hit; and
5. projects the analytic hit point and writes corrected fragment depth.

The file also contains a placeholder fragment entry point. The Rust pipeline uses the implementation entry point. Confirm the selected entry point in `renderer/pipeline.rs` before you rename or delete either function.

The renderer separates opaque and partially occupied atom instances before drawing. Changes to alpha handling must preserve the opaque/transparent pass contract and source-index mapping used for picking.

---

## Bond cylinder shader

**File**: `src-tauri/shaders/bond_cylinder.wgsl`

**Entry points**: `vs_main`, `fs_main`

**Bindings**: group 0 binding 0 camera uniform

Each bond instance supplies two Cartesian endpoints, a radius, and color. The vertex stage generates 12 radial faces, six vertices per face, for 72 vertices per instance. It constructs an orthonormal frame around the bond axis and expands the cylinder directly; there is no geometry shader and no stored cylinder mesh.

The current shader draws the cylinder side surface and applies ambient, diffuse, and specular terms in the fragment stage. A zero-length segment would make the axis normalization invalid, so CPU scene preparation must continue to reject or omit degenerate bonds.

Do not raise radial resolution or add caps only for publication output until you measure the interactive cost. An approved publication-rendering design can use a separate export path.

---

## Line shader

**File**: `src-tauri/src/renderer/shaders/line.wgsl`

**Entry points**: `vs_main`, `fs_main`

**Bindings**: group 0 binding 0 camera uniform

This is an unlit position/color pass. It is shared by unit-cell edges and renderer overlay lines, so changing its color or alpha behavior can affect several unrelated features. Geometry is prepared on the CPU as `LineVertex` pairs; the shader does not infer periodicity or measurement meaning.

---

## Marching Cubes compute shader

**File**: `src-tauri/shaders/marching_cubes.wgsl`

**Entry point**: compute `main`

**Workgroup size**: 4×4×4

| Binding | Access | Resource |
|---:|---|---|
| 0 | uniform | grid, lattice/origin, threshold, sign mode, capacity parameters |
| 1 | storage read | x-fastest scalar field |
| 2 | storage read | edge lookup table |
| 3 | storage read | triangle lookup table |
| 4 | storage read/write | bounded `IsoVertex` output |
| 5 | storage read/write | atomic vertex counter |

The compute stage returns early for invocation IDs outside the valid grid-cell range. It supports positive, negative, and both-sign classification. Edge crossings use linear interpolation; generated positions are transformed from grid fractions through the field lattice, while the sign flag is carried into the render stage.

The output counter may count attempted vertices, but writes must remain guarded by the allocated capacity. Rust calculates dispatch dimensions from the 4×4×4 declaration and resets/reads the counter around each compute pass. If the workgroup size changes, update Rust dispatch arithmetic and the GPU test together.

`renderer/isosurface.rs` contains a CPU reference/fallback using the same lookup tables. Behavior changes should be compared against that path where the two implementations are intended to agree.

---

## Isosurface render shader

**File**: `src-tauri/shaders/isosurface_render.wgsl`

**Entry points**: `vs_main`, `fs_main`

| Group/binding | Resource |
|---|---|
| 0/0 | camera uniform |
| 1/0 | isosurface material parameters |

The vertex stage consumes `IsoVertex` position, normal, and sign flag. The fragment stage selects the configured positive or negative material color and applies the current lighting model while preserving material alpha. Sign selection is a presentation decision; it must not modify the scalar field or reinterpret its units.

The render pipeline consumes the output buffer written by Marching Cubes. Any change to vertex stride, normal orientation, sign semantics, topology, culling, or blend mode therefore requires coordinated compute, Rust, and render-stage tests.

---

## Volume raycast shader

**File**: `src-tauri/shaders/volume_raycast.wgsl`

**Entry points**: `vs_main`, `fs_main`

| Group/binding | Access | Resource |
|---|---|---|
| 0/0 | uniform | camera matrices |
| 1/0 | uniform | volume/grid/lattice, range, opacity, mode, and clipping parameters |
| 1/1 | storage read | scalar field |
| 1/2 | sampled depth | opaque-scene depth texture |

The vertex stage emits the transformed fractional unit cube. The fragment stage reconstructs a camera ray, intersects it with the cube, restricts the interval using opaque depth, samples the scalar field trilinearly, applies the selected colormap/transfer function, and accumulates premultiplied color front-to-back.

Important coupling points:

- the uniform layout has explicit offsets mirrored by Rust buffer writes;
- grid flattening is x-fastest in both Rust and WGSL;
- the inverse lattice maps world positions back into fractional volume coordinates;
- render mode and sign filtering share fields with the isosurface presentation controls; and
- the pipeline blend state in `renderer/pipeline.rs` must agree with premultiplied shader output.

Reject non-finite or unusable scalar ranges before they reach this path. A shader fallback for invalid metadata would hide an importer or IPC error and produce irreproducible figures.

---

## Brillouin-zone blit shader

**File**: `src-tauri/src/renderer/shaders/bz_blit.wgsl`

**Entry points**: `vs_main`, `fs_main`

| Binding | Resource |
|---:|---|
| 0 | offscreen BZ texture |
| 1 | sampler |

The vertex stage creates a six-vertex screen quad from `vertex_index`; no vertex buffer is required. The fragment stage samples the offscreen BZ texture and applies the overlay's pad/border composition. The shader displays already prepared BZ geometry—it does not construct reciprocal-space vertices or labels.

---

## Adding or changing a shader

Follow one bounded node:

1. Identify the Rust pipeline owner, render pass order, target formats, depth state, blend state, and all consumers of the affected buffer.
2. Have Breaker add a focused failing gate for the behavior or layout. Visual changes also need a declared desktop scene and acceptance criteria.
3. Update WGSL and the necessary Rust layout/pipeline code together. Do not change scientific state, IPC, or unrelated panels as collateral work.
4. Run formatting/static checks, focused Rust renderer tests, the GPU test where available, and a desktop smoke test on a representative adapter.
5. Run the repository gates from [TestingGuide.md](TestingGuide.md) and let Auditor check buffer compatibility, bounds, non-finite inputs, device-limit assumptions, and regressions in other passes.

For a new shader, document:

- stage and exact entry-point names;
- bind groups, binding types, visibility, and minimum sizes;
- input/output structs with byte offsets and strides;
- texture formats, sampling rules, depth and blend behavior;
- buffer capacity and dispatch/draw bounds;
- renderer ownership and cleanup lifecycle; and
- the focused automated gate plus desktop verification scene.

Run `cargo check` and `pnpm run build`, but also execute the relevant wgpu pipeline. A GPU-dependent test may skip when no adapter is available. Record the skip and complete the desktop check before you mark the rendering Node complete.

---

## Interactive versus publication rendering

The interactive renderer prioritizes responsive structure inspection. Higher-fidelity lighting, anti-aliasing, soft shadows, SSAO, tiled high-resolution export, and publication color management remain legitimate visualization goals, but they must be designed as measured, separable rendering work. Do not quietly add their cost to every interactive frame or couple them to physical-state transactions.

See [Algorithms.md](Algorithms.md), [DeveloperGuide.md](DeveloperGuide.md), and [TestingGuide.md](TestingGuide.md) for algorithm conventions, ownership, and verification rules.
