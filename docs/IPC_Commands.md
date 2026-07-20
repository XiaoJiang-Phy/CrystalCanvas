# CrystalCanvas IPC Contract Reference

> Baseline: `v0.6.1` | Updated: 2026-07-20

This document describes the reviewed Rust/TypeScript IPC boundary. The machine-checked sources of truth are [ipc/inventory.json](../ipc/inventory.json), [src/ipc/commands.generated.ts](../src/ipc/commands.generated.ts), and [src/ipc/contracts.ts](../src/ipc/contracts.ts). After you change a command, event, or wire type, run `npm run ipc:inventory` and `npm run check:ipc`.

---

## Contract rules

- TypeScript sends **camelCase** arguments; Rust Tauri handlers receive **snake_case** arguments.
- Frontend code calls commands through `safeInvoke` and events through `safeListen`. These validate results and normalize native failures into `IpcException`.
- Browser/mock mutations return `not_in_tauri`; they never claim to have changed a structure.
- `CrystalState` remains Rust-owned. A successful structural mutation emits `state_changed { version }`; `App.tsx` is the sole complete-snapshot owner.
- Transactional structural commands follow `CrystalState → UndoStack → AppSettings → Renderer`. Parsing and expensive preparation do not retain the renderer lock.

All command results are either the typed result below or a structured error:

```ts
type IpcError = {
  code: 'invalid_argument' | 'io_error' | 'lock_poisoned' | 'not_in_tauri'
      | 'state_busy' | 'parse_error' | 'render_error' | 'internal_error';
  message: string;
  recoverable: boolean;
};
```

---

## Command inventory

The following names are registered in the current inventory. Exact argument and result shapes are exported as `IpcCommandContract` in `src/ipc/contracts.ts`.

| Domain | Commands |
|---|---|
| State, settings, and file output | `get_crystal_state`, `get_settings`, `update_settings`, `export_file`, `write_text_file`, `export_image` |
| Camera and viewport | `update_viewport_size`, `set_camera_projection`, `rotate_camera`, `zoom_camera`, `pan_camera`, `reset_camera`, `set_camera_view_axis`, `pick_atom`, `set_render_flags` |
| Structural editing | `load_cif_file`, `add_atom`, `delete_atoms`, `translate_atoms_screen`, `substitute_atoms`, `update_lattice_params`, `update_selection`, `restore_unitcell`, `undo`, `redo` |
| Geometry | `preview_supercell`, `apply_supercell`, `preview_slab`, `apply_slab`, `shift_termination`, `apply_niggli_reduce`, `apply_cell_standardize` |
| Analysis and measurement | `get_bond_analysis`, `add_measurement`, `get_measurements`, `get_measurement_labels_screen`, `clear_measurements` |
| Phonon | `load_phonon`, `load_phonon_interactive`, `load_axsf_phonon`, `set_phonon_mode`, `set_phonon_phase` |
| Volumetric | `load_volumetric_file`, `get_volumetric_info`, `set_isovalue`, `set_isosurface_color`, `set_isosurface_opacity`, `set_isosurface_sign_mode`, `set_volume_render_mode`, `set_volume_opacity_range`, `set_volume_density_cutoff`, `set_volume_colormap` |
| Reciprocal space | `compute_brillouin_zone`, `toggle_bz_display`, `get_kpath_info`, `set_bz_scale`, `generate_kpath_text`, `get_bz_label_positions` |
| Wannier | `load_wannier_hr`, `set_wannier_t_min`, `set_wannier_r_shell`, `set_wannier_orbital`, `toggle_wannier_onsite`, `toggle_hopping_display`, `clear_wannier` |
| Experimental Assistant | `check_api_key_status`, `llm_configure`, `llm_chat`, `llm_execute_command` |

---

## Command details

The argument column below is the frontend TypeScript wire shape. `—` means no caller arguments. Successful `null` results correspond to Rust `IpcResult<()>`.

### State, settings, and output

| Command | Arguments | Result | Notes |
|---|---|---|---|
| `get_crystal_state` | — | `CrystalState` | complete serialized projection; heavy renderer attachments are not serialized |
| `get_settings` | — | `AppSettingsDto` | atom/bond presentation settings |
| `update_settings` | `{ newSettings }` | `null` | validates the complete settings DTO before renderer update |
| `export_file` | `{ format, path }` | `null` | `POSCAR`, `VASP`, `LAMMPS`, or `QE` |
| `write_text_file` | `{ path, content }` | `null` | used for reviewed text exports such as generated k paths |
| `export_image` | `{ path, width, height, bgMode }` | `null` | background: `transparent`, `white`, `black`, or `default` |

### Camera and viewport

| Command | Arguments | Result | Notes |
|---|---|---|---|
| `update_viewport_size` | `{ width, height }` | `null` | physical viewport dimensions |
| `set_camera_projection` | `{ isPerspective }` | `null` | emits projection synchronization event |
| `rotate_camera` | `{ dx, dy }` | `null` | renderer-only camera update |
| `zoom_camera` | `{ delta }` | `null` | renderer-only camera update |
| `pan_camera` | `{ dx, dy }` | `null` | renderer-only camera update |
| `reset_camera` | — | `null` | resets active camera state |
| `set_camera_view_axis` | `{ axis }` | `null` | `a`, `b`, `c`, `a_star`, `b_star`, `c_star`, or `reset` |
| `pick_atom` | `{ x, y, screenW, screenH }` | `number \| null` | returns the intrinsic source index of the nearest prepared-scene hit |
| `set_render_flags` | `{ showCell, showBonds }` | `null` | presentation flags only |

### Structural editing and history

| Command | Arguments | Result | Notes |
|---|---|---|---|
| `load_cif_file` | `{ path }` | `null` | unified structure-loader entry despite the historical name |
| `add_atom` | `{ elementSymbol, atomicNumber, fractPos }` | `null` | fractional position triplet; committed structural mutation |
| `delete_atoms` | `{ indices }` | `null` | intrinsic indices only |
| `translate_atoms_screen` | `{ indices, dx, dy }` | `null` | existing committed translation command; drag-session replacement is future work |
| `substitute_atoms` | `{ indices, newElementSymbol, newAtomicNumber }` | `null` | committed structural mutation |
| `update_lattice_params` | `{ a, b, c, alpha, beta, gamma }` | `null` | finite validated lattice parameters |
| `update_selection` | `{ indices }` | `null` | selection/presentation update |
| `restore_unitcell` | — | `null` | restores the accepted base structure |
| `undo` | — | `null` | restores one versioned structural snapshot |
| `redo` | — | `null` | reapplies one versioned structural snapshot |

### Geometry

| Command | Arguments | Result | Notes |
|---|---|---|---|
| `preview_supercell` | `{ expansion: [n0, …, n8] }` | `CrystalState` | checked flat nine-integer contract; no commit |
| `apply_supercell` | `{ matrix: [[...], [...], [...]] }` | `null` | nested 3×3 integer contract; command boundary adapts it for the kernel; atomic commit |
| `preview_slab` | `{ miller, layers, vacuumA }` | `CrystalState` | no version or undo entry |
| `apply_slab` | `{ miller, layers, vacuumA }` | `null` | validated atomic commit |
| `shift_termination` | `{ targetLayerIdx, layerToleranceA? }` | `number` | returns the selected/available termination result defined by the backend |
| `apply_niggli_reduce` | — | `null` | committed cell transform |
| `apply_cell_standardize` | `{ toPrimitive }` | `null` | `true` for primitive, `false` for conventional |

### Analysis and measurements

| Command | Arguments | Result | Notes |
|---|---|---|---|
| `get_bond_analysis` | `{ thresholdFactor? }` | `BondAnalysisResult` | read/analysis result; does not invent structure state in React |
| `add_measurement` | `{ indices }` | `MeasurementOverlay` | 2, 3, or 4 intrinsic indices select distance, angle, or dihedral |
| `get_measurements` | — | `MeasurementOverlay[]` | versioned measurement projections |
| `get_measurement_labels_screen` | `{ width, height }` | `ScreenLabel[]` | renderer projection of labels |
| `clear_measurements` | — | `null` | clears committed measurement overlays |

### Phonon

| Command | Arguments | Result | Notes |
|---|---|---|---|
| `load_phonon` | `{ path }` | `PhononModeSummary[]` | supported single-source loader |
| `load_phonon_interactive` | `{ scfIn, scfOut, modes }` | `PhononModeSummary[]` | coordinated Quantum ESPRESSO-style sources |
| `load_axsf_phonon` | `{ path }` | `PhononModeSummary[]` | structure/mode import from AXSF |
| `set_phonon_mode` | `{ modeIndex? }` | `null` | `null` clears active mode |
| `set_phonon_phase` | `{ phase, amplitude? }` | `null` | renderer presentation; no version or undo record |

### Volumetric rendering

| Command | Arguments | Result | Notes |
|---|---|---|---|
| `load_volumetric_file` | `{ path }` | `VolumetricInfo` | parses and prepares the grid before commit |
| `get_volumetric_info` | — | `VolumetricInfo \| null` | explicit absence is `null` |
| `set_isovalue` | `{ value }` | `null` | finite range-dependent renderer update |
| `set_isosurface_color` | `{ color: [r, g, b, a] }` | `null` | normalized finite RGBA tuple |
| `set_isosurface_opacity` | `{ opacity }` | `null` | renderer update |
| `set_isosurface_sign_mode` | `{ mode }` | `null` | `positive`, `negative`, or `both` |
| `set_volume_render_mode` | `{ mode }` | `null` | `isosurface`, `volume`, or `both` |
| `set_volume_opacity_range` | `{ min, max, opacityScale }` | `null` | finite ordered range |
| `set_volume_density_cutoff` | `{ cutoff }` | `null` | finite renderer parameter |
| `set_volume_colormap` | `{ mode }` | `null` | one of the validated `VolumeColormap` values |

### Reciprocal space

| Command | Arguments | Result | Notes |
|---|---|---|---|
| `compute_brillouin_zone` | — | `BzInfo` | computes and prepares the BZ overlay |
| `toggle_bz_display` | `{ show }` | `null` | renderer visibility |
| `get_kpath_info` | — | `KPathInfo` | labeled fractional k points and segments |
| `set_bz_scale` | `{ scale }` | `null` | renderer-only overlay scale |
| `generate_kpath_text` | `{ npoints }` | `{ qe, vasp }` | returns text; writing is a separate command |
| `get_bz_label_positions` | `{ width, height }` | `ScreenLabel[]` | screen-projected labels |

### Wannier hopping overlay

| Command | Arguments | Result | Notes |
|---|---|---|---|
| `load_wannier_hr` | `{ path }` | `WannierInfo` | validates the model against the current intrinsic structure |
| `set_wannier_t_min` | `{ tMin }` | `null` | finite magnitude threshold |
| `set_wannier_r_shell` | `{ shellIdx, active }` | `null` | toggles one available lattice shell |
| `set_wannier_orbital` | `{ orbIdx, active }` | `null` | toggles one available orbital |
| `toggle_wannier_onsite` | `{ show }` | `null` | on-site visibility |
| `toggle_hopping_display` | `{ show }` | `null` | master overlay visibility |
| `clear_wannier` | — | `null` | removes overlay buffers and renderer ghosts |

### Experimental Assistant

| Command | Arguments | Result | Notes |
|---|---|---|---|
| `check_api_key_status` | `{ providerType }` | `boolean` | legacy experimental configuration query |
| `llm_configure` | `{ providerType, apiKey, model }` | `null` | configures the current experimental provider |
| `llm_chat` | `{ userMessage, selectedIndices? }` | `string` | returns the provider response for user review |
| `llm_execute_command` | `{ commandJson }` | `null` | still passes schema, validation, approval, and transaction boundaries |

---

## Representative call shapes

These examples intentionally show frontend camelCase names.

```ts
await safeInvoke('add_atom', {
  elementSymbol: 'Si',
  atomicNumber: 14,
  fractPos: [0.0, 0.0, 0.0],
});

await safeInvoke('apply_slab', {
  miller: [1, 1, 0],
  layers: 4,
  vacuumA: 15.0,
});

await safeInvoke('update_settings', { newSettings });

await safeInvoke('set_volume_opacity_range', {
  min: 0.1,
  max: 0.8,
  opacityScale: 1.0,
});

const info = await safeInvoke('load_wannier_hr', { path });
const measurement = await safeInvoke('add_measurement', { indices: [0, 1] });
```

`preview_supercell` accepts the contract's flat nine-value `expansion` and returns a non-committed `CrystalState` preview. Its low-level consumer is column-major. `apply_supercell` accepts a nested 3×3 `matrix`, adapts it at the command boundary, and commits through the transaction path. Do not substitute one public shape for the other or transpose values speculatively in a caller.

`set_phonon_phase` is the existing high-frequency animation path. Its per-frame IPC behavior is intentionally retained until the dedicated interaction-animation work replaces it; it is not a structural commit and must not create undo entries or versions.

### Structured error handling

```ts
try {
  await safeInvoke('apply_supercell', { matrix });
} catch (cause) {
  const error = cause instanceof IpcException
    ? cause
    : normalize_ipc_error(cause);
  setPanelError(error);
}
```

Do not set success-dependent local state before the awaited call resolves. If a command fails, retain the previous displayed value unless the backend event supplies an authoritative replacement.

---

## Events

| Event | Payload | Owner and purpose |
|---|---|---|
| `state_changed` | `{ version: number }` | `App.tsx` schedules the only complete `get_crystal_state` refresh |
| `undo_stack_changed` | `{ can_undo: boolean; can_redo: boolean }` | native menu and UI undo/redo availability |
| `volumetric_loaded` | `VolumetricInfo` | Volumetric panel refreshes data-specific controls |
| `view_projection_changed` | `{ is_perspective: boolean }` | projection UI synchronization |
| `menu-action` | `string` | native menu routing |
| `tauri://drag-drop`, `tauri://drag-enter` | `{ paths, position }` | Tauri v2 file-drop lifecycle |
| `tauri://file-drop`, `tauri://file-drop-hover` | `{ paths }` | supported legacy drop compatibility path |
| `tauri://drag-leave`, `tauri://file-drop-cancelled` | `null` or `undefined` | drop cleanup |

Release each listener's `unlisten` callback when React unmounts the owner. Keep listener-registration failures visible during development. Do not discard them silently.

---

## Snapshot pattern

```ts
const unlisten = await safeListen('state_changed', ({ payload }) => {
  const { version } = payload;
  // App.tsx de-duplicates versions and owns the complete refresh.
  requestSnapshotForVersion(version);
});

return () => unlisten();
```

Panels may update local presentation state after a successful non-structural renderer command. Do not update a `CrystalState` projection before a transactional command succeeds. Do not create a second full-state listener or refetch loop.

### Event ownership

| Event | Current frontend owner |
|---|---|
| `state_changed` | `src/App.tsx` |
| native menu and projection events | `src/hooks/useTauriMenu.ts` |
| Tauri v1/v2 file-drop compatibility events | `src/hooks/useFileDrop.ts` |
| `volumetric_loaded` | `src/components/panels/VolumetricPanel.tsx` |

Before adding a listener, search the repository and prove that the event does not already have an owner. Mount/unmount tests must show that every successful registration has exactly one cleanup.

---

## Maintaining the contract

1. Add or change the Rust command with snake_case parameters.
2. Update the TypeScript contract and use camelCase arguments.
3. Run `npm run ipc:inventory`, then `npm run check:ipc` and `npm run test:ipc`.
4. Update this reference only after the checked inventory is clean.

The Assistant commands remain documented because they are still registered, but they are legacy experimental surfaces rather than a roadmap area.
