# CrystalCanvas FAQ and Troubleshooting

> Baseline: `v0.6.1` | Updated: 2026-07-19

---

## Which platforms are supported?

macOS Intel/Metal is the baseline development target and macOS Apple Silicon is continuously verified. Ubuntu/Vulkan is a secondary build and rendering-verification target. Windows support is deferred until it is required by the maintainer workflow.

Do not treat a successful launch on a non-baseline platform as proof that every GPU path has been validated. Report the operating system, GPU, driver, backend, and an error log when opening an issue.

---

## macOS says that the developer cannot be verified

The released application is not signed with a paid Apple Developer certificate. Move it to `/Applications`, Control-click it, choose **Open**, and confirm the dialog. If needed, run:

```bash
sudo xattr -cr /Applications/CrystalCanvas.app
```

---

## A structure or scalar file does not load

Check that the source is one of the documented formats and that its contents match its extension. The importer reports a structured parse or I/O error instead of replacing the committed structure on failure.

For a private format, convert it to a supported public format or retain the original data until a concrete source-adapter requirement is defined. Do not relabel an unknown file merely to bypass format detection.

---

## The volumetric panel has no controls

This is expected before a scalar grid has been loaded. Data-specific controls appear only after `VolumetricInfo` has been received with a finite, non-zero range. A zero, non-finite, or otherwise unusable range leaves dependent controls disabled and reports its status rather than sending invalid values to the renderer.

If a large grid fails to load, preserve the original file and record the reported error, grid dimensions, GPU, and available memory. Do not assume that reducing a scientific grid is physically harmless; any resampling belongs in the producing workflow and must preserve the intended quantity and units.

---

## Slab construction is rejected for P1

The current slab command requires a conventional unit cell with detected symmetry because its Miller-index interpretation is tied to conventional axes. Standardize or reload a suitable conventional representation, then retry. A failed slab request is atomic and does not change the current structure.

---

## The Brillouin-zone display is not what I expected

The Brillouin-zone overlay is constructed from the committed lattice and the application's current dimensionality classification. Check the lattice parameters, periodic direction, and whether the imported structure contains the intended vacuum representation. Preserve the source structure and report the lattice plus the generated BZ information if the result is unexpected.

The overlay is for visualization. It does not calculate bands, transport coefficients, EPC quantities, or superconducting observables.

---

## Phonon loading or animation fails

The phonon source must be compatible with the currently loaded structure, including atom count and ordering. Reload the intended base structure before selecting the phonon source. Loading, selecting, or animating a phonon mode does not commit a structural edit.

---

## The Wannier network is empty or rejected

Load a compatible base structure before `wannier90_hr.dat`. The current overlay maps Wannier orbital indices to the available intrinsic atom positions; it rejects models that cannot be represented by that structure. Changing the structure or creating a supercell merely to satisfy the count is not a substitute for a physically compatible mapping.

---

## A browser preview reports `not_in_tauri`

That response is intentional. Browser/mock mode may render the UI but cannot mutate Rust `CrystalState` or native renderer resources. Run the Tauri desktop application for imports, structural operations, renderer mutations, and native dialogs.

---

## Space-group detection is unexpected

Do not manually round or alter coordinates merely to force a desired space group. Preserve the original input, verify its unit cell and coordinate convention in the producing code, and compare against an independently known standardized representation. If the discrepancy remains, report the original file and the detected result.

---

## Development build fails

Use the repository-local environment and the standard checks:

```bash
source dev_env.sh && cargo check --manifest-path src-tauri/Cargo.toml
pnpm install --frozen-lockfile
npm run check:ipc
pnpm run tauri dev
```

For a C++/FFI failure, include the full compiler error, operating system, and toolchain version. Do not delete build artifacts or change lockfiles as a first troubleshooting step.

See [UserManual.md](UserManual.md), [DeveloperGuide.md](DeveloperGuide.md), and [TestingGuide.md](TestingGuide.md) for the corresponding operating, architecture, and verification guidance.
