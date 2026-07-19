# CrystalCanvas Developer Guide

> Baseline: `v0.6.1` | Updated: 2026-07-19

CrystalCanvas is a visualization-first desktop application. It turns supplied crystal structures and three-dimensional scientific data into interactive real-space or reciprocal-space scenes and reproducible figures. It is not an electronic-structure, EPC, transport, superconductivity, TCI, or workflow solver.

For user-facing operation, see [UserManual.md](UserManual.md). For exact IPC wire shapes, see [IPC_Commands.md](IPC_Commands.md).

---

## Architecture

```text
React + TypeScript + TailwindCSS
    read-only snapshot projection and desktop workbench
                │ typed Tauri IPC
Rust + Tauri + wgpu/WGSL
    CrystalState, transactions, I/O, renderer, export
                │ thin exception-isolated FFI
C++ + Eigen + Spglib + Gemmi
    stateless crystallographic and geometry kernels
```

The important ownership boundaries are:

- Rust `CrystalState` is the only committed physical state.
- React state is a read-only snapshot projection, never a second physical-state store.
- `Renderer` owns GPU resources, boundary images, Wannier ghosts, and temporary previews. Those renderer-only objects never enter the intrinsic atom arrays.
- C++ kernels are stateless. C++ exceptions are converted to Rust errors before crossing the FFI boundary.

All lattice matrices crossing the Rust/C++ boundary are column-major: columns are the real-space basis vectors $\mathbf a$, $\mathbf b$, and $\mathbf c$. Physical and crystallographic calculations use `f64`; GPU presentation data use `f32`.

---

## Source layout

```text
src/
├── App.tsx                         # sole complete-snapshot owner
├── components/layout/              # shell, top chrome, left workspace, tool rail
├── components/panels/              # lazily loaded scientific panels
├── hooks/                          # viewport input, native menu, file-drop lifecycle
└── ipc/                            # generated names, contracts, browser mock

src-tauri/src/
├── commands/                       # Tauri command handlers by domain
├── crystal_state.rs                # intrinsic structure and serializable snapshot
├── transaction.rs                  # atomic mutations, scene commit, events
├── undo.rs                         # versioned undo/redo history
├── io/                             # public-format import and export
├── renderer/                       # wgpu renderer, picking, volume and BZ paths
├── volumetric.rs, phonon.rs,
│   wannier.rs, brillouin_zone.rs   # visualization data and presentation logic
└── ffi/                            # thin Rust/C++ bridge

cpp/
├── src/                            # stateless geometry and crystallography kernels
└── tests/                          # independent C++ regression tests

ipc/inventory.json                  # generated Rust/TypeScript command and event inventory
scripts/                            # IPC, state-refresh, and UI contract gates
```

---

## Reading the main data flows

### Startup and snapshot refresh

At startup, `App.tsx` registers the `state_changed` listener before requesting the initial complete snapshot. The listener payload contains a committed version. `App.tsx` ignores versions already applied or already pending, calls `get_crystal_state`, validates the returned DTO, and applies only a newer snapshot.

```text
Rust CrystalState
    └─ emit state_changed { version }
          └─ App.tsx de-duplicates version
                └─ safeInvoke(get_crystal_state)
                      └─ validated read-only React projection
```

Panels receive the projection through props. A panel may own local display state such as a selected render mode, an open modal, or an in-flight operation, but it must not reconstruct or independently refresh the complete physical state.

### Loading a structure

`commands/file_io.rs::load_cif_file` dispatches to the appropriate importer, validates the prepared state, builds renderer resources, and only then replaces committed state through the approved ownership path. `BaseCrystalState` stores the accepted load baseline used by **Restore Unit Cell**. A parsing or renderer-preparation failure leaves the previous structure active.

```text
path → importer/parser → prepared CrystalState → invariant validation
     → prepared renderer scene → atomic state/undo/version commit
     → state_changed { version } → App.tsx snapshot refresh
```

### Renderer-only controls

Camera movement, selection presentation, BZ scale, isovalue, opacity, colormap, and Wannier visibility do not all represent structural mutations. A renderer-only command may update GPU presentation or a non-structural attachment without allocating a structural undo record. Document that distinction when adding a command; do not emit `state_changed` merely to force a panel refresh.

---

## Committed state and transactions

`CrystalState` contains the intrinsic atoms, lattice, occupancy, selection, measurements, and versioned structural data. Its periodic visual duplicates are constructed only when preparing a renderer scene. Wannier ghost endpoints are likewise renderer data, not physical sites.

Structural changes use the transaction helpers in `transaction.rs` rather than independently changing version, undo history, or renderer buffers. A successful transaction:

1. performs preflight validation without holding the `Renderer` lock;
2. obtains committed-state ownership;
3. records undo history and changes the state exactly once;
4. prepares and commits the renderer scene;
5. emits `state_changed { version }` and the corresponding undo-stack state.

If validation, allocation, or renderer preparation fails, committed state, version, and undo history remain unchanged. The required lock order is:

```text
CrystalState → UndoStack → AppSettings → Renderer
```

Use `lock()` when a command must establish ownership or commit state. `try_lock()` is reserved for pointer-rate previews and other explicitly non-authoritative paths.

### Transaction helper selection

The helpers in `transaction.rs` encode different ownership needs:

| Helper | Use |
|---|---|
| `with_state_read` | authoritative read requiring the state lock |
| `with_state_read_try` | explicitly best-effort, pointer-rate read |
| `with_state_update` | validated mutation using the standard camera behavior |
| `with_structural_state_update` | structural mutation with structural cleanup and history semantics |
| `with_prepared_state_update` | commit a fully parsed or computed candidate without repeating expensive work under locks |
| `with_prepared_state_update_and_refit` | prepared replacement that also requires camera refit |

Do not increment `version` in an individual command when a transaction helper owns the commit. Undo and redo likewise restore a versioned structural snapshot through their dedicated commands rather than replaying frontend state.

### Structural cleanup

A successful structure replacement or topology-changing edit must not leave incompatible attached data behind. Phonon state, volumetric grids, BZ caches, Wannier overlays, measurements, selection, and renderer resources are either retained by an explicitly compatible operation or cleared by the structural transaction policy. Add a regression whenever a new attachment is introduced.

---

## Renderer and interaction boundaries

The interactive viewport is deliberately simple. It uses complete scene rebuilds as the baseline until measured evidence justifies more complex GPU protocols. Do not introduce delta snapshots, dirty GPU ranges, spatial indexes, or zero-copy GPU protocols on speculation.

The renderer currently presents:

- impostor-sphere atoms, bond cylinders, unit-cell and measurement lines;
- structure-aware isosurfaces and direct volume rendering;
- Brillouin-zone overlays and Wannier hopping networks;
- camera, picking, selection, and renderer-only periodic imagery.

An interaction preview may update only temporary renderer state. It must not allocate a committed version, write an undo entry, or request a full frontend snapshot. A later commit performs one complete validation and one transaction.

High-fidelity publication rendering is a future offscreen/export path. It must remain separable from the low-cost interactive viewport.

### Prepared atom scenes

`renderer/instance.rs` maps intrinsic state into `RenderAtomInstance` records. Each record carries its source intrinsic index and image shift. `prepare_atom_scene` separates opaque and transparent atom buffers and builds picking data without changing `CrystalState`. This mapping is what allows a visual periodic image to select its intrinsic source atom.

Renderer work should follow a prepare/commit shape:

1. clone or read the minimum state/settings data;
2. parse, recompute, and allocate outside the renderer lock;
3. acquire `Renderer` only to replace the prepared resource;
4. report allocation or GPU preparation failure before committed state changes.

---

## Frontend snapshots and events

`App.tsx` owns the only complete `get_crystal_state` refresh path. It listens to `state_changed`, validates the `{ version }` payload, de-duplicates pending versions, then refreshes the snapshot. Panels consume the resulting projection; they must not issue their own complete-state refetches after mutations.

Frontend IPC uses `safeInvoke` and `safeListen`, which validate wire data and surface structured `IpcException` errors. In browser/mock mode, mutations fail explicitly with `not_in_tauri`; they must never simulate a successful native mutation.

Tauri argument names are camelCase in TypeScript and snake_case in Rust. The checked contract layer enforces this boundary.

### Panel implementation conventions

Scientific panels are lazy-loaded through `components/panels/index.ts` and mounted by `RightSidebar.tsx`. Use the existing primitives in `components/panels/shared.tsx`:

- `ActionButton` for operations and operation-specific busy labels;
- `NumberInput`, `RangeInput`, and `SelectInput` for labeled controls;
- `PanelError` for visible structured IPC failures.

Commit local display state only after an awaited renderer IPC succeeds. If an operation can be repeated, guard it with a specific in-flight state rather than a single ambiguous boolean. Every input needs an associated label, invalid state, and disabled/busy behavior. Every listener needs an unlisten cleanup on unmount.

### Browser mode

`utils/tauri-mock.ts` exists so the React shell can render outside Tauri. It validates command names and provides read-only or UI-safe behavior, but a native mutation returns `not_in_tauri`. New panels must display that error rather than swallowing it or fabricating success.

---

## Imported scientific data

Supported public formats are normalized before renderer consumption. The renderer does not branch on a producer-specific file format.

- Crystal structures: CIF, PDB, XYZ, POSCAR, and supported Quantum ESPRESSO input.
- Scalar grids: CHGCAR/LOCPOT, Gaussian Cube, and XSF DATAGRID data.
- Modal/vector data: supported phonon and AXSF inputs.
- Reciprocal-space overlays: Brillouin-zone information and Wannier90 `wannier90_hr.dat` hopping networks.

Private or self-developed data formats receive no speculative container, plug-in, or converter framework. When a concrete dataset exists, its source adapter or converter must normalize declared coordinates, units, topology, and scalar/vector fields before reaching the renderer.

### Adding an importer

1. Place producer-specific parsing in `src-tauri/src/io/`.
2. Declare the accepted filename/extension, coordinate basis, lattice convention, units, scalar normalization, periodic axes, and resource bounds.
3. Return a normalized candidate structure or visualization DTO; do not update global state inside the parser.
4. Add dispatch only after an independent parser fixture is red.
5. Commit the candidate through the prepared-state transaction path and verify failure atomicity.

If the source is private, prefer an external converter when normalization can be performed reliably without adding producer-specific behavior to the renderer.

---

## C++ and FFI contributions

The C++ layer is for stateless crystallographic and geometry kernels. Public functions are declared in `cpp/include/physics_kernel.hpp`, implemented in `cpp/src/physics_kernel.cpp`, and exposed through `src-tauri/src/ffi/bridge.rs`.

When extending this boundary:

1. establish a C++ Breaker test under `cpp/tests/`;
2. use explicit Eigen column-major types and validate all sizes before allocation;
3. expose a thin C-compatible function with caller-owned buffers or explicit result objects;
4. catch exceptions in C++ and convert failure to the existing error convention;
5. add a Rust FFI round-trip or command-level atomicity test; and
6. keep parsing, recomputation, and C++ execution outside the renderer lock.

The `cxx` bridge provides a typed boundary, but that does not imply every value is transferred without copying. Do not document or design around zero-copy behavior unless it is measured and explicitly implemented.

---

## Adding or changing IPC

1. Add the snake_case Rust command in the appropriate `commands/` module and register it in `main.rs`.
2. Add the exact camelCase frontend arguments and result type to `IpcCommandContract`.
3. Add a runtime validator for any non-trivial result or event payload.
4. Call it through `safeInvoke`; do not import Tauri `invoke` directly into panels.
5. Run `npm run ipc:inventory` and review the generated inventory diff.
6. Run `npm run check:ipc`, `npm run test:ipc`, TypeScript, and the focused Rust test.
7. Update [IPC_Commands.md](IPC_Commands.md) after the checked contract is green.

For events, define one owner, validate the payload, expose registration failure in development, and prove listener cleanup. A new event is not a shortcut around the single snapshot-refresh path.

---

## Assistant boundary

The Assistant is a legacy experimental surface. It is closed by default and receives no planned product expansion. It must not become an autonomous agent, workflow manager, RAG system, or alternate state authority. Any command it proposes remains subject to the same validation, user approval, transaction, and snapshot rules as a direct UI action.

---

## Verification

Run the standard repository gates before merging a code or contract change:

```bash
source dev_env.sh && cargo check --manifest-path src-tauri/Cargo.toml
source dev_env.sh && cargo test --no-fail-fast --manifest-path src-tauri/Cargo.toml
cmake --build cpp/tests/build
ctest --test-dir cpp/tests/build --output-on-failure
pnpm install --frozen-lockfile
npm run ipc:inventory
npm run check:ipc
npm run test:ipc
./node_modules/.bin/tsc --noEmit
pnpm run build
git diff --check
```

See [TestingGuide.md](TestingGuide.md) for the Node workflow and test layers, [Algorithms.md](Algorithms.md) for implementation-level algorithm notes, and [Shader_Reference.md](Shader_Reference.md) for the current WGSL inventory.
