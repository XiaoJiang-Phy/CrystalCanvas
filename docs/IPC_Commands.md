# CrystalCanvas IPC Command Reference

> Version: v0.6.0 | Updated: 2026-04-14

Complete reference for all `#[tauri::command]` endpoints in `commands.rs`. Every command listed here is registered in `main.rs` via `tauri::generate_handler![]` and callable from the React/TypeScript frontend via `invoke()`.

---

## Conventions

- **Argument types** are Rust types. In TypeScript, `f32`/`f64` → `number`, `String` → `string`, `Vec<T>` → `T[]`, `Option<T>` → `T | undefined`.
- **State parameters** (e.g., `State<'_, Mutex<CrystalState>>`) are injected by Tauri and **not** passed by the caller.
- **Lock ordering**: `crystal_state` → `settings` → `renderer`. All commands follow this ordering to prevent deadlocks.
- **`state_changed` event**: Emitted after any mutation to `CrystalState`. The frontend listens for this event to trigger `get_crystal_state()` and re-render UI panels.

---

## 1. Core State & Settings

### `get_crystal_state`

Fetch the complete backend state tree. Called on startup and after every `state_changed` event.

```
invoke("get_crystal_state") → CrystalState
```

- **Arguments**: None (state injected by Tauri)
- **Returns**: Full `CrystalState` clone (cell params, atoms, labels, positions, spacegroup, version counter)
- **Side effects**: None (read-only)
- **Note**: Heavy fields (`volumetric_data`, `bz_cache`, `wannier_overlay`) are `#[serde(skip)]` and will not appear in the serialized response.

---

### `get_settings`

```
invoke("get_settings") → AppSettings
```

- **Returns**: `{ atom_scale: f32, bond_tolerance: f32, bond_radius: f32, bond_color: [f32; 4], custom_atom_colors: HashMap<String, [f32; 4]> }`
- **Side effects**: None

---

### `update_settings`

```
invoke("update_settings", { new_settings: AppSettings }) → ()
```

- **Arguments**: `new_settings: AppSettings` — complete settings object (not a partial diff)
- **Side effects**: Replaces settings state, rebuilds atom + bond instance buffers, persists to disk
- **Lock order**: `crystal_state` → `renderer`

---

### `export_file`

```
invoke("export_file", { format: string, path: string }) → ()
```

- **Arguments**:
  - `format`: `"POSCAR"` | `"VASP"` | `"LAMMPS"` | `"QE"` (case-insensitive)
  - `path`: absolute filesystem path
- **Side effects**: File I/O only, no state mutation

---

### `write_text_file`

```
invoke("write_text_file", { path: string, content: string }) → ()
```

- **Arguments**: Arbitrary path + text content
- **Side effects**: File I/O (used by k-path export to write QE/VASP k-point files)

---

### `export_image`

```
invoke("export_image", { path: string, width: u32, height: u32, bg_mode: string }) → ()
```

- **Arguments**:
  - `path`: output file path (`.png` or `.jpg`/`.jpeg`)
  - `width`, `height`: output resolution in pixels
  - `bg_mode`: `"dark"` | `"light"` | `"transparent"` (PNG preserves alpha; JPEG composites onto white)
- **Side effects**: Offscreen render → file I/O

---

## 2. 3D Viewport & Camera

### `update_viewport_size`

```
invoke("update_viewport_size", { width: u32, height: u32 }) → ()
```

- **Arguments**: Physical pixel dimensions from `ResizeObserver`
- **Side effects**: Reconfigures wgpu surface, reallocates depth/color textures, updates camera aspect ratio

---

### `set_camera_projection`

```
invoke("set_camera_projection", { is_perspective: bool }) → ()
```

- **Side effects**: Switches camera mode. Orthographic scale auto-computed from lattice extent. Emits `view_projection_changed` event.

---

### `rotate_camera`

```
invoke("rotate_camera", { dx: f32, dy: f32 }) → ()
```

- **Arguments**: Mouse delta in pixels (from `mousemove`)
- **Behavior**: Orbital rotation around target. **Disabled when BZ viewport is active** (labels require a fixed camera).

---

### `zoom_camera`

```
invoke("zoom_camera", { delta: f32 }) → ()
```

- **Arguments**: Scroll wheel delta
- **Behavior**: When BZ is active, zooms the BZ sub-viewport camera instead of the main camera.

---

### `pan_camera`

```
invoke("pan_camera", { dx: f32, dy: f32 }) → ()
```

- **Arguments**: Right-click drag delta in pixels
- **Behavior**: When BZ is active, pans the BZ sub-viewport.

---

### `reset_camera`

```
invoke("reset_camera") → ()
```

- **Side effects**: Snaps camera to `[center + (0, 0, extent * 2.5)]` looking at cell center. Adjusts orthographic scale.

---

### `set_camera_view_axis`

```
invoke("set_camera_view_axis", { axis: string }) → ()
```

- **Arguments**: `"a"` | `"b"` | `"c"` | `"a_star"` | `"b_star"` | `"c_star"` | `"reset"`
- **Behavior**: Computes real-space lattice vectors from cell parameters, places camera along the specified direction at distance `extent * 2.5`. Reciprocal axes ($\mathbf{a}^* = \mathbf{b} \times \mathbf{c}$, etc.) are computed via cross products.

---

### `pick_atom`

```
invoke("pick_atom", { x: f32, y: f32, screen_w: f32, screen_h: f32 }) → Option<usize>
```

- **Arguments**: Pointer position + viewport size in CSS pixels
- **Returns**: Atom index of the nearest hit, or `null` if no atom under cursor
- **Algorithm**: Unprojects cursor to NDC → ray origin/direction (handles both perspective and orthographic) → ray-sphere intersection against all `cart_positions` with hit radius 1.5 Å
- **Side effects**: None (read-only)

---

### `set_render_flags`

```
invoke("set_render_flags", { show_cell: bool, show_bonds: bool }) → ()
```

- **Side effects**: Toggles renderer booleans. No geometry rebuild — the render loop reads these flags per-frame.

---

## 3. Structural Editing

All commands in this section follow the standard mutation pattern:
1. Lock `crystal_state` → mutate → increment `version`
2. Lock `settings` → build instance data
3. Lock `renderer` → upload buffers → update lines/bonds/camera
4. Emit `state_changed`

### `load_cif_file`

```
invoke("load_cif_file", { path: string }) → ()
```

- **Behavior**: Dispatches by file extension to the appropriate parser (CIF/PDB via Gemmi FFI, POSCAR, QE, XYZ, CHGCAR, Cube, XSF). Replaces entire `CrystalState`. Saves a clone to `BaseCrystalState` for restore. If file contains volumetric data, uploads to GPU (wrapped in `catch_unwind` for OOM safety). Auto-centers camera.
- **Emits**: `state_changed`, optionally `volumetric_loaded` (payload: `VolumetricInfo`)

---

### `add_atom`

```
invoke("add_atom", {
  element_symbol: string,
  atomic_number: u8,
  fract_pos: [f64, f64, f64]
}) → ()
```

- **Behavior**: Auto-formats element symbol (e.g., `"na"` → `"Na"`). If `atomic_number == 0`, resolves from symbol. Calls `cs.try_add_atom()` which performs MIC collision detection (fails if within 0.5 Å of existing atoms).

---

### `delete_atoms`

```
invoke("delete_atoms", { indices: number[] }) → ()
```

- **Behavior**: Sorts indices high-to-low internally before removal to prevent index-shifting bugs.

---

### `translate_atoms_screen`

```
invoke("translate_atoms_screen", { indices: number[], dx: f32, dy: f32 }) → ()
```

- **Behavior**: Reads the current camera view plane, converts screen delta to a world-space translation vector (using `right × up` decomposition), applies Cartesian shift to selected atoms. Pan speed: `0.001 * eye-target distance`.

---

### `substitute_atoms`

```
invoke("substitute_atoms", {
  indices: number[],
  new_element_symbol: string,
  new_atomic_number: u8
}) → ()
```

- **Behavior**: Replaces element and atomic number for specified atoms. Auto-resolves `atomic_number` from symbol if zero.

---

### `update_lattice_params`

```
invoke("update_lattice_params", {
  a: f64, b: f64, c: f64,
  alpha: f64, beta: f64, gamma: f64
}) → ()
```

- **Behavior**: Updates cell parameters, recomputes Cartesian positions via `fractional_to_cartesian()`, re-detects space group via Spglib.

---

### `update_selection`

```
invoke("update_selection", { indices: number[] }) → ()
```

- **Behavior**: Sets `cs.selected_atoms`, rebuilds atom instances (selected atoms get highlight color) and bond instances (selected bonds get contrast color).

---

### `restore_unitcell`

```
invoke("restore_unitcell") → ()
```

- **Behavior**: Restores crystal state from `BaseCrystalState` (saved at last file load). Resets phonon animation state. Fails if no base state exists.

---

## 4. Symmetry & Advanced Transforms

### `preview_supercell`

```
invoke("preview_supercell", { expansion: [i32; 9] }) → CrystalState
```

- **Arguments**: Flattened 3x3 expansion matrix in row-major order
- **Returns**: A full `CrystalState` preview (not committed to state)
- **Side effects**: None (read-only)

---

### `apply_supercell`

```
invoke("apply_supercell", { matrix: [[i32; 3]; 3] }) → ()
```

- **Arguments**: 3x3 expansion matrix as nested array `[[a_a, a_b, a_c], [b_a, b_b, b_c], [c_a, c_b, c_c]]`
- **Behavior**: Internally flattens to `[i32; 9]`, calls `cs.generate_supercell()`, replaces state, re-detects spacegroup, auto-adjusts camera.

---

### `preview_slab`

```
invoke("preview_slab", { miller: [i32; 3], layers: i32, vacuum_a: f64 }) → CrystalState
```

- **Returns**: Slab preview without modifying state

---

### `apply_slab`

```
invoke("apply_slab", { miller: [i32; 3], layers: i32, vacuum_a: f64 }) → ()
```

- **Behavior**: Diophantine surface basis → c-axis orthogonalization → vacuum insertion. Guarantees $\alpha = \beta = 90°$.

---

### `shift_termination`

```
invoke("shift_termination", {
  target_layer_idx: i32,
  layer_tolerance_a: Option<f64>  // default: 0.3 Å
}) → i32
```

- **Returns**: Total number of detected layers (integer)
- **Behavior**: Shifts the slab termination to expose a different surface layer.

---

### `apply_niggli_reduce`

```
invoke("apply_niggli_reduce") → ()
```

- **Behavior**: Delegates to Spglib for Niggli cell reduction. Updates state, camera, and renderer.

---

### `apply_cell_standardize`

```
invoke("apply_cell_standardize", { to_primitive: bool }) → ()
```

- **Behavior**: `to_primitive: true` → primitive cell; `false` → conventional cell (with F/I/C centerings).

---

## 5. Phonon Animation

### `load_phonon`

```
invoke("load_phonon", { path: string }) → PhononModeSummary[]
```

- **Returns**: Array of `{ frequency: f64, is_imaginary: bool, label: String }` for UI dropdown
- **Precondition**: A crystal structure must be loaded first. Warns on atom count mismatch.
- **Supported formats**: Molden `.mold`, QE `dynmat.dat` / `modes`

---

### `load_phonon_interactive`

```
invoke("load_phonon_interactive", {
  scf_in: string,
  scf_out: string,
  modes: string
}) → PhononModeSummary[]
```

- **Behavior**: Loads crystal from `scf_in` (QE input), phonon from `modes`. Replaces current state entirely. `scf_out` is accepted but currently unused.

---

### `load_axsf_phonon`

```
invoke("load_axsf_phonon", { path: string }) → PhononModeSummary[]
```

- **Behavior**: Parses animated XSF format containing both structure and phonon eigenvectors.

---

### `set_phonon_mode`

```
invoke("set_phonon_mode", { mode_index: Option<usize> }) → ()
```

- **Behavior**: `Some(idx)` → activate mode, hide bonds (physics vis mode). `None` → deactivate animation.

---

### `set_phonon_phase`

```
invoke("set_phonon_phase", {
  phase: f64,             // radians [0, 2π]
  amplitude: Option<f64>  // default: 1.0
}) → ()
```

- **Behavior**: Computes displaced positions: $\mathbf{r}_i' = \mathbf{r}_i + A \cdot \mathbf{e}_i \cdot \sin(\phi)$. Rebuilds atom instances with displaced coordinates. Called ~60 times/sec during animation.
- **Side effects**: Updates renderer atoms only (no `state_changed` emit — high-frequency hot path)

---

## 6. Volumetric Pipeline

### `load_volumetric_file`

```
invoke("load_volumetric_file", { path: string }) → VolumetricInfo
```

- **Returns**: `{ grid_dims: [usize; 3], data_min: f32, data_max: f32, format: string }`
- **Behavior**: Parses file (dispatch by extension + filename heuristics for extensionless CHGCAR). Replaces structure + volumetric data. GPU upload wrapped in `catch_unwind` for OOM recovery. Auto-detects signed data ($\min < -0.01 \cdot |\max|$) and enables Coolwarm colormap + signed mapping.
- **Emits**: `state_changed`, `volumetric_loaded`

---

### `get_volumetric_info`

```
invoke("get_volumetric_info") → Option<VolumetricInfo>
```

- **Returns**: `null` if no volumetric data loaded

---

### `set_isovalue`

```
invoke("set_isovalue", { value: f32 }) → ()
```

- **Behavior**: Re-dispatches Marching Cubes compute shader at new threshold. Auto-syncs isosurface color with volume colormap via sqrt-stretched signed mapping: $t = 0.5 \pm 0.5\sqrt{|v/v_\max|}$. In `Both` mode, also syncs volume clip threshold and density cutoff.

---

### `set_isosurface_color`

```
invoke("set_isosurface_color", { color: [f32; 4] }) → ()
```

- **Arguments**: RGBA color (`[r, g, b, a]`, values in `[0, 1]`)

---

### `set_isosurface_opacity`

```
invoke("set_isosurface_opacity", { opacity: f32 }) → ()
```

---

### `set_isosurface_sign_mode`

```
invoke("set_isosurface_sign_mode", { mode: string }) → ()
```

- **Arguments**: `"positive"` (→ sign_mode 0) | `"negative"` (→ 1) | `"both"` (→ 2, dual-color)
- **Behavior**: Updates compute shader sign flag, re-dispatches Marching Cubes, syncs volume signed mapping.

---

### `set_volume_render_mode`

```
invoke("set_volume_render_mode", { mode: string }) → ()
```

- **Arguments**: `"isosurface"` | `"volume"` | `"both"`
- **Behavior**: In `Both` mode, syncs volume clip/density thresholds with current isovalue. In other modes, resets clip to 0.

---

### `set_volume_opacity_range`

```
invoke("set_volume_opacity_range", { min: f32, max: f32, opacity_scale: f32 }) → ()
```

- **Arguments**: `opacity_scale` clamped to `[0.01, 10.0]`

---

### `set_volume_density_cutoff`

```
invoke("set_volume_density_cutoff", { cutoff: f32 }) → ()
```

- **Arguments**: Non-negative density threshold (clamped to `>= 0.0`)

---

### `set_volume_colormap`

```
invoke("set_volume_colormap", { mode: string }) → ()
```

- **Arguments**: `"viridis"` (default, mode 0) | `"grayscale"` (1) | `"inferno"` (2) | `"plasma"` (3) | `"coolwarm"` (4) | `"hot"` (5) | `"magma"` (6) | `"cividis"` (7) | `"turbo"` (8) | `"rdylbu"` (9)
- **Behavior**: Updates volume raycast shader uniform. Re-syncs isosurface color with the new colormap.

---

## 7. Brillouin Zone & K-Path

### `compute_brillouin_zone`

```
invoke("compute_brillouin_zone") → BzInfoResponse
```

- **Returns**: `{ bravais_type: string, spacegroup: i32, vertices_count, edges_count, faces_count, is_2d: bool }`
- **Behavior**: Detects 2D/3D, constructs Wigner-Seitz cell, generates k-path, caches result in `cs.bz_cache`, uploads BZ wireframe to renderer.

---

### `toggle_bz_display`

```
invoke("toggle_bz_display", { show: bool }) → ()
```

- **Behavior**: `true` → activates BZ sub-viewport overlay (orthographic, rotation-locked). `false` → deactivates.

---

### `get_kpath_info`

```
invoke("get_kpath_info") → KPathInfoResponse
```

- **Returns**: `{ points: [{ label: string, coord_frac: [f64; 3] }], segments: string[][] }`
- **Precondition**: BZ must have been computed. Fails with `"Brillouin Zone not computed yet"` otherwise.

---

### `set_bz_scale`

```
invoke("set_bz_scale", { scale: f32 }) → ()
```

- **Arguments**: Clamped to `[0.15, 1.0]`

---

### `generate_kpath_text`

```
invoke("generate_kpath_text", { npoints: u32 }) → KPathTextResponse
```

- **Returns**: `{ qe: string, vasp: string }` — ready-to-save file contents
- **Behavior**: Generates uniformly-spaced k-point grids along the path. Segment density is proportional to Cartesian path length. For 2D materials, forces $k_z = 0$.

---

### `get_bz_label_positions`

```
invoke("get_bz_label_positions", { width: f32, height: f32 }) → BzLabelPos[]
```

- **Returns**: `[{ label: string, x: f32, y: f32 }]` — screen-space pixel coordinates for HTML label overlay
- **Behavior**: Reconstructs the BZ sub-viewport camera (must exactly match `BzSubViewport::update_bz`), projects k-points from reciprocal space → NDC → screen coordinates.

---

## 8. Wannier Tight-Binding

### `load_wannier_hr`

```
invoke("load_wannier_hr", { path: string }) → WannierInfo
```

- **Returns**: `{ num_wann: usize, r_shells: [i32; 3][], t_max: f64 }`
- **Precondition**: `num_atoms >= num_wann` (atom positions map to orbital centers)
- **Side effects**: Builds `WannierOverlay`, generates hopping + ghost atom instances, hides chemical bonds, shows hopping lines.

---

### `set_wannier_t_min`

```
invoke("set_wannier_t_min", { t_min: f64 }) → ()
```

- **Behavior**: Updates `overlay.t_min_threshold`, calls `filter_and_rebuild()`, regenerates hopping + ghost atom instances.

---

### `set_wannier_r_shell`

```
invoke("set_wannier_r_shell", { shell_idx: usize, active: bool }) → ()
```

- **Behavior**: Toggles specific R-shell (`overlay.active_r_shells[shell_idx]`), rebuilds visible hoppings.

---

### `set_wannier_orbital`

```
invoke("set_wannier_orbital", { orb_idx: usize, active: bool }) → ()
```

- **Behavior**: Toggles specific orbital (`overlay.active_orbitals[orb_idx]`), rebuilds visible hoppings.

---

### `toggle_wannier_onsite`

```
invoke("toggle_wannier_onsite", { show: bool }) → ()
```

- **Behavior**: Show/hide $\mathbf{R} = 0, m = n$ on-site terms.

---

### `toggle_hopping_display`

```
invoke("toggle_hopping_display", { show: bool }) → ()
```

- **Side effects**: Toggles `renderer.show_hoppings` flag only (no geometry rebuild).

---

### `clear_wannier`

```
invoke("clear_wannier") → ()
```

- **Behavior**: Purges `wannier_overlay`, clears hopping buffer, removes ghost atoms, restores chemical bonds.

---

## 9. LLM Command Bus

### `check_api_key_status`

```
invoke("check_api_key_status", { provider_type: string }) → bool
```

- **Arguments**: `"openai"` | `"deepseek"` | `"claude"` | `"gemini"` | `"ollama"`
- **Returns**: `true` if a key exists in OS Keychain or `.env`

---

### `llm_configure`

```
invoke("llm_configure", {
  provider_type: string,
  api_key: string,
  model: string
}) → ()
```

- **Arguments**:
  - `provider_type`: `"openai"` | `"deepseek"` | `"claude"` | `"gemini"` | `"ollama"`
  - `api_key`: Raw key string (masked `"********"` triggers Keychain fallback). For Ollama, ignored.
  - `model`: Model identifier (e.g., `"gpt-4o"`, `"deepseek-chat"`, `"llama3"`)
- **Behavior**: Resolves API key priority: provided string → OS Keychain → `.env` fallback. Stores `ProviderConfig` in managed state.

---

### `llm_chat`

```
invoke("llm_chat", {
  user_message: string,
  selected_indices: Option<number[]>
}) → string   // async
```

- **Behavior** (async): Builds crystal context from current state + selected atoms → constructs system prompt → sends to configured LLM provider → returns raw response string (typically JSON containing `CrystalCommand` payloads).

---

### `llm_execute_command`

```
invoke("llm_execute_command", { command_json: string }) → ()
```

- **Behavior**: Three-layer validation pipeline:
  1. **Schema parse**: `serde_json::from_str` → `CrystalCommand` enum
  2. **Physics sandbox**: `sandbox::validate_command()` (collision detection, parameter bounds)
  3. **Router execution**: `router::execute_command()` mutates `CrystalState`
- **Emits**: `state_changed`

---

## 10. Backend → Frontend Events

Events emitted by the Rust backend that the frontend should listen for:

| Event | Payload | Emitted By |
|---|---|---|
| `state_changed` | `()` | All mutating commands |
| `volumetric_loaded` | `VolumetricInfo` | `load_cif_file`, `load_volumetric_file` |
| `view_projection_changed` | `{ is_perspective: bool }` | `set_camera_projection`, menu handler |
| `menu-action` | `string` | Native menu event handler |

---

## TypeScript Usage Pattern

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Listen for state changes (register once at startup)
listen('state_changed', async () => {
  const state = await invoke<CrystalState>('get_crystal_state');
  updateUI(state);
});

// Mutation: Build supercell
async function buildSupercell(na: number, nb: number, nc: number) {
  await invoke('apply_supercell', {
    matrix: [[na, 0, 0], [0, nb, 0], [0, 0, nc]]
  });
  // state_changed listener will auto-update UI
}

// Read-only: Pick atom under cursor
async function onClick(e: MouseEvent) {
  const idx = await invoke<number | null>('pick_atom', {
    x: e.clientX, y: e.clientY,
    screenW: window.innerWidth, screenH: window.innerHeight
  });
  if (idx !== null) {
    await invoke('update_selection', { indices: [idx] });
  }
}
```

---

*CrystalCanvas v0.6.0 — Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors. Dual-licensed under MIT and Apache-2.0.*
