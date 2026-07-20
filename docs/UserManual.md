# CrystalCanvas User Manual

> Baseline: `v0.6.1` | Updated: 2026-07-20

CrystalCanvas is a desktop application for structure-aware three-dimensional scientific visualization. It displays supplied crystal structures, scalar fields, phonon modes, Wannier hopping networks, and reciprocal-space scenes. It does not run DFT, EPC, transport, superconductivity, or other electronic-structure solvers.

---

## Installation

Download the macOS application from [GitHub Releases](https://github.com/XiaoJiang-Phy/CrystalCanvas/releases). The development baseline supports macOS on Intel and Apple Silicon.

The application does not have a paid Apple Developer signature. Complete these steps for the first launch:

1. Move `CrystalCanvas.app` to `/Applications`.
2. Control-click the application.
3. Select **Open**.
4. Confirm the macOS dialog.

If Gatekeeper still blocks the first launch:

```bash
sudo xattr -cr /Applications/CrystalCanvas.app
```

To build from source:

```bash
git clone https://github.com/XiaoJiang-Phy/CrystalCanvas.git
cd CrystalCanvas
source dev_env.sh
pnpm install --frozen-lockfile
pnpm run tauri dev
```

---

## Workspace overview

The center of the window is the native 3D viewport. The React workbench overlays it without becoming a second structure store.

- **Top chrome**: interaction mode, direct/reciprocal axis views, Reset View, Labels, experimental Assistant toggle, theme, and settings.
- **Left workspace**: the current structure summary, editable lattice parameters, and the intrinsic-atom table. Coordinates in this table are fractional; visual boundary images and Wannier ghosts are not listed as atoms.
- **Right tool rail**: open one inspector at a time: Bonds & Polyhedra, Isosurface / Volume, Phonon Modes, Brillouin Zone, Wannier / Hopping, Supercell, Slab (hkl), Add / Delete Atoms, or Measurements.
- **Bottom status bar**: interaction mode, space group, cell volume, active phonon mode, bond count, intrinsic atom count, and selection count.

### Viewport interaction

- **Select**: choose atoms; use Shift for a multi-selection.
- **Move**: translate the selected atoms in the active interaction plane.
- **Rotate**: orbit the camera from empty viewport space.
- **Measure**: select the required atoms for a distance, angle, or dihedral measurement.
- **Pan and zoom**: use the viewport gesture or the available pointer/trackpad controls.

Use the top `a`, `b`, `c`, `a*`, `b*`, and `c*` controls for aligned camera views. **Reset View** returns to the default camera.

---

## Load, inspect, and edit a structure

Open a structure from the native menu. You can also drop a supported file on the window. After a successful load, the left workspace shows the committed lattice and intrinsic sites. Select a table row to select the corresponding scene atom.

The structure tools provide:

- atom addition, deletion, substitution, and selection;
- lattice-parameter editing with validation;
- Niggli reduction plus primitive/conventional cell standardization;
- supercell preview and commit;
- slab preview, commit, and termination shifting;
- undo and redo through the native menu;
- bond and coordination analysis; and
- distance, angle, and dihedral measurements.

Structural changes are validated and committed atomically. If an operation fails, the structure, version, and undo history remain unchanged.

### Slabs

The **Slab (hkl)** inspector accepts Miller indices, a layer count, and vacuum thickness in Å. Slab generation requires a conventional cell with detected symmetry. Replace a P1 input with an appropriate conventional representation before you generate the slab. **Preview** does not commit the structure. **Apply** commits it.

---

## Volumetric fields

Open **Isosurface / Volume**. Select **Load Volumetric Data**. Before a grid is loaded, the panel shows an explicit empty state. After a valid grid is available, the panel shows its dimensions, range, and format. It then enables the render controls.

Available presentation controls include:

- isosurface, volume, or combined mode;
- positive, negative, or both isosurface signs;
- isovalue, opacity, density cutoff, and colormap; and
- data-range-dependent controls only when the imported range is finite and non-zero.

An invalid or unavailable range disables the dependent controls. CrystalCanvas does not send an unusable value to the renderer. When you prepare a figure, record the selected range, sign convention, and colormap.

---

## Phonon modes

The source structure and mode data must use the same atom order and atom count. To show a phonon mode:

1. Open **Phonon Modes**.
2. Load a supported phonon or AXSF source.
3. Select a mode.
4. Set the amplitude.
5. Start or pause the animation.

Phonon animation is a renderer presentation state, not a structural edit: playing, stopping, changing phase, or changing amplitude does not create undo entries or committed structure versions.

---

## Reciprocal space and Wannier networks

The **Brillouin Zone** inspector constructs and shows the Brillouin-zone overlay for the current structure and provides high-symmetry path information. The overlay is a visualization aid, not a band-structure calculation.

The **Wannier / Hopping** inspector loads a `wannier90_hr.dat` model, then exposes orbital, lattice-shell, magnitude, on-site, and visibility controls. The model must be compatible with the current structure. Neighboring-cell endpoints appear as renderer-only ghosts and do not alter the atom table or the committed structure.

---

## Experimental Assistant

The Assistant is an optional legacy experimental surface. It is closed by default. It is not required for a structure or visualization workflow, and it is not a research agent or solver. Review every proposed command before you approve it. An approved command uses the same validation and transaction rules as a direct UI action.

---

## Supported formats

| Data | Supported input | Supported output |
|---|---|---|
| Crystal structure | CIF, PDB, XYZ, POSCAR/CONTCAR, supported Quantum ESPRESSO input | POSCAR/VASP, LAMMPS data, Quantum ESPRESSO input |
| Scalar field | CHGCAR/LOCPOT, Gaussian Cube, XSF DATAGRID | — |
| Phonon animation | supported phonon inputs and AXSF | — |
| Wannier network | `wannier90_hr.dat` | — |

For a private or self-developed data source, do not assume a custom import format exists. A source adapter will be added only when a concrete dataset, coordinate convention, units, and target visualization are known.

---

## Figure export

Use the native export command to write the current rendered scene. The current release captures the interactive scene. It does not guarantee reproducible high-fidelity profiles, advanced lighting, or tiled 4K/8K output. Those capabilities remain planned publication-rendering work.

For troubleshooting, see [FAQ.md](FAQ.md). For a description of the product direction, see [ROADMAP.md](../ROADMAP.md).
