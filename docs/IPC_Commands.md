# CrystalCanvas IPC Contract Reference

> Baseline: `v0.6.1` | Updated: 2026-07-19

This document describes the reviewed Rust/TypeScript IPC boundary. The machine-checked sources of truth are [ipc/inventory.json](../ipc/inventory.json), [src/ipc/commands.generated.ts](../src/ipc/commands.generated.ts), and [src/ipc/contracts.ts](../src/ipc/contracts.ts). Run `npm run ipc:inventory` and `npm run check:ipc` after changing a command, event, or wire type.

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

`preview_supercell` accepts a flat 3×3 `expansion` matrix and returns a non-committed `CrystalState` preview. `apply_supercell` accepts a nested 3×3 `matrix` and commits through the transaction path. Do not substitute one shape for the other.

`set_phonon_phase` is the existing high-frequency animation path. Its per-frame IPC behavior is intentionally retained until the dedicated interaction-animation work replaces it; it is not a structural commit and must not create undo entries or versions.

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

Listeners must release their `unlisten` callback on React unmount. Development failures to register a listener must remain visible; do not silently discard them.

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

Panels may update their local presentation state after a successful, non-structural renderer command. They must not optimistically update `CrystalState` projections before a transactional command succeeds, and they must not create a second full-state listener or refetch loop.

---

## Maintaining the contract

1. Add or change the Rust command with snake_case parameters.
2. Update the TypeScript contract and use camelCase arguments.
3. Run `npm run ipc:inventory`, then `npm run check:ipc` and `npm run test:ipc`.
4. Update this reference only after the checked inventory is clean.

The Assistant commands remain documented because they are still registered, but they are legacy experimental surfaces rather than a roadmap area.
