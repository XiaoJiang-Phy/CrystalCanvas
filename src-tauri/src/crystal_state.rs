//! Core crystal state — Single Source of Truth (SSoT) with SoA layout for physics and rendering
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::ffi;
use crate::renderer::instance::covalent_radius;
use serde::{Deserialize, Serialize};

/// Error returned when trying to add an overlapping atom
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionError;

pub(crate) const MAX_STRUCTURAL_ATOMS: usize = 10_000;

const MIN_LATTICE_VOLUME_A3: f64 = 1.0e-12;
const MAX_LATTICE_CONDITION: f64 = 1.0e12;
const MAX_MILLER_INDEX_ABS: i64 = 128;
const MAX_ENUMERATION_OVERHEAD: usize = 256;

pub fn validate_fractional_position(position: [f64; 3]) -> Result<(), &'static str> {
    if position.iter().all(|component| component.is_finite()) {
        Ok(())
    } else {
        Err("fractional position components must be finite")
    }
}

impl CrystalState {
    pub(crate) fn validate_cartesian_positions(&self) -> Result<(), &'static str> {
        if self
            .cart_positions
            .iter()
            .flatten()
            .all(|component| component.is_finite())
        {
            Ok(())
        } else {
            Err("Cartesian position cannot be represented as finite f32")
        }
    }
}

pub fn validate_lattice_parameters(
    a: f64,
    b: f64,
    c: f64,
    alpha: f64,
    beta: f64,
    gamma: f64,
) -> Result<(), &'static str> {
    if ![a, b, c]
        .iter()
        .all(|length| length.is_finite() && *length > 0.0)
    {
        return Err("lattice lengths must be finite and positive");
    }
    if ![alpha, beta, gamma]
        .iter()
        .all(|angle| angle.is_finite() && *angle > 0.0 && *angle < 180.0)
    {
        return Err("lattice angles must be finite and between 0 and 180 degrees");
    }
    if alpha + beta <= gamma
        || alpha + gamma <= beta
        || beta + gamma <= alpha
        || alpha + beta + gamma >= 360.0
    {
        return Err("lattice angles do not define a three-dimensional cell");
    }

    let alpha = alpha.to_radians();
    let beta = beta.to_radians();
    let gamma = gamma.to_radians();
    let sin_gamma = gamma.sin();
    if !sin_gamma.is_finite() || sin_gamma.abs() <= f64::EPSILON {
        return Err("lattice angles produce a singular basis");
    }

    let cx = c * beta.cos();
    let cy = c * (alpha.cos() - beta.cos() * gamma.cos()) / sin_gamma;
    let cz_squared = c * c - cx * cx - cy * cy;
    if !cz_squared.is_finite() || cz_squared <= 0.0 {
        return Err("lattice parameters do not define a three-dimensional cell");
    }
    let cz = cz_squared.sqrt();
    let matrix = [
        a, 0.0, 0.0,
        b * gamma.cos(), b * sin_gamma, 0.0,
        cx, cy, cz,
    ];
    if !matrix.iter().all(|value| value.is_finite()) {
        return Err("lattice basis must be finite");
    }

    let determinant = a * b * sin_gamma * cz;
    if !determinant.is_finite() || determinant.abs() <= MIN_LATTICE_VOLUME_A3 {
        return Err("lattice volume is too small or non-finite");
    }

    let inverse = [
        (matrix[4] * matrix[8] - matrix[7] * matrix[5]) / determinant,
        (matrix[7] * matrix[2] - matrix[1] * matrix[8]) / determinant,
        (matrix[1] * matrix[5] - matrix[4] * matrix[2]) / determinant,
        (matrix[6] * matrix[5] - matrix[3] * matrix[8]) / determinant,
        (matrix[0] * matrix[8] - matrix[6] * matrix[2]) / determinant,
        (matrix[3] * matrix[2] - matrix[0] * matrix[5]) / determinant,
        (matrix[3] * matrix[7] - matrix[6] * matrix[4]) / determinant,
        (matrix[6] * matrix[1] - matrix[0] * matrix[7]) / determinant,
        (matrix[0] * matrix[4] - matrix[3] * matrix[1]) / determinant,
    ];
    if !inverse.iter().all(|value| value.is_finite()) {
        return Err("lattice inverse must be finite");
    }

    let matrix_norm = matrix[0].abs() + matrix[3].abs() + matrix[6].abs();
    let matrix_norm = matrix_norm.max(matrix[1].abs() + matrix[4].abs() + matrix[7].abs());
    let matrix_norm = matrix_norm.max(matrix[2].abs() + matrix[5].abs() + matrix[8].abs());
    let inverse_norm = inverse[0].abs() + inverse[3].abs() + inverse[6].abs();
    let inverse_norm = inverse_norm.max(inverse[1].abs() + inverse[4].abs() + inverse[7].abs());
    let inverse_norm = inverse_norm.max(inverse[2].abs() + inverse[5].abs() + inverse[8].abs());
    let condition = matrix_norm * inverse_norm;
    if !condition.is_finite() || condition > MAX_LATTICE_CONDITION {
        return Err("lattice basis is too ill-conditioned");
    }

    Ok(())
}

fn lattice_parameters_from_col_major(lattice: &[f64; 9]) -> Result<[f64; 6], &'static str> {
    if !lattice.iter().all(|value| value.is_finite()) {
        return Err("FFI lattice output must be finite");
    }
    let vectors = [
        [lattice[0], lattice[1], lattice[2]],
        [lattice[3], lattice[4], lattice[5]],
        [lattice[6], lattice[7], lattice[8]],
    ];
    let norms = vectors.map(|vector| vector[0].hypot(vector[1]).hypot(vector[2]));
    if !norms.iter().all(|norm| norm.is_finite() && *norm > 0.0) {
        return Err("FFI lattice output vectors must be finite and non-zero");
    }
    let angle = |left: usize, right: usize| -> Result<f64, &'static str> {
        let cosine = (0..3)
            .map(|component| {
                (vectors[left][component] / norms[left])
                    * (vectors[right][component] / norms[right])
            })
            .sum::<f64>();
        if !cosine.is_finite() {
            return Err("FFI lattice output angle must be finite");
        }
        let angle = cosine.clamp(-1.0, 1.0).acos().to_degrees();
        if angle.is_finite() {
            Ok(angle)
        } else {
            Err("FFI lattice output angle must be finite")
        }
    };
    let alpha = angle(1, 2)?;
    let beta = angle(2, 0)?;
    let gamma = angle(0, 1)?;
    validate_lattice_parameters(norms[0], norms[1], norms[2], alpha, beta, gamma)?;
    Ok([norms[0], norms[1], norms[2], alpha, beta, gamma])
}

fn atomic_number_sources(
    atomic_numbers: &[u8],
) -> Result<[Option<usize>; 256], &'static str> {
    let mut sources = [None; 256];
    for (index, atomic_number) in atomic_numbers.iter().copied().enumerate() {
        if atomic_number == 0 {
            return Err("input structure contains an invalid atomic number");
        }
        sources[usize::from(atomic_number)].get_or_insert(index);
    }
    Ok(sources)
}

pub fn validate_slab_request(
    miller: [i32; 3],
    layers: i32,
    vacuum_a: f64,
) -> Result<(), &'static str> {
    if miller == [0, 0, 0] {
        return Err("Miller indices must not all be zero");
    }
    if miller
        .iter()
        .any(|component| i64::from(*component).abs() > MAX_MILLER_INDEX_ABS)
    {
        return Err("Miller indices exceed the supported resource range");
    }
    if layers <= 0 || layers as usize > MAX_STRUCTURAL_ATOMS {
        return Err("slab layers must be positive and within the structural resource limit");
    }
    if !vacuum_a.is_finite() || vacuum_a < 0.0 {
        return Err("vacuum thickness must be finite and non-negative");
    }
    Ok(())
}

fn validate_slab_output_size(atom_count: usize, layers: i32) -> Result<usize, &'static str> {
    let output_atoms = atom_count
        .checked_mul(layers as usize)
        .ok_or("slab atom count overflow")?;
    if output_atoms > MAX_STRUCTURAL_ATOMS {
        return Err("slab exceeds the structural atom limit");
    }
    Ok(output_atoms)
}

pub fn validate_supercell_request(
    expansion: &[i32; 9],
    atom_count: usize,
) -> Result<usize, &'static str> {
    if atom_count == 0 {
        return Err("cannot generate a supercell from an empty crystal");
    }

    let matrix = expansion.map(i128::from);
    let product = |left: i128, right: i128| {
        left.checked_mul(right)
            .ok_or("supercell determinant overflow")
    };
    let subtract = |left: i128, right: i128| {
        left.checked_sub(right)
            .ok_or("supercell determinant overflow")
    };
    let add = |left: i128, right: i128| {
        left.checked_add(right)
            .ok_or("supercell determinant overflow")
    };
    let minor_00 = subtract(product(matrix[4], matrix[8])?, product(matrix[5], matrix[7])?)?;
    let minor_01 = subtract(product(matrix[3], matrix[8])?, product(matrix[5], matrix[6])?)?;
    let minor_02 = subtract(product(matrix[3], matrix[7])?, product(matrix[4], matrix[6])?)?;
    let determinant = add(
        subtract(product(matrix[0], minor_00)?, product(matrix[1], minor_01)?)?,
        product(matrix[2], minor_02)?,
    )?;
    if determinant <= 0 {
        return Err("supercell transformation determinant must be positive");
    }

    let copies = usize::try_from(determinant)
        .map_err(|_| "supercell determinant exceeds platform capacity")?;
    let output_atoms = atom_count
        .checked_mul(copies)
        .ok_or("supercell atom count overflow")?;
    if output_atoms > MAX_STRUCTURAL_ATOMS {
        return Err("supercell exceeds the structural atom limit");
    }

    let mut enumeration_width = 1usize;
    for row in 0..3 {
        let mut minimum = 0i128;
        let mut maximum = 0i128;
        for column in 0..3 {
            let value = matrix[column * 3 + row];
            minimum = minimum
                .checked_add(value.min(0))
                .ok_or("supercell enumeration bound overflow")?;
            maximum = maximum
                .checked_add(value.max(0))
                .ok_or("supercell enumeration bound overflow")?;
        }
        if minimum < i128::from(i32::MIN) + 1 || maximum > i128::from(i32::MAX) - 1 {
            return Err("supercell enumeration bounds exceed the FFI integer range");
        }
        let width = maximum
            .checked_sub(minimum)
            .and_then(|span| span.checked_add(3))
            .and_then(|span| usize::try_from(span).ok())
            .ok_or("supercell enumeration width overflow")?;
        enumeration_width = enumeration_width
            .checked_mul(width)
            .ok_or("supercell enumeration volume overflow")?;
    }
    let work_items = atom_count
        .checked_mul(enumeration_width)
        .ok_or("supercell enumeration work overflow")?;
    let work_limit = output_atoms
        .checked_mul(MAX_ENUMERATION_OVERHEAD)
        .ok_or("supercell enumeration work limit overflow")?;
    if work_items > work_limit {
        return Err("supercell enumeration overhead exceeds the resource limit");
    }

    Ok(output_atoms)
}

pub fn validate_atom_request(
    element_symbol: &str,
    atomic_number: u8,
    fract_pos: [f64; 3],
    atom_count: usize,
) -> Result<(), &'static str> {
    validate_fractional_position(fract_pos)?;
    if element_symbol.is_empty() || atomic_number == 0 {
        return Err("atom element identity is invalid");
    }
    if atom_count >= MAX_STRUCTURAL_ATOMS {
        return Err("adding an atom would exceed the structural atom limit");
    }
    Ok(())
}

pub(crate) struct AtomTranslationRollback {
    atoms: Vec<AtomTranslationRollbackEntry>,
}

struct AtomTranslationRollbackEntry {
    index: usize,
    fractional: [f64; 3],
    cartesian: [f32; 3],
}

// =========================================================================
// Bond Analysis Data Structures (M10)
// =========================================================================

/// A single chemical bond between two atoms.
#[derive(Clone, Debug, Serialize)]
pub struct BondInfo {
    pub atom_i: usize,
    pub atom_j: usize,
    pub distance: f64, // Angstroms
}

/// Coordination environment for a single atom.
#[derive(Clone, Debug, Serialize)]
pub struct CoordinationInfo {
    pub center_idx: usize,
    pub element: String,
    pub coordination_number: usize,
    pub neighbor_indices: Vec<usize>,
    pub neighbor_distances: Vec<f64>,
    pub polyhedron_type: String, // e.g. "Octahedron", "Tetrahedron", ""
}

/// Complete bond analysis result.
#[derive(Clone, Debug, Serialize, Default)]
pub struct BondAnalysis {
    pub bonds: Vec<BondInfo>,
    pub coordination: Vec<CoordinationInfo>,
    pub threshold_factor: f64,
}

#[derive(Clone)]
pub struct BrillouinZoneCache {
    pub bz: crate::brillouin_zone::BrillouinZone,
    pub kpath: crate::kpath::KPath,
    pub source_version: u32,
    pub vacuum_axis: Option<usize>,
}

/// Type of measurement annotation
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MeasurementKind {
    Distance,
    Angle,
    Dihedral,
}

/// Measurement overlay payload for renderer/frontend
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeasurementOverlay {
    pub indices: Vec<usize>,
    pub kind: MeasurementKind,
    pub value: f64,               // Angstroms or degrees
    pub label_position: [f32; 3], // Cartesian midpoint
}

/// The central crystal structure state, holding all atom data in SoA layout.
/// - f64 fields for physics calculations (fractional coords)
/// - f32 fields for GPU rendering (Cartesian coords, populated on demand)
#[derive(Clone, Serialize)]
pub struct CrystalState {
    pub name: String,
    // Unit cell parameters (angstroms, degrees)
    pub cell_a: f64,
    pub cell_b: f64,
    pub cell_c: f64,
    pub cell_alpha: f64,
    pub cell_beta: f64,
    pub cell_gamma: f64,
    // Space group
    pub spacegroup_hm: String,
    pub spacegroup_number: i32,
    // SoA layout — f64 for physics
    pub labels: Vec<String>,
    pub elements: Vec<String>,
    pub fract_x: Vec<f64>,
    pub fract_y: Vec<f64>,
    pub fract_z: Vec<f64>,
    pub occupancies: Vec<f64>,
    pub atomic_numbers: Vec<u8>,
    // f32 for GPU rendering (populated on demand)
    pub cart_positions: Vec<[f32; 3]>,
    // State version to trigger frontend reactivity
    pub version: u32,
    pub is_2d: bool,
    pub vacuum_axis: Option<usize>,
    // Bond analysis cache (M10) — recomputed on state change
    #[serde(skip)]
    pub bond_analysis: Option<BondAnalysis>,
    // Phonon data and animation state (M10)
    #[serde(skip)]
    pub phonon_data: Option<crate::phonon::PhononData>,
    #[serde(skip)]
    pub active_phonon_mode: Option<usize>,
    #[serde(skip)]
    pub phonon_phase: f64,
    /// Number of atoms before boundary mirroring (visual duplicates)
    pub intrinsic_sites: usize,
    #[serde(skip)]
    pub selected_atoms: Vec<usize>,
    #[serde(skip)]
    pub volumetric_data: Option<crate::volumetric::VolumetricData>,
    #[serde(skip)]
    pub bz_cache: Option<BrillouinZoneCache>,
    #[serde(skip)]
    pub wannier_overlay: Option<crate::wannier::WannierOverlay>,
    pub measurements: Vec<MeasurementOverlay>,
}

impl Default for CrystalState {
    fn default() -> Self {
        Self {
            name: String::new(),
            cell_a: 0.0,
            cell_b: 0.0,
            cell_c: 0.0,
            cell_alpha: 90.0,
            cell_beta: 90.0,
            cell_gamma: 90.0,
            spacegroup_hm: String::new(),
            spacegroup_number: 0,
            labels: Vec::new(),
            elements: Vec::new(),
            fract_x: Vec::new(),
            fract_y: Vec::new(),
            fract_z: Vec::new(),
            occupancies: Vec::new(),
            atomic_numbers: Vec::new(),
            cart_positions: Vec::new(),
            version: 0,
            is_2d: false,
            vacuum_axis: None,
            bond_analysis: None,
            phonon_data: None,
            active_phonon_mode: None,
            phonon_phase: 0.0,
            intrinsic_sites: 0,
            selected_atoms: Vec::new(),
            volumetric_data: None,
            bz_cache: None,
            wannier_overlay: None,
            measurements: Vec::new(),
        }
    }
}

impl CrystalState {
    pub fn invalidate_structure_bound_data(&mut self) {
        self.bond_analysis = None;
        self.phonon_data = None;
        self.active_phonon_mode = None;
        self.phonon_phase = 0.0;
        self.volumetric_data = None;
        self.bz_cache = None;
        self.wannier_overlay = None;
    }

    /// Construct a CrystalState by parsing a CIF file.
    pub fn from_cif(path: &str) -> std::result::Result<Self, String> {
        let ffi_data =
            ffi::parse_cif_file(path).map_err(|e| format!("Failed to parse CIF: {}", e))?;
        Ok(Self::from_ffi(ffi_data))
    }
    /// Construct from FFI data returned by the C++ parser.
    pub fn from_ffi(data: ffi::FfiCrystalData) -> Self {
        let n = data.sites.len();
        log::info!(
            "[from_ffi] C++ returned {} sites, spacegroup_hm='{}', spacegroup_number={}",
            n,
            data.spacegroup_hm,
            data.spacegroup_number
        );

        // Collect sites, wrapping fractional coordinates into [0, 1)
        let mut labels: Vec<String> = Vec::with_capacity(n);
        let mut elements: Vec<String> = Vec::with_capacity(n);
        let mut fract_x: Vec<f64> = Vec::with_capacity(n);
        let mut fract_y: Vec<f64> = Vec::with_capacity(n);
        let mut fract_z: Vec<f64> = Vec::with_capacity(n);
        let mut occupancies: Vec<f64> = Vec::with_capacity(n);
        let mut atomic_numbers: Vec<u8> = Vec::with_capacity(n);

        let eps = 1e-4;

        for site in data.sites {
            let fx = site.fract_x - site.fract_x.floor();
            let fy = site.fract_y - site.fract_y.floor();
            let fz = site.fract_z - site.fract_z.floor();

            // Deduplicate: skip if an atom with identical (wrapped) coords already exists
            let mut duplicate = false;
            for k in 0..fract_x.len() {
                if (fract_x[k] - fx).abs() < eps
                    && (fract_y[k] - fy).abs() < eps
                    && (fract_z[k] - fz).abs() < eps
                    && atomic_numbers[k] == site.atomic_number
                {
                    duplicate = true;
                    break;
                }
            }
            if duplicate {
                continue;
            }

            labels.push(site.label);
            elements.push(site.element_symbol);
            fract_x.push(fx);
            fract_y.push(fy);
            fract_z.push(fz);
            occupancies.push(site.occ);
            atomic_numbers.push(site.atomic_number);
        }

        let actual_n = labels.len();
        log::info!(
            "[from_ffi] After wrapping & dedup: {} sites (from {} raw)",
            actual_n,
            n
        );

        let mut state = CrystalState {
            name: data.name,
            cell_a: data.a,
            cell_b: data.b,
            cell_c: data.c,
            cell_alpha: if data.alpha == 0.0 { 90.0 } else { data.alpha },
            cell_beta: if data.beta == 0.0 { 90.0 } else { data.beta },
            cell_gamma: if data.gamma == 0.0 { 90.0 } else { data.gamma },
            spacegroup_hm: data.spacegroup_hm,
            spacegroup_number: data.spacegroup_number,
            labels,
            elements,
            fract_x,
            fract_y,
            fract_z,
            occupancies,
            atomic_numbers,
            cart_positions: Vec::new(),
            version: 0,
            is_2d: false,
            vacuum_axis: None,
            bond_analysis: None,
            phonon_data: None,
            active_phonon_mode: None,
            phonon_phase: 0.0,
            intrinsic_sites: actual_n,
            selected_atoms: Vec::new(),
            volumetric_data: None,
            bz_cache: None,
            wannier_overlay: None,
            measurements: Vec::new(),
        };

        state.fractional_to_cartesian();
        state.detect_spacegroup();
        state
    }

    pub fn detect_spacegroup(&mut self) {
        let n_intrinsic = self.intrinsic_sites;
        if n_intrinsic == 0 {
            return;
        }

        // Prepare lattice in col-major
        let lattice_col_major = self.get_lattice_col_major();

        let mut flat_positions = Vec::with_capacity(n_intrinsic * 3);
        let mut types = Vec::with_capacity(n_intrinsic);
        for i in 0..n_intrinsic {
            flat_positions.push(self.fract_x[i]);
            flat_positions.push(self.fract_y[i]);
            flat_positions.push(self.fract_z[i]);
            types.push(self.atomic_numbers[i] as i32);
        }

        let sg = unsafe {
            ffi::get_spacegroup(
                lattice_col_major.as_ptr(),
                flat_positions.as_ptr(),
                types.as_ptr(),
                n_intrinsic,
                1e-4, // symprec - relaxed slightly for better robustness
            )
        };

        if sg > 0 {
            self.spacegroup_number = sg;
            self.spacegroup_hm = format!("#{}", sg);
        }
    }

    /// Number of atom sites.
    pub fn num_atoms(&self) -> usize {
        self.labels.len()
    }

    /// Get the 3x3 lattice matrix in column-major layout.
    pub fn get_lattice_col_major(&self) -> [f64; 9] {
        let alpha = self.cell_alpha.to_radians();
        let beta = self.cell_beta.to_radians();
        let gamma = self.cell_gamma.to_radians();
        let a = self.cell_a;
        let b = self.cell_b;
        let c = self.cell_c;

        let cx = c * beta.cos();
        let cy = c * (alpha.cos() - beta.cos() * gamma.cos()) / gamma.sin();
        let cz = (c * c - cx * cx - cy * cy).max(0.0).sqrt();

        [
            a,
            0.0,
            0.0,
            b * gamma.cos(),
            b * gamma.sin(),
            0.0,
            cx,
            cy,
            cz,
        ]
    }

    /// Set unit cell parameters from a 3x3 column-major lattice matrix.
    pub fn set_lattice_col_major(&mut self, mat: &[f64; 9]) {
        let vx = [mat[0], mat[1], mat[2]];
        let vy = [mat[3], mat[4], mat[5]];
        let vz = [mat[6], mat[7], mat[8]];

        let a = (vx[0] * vx[0] + vx[1] * vx[1] + vx[2] * vx[2]).sqrt();
        let b = (vy[0] * vy[0] + vy[1] * vy[1] + vy[2] * vy[2]).sqrt();
        let c = (vz[0] * vz[0] + vz[1] * vz[1] + vz[2] * vz[2]).sqrt();

        let dot_ab = vx[0] * vy[0] + vx[1] * vy[1] + vx[2] * vy[2];
        let dot_bc = vy[0] * vz[0] + vy[1] * vz[1] + vy[2] * vz[2];
        let dot_ca = vz[0] * vx[0] + vz[1] * vx[1] + vz[2] * vx[2];

        self.cell_gamma = (dot_ab / (a * b)).clamp(-1.0, 1.0).acos().to_degrees();
        self.cell_alpha = (dot_bc / (b * c)).clamp(-1.0, 1.0).acos().to_degrees();
        self.cell_beta = (dot_ca / (c * a)).clamp(-1.0, 1.0).acos().to_degrees();
        
        self.cell_a = a;
        self.cell_b = b;
        self.cell_c = c;
    }

    /// Reduce lattice to its Niggli form.
    /// Fractional coordinates are transformed via $\mathbf{f}' = L_{\text{new}}^{-1} \cdot L_{\text{old}} \cdot \mathbf{f}$.
    pub fn niggli_reduce(&mut self) -> Result<(), String> {
        let old_lattice = self.get_lattice_col_major();
        let mut new_lattice = old_lattice;
        let status = unsafe { ffi::niggli_reduce(new_lattice.as_mut_ptr(), 1e-4) };
        if status != 0 {
            return Err("Niggli reduce failed".to_string());
        }
        self.transform_fractional_coords(&old_lattice, &new_lattice);
        self.set_lattice_col_major(&new_lattice);
        self.fractional_to_cartesian();
        Ok(())
    }

    /// Reduce lattice to its Delaunay form.
    /// Fractional coordinates are transformed via $\mathbf{f}' = L_{\text{new}}^{-1} \cdot L_{\text{old}} \cdot \mathbf{f}$.
    pub fn delaunay_reduce(&mut self) -> Result<(), String> {
        let old_lattice = self.get_lattice_col_major();
        let mut new_lattice = old_lattice;
        let status = unsafe { ffi::delaunay_reduce(new_lattice.as_mut_ptr(), 1e-4) };
        if status != 0 {
            return Err("Delaunay reduce failed".to_string());
        }
        self.transform_fractional_coords(&old_lattice, &new_lattice);
        self.set_lattice_col_major(&new_lattice);
        self.fractional_to_cartesian();
        Ok(())
    }

    /// Transform fractional coordinates from one lattice basis to another.
    /// $\mathbf{f}_{\text{new}} = L_{\text{new}}^{-1} \cdot L_{\text{old}} \cdot \mathbf{f}_{\text{old}}$
    fn transform_fractional_coords(&mut self, old_lat: &[f64; 9], new_lat: &[f64; 9]) {
        // L_new^{-1} via Cramer's rule (3x3 ColMajor)
        let det = new_lat[0] * (new_lat[4] * new_lat[8] - new_lat[5] * new_lat[7])
                - new_lat[3] * (new_lat[1] * new_lat[8] - new_lat[2] * new_lat[7])
                + new_lat[6] * (new_lat[1] * new_lat[5] - new_lat[2] * new_lat[4]);

        if det.abs() < 1e-15 {
            return; // degenerate lattice, bail
        }
        let inv_det = 1.0 / det;

        // Cofactor matrix of new_lat (ColMajor), transposed = adjugate
        let inv = [
            (new_lat[4] * new_lat[8] - new_lat[5] * new_lat[7]) * inv_det,  // [0]
            (new_lat[2] * new_lat[7] - new_lat[1] * new_lat[8]) * inv_det,  // [1]
            (new_lat[1] * new_lat[5] - new_lat[2] * new_lat[4]) * inv_det,  // [2]
            (new_lat[5] * new_lat[6] - new_lat[3] * new_lat[8]) * inv_det,  // [3]
            (new_lat[0] * new_lat[8] - new_lat[2] * new_lat[6]) * inv_det,  // [4]
            (new_lat[2] * new_lat[3] - new_lat[0] * new_lat[5]) * inv_det,  // [5]
            (new_lat[3] * new_lat[7] - new_lat[4] * new_lat[6]) * inv_det,  // [6]
            (new_lat[1] * new_lat[6] - new_lat[0] * new_lat[7]) * inv_det,  // [7]
            (new_lat[0] * new_lat[4] - new_lat[1] * new_lat[3]) * inv_det,  // [8]
        ];

        // M = L_new^{-1} * L_old  (both ColMajor, result ColMajor)
        let mut m = [0.0f64; 9];
        for col in 0..3 {
            for row in 0..3 {
                m[col * 3 + row] = inv[row] * old_lat[col * 3]
                    + inv[3 + row] * old_lat[col * 3 + 1]
                    + inv[6 + row] * old_lat[col * 3 + 2];
            }
        }

        let n = self.intrinsic_sites;
        for i in 0..n {
            let fx = self.fract_x[i];
            let fy = self.fract_y[i];
            let fz = self.fract_z[i];

            let mut nx = m[0] * fx + m[3] * fy + m[6] * fz;
            let mut ny = m[1] * fx + m[4] * fy + m[7] * fz;
            let mut nz = m[2] * fx + m[5] * fy + m[8] * fz;

            // Wrap to [0, 1)
            nx -= nx.floor();
            ny -= ny.floor();
            nz -= nz.floor();

            self.fract_x[i] = nx;
            self.fract_y[i] = ny;
            self.fract_z[i] = nz;
        }
    }

    /// Standardize cell to its primitive representation.
    pub fn to_primitive(&mut self) -> Result<(), String> {
        self.standardize(true)
    }

    /// Standardize cell to its conventional representation.
    pub fn to_conventional(&mut self) -> Result<(), String> {
        self.standardize(false)
    }

    /// Standardize the cell (internal implementation)
    fn standardize(&mut self, to_primitive: bool) -> Result<(), String> {
        let n_atoms = self.intrinsic_sites;
        if n_atoms == 0 {
            return Ok(());
        }

        let mut lattice = self.get_lattice_col_major();
        // Spglib may need up to 4x atoms when converting primitive -> face-centered conventional
        let capacity = n_atoms * 4; 
        
        let mut flat_positions = Vec::with_capacity(capacity * 3);
        let mut types = Vec::with_capacity(capacity);

        for i in 0..n_atoms {
            flat_positions.push(self.fract_x[i]);
            flat_positions.push(self.fract_y[i]);
            flat_positions.push(self.fract_z[i]);
            types.push(self.atomic_numbers[i] as i32);
        }
        
        // Resize to capacity padding with zeros so C++ can write safely
        flat_positions.resize(capacity * 3, 0.0);
        types.resize(capacity, 0);

        let new_size = unsafe {
            ffi::standardize_cell(
                lattice.as_mut_ptr(),
                flat_positions.as_mut_ptr(),
                types.as_mut_ptr(),
                n_atoms,
                capacity,
                if to_primitive { 1 } else { 0 },
                1e-4,
            )
        };

        if new_size <= 0 {
            return Err("Spglib standardize_cell failed".to_string());
        }
        
        let new_size = new_size as usize;
        self.set_lattice_col_major(&lattice);
        
        // Rebuild atom lists
        let mut new_labels = Vec::with_capacity(new_size);
        let mut new_elements = Vec::with_capacity(new_size);
        let mut new_fract_x = Vec::with_capacity(new_size);
        let mut new_fract_y = Vec::with_capacity(new_size);
        let mut new_fract_z = Vec::with_capacity(new_size);
        let mut new_atomic_numbers = Vec::with_capacity(new_size);
        let mut new_occupancies = vec![1.0; new_size];

        for i in 0..new_size {
            let t = types[i] as u8;
            let mut label = format!("Element{}", t);
            let mut elem = "Unknown".to_string();

            for j in 0..n_atoms {
                if self.atomic_numbers[j] == t {
                    label = self.labels[j].clone();
                    elem = self.elements[j].clone();
                    break;
                }
            }

            // Wrap coordinates to [0, 1) properly
            let mut fx = flat_positions[3 * i];
            let mut fy = flat_positions[3 * i + 1];
            let mut fz = flat_positions[3 * i + 2];
            fx = fx - fx.floor();
            fy = fy - fy.floor();
            fz = fz - fz.floor();

            new_labels.push(label);
            new_elements.push(elem);
            new_fract_x.push(fx);
            new_fract_y.push(fy);
            new_fract_z.push(fz);
            new_atomic_numbers.push(t);
        }

        self.labels = new_labels;
        self.elements = new_elements;
        self.fract_x = new_fract_x;
        self.fract_y = new_fract_y;
        self.fract_z = new_fract_z;
        self.atomic_numbers = new_atomic_numbers;
        self.occupancies = new_occupancies;
        self.intrinsic_sites = new_size;
        
        self.fractional_to_cartesian();
        self.detect_spacegroup();

        Ok(())
    }

    /// Convert fractional coordinates to Cartesian using the unit cell matrix.
    /// Populates `cart_positions` as f32 for GPU upload.
    pub fn fractional_to_cartesian(&mut self) {
        let (a, b, c) = (self.cell_a, self.cell_b, self.cell_c);
        let alpha_rad = self.cell_alpha.to_radians();
        let beta_rad = self.cell_beta.to_radians();
        let gamma_rad = self.cell_gamma.to_radians();

        let cos_alpha = alpha_rad.cos();
        let cos_beta = beta_rad.cos();
        let cos_gamma = gamma_rad.cos();
        let sin_gamma = gamma_rad.sin();

        // Orthogonalization matrix (PDB convention: a along X, c* along Z)
        let m00 = a;
        let m01 = b * cos_gamma;
        let m02 = c * cos_beta;
        let m11 = b * sin_gamma;
        let m12 = c * (cos_alpha - cos_beta * cos_gamma) / sin_gamma;
        let m22 = c
            * ((1.0 - cos_alpha * cos_alpha - cos_beta * cos_beta - cos_gamma * cos_gamma
                + 2.0 * cos_alpha * cos_beta * cos_gamma)
                .sqrt())
            / sin_gamma;

        self.cart_positions.clear();
        self.cart_positions.reserve(self.num_atoms());

        for i in 0..self.num_atoms() {
            let fx = self.fract_x[i];
            let fy = self.fract_y[i];
            let fz = self.fract_z[i];
            let x = (m00 * fx + m01 * fy + m02 * fz) as f32;
            let y = (m11 * fy + m12 * fz) as f32;
            let z = (m22 * fz) as f32;
            self.cart_positions.push([x, y, z]);
        }
    }

    /// Calculate the geometric center of the unit cell.
    pub fn unit_cell_center(&self) -> [f32; 3] {
        let alpha = self.cell_alpha.to_radians();
        let beta = self.cell_beta.to_radians();
        let gamma = self.cell_gamma.to_radians();

        let cx = self.cell_c * beta.cos();
        let cy = self.cell_c * (alpha.cos() - beta.cos() * gamma.cos()) / gamma.sin();
        let cz = (self.cell_c * self.cell_c - cx * cx - cy * cy)
            .max(0.0)
            .sqrt();

        // The center in fractional coordinates is exactly (0.5, 0.5, 0.5)
        let x = 0.5 * self.cell_a + 0.5 * self.cell_b * gamma.cos() + 0.5 * cx;
        let y = 0.5 * self.cell_b * gamma.sin() + 0.5 * cy;
        let z = 0.5 * cz;

        [x as f32, y as f32, z as f32]
    }

    /// Translates specific atoms by a Cartesian vector (x, y, z), updating their fractional coords.
    pub(crate) fn translate_atoms_cartesian(
        &mut self,
        indices: &[usize],
        translation: glam::Vec3,
    ) -> Result<AtomTranslationRollback, std::collections::TryReserveError> {
        let (a, b, c) = (self.cell_a as f32, self.cell_b as f32, self.cell_c as f32);
        let alpha_rad = self.cell_alpha.to_radians() as f32;
        let beta_rad = self.cell_beta.to_radians() as f32;
        let gamma_rad = self.cell_gamma.to_radians() as f32;
        
        let cos_alpha = alpha_rad.cos();
        let cos_beta = beta_rad.cos();
        let cos_gamma = gamma_rad.cos();
        let sin_gamma = gamma_rad.sin();
        
        let m00 = a;
        let m01 = b * cos_gamma;
        let m02 = c * cos_beta;
        let m11 = b * sin_gamma;
        let m12 = c * (cos_alpha - cos_beta * cos_gamma) / sin_gamma;
        let m22 = c
            * ((1.0 - cos_alpha * cos_alpha - cos_beta * cos_beta - cos_gamma * cos_gamma
                + 2.0 * cos_alpha * cos_beta * cos_gamma)
                .max(0.0)
                .sqrt())
            / sin_gamma;
        
        let d_frac_z = translation.z / m22;
        let d_frac_y = (translation.y - m12 * d_frac_z) / m11;
        let d_frac_x = (translation.x - m01 * d_frac_y - m02 * d_frac_z) / m00;
        
        let mut atoms = Vec::new();
        atoms.try_reserve_exact(indices.len())?;
        for &idx in indices {
            if idx < self.num_atoms() {
                atoms.push(AtomTranslationRollbackEntry {
                    index: idx,
                    fractional: [self.fract_x[idx], self.fract_y[idx], self.fract_z[idx]],
                    cartesian: self.cart_positions[idx],
                });
                self.fract_x[idx] += d_frac_x as f64;
                self.fract_y[idx] += d_frac_y as f64;
                self.fract_z[idx] += d_frac_z as f64;
                self.cart_positions[idx][0] += translation.x;
                self.cart_positions[idx][1] += translation.y;
                self.cart_positions[idx][2] += translation.z;
            }
        }
        Ok(AtomTranslationRollback { atoms })
    }

    pub(crate) fn rollback_atom_translation(&mut self, rollback: AtomTranslationRollback) {
        for atom in rollback.atoms.into_iter().rev() {
            self.fract_x[atom.index] = atom.fractional[0];
            self.fract_y[atom.index] = atom.fractional[1];
            self.fract_z[atom.index] = atom.fractional[2];
            self.cart_positions[atom.index] = atom.cartesian;
        }
    }
    /// Generate a slab based on Miller indices and layers.
    /// Returns a new CrystalState representing the slab.
    pub fn generate_slab(
        &self,
        miller: [i32; 3],
        layers: i32,
        vacuum_a: f64,
    ) -> Result<Self, String> {
        let n_atoms = self.intrinsic_sites;
        if n_atoms == 0 {
            return Err("Cannot generate slab from empty crystal".to_string());
        }
        validate_slab_request(miller, layers, vacuum_a).map_err(str::to_string)?;
        let expected_upper = validate_slab_output_size(n_atoms, layers).map_err(str::to_string)?;
        validate_lattice_parameters(
            self.cell_a,
            self.cell_b,
            self.cell_c,
            self.cell_alpha,
            self.cell_beta,
            self.cell_gamma,
        )
        .map_err(str::to_string)?;

        if self.spacegroup_number == 1 {
            return Err(
                "Slab generation requires a conventional unit cell with symmetry \
                 (spacegroup ≠ P1). Miller indices (hkl) are defined relative to \
                 conventional axes. Please load or convert to a conventional cell first."
                    .to_string(),
            );
        }

        let lattice_col_major = self.get_lattice_col_major();

        let input_components = n_atoms
            .checked_mul(3)
            .ok_or_else(|| "slab input capacity overflow".to_string())?;
        let mut flat_positions = Vec::with_capacity(input_components);
        let mut types = Vec::with_capacity(n_atoms);
        for i in 0..n_atoms {
            flat_positions.push(self.fract_x[i]);
            flat_positions.push(self.fract_y[i]);
            flat_positions.push(self.fract_z[i]);
            types.push(self.atomic_numbers[i] as i32);
        }

        let n_upper = unsafe {
            ffi::get_slab_size_v2(lattice_col_major.as_ptr(), miller.as_ptr(), layers, n_atoms)
        };

        if n_upper <= 0 || n_upper as usize != expected_upper {
            return Err("Invalid slab size calculation".to_string());
        }

        let n_upper_usize = n_upper as usize;
        let output_components = n_upper_usize
            .checked_mul(3)
            .ok_or_else(|| "slab output capacity overflow".to_string())?;
        let mut out_lattice = [0.0f64; 9];
        let mut out_positions = vec![0.0f64; output_components];
        let mut out_types = vec![0i32; n_upper_usize];

        let n_actual = unsafe {
            ffi::build_slab_v2(
                lattice_col_major.as_ptr(),
                flat_positions.as_ptr(),
                types.as_ptr(),
                n_atoms,
                miller.as_ptr(),
                layers,
                vacuum_a,
                n_upper_usize,
                out_lattice.as_mut_ptr(),
                out_positions.as_mut_ptr(),
                out_types.as_mut_ptr(),
            )
        };

        if n_actual <= 0 || n_actual as usize > n_upper_usize {
            return Err("build_slab_v2 returned 0 atoms".to_string());
        }
        let n_actual_usize = n_actual as usize;

        let [new_a, new_b, new_c, new_alpha, new_beta, new_gamma] =
            lattice_parameters_from_col_major(&out_lattice).map_err(str::to_string)?;
        let type_sources = atomic_number_sources(&self.atomic_numbers[..n_atoms])
            .map_err(str::to_string)?;

        let mut new_state = CrystalState {
            name: format!(
                "{}_slab_{}_{}_{}",
                self.name, miller[0], miller[1], miller[2]
            ),
            cell_a: new_a,
            cell_b: new_b,
            cell_c: new_c,
            cell_alpha: new_alpha,
            cell_beta: new_beta,
            cell_gamma: new_gamma,
            spacegroup_hm: "P1".to_string(),
            spacegroup_number: 1,
            labels: Vec::with_capacity(n_actual_usize),
            elements: Vec::with_capacity(n_actual_usize),
            fract_x: Vec::with_capacity(n_actual_usize),
            fract_y: Vec::with_capacity(n_actual_usize),
            fract_z: Vec::with_capacity(n_actual_usize),
            occupancies: vec![1.0; n_actual_usize],
            atomic_numbers: Vec::with_capacity(n_actual_usize),
            cart_positions: Vec::new(),
            version: self.version,
            is_2d: false,
            vacuum_axis: None,
            bond_analysis: None,
            phonon_data: None,
            active_phonon_mode: None,
            phonon_phase: 0.0,
            intrinsic_sites: n_actual_usize,
            selected_atoms: Vec::new(),
            volumetric_data: None,
            bz_cache: None,
            wannier_overlay: None,
            measurements: Vec::new(),
        };

        for i in 0..n_actual_usize {
            let position = [
                out_positions[3 * i],
                out_positions[3 * i + 1],
                out_positions[3 * i + 2],
            ];
            validate_fractional_position(position).map_err(str::to_string)?;
            let t = u8::try_from(out_types[i])
                .map_err(|_| "FFI slab output contains an invalid atomic number".to_string())?;
            let source_index = type_sources[usize::from(t)]
                .ok_or_else(|| "FFI slab output contains an unknown atomic number".to_string())?;

            new_state.labels.push(self.labels[source_index].clone());
            new_state.elements.push(self.elements[source_index].clone());
            new_state.fract_x.push(position[0]);
            new_state.fract_y.push(position[1]);
            new_state.fract_z.push(position[2]);
            new_state.atomic_numbers.push(t);
        }

        new_state.fractional_to_cartesian();
        new_state
            .validate_cartesian_positions()
            .map_err(str::to_string)?;
        new_state.detect_spacegroup();

        Ok(new_state)
    }

    /// Shift slab termination to expose a different surface layer.
    /// Returns the number of detected layers for UI feedback.
    pub fn shift_termination(
        &mut self,
        target_layer_idx: i32,
        layer_tolerance_a: f64,
    ) -> Result<i32, String> {
        let n_atoms = self.intrinsic_sites;
        if n_atoms == 0 {
            return Err("Cannot shift termination of empty crystal".to_string());
        }

        let lattice_col_major = self.get_lattice_col_major();

        let mut flat_positions = Vec::with_capacity(n_atoms * 3);
        for i in 0..n_atoms {
            flat_positions.push(self.fract_x[i]);
            flat_positions.push(self.fract_y[i]);
            flat_positions.push(self.fract_z[i]);
        }

        let max_layers: usize = 128;
        let mut layer_centers = vec![0.0f64; max_layers];

        let n_layers = unsafe {
            ffi::cluster_slab_layers(
                flat_positions.as_ptr(),
                n_atoms,
                lattice_col_major.as_ptr(),
                layer_tolerance_a,
                layer_centers.as_mut_ptr(),
                max_layers,
            )
        };

        if target_layer_idx < 0 || target_layer_idx >= n_layers {
            return Err(format!(
                "Layer index {} out of range [0, {})",
                target_layer_idx, n_layers
            ));
        }

        unsafe {
            ffi::shift_slab_termination(
                flat_positions.as_mut_ptr(),
                n_atoms,
                lattice_col_major.as_ptr(),
                target_layer_idx,
                layer_centers.as_ptr(),
                n_layers,
            );
        }

        for i in 0..n_atoms {
            self.fract_x[i] = flat_positions[3 * i];
            self.fract_y[i] = flat_positions[3 * i + 1];
            self.fract_z[i] = flat_positions[3 * i + 2];
        }

        self.fractional_to_cartesian();

        Ok(n_layers)
    }

    /// Generate a supercell based on a 3x3 expansion matrix (ColMajor).
    pub fn generate_supercell(&self, expansion: &[i32; 9]) -> Result<Self, String> {
        let n_atoms = self.num_atoms();
        if n_atoms == 0 {
            return Err("Cannot generate supercell from empty crystal".to_string());
        }
        let expected_atoms =
            validate_supercell_request(expansion, n_atoms).map_err(str::to_string)?;
        validate_lattice_parameters(
            self.cell_a,
            self.cell_b,
            self.cell_c,
            self.cell_alpha,
            self.cell_beta,
            self.cell_gamma,
        )
        .map_err(str::to_string)?;

        let alpha = self.cell_alpha.to_radians();
        let beta = self.cell_beta.to_radians();
        let gamma = self.cell_gamma.to_radians();
        let a = self.cell_a;
        let b = self.cell_b;
        let c = self.cell_c;

        let cx = c * beta.cos();
        let cy = c * (alpha.cos() - beta.cos() * gamma.cos()) / gamma.sin();
        let cz = (c * c - cx * cx - cy * cy).sqrt();

        let lattice_col_major = [
            a,
            0.0,
            0.0,
            b * gamma.cos(),
            b * gamma.sin(),
            0.0,
            cx,
            cy,
            cz,
        ];

        let input_components = n_atoms
            .checked_mul(3)
            .ok_or_else(|| "supercell input capacity overflow".to_string())?;
        let mut flat_positions = Vec::with_capacity(input_components);
        let mut types = Vec::with_capacity(n_atoms);
        for i in 0..n_atoms {
            flat_positions.push(self.fract_x[i]);
            flat_positions.push(self.fract_y[i]);
            flat_positions.push(self.fract_z[i]);
            types.push(self.atomic_numbers[i] as i32);
        }

        let output_components = expected_atoms
            .checked_mul(3)
            .ok_or_else(|| "supercell output capacity overflow".to_string())?;
        let mut out_lattice = [0.0f64; 9];
        let mut out_positions = vec![0.0f64; output_components];
        let mut out_types = vec![0i32; expected_atoms];

        let n_new = unsafe {
            ffi::build_supercell_checked(
                lattice_col_major.as_ptr(),
                flat_positions.as_ptr(),
                types.as_ptr(),
                n_atoms,
                expansion.as_ptr(),
                expected_atoms,
                out_lattice.as_mut_ptr(),
                out_positions.as_mut_ptr(),
                out_types.as_mut_ptr(),
            )
        };
        if n_new <= 0 || n_new as usize != expected_atoms {
            return Err("supercell build returned an unexpected atom count".to_string());
        }
        let n_new_usize = n_new as usize;

        let [new_a, new_b, new_c, new_alpha, new_beta, new_gamma] =
            lattice_parameters_from_col_major(&out_lattice).map_err(str::to_string)?;
        let type_sources = atomic_number_sources(&self.atomic_numbers[..n_atoms])
            .map_err(str::to_string)?;

        let mut new_state = CrystalState {
            name: format!("{}_supercell", self.name),
            cell_a: new_a,
            cell_b: new_b,
            cell_c: new_c,
            cell_alpha: new_alpha,
            cell_beta: new_beta,
            cell_gamma: new_gamma,
            spacegroup_hm: "P1".to_string(),
            spacegroup_number: 1,
            labels: Vec::with_capacity(n_new_usize),
            elements: Vec::with_capacity(n_new_usize),
            fract_x: Vec::with_capacity(n_new_usize),
            fract_y: Vec::with_capacity(n_new_usize),
            fract_z: Vec::with_capacity(n_new_usize),
            occupancies: vec![1.0; n_new_usize],
            atomic_numbers: Vec::with_capacity(n_new_usize),
            cart_positions: Vec::new(),
            version: self.version,
            is_2d: false,
            vacuum_axis: None,
            bond_analysis: None,
            phonon_data: None,
            active_phonon_mode: None,
            phonon_phase: 0.0,
            intrinsic_sites: n_new_usize,
            selected_atoms: Vec::new(),
            volumetric_data: None,
            bz_cache: None,
            wannier_overlay: None,
            measurements: Vec::new(),
        };

        for i in 0..n_new_usize {
            let position = [
                out_positions[3 * i],
                out_positions[3 * i + 1],
                out_positions[3 * i + 2],
            ];
            validate_fractional_position(position).map_err(str::to_string)?;
            let t = u8::try_from(out_types[i]).map_err(|_| {
                "FFI supercell output contains an invalid atomic number".to_string()
            })?;
            let source_index = type_sources[usize::from(t)].ok_or_else(|| {
                "FFI supercell output contains an unknown atomic number".to_string()
            })?;

            new_state.labels.push(self.labels[source_index].clone());
            new_state.elements.push(self.elements[source_index].clone());
            new_state.fract_x.push(position[0]);
            new_state.fract_y.push(position[1]);
            new_state.fract_z.push(position[2]);
            new_state.atomic_numbers.push(t);
        }

        new_state.fractional_to_cartesian();
        new_state
            .validate_cartesian_positions()
            .map_err(str::to_string)?;
        new_state.detect_spacegroup();

        Ok(new_state)
    }

    /// Add a new atom to the crystal, checking for collisions first
    pub fn try_add_atom(
        &mut self,
        element_symbol: &str,
        atomic_number: u8,
        fract_pos: [f64; 3],
    ) -> Result<(), CollisionError> {
        validate_atom_request(element_symbol, atomic_number, fract_pos, self.num_atoms())
            .map_err(|_| CollisionError)?;
        let n_intrinsic = self.intrinsic_sites;
        if n_intrinsic > 0 {
            validate_lattice_parameters(
                self.cell_a,
                self.cell_b,
                self.cell_c,
                self.cell_alpha,
                self.cell_beta,
                self.cell_gamma,
            )
            .map_err(|_| CollisionError)?;
            let lattice_col_major = self.get_lattice_col_major();
            let intrinsic_components = n_intrinsic.checked_mul(3).ok_or(CollisionError)?;
            let mut flat_intrinsic = Vec::with_capacity(intrinsic_components);
            for i in 0..n_intrinsic {
                flat_intrinsic.push(self.fract_x[i]);
                flat_intrinsic.push(self.fract_y[i]);
                flat_intrinsic.push(self.fract_z[i]);
            }

            let is_overlap = unsafe {
                ffi::check_overlap_mic(
                    lattice_col_major.as_ptr(),
                    flat_intrinsic.as_ptr(),
                    n_intrinsic,
                    fract_pos.as_ptr(),
                    0.5, // 0.5Å threshold
                )
            };

            if is_overlap {
                return Err(CollisionError);
            }
        }

        let label = format!("{}{}", element_symbol, self.num_atoms() + 1);
        self.labels.push(label);
        self.elements.push(element_symbol.to_string());
        self.fract_x.push(fract_pos[0]);
        self.fract_y.push(fract_pos[1]);
        self.fract_z.push(fract_pos[2]);
        self.occupancies.push(1.0);
        self.atomic_numbers.push(atomic_number);
        self.intrinsic_sites += 1;
        self.fractional_to_cartesian();

        Ok(())
    }

    /// Delete atoms by their indices
    pub fn delete_atoms(&mut self, indices: &[usize]) {
        let mut sorted_indices = indices.to_vec();
        sorted_indices.sort_unstable();
        sorted_indices.dedup();

        self.delete_atoms_sorted_unique(&sorted_indices);
    }

    pub(crate) fn delete_atoms_sorted_unique(&mut self, indices: &[usize]) {
        for &idx in indices.iter().rev() {
            if idx < self.num_atoms() {
                self.labels.remove(idx);
                self.elements.remove(idx);
                self.fract_x.remove(idx);
                self.fract_y.remove(idx);
                self.fract_z.remove(idx);
                self.occupancies.remove(idx);
                self.atomic_numbers.remove(idx);
            }
        }
        self.intrinsic_sites = self.num_atoms();
        self.fractional_to_cartesian();
    }

    /// Substitute atoms by their indices with a new element
    pub fn substitute_atoms(
        &mut self,
        indices: &[usize],
        new_element_symbol: &str,
        new_atomic_number: u8,
    ) {
        for &idx in indices {
            if idx < self.num_atoms() {
                self.labels[idx] = format!("{}{}", new_element_symbol, idx + 1);
                self.elements[idx] = new_element_symbol.to_string();
                self.atomic_numbers[idx] = new_atomic_number;
            }
        }
    }

    // =====================================================================
    // Bond Analysis (M10)
    // =====================================================================

    /// Compute bond analysis: find all bonds and per-atom coordination shells.
    /// Requires `cart_positions` to be populated first.
    pub fn compute_bond_analysis(&mut self, threshold_factor: f64) {
        let n = self.intrinsic_sites;
        if n == 0 {
            self.bond_analysis = Some(BondAnalysis::default());
            return;
        }

        // Prepare flat arrays for C++ kernel
        // We ONLY use intrinsic atoms because MIC handles periodicity.
        // Mirroring for visual continuity should be ignored by physics.
        let mut flat_cart = Vec::with_capacity(n * 3);
        let mut flat_frac = Vec::with_capacity(n * 3);
        let mut covalent_radii = Vec::with_capacity(n);

        for i in 0..n {
            let pos = self.cart_positions[i];
            flat_cart.push(pos[0] as f64);
            flat_cart.push(pos[1] as f64);
            flat_cart.push(pos[2] as f64);

            flat_frac.push(self.fract_x[i]);
            flat_frac.push(self.fract_y[i]);
            flat_frac.push(self.fract_z[i]);
            
            covalent_radii.push(covalent_radius(self.atomic_numbers[i]) as f64);
        }

        // Flatten the 3x3 lattice in column-major
        let lattice_col_major = self.get_lattice_col_major();

        let min_bond_length = 0.4; // Angstroms
        let max_bonds = n * n; // Upper bound (for safety)
        let max_bonds = max_bonds.min(100_000); // Hard cap

        let mut out_i = vec![0i32; max_bonds];
        let mut out_j = vec![0i32; max_bonds];
        let mut out_dist = vec![0.0f64; max_bonds];

        let bond_count = unsafe {
            ffi::compute_bonds(
                lattice_col_major.as_ptr(),
                flat_cart.as_ptr(),
                flat_frac.as_ptr(),
                covalent_radii.as_ptr(),
                n,
                threshold_factor,
                min_bond_length,
                out_i.as_mut_ptr(),
                out_j.as_mut_ptr(),
                out_dist.as_mut_ptr(),
                max_bonds,
            )
        };

        let bond_count = bond_count.max(0) as usize;
        let mut bonds = Vec::with_capacity(bond_count);
        for k in 0..bond_count {
            bonds.push(BondInfo {
                atom_i: out_i[k] as usize,
                atom_j: out_j[k] as usize,
                distance: out_dist[k],
            });
        }

        // Per-atom coordination
        let max_neighbors: usize = 24; // Enough for most coordination shells
        let mut coordination = Vec::with_capacity(n);
        for i in 0..n {
            let mut neigh_idx = vec![0i32; max_neighbors];
            let mut neigh_dist = vec![0.0f64; max_neighbors];

            let cn = unsafe {
                ffi::find_coordination_shell(
                    lattice_col_major.as_ptr(),
                    flat_cart.as_ptr(),
                    flat_frac.as_ptr(),
                    covalent_radii.as_ptr(),
                    n,
                    i,
                    threshold_factor,
                    min_bond_length,
                    neigh_idx.as_mut_ptr(),
                    neigh_dist.as_mut_ptr(),
                    max_neighbors,
                )
            };

            let cn = cn.max(0) as usize;
            let neighbor_indices: Vec<usize> =
                neigh_idx[..cn].iter().map(|&v| v as usize).collect();
            let neighbor_distances: Vec<f64> = neigh_dist[..cn].to_vec();

            let poly_type = classify_polyhedron(cn);

            coordination.push(CoordinationInfo {
                center_idx: i,
                element: self.elements[i].clone(),
                coordination_number: cn,
                neighbor_indices,
                neighbor_distances,
                polyhedron_type: poly_type.to_string(),
            });
        }

        self.bond_analysis = Some(BondAnalysis {
            bonds,
            coordination,
            threshold_factor,
        });
    }

    /// Detect if the system is 2D and set `is_2d` and `vacuum_axis` accordingly.
    pub fn detect_2d(&mut self) {
        let mut best_axis = None;
        let mut max_gap = 0.0;
        let mut best_ratio = 0.0;
        
        let axes = [0, 1, 2];
        let lengths = [self.cell_a, self.cell_b, self.cell_c];
        
        for &axis in &axes {
            let mut coords = Vec::with_capacity(self.intrinsic_sites);
            for i in 0..self.intrinsic_sites {
                let mut v = match axis {
                    0 => self.fract_x[i],
                    1 => self.fract_y[i],
                    2 => self.fract_z[i],
                    _ => unreachable!(),
                };
                v = v - v.floor();
                coords.push(v);
            }
            
            if coords.is_empty() {
                continue;
            }
            coords.sort_by(|a, b| a.total_cmp(b));
            
            let mut current_max_gap = 0.0;
            let n = coords.len();
            for i in 0..(n-1) {
                let gap = coords[i+1] - coords[i];
                if gap > current_max_gap {
                    current_max_gap = gap;
                }
            }
            // wraparound gap
            let wrap_gap = 1.0 - coords[n-1] + coords[0];
            if wrap_gap > current_max_gap {
                current_max_gap = wrap_gap;
            }
            
            let other_len = (lengths[(axis + 1) % 3] + lengths[(axis + 2) % 3]) / 2.0;
            if other_len > 0.0 {
                let ratio = lengths[axis] / other_len;
                if current_max_gap > 0.35 && ratio > 2.0 {
                    if current_max_gap > max_gap {
                        max_gap = current_max_gap;
                        best_axis = Some(axis);
                        best_ratio = ratio;
                    }
                }
            }
        }
        
        if let Some(axis) = best_axis {
            self.is_2d = true;
            self.vacuum_axis = Some(axis);
        } else {
            self.is_2d = false;
            self.vacuum_axis = None;
        }
        
        log::info!(
            "[detect_2d] is_2d={}, vacuum_axis={:?}, max_gap={:.3}, ratio={:.2}",
            self.is_2d,
            self.vacuum_axis,
            max_gap,
            best_ratio
        );
    }
    
    /// Get the two real-space lattice vectors spanning the periodic plane.
    pub fn get_inplane_lattice(&self) -> ([f64; 3], [f64; 3]) {
        let lattice = self.get_lattice_col_major(); 
        let v1 = [lattice[0], lattice[1], lattice[2]];
        let v2 = [lattice[3], lattice[4], lattice[5]];
        let v3 = [lattice[6], lattice[7], lattice[8]];
        
        if let Some(axis) = self.vacuum_axis {
            match axis {
                0 => (v2, v3),
                1 => (v1, v3),
                _ => (v1, v2),
            }
        } else {
            (v1, v2)
        }
    }

    pub fn measure_distance(&self, i: usize, j: usize) -> Result<f64, String> {
        if i >= self.cart_positions.len() || j >= self.cart_positions.len() {
            return Err("Atom index out of bounds".to_string());
        }
        let pi = self.cart_positions[i];
        let pj = self.cart_positions[j];
        let dx = (pi[0] as f64) - (pj[0] as f64);
        let dy = (pi[1] as f64) - (pj[1] as f64);
        let dz = (pi[2] as f64) - (pj[2] as f64);
        Ok((dx * dx + dy * dy + dz * dz).sqrt())
    }

    pub fn measure_angle(&self, i: usize, j: usize, k: usize) -> Result<f64, String> {
        if i >= self.cart_positions.len()
            || j >= self.cart_positions.len()
            || k >= self.cart_positions.len()
        {
            return Err("Atom index out of bounds".to_string());
        }
        let pi = self.cart_positions[i];
        let pj = self.cart_positions[j];
        let pk = self.cart_positions[k];
        
        let v1 = glam::DVec3::new(
            (pi[0] as f64) - (pj[0] as f64),
            (pi[1] as f64) - (pj[1] as f64),
            (pi[2] as f64) - (pj[2] as f64),
        )
        .normalize_or_zero();
        let v2 = glam::DVec3::new(
            (pk[0] as f64) - (pj[0] as f64),
            (pk[1] as f64) - (pj[1] as f64),
            (pk[2] as f64) - (pj[2] as f64),
        )
        .normalize_or_zero();
        
        let dot = v1.dot(v2).clamp(-1.0, 1.0);
        Ok(dot.acos().to_degrees())
    }

    pub fn measure_dihedral(&self, i: usize, j: usize, k: usize, l: usize) -> Result<f64, String> {
        if i >= self.cart_positions.len()
            || j >= self.cart_positions.len()
            || k >= self.cart_positions.len()
            || l >= self.cart_positions.len()
        {
            return Err("Atom index out of bounds".to_string());
        }
        let pi = self.cart_positions[i];
        let pj = self.cart_positions[j];
        let pk = self.cart_positions[k];
        let pl = self.cart_positions[l];
        
        let b1 = glam::DVec3::new(
            (pj[0] as f64) - (pi[0] as f64),
            (pj[1] as f64) - (pi[1] as f64),
            (pj[2] as f64) - (pi[2] as f64),
        );
        let b2 = glam::DVec3::new(
            (pk[0] as f64) - (pj[0] as f64),
            (pk[1] as f64) - (pj[1] as f64),
            (pk[2] as f64) - (pj[2] as f64),
        );
        let b3 = glam::DVec3::new(
            (pl[0] as f64) - (pk[0] as f64),
            (pl[1] as f64) - (pk[1] as f64),
            (pl[2] as f64) - (pk[2] as f64),
        );
        
        let n1 = b1.cross(b2).normalize_or_zero();
        let n2 = b2.cross(b3).normalize_or_zero();
        let m = n1.cross(b2.normalize_or_zero());
        
        let x = n1.dot(n2);
        let y = m.dot(n2);
        Ok(y.atan2(x).to_degrees())
    }

    pub fn add_measurement(&mut self, indices: &[usize]) -> Result<MeasurementOverlay, String> {
        let (kind, value, pos) = match indices.len() {
            2 => {
                let d = self.measure_distance(indices[0], indices[1])?;
                let pi = glam::Vec3::from_array(self.cart_positions[indices[0]]);
                let pj = glam::Vec3::from_array(self.cart_positions[indices[1]]);
                (MeasurementKind::Distance, d, ((pi + pj) * 0.5).into())
            }
            3 => {
                let a = self.measure_angle(indices[0], indices[1], indices[2])?;
                let p0 = glam::Vec3::from_array(self.cart_positions[indices[0]]);
                let p1 = glam::Vec3::from_array(self.cart_positions[indices[1]]);
                let p2 = glam::Vec3::from_array(self.cart_positions[indices[2]]);
                // Place label slightly displaced from the central vertex along the bisector
                let v1 = (p0 - p1).normalize_or_zero();
                let v2 = (p2 - p1).normalize_or_zero();
                let mut bisector = (v1 + v2).normalize_or_zero();
                if bisector.length_squared() < 1e-6 {
                    // Fallback for 180 degree angle (collinear)
                    bisector = glam::Vec3::new(v1.y, -v1.x, 0.0); // Arbitrary orthogonal vector
                    if bisector.length_squared() < 1e-6 {
                        bisector = glam::Vec3::new(0.0, 0.0, 1.0);
                    }
                    bisector = bisector.normalize_or_zero();
                }
                (MeasurementKind::Angle, a, (p1 + bisector * 1.5).into())
            }
            4 => {
                let d = self.measure_dihedral(indices[0], indices[1], indices[2], indices[3])?;
                let pj = glam::Vec3::from_array(self.cart_positions[indices[1]]);
                let pk = glam::Vec3::from_array(self.cart_positions[indices[2]]);
                // Midpoint of central bond
                (MeasurementKind::Dihedral, d, ((pj + pk) * 0.5).into())
            }
            _ => return Err("Measurement must contain 2, 3, or 4 atoms".to_string()),
        };

        let overlay = MeasurementOverlay {
            indices: indices.to_vec(),
            kind,
            value,
            label_position: pos,
        };
        self.measurements.push(overlay.clone());
        Ok(overlay)
    }

    pub fn clear_measurements(&mut self) {
        self.measurements.clear();
    }
}

/// Classify polyhedron type from coordination number.
fn classify_polyhedron(cn: usize) -> &'static str {
    match cn {
        2 => "Linear",
        3 => "Trigonal Planar",
        4 => "Tetrahedron",
        5 => "Trigonal Bipyramid",
        6 => "Octahedron",
        8 => "Cube",
        12 => "Cuboctahedron",
        _ => "",
    }
}

// =========================================================================
// Bond Statistics (M10 Node 3)
// =========================================================================

/// Bond length statistics for a specific element pair (e.g., Ti-O).
#[derive(Clone, Debug, Serialize)]
pub struct BondLengthStat {
    pub element_a: String,
    pub element_b: String,
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
}

impl BondAnalysis {
    /// Compute per-element-pair bond length statistics.
    pub fn bond_length_stats(&self, elements: &[String]) -> Vec<BondLengthStat> {
        use std::collections::HashMap;

        let mut groups: HashMap<(String, String), Vec<f64>> = HashMap::new();

        for bond in &self.bonds {
            let mut ea = elements[bond.atom_i].clone();
            let mut eb = elements[bond.atom_j].clone();
            // Canonical key: alphabetical order
            if ea > eb {
                std::mem::swap(&mut ea, &mut eb);
            }
            groups.entry((ea, eb)).or_default().push(bond.distance);
        }

        let mut stats: Vec<BondLengthStat> = groups
            .into_iter()
            .map(|((ea, eb), dists)| {
                let count = dists.len();
                let min = dists.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = dists.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let mean = dists.iter().sum::<f64>() / count as f64;
                BondLengthStat {
                    element_a: ea,
                    element_b: eb,
                    count,
                    min,
                    max,
                    mean,
                }
            })
            .collect();

        stats.sort_by(|a, b| {
            a.element_a
                .cmp(&b.element_a)
                .then(a.element_b.cmp(&b.element_b))
        });
        stats
    }

    /// Compute polyhedral distortion index for a coordination environment.
    /// Δ = (1/n) Σ |dᵢ − d̄| / d̄
    /// Returns 0.0 for perfect polyhedra, increases with distortion.
    pub fn distortion_index(coord: &CoordinationInfo) -> f64 {
        let n = coord.neighbor_distances.len();
        if n == 0 {
            return 0.0;
        }
        let d_mean = coord.neighbor_distances.iter().sum::<f64>() / n as f64;
        if d_mean < 1e-10 {
            return 0.0;
        }
        let delta: f64 = coord
            .neighbor_distances
            .iter()
            .map(|d| (d - d_mean).abs() / d_mean)
            .sum::<f64>()
            / n as f64;
        delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_crystal() -> CrystalState {
        let mut state = CrystalState {
            name: "Test".to_string(),
            cell_a: 1.0,
            cell_b: 1.0,
            cell_c: 1.0,
            cell_alpha: 90.0,
            cell_beta: 90.0,
            cell_gamma: 90.0,
            spacegroup_hm: "P1".to_string(),
            spacegroup_number: 1,
            labels: vec!["H1".to_string(), "O1".to_string()],
            elements: vec!["H".to_string(), "O".to_string()],
            fract_x: vec![0.0, 0.5],
            fract_y: vec![0.0, 0.5],
            fract_z: vec![0.0, 0.5],
            occupancies: vec![1.0, 1.0],
            atomic_numbers: vec![1, 8],
            cart_positions: vec![],
            version: 1,
            is_2d: false,
            vacuum_axis: None,
            bond_analysis: None,
            phonon_data: None,
            active_phonon_mode: None,
            phonon_phase: 0.0,
            intrinsic_sites: 2,
            selected_atoms: vec![],
            volumetric_data: None,
            bz_cache: None,
            wannier_overlay: None,
            measurements: Vec::new(),
        };
        state.fractional_to_cartesian();
        state
    }

    #[test]
    fn test_try_add_atom() {
        let mut c = dummy_crystal();
        let initial_version = c.version;
        // Change cell to 5.0 to safely add atoms
        c.cell_a = 5.0;
        c.cell_b = 5.0;
        c.cell_c = 5.0;
        let res = c.try_add_atom("C", 6, [0.25, 0.25, 0.25]);
        assert!(res.is_ok(), "Should be added successfully");
        assert_eq!(c.num_atoms(), 3, "Should have 3 atoms");
        assert_eq!(c.labels[2], "C3", "Label should be C3");
        assert_eq!(c.elements[2], "C", "Element should be C");
        assert_eq!(c.atomic_numbers[2], 6, "Atomic number should be 6");
        assert_eq!(
            c.version,
            initial_version,
            "Low-level mutation must not commit the state version"
        );
        assert_eq!(
            c.cart_positions.len(),
            3,
            "Cartesian positions should be updated"
        );
    }

    #[test]
    fn test_delete_atoms() {
        let mut c = dummy_crystal();
        let initial_version = c.version;
        c.delete_atoms(&[0]); // Delete H1
        assert_eq!(c.num_atoms(), 1, "Should have 1 atom remaining");
        assert_eq!(c.labels[0], "O1", "Remaining atom should be O1");
        assert_eq!(
            c.version,
            initial_version,
            "Low-level mutation must not commit the state version"
        );
        assert_eq!(
            c.cart_positions.len(),
            1,
            "Cartesian positions should be updated"
        );

        // Delete out of bounds should be safe
        c.delete_atoms(&[5]);
        assert_eq!(c.num_atoms(), 1);
    }

    #[test]
    fn test_substitute_atoms() {
        let mut c = dummy_crystal();
        let initial_version = c.version;
        c.substitute_atoms(&[1], "S", 16); // Substitute O1 with S
        assert_eq!(c.num_atoms(), 2, "Should still have 2 atoms");
        assert_eq!(c.labels[1], "S2", "Label should be S2");
        assert_eq!(c.elements[1], "S", "Element should be S");
        assert_eq!(c.atomic_numbers[1], 16, "Atomic number should be 16");
        assert_eq!(
            c.version,
            initial_version,
            "Low-level mutation must not commit the state version"
        );
    }

    #[test]
    fn test_compute_bond_analysis() {
        let mut c = dummy_crystal();
        // Move atoms close together to form a bond
        c.cell_a = 5.0;
        c.cell_b = 5.0;
        c.cell_c = 5.0;
        c.fract_x = vec![0.0, 0.1]; // distance is 0.5 Angstroms
        c.fract_y = vec![0.0, 0.0];
        c.fract_z = vec![0.0, 0.0];
        c.fractional_to_cartesian();
        
        c.compute_bond_analysis(1.2);
        let analysis = c.bond_analysis.as_ref().unwrap();
        // Since H and O are close, there should be 1 bond
        assert_eq!(analysis.bonds.len(), 1, "Should detect 1 bond");
        
        assert_eq!(
            analysis.coordination.len(),
            2,
            "Should have 2 coordination shells"
        );
        assert_eq!(analysis.coordination[0].coordination_number, 1);
        assert_eq!(analysis.coordination[1].coordination_number, 1);
    }

    #[test]
    fn test_distortion_index() {
        let coord = CoordinationInfo {
            center_idx: 0,
            element: "Ti".to_string(),
            coordination_number: 6,
            neighbor_indices: vec![1, 2, 3, 4, 5, 6],
            neighbor_distances: vec![2.0, 2.0, 2.0, 2.0, 2.0, 2.0], // Perfect octahedron
            polyhedron_type: "Octahedron".to_string(),
        };
        
        let delta = BondAnalysis::distortion_index(&coord);
        assert!(
            (delta - 0.0).abs() < 1e-10,
            "Perfect octahedron should have 0 distortion"
        );

        let distorted_coord = CoordinationInfo {
            center_idx: 0,
            element: "Ti".to_string(),
            coordination_number: 6,
            neighbor_indices: vec![1, 2, 3, 4, 5, 6],
            neighbor_distances: vec![1.9, 2.1, 1.9, 2.1, 1.9, 2.1], // Distorted
            polyhedron_type: "Octahedron".to_string(),
        };
        
        let delta_distorted = BondAnalysis::distortion_index(&distorted_coord);
        // mean is 2.0, |d - d_mean| is 0.1 for all
        // Delta = (1/6) * (6 * 0.1 / 2.0) = 0.05
        assert!(
            (delta_distorted - 0.05).abs() < 1e-10,
            "Distortion should be exactly 0.05"
        );
    }

    #[test]
    fn test_distance_measurement() {
        let mut cs = CrystalState::default();
        // T-1 & T-6: Exactly representable f32 coordinates and tight precision
        cs.cart_positions = vec![[1.0, 2.0, 3.0], [4.5, -0.5, 1.5]];
        
        let dist = cs.measure_distance(0, 1).unwrap();
        // dx=3.5, dy=-2.5, dz=-1.5. sum_sq = 12.25 + 6.25 + 2.25 = 20.75
        let expected = 20.75f64.sqrt();
        assert!(
            (dist - expected).abs() < 1e-12,
            "Distance precision failure on bit-exact coords"
        );
        
        // Zero distance
        let dist0 = cs.measure_distance(0, 0).unwrap();
        assert!(dist0.abs() < f64::EPSILON);
    }

    #[test]
    fn test_angle_measurement() {
        let mut cs = CrystalState::default();
        // T-2: Non-special angle (60 degrees)
        let sqrt3_2 = (3.0f32.sqrt() / 2.0) as f32;
        cs.cart_positions = vec![
            [1.0, 0.0, 0.0],       // 0: i
            [0.0, 0.0, 0.0],       // 1: j (vertex)
            [0.5, sqrt3_2, 0.0],   // 2: k (60 deg from i)
            [0.0, 0.0, 0.0],       // 3: degenerate overlapping vertex
        ];
        
        let angle60 = cs.measure_angle(0, 1, 2).unwrap();
        assert!(
            (angle60 - 60.0).abs() < 1e-5,
            "Angle precision failure on 60 degree case"
        );
        
        // Collinear 180
        cs.cart_positions.push([-1.0, 0.0, 0.0]);
        let angle180 = cs.measure_angle(0, 1, 4).unwrap();
        assert!((angle180 - 180.0).abs() < 1e-5);

        // T-5: Boundary safety
        let angle_degen = cs.measure_angle(0, 1, 3).unwrap();
        assert!(angle_degen.is_finite());
    }

    #[test]
    fn test_dihedral_measurement() {
        let mut cs = CrystalState::default();
        // T-3: Sign-sensitive check. i-j-k-l twist.
        // i: (1,1,0), j: (0,0,0), k: (0,0,1), l: (1,0,1)
        // This configuration has a specific 45 degree twist.
        cs.cart_positions = vec![
            [1.0, 1.0, 0.0], // 0: i
            [0.0, 0.0, 0.0], // 1: j
            [0.0, 0.0, 1.0], // 2: k
            [1.0, 0.0, 1.0], // 3: l
        ];
        
        let dh = cs.measure_dihedral(0, 1, 2, 3).unwrap();
        // Hand calculate: b1=(-1,-1,0), b2=(0,0,1), b3=(1,0,0)
        // n1 = b1 x b2 = (-1, 1, 0), n2 = b2 x b3 = (0, 1, 0)
        // cos(phi) = n1.n2 / (|n1||n2|) = 1 / sqrt(2) -> 45 deg
        // Verify exact sign (+45.0)
        assert!(
            (dh - 45.0).abs() < 1e-5,
            "Dihedral sign or value failure: expected +45.0, got {}",
            dh
        );
        
        // Breaker: Collinear backbone (j, k, l collinear)
        cs.cart_positions.push([0.0, 0.0, 2.0]); // 4: k'
        cs.cart_positions.push([0.0, 0.0, 3.0]); // 5: l'
        let dh_degen = cs.measure_dihedral(0, 1, 4, 5).unwrap();
        assert!(dh_degen.is_finite());
    }

    #[test]
    fn test_add_measurement_logic() {
        let mut cs = CrystalState::default();
        // T-4: Verify label positions and overlay construction
        cs.cart_positions = vec![
            [0.0, 0.0, 0.0], // 0
            [2.0, 0.0, 0.0], // 1
            [2.0, 2.0, 0.0], // 2
        ];

        // Distance label at midpoint
        let m2 = cs.add_measurement(&[0, 1]).unwrap();
        assert_eq!(m2.kind, MeasurementKind::Distance);
        assert!((m2.label_position[0] - 1.0).abs() < 1e-5);
        assert!(m2.label_position[1].abs() < 1e-5);

        // Angle label displacement
        let m3 = cs.add_measurement(&[0, 1, 2]).unwrap();
        assert_eq!(m3.kind, MeasurementKind::Angle);
        // vertex is (2,0,0). v1=(-1,0,0), v2=(0,1,0). bisector=(-1,1,0)/sqrt(2)
        // pos = vertex + bisector * 1.5
        let expected_x = 2.0 - 1.5 / 2.0f32.sqrt();
        let expected_y = 0.0 + 1.5 / 2.0f32.sqrt();
        assert!((m3.label_position[0] - expected_x).abs() < 1e-5);
        assert!((m3.label_position[1] - expected_y).abs() < 1e-5);
    }

    #[test]
    fn test_measure_bounds_and_empty() {
        // T-5: Empty and OOB safety
        let cs = CrystalState::default();
        assert!(cs.measure_distance(0, 0).is_err());
        assert!(cs.measure_angle(0, 1, 2).is_err());
        assert!(cs.measure_dihedral(0, 1, 2, 3).is_err());
    }
}
