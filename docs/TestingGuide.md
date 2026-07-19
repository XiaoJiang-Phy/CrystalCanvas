# CrystalCanvas Testing and TDD Guide

> Baseline: `v0.6.1` | Updated: 2026-07-19

CrystalCanvas uses small, independently gated development Nodes. A passing software test establishes only its stated contract; it is not a proof that a scientific model, convergence setting, or imported dataset is physically correct.

---

## Node workflow

Each Node follows this order:

1. **Breaker** adds an independent failing gate for the required behavior and its important counterexamples.
2. **Builder** makes the smallest production-code change that satisfies the established gate. Builder does not add a new test to make its own implementation pass.
3. Run the focused gate and the relevant broader gates.
4. **Auditor** checks ownership, lock order, contract changes, regressions, and scope before the Node receives its own commit.

Do not proceed to the next Node while its gate is red. Do not weaken a tolerance, timing condition, physical expectation, or assertion merely to make a test pass.

---

## Scientific and numerical tests

For structural, slab, Brillouin-zone, phonon, or scalar-field work, run the `sci-validator` guard before building a physical test. A deterministic physical PASS requires a declared manifest, conventions, units, and an applicable registered executor. Without that evidence, record the software behavior without calling it a physics PASS; preserve unexpected evidence for `physics-audit` rather than tuning it away.

Project-wide software-test constraints:

- each test owns an isolated `CrystalState` and does not leak global state;
- Rust/C++ lattice matrices use explicit column-major layout;
- coordinate assertions use the project `1e-5 Å` convention where that convention is applicable;
- physical-property tolerances require an explicit unit and provenance in the test or its fixture;
- C++ exceptions never cross the FFI boundary; and
- renderer-only periodic images and Wannier ghosts are never accepted as intrinsic physical atoms.

---

## Test layers

### C++ kernel tests

`cpp/tests/` contains six CTest executables covering Spglib robustness, supercell matrices, slab generation, surface bases, layer/termination behavior, and structural edge cases.

```bash
cmake --build cpp/tests/build
ctest --test-dir cpp/tests/build --output-on-failure
```

The slab and surface-basis suites are geometry regression tests. Their acceptance criteria must state the lattice convention, Miller-index family, expected termination/layer behavior, and atomicity expectation.

### Rust/Tauri tests

`src-tauri/tests/` covers import/export, FFI, space-group, overlap, picking, bond, Marching Cubes, command-schema, PHYS-1, and SYNC-1 behavior.

```bash
source dev_env.sh && cargo test --no-fail-fast --manifest-path src-tauri/Cargo.toml
```

Notable v0.6.1 regression families include:

- `test_phys_1a_input_gate.rs`, `test_phys_1b_structural_invariants.rs`, and the PHYS-1C atomicity tests;
- `test_sync_1c_tauri_smoke.rs` for versioned snapshot synchronization;
- importer/exporter and FFI round-trip tests; and
- isolated geometry and renderer-adjacent algorithm tests.

### TypeScript, IPC, and UI contract gates

The repository has source-level Node gates, not an unconfigured frontend test backlog. The current suite contains 96 `node:test` cases across IPC inventory, runtime contracts, state refresh, and UI contracts.

```bash
pnpm install --frozen-lockfile
npm run ipc:inventory
npm run check:ipc
npm run test:ipc
./node_modules/.bin/tsc --noEmit
pnpm run build
```

These gates check, among other things:

- Rust command/event inventory against TypeScript contracts;
- camelCase frontend arguments and snake_case Rust parameters;
- `state_changed { version }` ownership and complete snapshot refresh;
- listener lifecycle and Tauri/browser-mock boundaries;
- UI-1 structure workspace, tool rail, panel, accessibility, and error-surface contracts; and
- lazy panel boundaries and the currently retained phonon frame path.

Source-level UI gates complement, but do not replace, desktop verification of native menus, GPU rendering, pointer interaction, and file dialogs.

---

## Standard full gate

Run this sequence before a release-quality commit that affects code, IPC, shaders, or scientific behavior:

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

For a documentation-only Node, run the link, stale-term, and `git diff --check` verification relevant to the changed documents. Do not run expensive or unrelated suites merely to label documentation as physically validated.

---

## Test-data policy

Fixtures in `tests/data/` and explicit test constructors are evidence with declared scope, not generic material truth. Keep the source, coordinate convention, unit, and expected behavior visible in a fixture or test name. Do not overwrite a reference structure to hide an anomalous result.

Use real structures for slab regressions only when the expected layer, termination, stoichiometry, minimum-distance, and atomicity assertions are independently stated. A structure that is only visually plausible is not an adequate physical regression fixture.

---

## Release automation and CI

`.github/workflows/release.yml` builds release artifacts when a `v*` tag is pushed. It is release automation, not a replacement for the full validation sequence above and not evidence that every platform or GPU path has passed the complete gate.

The platform priority remains macOS Intel/Metal first, macOS Apple Silicon second, Ubuntu/Vulkan selected verification third, and Windows deferred.

See [DeveloperGuide.md](DeveloperGuide.md) for ownership rules, [IPC_Commands.md](IPC_Commands.md) for contract maintenance, and [Algorithms.md](Algorithms.md) for implementation-level scientific visualization notes.
