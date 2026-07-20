# CrystalCanvas Testing and TDD Guide

> Baseline: `v0.6.1` | Updated: 2026-07-20

CrystalCanvas uses small, independently gated development Nodes. A passing software test establishes only its stated contract. It does not prove that a scientific model, convergence setting, or imported dataset is physically correct.

---

## Node workflow

Use this order for each behavior-changing production Node:

1. **Breaker** adds an independent failing gate for the required behavior and its important counterexamples.
2. **Builder** makes the smallest production-code change that satisfies the established gate. Builder does not add a new test to make its own implementation pass.
3. Run the focused gate and the relevant broader gates.
4. **Auditor** checks ownership, lock order, contract changes, regressions, and scope before the Node receives its own commit.

Do not proceed to the next Node while its gate is red. Do not weaken a valid tolerance, timing condition, physical expectation, or assertion to make a test pass.

Documentation, read-only analysis, and mechanical refactoring do not require a new test. Run the relevant existing validation. An explicitly labeled non-production prototype may defer a new test, but it cannot support a release or a scientific conclusion until the applicable gate passes.

### What an independent red gate should contain

A useful Breaker gate fixes observable behavior rather than copying an intended implementation. Include:

- the successful case and at least one realistic counterexample;
- exact ownership expectations such as version, undo count, listener count, or unchanged state;
- finite/range/resource checks at the external boundary;
- cleanup and repeatability when the behavior has a lifecycle; and
- the command/event occurrence count when accidental duplicate IPC is a risk.

For an interaction Node, a gate might assert that 1,000 pointer deltas create no committed snapshots and pointer-up creates one version plus one undo entry. For a panel Node, it might assert that a failed renderer command preserves the previously committed local control state and displays the structured error.

Avoid gates that only search for a preferred variable name or duplicate the exact code the Builder is expected to write. Source-level contract tests are appropriate when they protect architectural ownership that is otherwise difficult to exercise without a desktop runtime.

---

## Scientific and numerical tests

Run the `sci-validator` guard before you change or execute structural, slab, Brillouin-zone, phonon, or scalar-field behavior, or before you make a physical claim from a test. Read-only work that adds no physical claim is exempt. A deterministic physical PASS requires a declared manifest, conventions, units, and an applicable registered executor. Without that evidence, record only the software behavior. Do not call it a physics PASS. Preserve unexpected evidence for `physics-audit`; do not tune it away.

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

| Executable source | Primary responsibility |
|---|---|
| `test_spglib_robustness.cpp` | space-group and malformed-input robustness |
| `test_supercell_eigen.cpp` | integer expansion, determinant, lattice, and atom count |
| `test_slab_surface_basis.cpp` | Miller families, signs, non-primitive indices, and basis failure cases |
| `test_slab_builder.cpp` | full slab construction, vacuum, deduplication, and resource bounds |
| `test_slab_layers.cpp` | layer clustering and termination shifting |
| `test_slab_eigen.cpp` | legacy slab regression coverage retained during migration |

For a fresh checkout, configure the C++ tests before you build them:

```bash
cmake -S cpp/tests -B cpp/tests/build
```

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

Run a focused integration test during development:

```bash
source dev_env.sh && cargo test --manifest-path src-tauri/Cargo.toml \
  --test test_phys_1c_failure_atomicity -- --nocapture
```

Use `--no-fail-fast` for the final Rust gate. This option keeps independent failures visible. A desktop-dependent smoke test may report an explicit skip when no native Tauri runtime or GPU surface is available. A skip is not a desktop PASS.

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

| Script | Responsibility |
|---|---|
| `ipc-inventory.test.mjs` | command/event discovery, registration, naming, and forbidden bypasses |
| `ipc-contract-runtime.test.mjs` | DTO validation, structured errors, lock and renderer boundaries |
| `sync-1a-state-refresh.test.mjs` | version payload and refresh trigger contract |
| `sync-1b-no-explicit-refetch.test.mjs` | prevents panels from creating full snapshot refreshes |
| `sync-1c-single-refresh.test.mjs` | unique owner and duplicate-version coalescing |
| `ui-contract.test.mjs` | UI-1 surfaces, shared primitives, lifecycle, accessibility, and regressions |

Run one source-level suite directly when iterating:

```bash
node --test scripts/ui-contract.test.mjs
node --test scripts/ipc-contract-runtime.test.mjs
```

Do not change a source contract only because a formatting refactor makes its regular expression inconvenient. First determine whether the test protects an architecture invariant. If it does, preserve the invariant and update only the necessary test expression.

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

For a documentation-only Node, check links, stale terms, and `git diff --check`. Run only the checks that apply to the changed documents. Do not run unrelated suites to label documentation as physically validated.

### Choosing a focused gate

| Change | Minimum focused verification before full gates |
|---|---|
| Rust structure mutation | relevant PHYS/transaction Rust test plus IPC contract test |
| C++ slab/supercell kernel | matching CTest executable plus Rust FFI integration test |
| IPC argument/result | inventory generation, `check:ipc`, runtime contract test, TypeScript |
| React panel behavior | `ui-contract.test.mjs`, relevant IPC runtime test, TypeScript |
| WGSL or GPU layout | focused Rust renderer test where available, cargo check/test, build, desktop GPU smoke |
| event/listener lifecycle | IPC/UI lifecycle gate plus mount/unmount desktop or browser check |

The focused gate shortens iteration; it does not replace the standard full gate before a release-quality commit.

---

## Test-data policy

Fixtures in `tests/data/` and explicit test constructors are evidence with declared scope, not generic material truth. Keep the source, coordinate convention, unit, and expected behavior visible in a fixture or test name. Do not overwrite a reference structure to hide an anomalous result.

Use real structures for slab regressions only when the expected layer, termination, stoichiometry, minimum-distance, and atomicity assertions are independently stated. A structure that is only visually plausible is not an adequate physical regression fixture.

### Fixture hygiene

- Keep small textual fixtures reviewable and preserve their provenance.
- Never modify a fixture and its expected output in the same unexplained mechanical change.
- Use temporary directories for exporter tests; do not write into repository fixtures.
- Test malformed input with minimal dedicated files or in-memory strings, not by corrupting a shared reference.
- For large scalar grids, test resource checks with compact synthetic metadata unless the actual grid topology is the behavior under test.

---

## Desktop verification

Use the native desktop path for behavior that source contracts cannot establish. Record the platform and backend. Then run the relevant checks:

- load a real structure and confirm left-workspace intrinsic counts;
- perform undo/redo from the native menu and verify scene plus menu state;
- mount/unmount or reopen affected panels and check for duplicate listeners;
- load volumetric/Wannier/phonon data and verify failure surfaces;
- exercise pointer interactions at normal and sustained rates; and
- export an image when camera, color, transparency, or renderer code changed.

Do not report a manual desktop PASS from a browser mock.

---

## Failure triage

When a gate fails:

1. preserve the first complete failure output;
2. identify whether the failure is deterministic, environment-dependent, or a skipped native capability;
3. rerun the smallest focused command once to establish reproducibility;
4. inspect ownership and input evidence before changing code or tolerance; and
5. hand unexpected physical evidence to `physics-audit` without assigning a cause from the test result alone.

Unrelated pre-existing warnings are not authority to edit nearby code. In particular, leave the known `new_occupancies` unused-`mut` and isosurface `max_vertices` warnings outside unrelated Nodes.

---

## Release automation and CI

`.github/workflows/release.yml` builds release artifacts when a `v*` tag is pushed. It is release automation, not a replacement for the full validation sequence above and not evidence that every platform or GPU path has passed the complete gate.

The platform priority remains macOS Intel/Metal first, macOS Apple Silicon second, Ubuntu/Vulkan selected verification third, and Windows deferred.

Before handing a Node to Auditor, provide the focused red/green evidence, the production-code scope, the full gates actually run, any explicit skips, and the exact remaining warnings. Auditor findings should be resolved through a new Breaker counterexample before a non-trivial Builder correction.

See [DeveloperGuide.md](DeveloperGuide.md) for ownership rules, [IPC_Commands.md](IPC_Commands.md) for contract maintenance, and [Algorithms.md](Algorithms.md) for implementation-level scientific visualization notes.
