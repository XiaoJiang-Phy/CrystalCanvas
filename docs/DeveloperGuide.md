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

---

## Frontend snapshots and events

`App.tsx` owns the only complete `get_crystal_state` refresh path. It listens to `state_changed`, validates the `{ version }` payload, de-duplicates pending versions, then refreshes the snapshot. Panels consume the resulting projection; they must not issue their own complete-state refetches after mutations.

Frontend IPC uses `safeInvoke` and `safeListen`, which validate wire data and surface structured `IpcException` errors. In browser/mock mode, mutations fail explicitly with `not_in_tauri`; they must never simulate a successful native mutation.

Tauri argument names are camelCase in TypeScript and snake_case in Rust. The checked contract layer enforces this boundary.

---

## Imported scientific data

Supported public formats are normalized before renderer consumption. The renderer does not branch on a producer-specific file format.

- Crystal structures: CIF, PDB, XYZ, POSCAR, and supported Quantum ESPRESSO input.
- Scalar grids: CHGCAR/LOCPOT, Gaussian Cube, and XSF DATAGRID data.
- Modal/vector data: supported phonon and AXSF inputs.
- Reciprocal-space overlays: Brillouin-zone information and Wannier90 `wannier90_hr.dat` hopping networks.

Private or self-developed data formats receive no speculative container, plug-in, or converter framework. When a concrete dataset exists, its source adapter or converter must normalize declared coordinates, units, topology, and scalar/vector fields before reaching the renderer.

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
