//! Immutable, structure-only scene data for one-way publication export.

use crate::ipc::{IpcError, IpcResult};
use crate::renderer::camera::Camera;
use crate::renderer::instance::{
    AtomInstance, BondInstance, RenderAtomInstance, build_cell_lines, build_instance_data,
    build_periodic_atom_instances, build_publication_bond_instances_with_count,
    publication_bond_instance_count,
};
use crate::renderer::publication_look::{PublicationBondColorMode, PublicationLookProfile};
use crate::renderer::renderer::{PublicationExportSourceState, Renderer};
use crate::{crystal_state::CrystalState, settings::AppSettings};

const MAX_PUBLICATION_GLB_NODES: usize = 75_000;
const MAX_PUBLICATION_GLB_BYTES: usize = 96 * 1024 * 1024;
const MAX_PUBLICATION_GLB_PEAK_CPU_BYTES: usize = 320 * 1024 * 1024;
const GLB_JSON_BYTES_PER_NODE: usize = 1024;
const GLB_FIXED_BYTES: usize = 256 * 1024;

#[derive(Clone)]
pub struct PublicationSceneAtom {
    pub atom: AtomInstance,
    pub source_atom_index: usize,
    pub image_shift: [i32; 3],
}

/// Fixed resource envelope for a single Blender artifact. The writer must not
/// allocate or serialize beyond this accepted snapshot.
#[derive(Clone, Copy)]
pub struct PublicationGlbAdmission {
    pub atom_instances: usize,
    pub bonds: usize,
    pub nodes: usize,
    pub max_glb_bytes: usize,
    pub max_peak_cpu_bytes: usize,
}

#[derive(Clone)]
pub struct PublicationSceneSnapshot {
    pub atoms: Vec<PublicationSceneAtom>,
    pub bonds: Vec<BondInstance>,
    pub cell_edges: Vec<([f32; 3], [f32; 3], [f32; 4])>,
    pub camera: Camera,
    pub look_profile: PublicationLookProfile,
    pub intrinsic_atom_count: usize,
    pub show_bonds: bool,
    pub show_cell: bool,
    pub glb_admission: PublicationGlbAdmission,
}

impl PublicationSceneSnapshot {
    fn validate(&self) -> IpcResult<()> {
        let _bond_color_mode: PublicationBondColorMode = self.look_profile.bond_color_mode;
        let scene_nodes = self
            .atoms
            .len()
            .checked_add(self.bonds.len())
            .and_then(|count| count.checked_add(self.cell_edges.len()))
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| IpcError::render("publication scene count overflow"))?;
        if self.atoms.len() != self.glb_admission.atom_instances
            || self.bonds.len() != self.glb_admission.bonds
            || scene_nodes != self.glb_admission.nodes
            || scene_nodes > MAX_PUBLICATION_GLB_NODES
            || self.glb_admission.max_glb_bytes > MAX_PUBLICATION_GLB_BYTES
            || self.glb_admission.max_peak_cpu_bytes > MAX_PUBLICATION_GLB_PEAK_CPU_BYTES
        {
            return Err(IpcError::render(
                "publication GLB scene changed after admission",
            ));
        }
        let bond_count = u32::try_from(self.bonds.len())
            .map_err(|_| IpcError::render("publication bond scene changed after admission"))?;
        if usize::try_from(bond_count)
            .map_err(|_| IpcError::render("publication bond scene changed after admission"))?
            != self.bonds.len()
        {
            return Err(IpcError::render(
                "publication bond scene changed after admission",
            ));
        }
        Ok(())
    }
}

pub fn build_publication_scene_snapshot(
    source: &CrystalState,
    settings: &AppSettings,
    renderer: &Renderer,
    look_profile: PublicationLookProfile,
) -> IpcResult<PublicationSceneSnapshot> {
    reject_nonstructural_state(source, renderer)?;
    let atom_count = source.cart_positions.len();
    if source.atomic_numbers.len() != atom_count
        || source.elements.len() != atom_count
        || source.fract_x.len() != atom_count
        || source.fract_y.len() != atom_count
        || source.fract_z.len() != atom_count
        || source.occupancies.len() != atom_count
    {
        return Err(IpcError::render(
            "publication scene has inconsistent atom arrays",
        ));
    }
    if !source
        .cart_positions
        .iter()
        .flatten()
        .all(|value| value.is_finite())
        || !source
            .fract_x
            .iter()
            .chain(&source.fract_y)
            .chain(&source.fract_z)
            .all(|value| value.is_finite())
        || !source.occupancies.iter().all(|value| value.is_finite())
    {
        return Err(IpcError::render(
            "publication scene contains a non-finite coordinate",
        ));
    }
    let expected_atoms = periodic_atom_instance_count(source)?;
    let expected_bonds = if renderer.show_bonds {
        usize::try_from(publication_bond_instance_count(
            source,
            settings,
            look_profile.bond_color_mode,
        )?)
        .map_err(|_| IpcError::render("publication GLB bond count exceeds addressable memory"))?
    } else {
        0
    };
    let expected_cell_edges = usize::from(renderer.show_cell) * 12;
    let expected_nodes = expected_atoms
        .checked_add(expected_bonds)
        .and_then(|count| count.checked_add(expected_cell_edges))
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| IpcError::render("publication GLB node count overflow"))?;
    let estimated_glb_bytes = expected_nodes
        .checked_mul(GLB_JSON_BYTES_PER_NODE)
        .and_then(|bytes| bytes.checked_add(GLB_FIXED_BYTES))
        .ok_or_else(|| IpcError::render("publication GLB byte budget overflow"))?;
    let estimated_peak_cpu_bytes = estimated_glb_bytes
        .checked_mul(3)
        .and_then(|bytes| bytes.checked_add(GLB_FIXED_BYTES))
        .ok_or_else(|| IpcError::render("publication GLB peak memory budget overflow"))?;
    if expected_nodes > MAX_PUBLICATION_GLB_NODES
        || estimated_glb_bytes > MAX_PUBLICATION_GLB_BYTES
        || estimated_peak_cpu_bytes > MAX_PUBLICATION_GLB_PEAK_CPU_BYTES
    {
        return Err(IpcError::render(
            "publication GLB exceeds the admitted resource budget",
        ));
    }
    let glb_admission = PublicationGlbAdmission {
        atom_instances: expected_atoms,
        bonds: expected_bonds,
        nodes: expected_nodes,
        max_glb_bytes: estimated_glb_bytes,
        max_peak_cpu_bytes: estimated_peak_cpu_bytes,
    };
    let intrinsic = build_instance_data(
        &source.cart_positions,
        &source.atomic_numbers,
        &source.elements,
        &source.occupancies,
        settings,
        &[],
    )?;
    let periodic: Vec<RenderAtomInstance> = build_periodic_atom_instances(source, &intrinsic)?;
    let mut atoms = Vec::new();
    atoms
        .try_reserve_exact(periodic.len())
        .map_err(|_| IpcError::render("unable to allocate publication atom scene"))?;
    for instance in periodic {
        if !instance.atom.position.iter().all(|value| value.is_finite())
            || !instance.atom.radius.is_finite()
            || instance.atom.radius <= 0.0
            || !instance
                .atom
                .color
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            return Err(IpcError::render(
                "publication atom scene contains an invalid value",
            ));
        }
        if instance.source_atom_index >= source.elements.len() {
            return Err(IpcError::render("publication atom source index is invalid"));
        }
        atoms.push(PublicationSceneAtom {
            atom: instance.atom,
            source_atom_index: instance.source_atom_index,
            image_shift: instance.image_shift,
        });
    }
    let bonds = if renderer.show_bonds {
        build_publication_bond_instances_with_count(
            source,
            settings,
            look_profile.bond_color_mode,
            u32::try_from(expected_bonds)
                .map_err(|_| IpcError::render("publication GLB bond count exceeds GPU range"))?,
        )?
    } else {
        Vec::new()
    };
    let mut cell_edges = Vec::new();
    if renderer.show_cell {
        let lines = build_cell_lines(source);
        if lines.len() != 24 {
            return Err(IpcError::render("publication cell line count is invalid"));
        }
        cell_edges
            .try_reserve_exact(lines.len() / 2)
            .map_err(|_| IpcError::render("unable to allocate publication cell scene"))?;
        for pair in lines.chunks_exact(2) {
            if !pair
                .iter()
                .flat_map(|line| line.position.iter().chain(line.color.iter()))
                .all(|value| value.is_finite())
            {
                return Err(IpcError::render(
                    "publication cell scene contains an invalid value",
                ));
            }
            cell_edges.push((pair[0].position, pair[1].position, pair[0].color));
        }
    }
    let snapshot = PublicationSceneSnapshot {
        atoms,
        bonds,
        cell_edges,
        camera: renderer.camera,
        look_profile,
        intrinsic_atom_count: source.intrinsic_sites,
        show_bonds: renderer.show_bonds,
        show_cell: renderer.show_cell,
        glb_admission,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

fn periodic_atom_instance_count(source: &CrystalState) -> IpcResult<usize> {
    source
        .fract_x
        .iter()
        .zip(&source.fract_y)
        .zip(&source.fract_z)
        .try_fold(0usize, |count, ((x, y), z)| {
            let shift_count = usize::from(x.abs() < 1.0e-4 || (*x - 1.0).abs() < 1.0e-4)
                .checked_add(1)
                .and_then(|x_count| {
                    x_count.checked_mul(
                        usize::from(y.abs() < 1.0e-4 || (*y - 1.0).abs() < 1.0e-4)
                            .checked_add(1)?,
                    )
                })
                .and_then(|xy_count| {
                    xy_count.checked_mul(
                        usize::from(z.abs() < 1.0e-4 || (*z - 1.0).abs() < 1.0e-4)
                            .checked_add(1)?,
                    )
                })
                .ok_or_else(|| IpcError::render("publication periodic image count overflow"))?;
            count
                .checked_add(shift_count)
                .ok_or_else(|| IpcError::render("publication periodic image count overflow"))
        })
}

fn reject_nonstructural_state(source: &CrystalState, renderer: &Renderer) -> IpcResult<()> {
    let request = renderer.publication_export_request(
        1,
        1,
        PublicationExportSourceState {
            has_measurement_state: !source.measurements.is_empty(),
            has_selection_highlights: !source.selected_atoms.is_empty(),
            has_wannier_overlay: source.wannier_overlay.is_some(),
            has_active_phonon_state: source.active_phonon_mode.is_some(),
        },
        0,
    );
    let rejected = [
        (
            "measurements",
            request.has_measurement_state || request.has_measurement_overlays,
        ),
        ("selected_atoms", request.has_selection_highlights),
        (
            "active_phonon_mode",
            request.has_active_phonon_state
                || request.has_phonon_presentation
                || request.has_atom_drag,
        ),
        ("wannier_overlay", request.has_wannier_overlay),
        ("isosurface", request.has_isosurface),
        ("volume", request.has_volume),
        ("brillouin", request.show_bz),
    ];
    if let Some((name, _)) = rejected.into_iter().find(|(_, active)| *active) {
        return Err(IpcError::render(format!(
            "publication Blender export rejects {name}"
        )));
    }
    if request.has_hopping_overlays {
        return Err(IpcError::render(
            "publication Blender export rejects hoppings",
        ));
    }
    Ok(())
}
