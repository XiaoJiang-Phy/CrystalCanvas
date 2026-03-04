//! Core crystal state — Single Source of Truth (SSoT) with SoA layout for physics and rendering
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::ffi;
use crate::renderer::instance::covalent_radius;
use serde::Serialize;

/// Error returned when trying to add an overlapping atom
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionError;

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
            bond_analysis: None,
            phonon_data: None,
            active_phonon_mode: None,
            phonon_phase: 0.0,
            intrinsic_sites: 0,
            selected_atoms: Vec::new(),
        }
    }
}

impl CrystalState {
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
            bond_analysis: None,
            phonon_data: None,
            active_phonon_mode: None,
            phonon_phase: 0.0,
            intrinsic_sites: actual_n,
            selected_atoms: Vec::new(),
        };

        state.fractional_to_cartesian();
        state.detect_spacegroup();
        state
    }

    /// Duplicate atoms residing on the fractional boundaries (0.0 or 1.0) for visual continuity
    pub fn apply_boundary_mirroring(&mut self) {
        if self.num_atoms() == 0 {
            return;
        }
        let eps = 1e-4;
        let mut new_labels = Vec::new();
        let mut new_elements = Vec::new();
        let mut new_fract_x = Vec::new();
        let mut new_fract_y = Vec::new();
        let mut new_fract_z = Vec::new();
        let mut new_occupancies = Vec::new();
        let mut new_atomic_numbers = Vec::new();

        for i in 0..self.num_atoms() {
            let x = self.fract_x[i];
            let y = self.fract_y[i];
            let z = self.fract_z[i];

            for dx in 0..=1 {
                for dy in 0..=1 {
                    for dz in 0..=1 {
                        if dx == 0 && dy == 0 && dz == 0 {
                            continue;
                        }

                        let mut add = true;
                        let mut nx = x;
                        let mut ny = y;
                        let mut nz = z;

                        if dx == 1 {
                            if x.abs() < eps || (x - 1.0).abs() < eps {
                                nx = if x.abs() < eps { x + 1.0 } else { x - 1.0 };
                            } else {
                                add = false;
                            }
                        }
                        if dy == 1 {
                            if y.abs() < eps || (y - 1.0).abs() < eps {
                                ny = if y.abs() < eps { y + 1.0 } else { y - 1.0 };
                            } else {
                                add = false;
                            }
                        }
                        if dz == 1 {
                            if z.abs() < eps || (z - 1.0).abs() < eps {
                                nz = if z.abs() < eps { z + 1.0 } else { z - 1.0 };
                            } else {
                                add = false;
                            }
                        }

                        if add {
                            // Only add if not already in the list
                            let mut exists = false;
                            for j in 0..self.num_atoms() {
                                if (self.fract_x[j] - nx).abs() < eps
                                    && (self.fract_y[j] - ny).abs() < eps
                                    && (self.fract_z[j] - nz).abs() < eps
                                {
                                    exists = true;
                                    break;
                                }
                            }
                            if !exists {
                                new_labels.push(self.labels[i].clone());
                                new_elements.push(self.elements[i].clone());
                                new_fract_x.push(nx);
                                new_fract_y.push(ny);
                                new_fract_z.push(nz);
                                new_occupancies.push(self.occupancies[i]);
                                new_atomic_numbers.push(self.atomic_numbers[i]);
                            }
                        }
                    }
                }
            }
        }

        self.labels.extend(new_labels);
        self.elements.extend(new_elements);
        self.fract_x.extend(new_fract_x);
        self.fract_y.extend(new_fract_y);
        self.fract_z.extend(new_fract_z);
        self.occupancies.extend(new_occupancies);
        self.atomic_numbers.extend(new_atomic_numbers);
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
            self.spacegroup_hm = format!("Spglib #{}", sg);
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
    /// Generate a slab based on Miller indices and layers.
    /// Returns a new CrystalState representing the slab.
    pub fn generate_slab(
        &self,
        miller: [i32; 3],
        layers: i32,
        vacuum_a: f64,
    ) -> Result<Self, String> {
        let n_atoms = self.num_atoms();
        if n_atoms == 0 {
            return Err("Cannot generate slab from empty crystal".to_string());
        }

        // We will construct the true 3x3 lattice in column-major properly
        // PDB convention: a along x, b in xy plane, c in xyz
        let alpha = self.cell_alpha.to_radians();
        let beta = self.cell_beta.to_radians();
        let gamma = self.cell_gamma.to_radians();
        let a = self.cell_a;
        let b = self.cell_b;
        let c = self.cell_c;

        let cx = c * beta.cos();
        let cy = c * (alpha.cos() - beta.cos() * gamma.cos()) / gamma.sin();
        let cz = (c * c - cx * cx - cy * cy).sqrt();

        // Eigen uses Column-Major! So we pack col 0, col 1, col 2
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

        // Prepare flat positions
        let mut flat_positions = Vec::with_capacity(n_atoms * 3);
        let mut types = Vec::with_capacity(n_atoms);
        for i in 0..n_atoms {
            flat_positions.push(self.fract_x[i]);
            flat_positions.push(self.fract_y[i]);
            flat_positions.push(self.fract_z[i]);
            // Temporarily use atomic_number cast to i32 as "type"
            types.push(self.atomic_numbers[i] as i32);
        }

        let n_new = unsafe {
            ffi::get_slab_size(
                lattice_col_major.as_ptr(),
                miller.as_ptr(),
                layers,
                vacuum_a,
                n_atoms,
            )
        };

        if n_new <= 0 {
            return Err("Invalid slab size calculation".to_string());
        }

        let n_new_usize = n_new as usize;
        let mut out_lattice = vec![0.0f64; 9];
        let mut out_positions = vec![0.0f64; n_new_usize * 3];
        let mut out_types = vec![0i32; n_new_usize];

        unsafe {
            ffi::build_slab(
                lattice_col_major.as_ptr(),
                flat_positions.as_ptr(),
                types.as_ptr(),
                n_atoms,
                miller.as_ptr(),
                layers,
                vacuum_a,
                out_lattice.as_mut_ptr(),
                out_positions.as_mut_ptr(),
                out_types.as_mut_ptr(),
            );
        }

        // Reconstruct new lattice parameters from the 3x3 out_lattice
        // out_lattice is Column-Major:
        // [vx_x, vx_y, vx_z, vy_x, vy_y, vy_z, vz_x, vz_y, vz_z]
        let vx = [out_lattice[0], out_lattice[1], out_lattice[2]];
        let vy = [out_lattice[3], out_lattice[4], out_lattice[5]];
        let vz = [out_lattice[6], out_lattice[7], out_lattice[8]];

        // length
        let new_a = (vx[0] * vx[0] + vx[1] * vx[1] + vx[2] * vx[2]).sqrt();
        let new_b = (vy[0] * vy[0] + vy[1] * vy[1] + vy[2] * vy[2]).sqrt();
        let new_c = (vz[0] * vz[0] + vz[1] * vz[1] + vz[2] * vz[2]).sqrt();

        // angles (dot products)
        let dot_ab = vx[0] * vy[0] + vx[1] * vy[1] + vx[2] * vy[2];
        let dot_bc = vy[0] * vz[0] + vy[1] * vz[1] + vy[2] * vz[2];
        let dot_ca = vz[0] * vx[0] + vz[1] * vx[1] + vz[2] * vx[2];

        let new_gamma = (dot_ab / (new_a * new_b)).acos().to_degrees();
        let new_alpha = (dot_bc / (new_b * new_c)).acos().to_degrees();
        let new_beta = (dot_ca / (new_c * new_a)).acos().to_degrees();

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
            spacegroup_hm: "P1".to_string(), // slabs typically break symmetry
            spacegroup_number: 1,
            labels: Vec::with_capacity(n_new_usize),
            elements: Vec::with_capacity(n_new_usize),
            fract_x: Vec::with_capacity(n_new_usize),
            fract_y: Vec::with_capacity(n_new_usize),
            fract_z: Vec::with_capacity(n_new_usize),
            occupancies: vec![1.0; n_new_usize],
            atomic_numbers: Vec::with_capacity(n_new_usize),
            cart_positions: Vec::new(),
            version: 1,
            bond_analysis: None,
            phonon_data: None,
            active_phonon_mode: None,
            phonon_phase: 0.0,
            intrinsic_sites: n_new_usize,
            selected_atoms: Vec::new(),
        };

        // Create a fast lookup for original atoms by their atomic_number to get label/element
        // (assuming homonuclear atoms or simple mapping for now)
        // A robust way is to map the original `types.push` index back to the atom.
        // build_supercell essentially preserves the atom type integer.
        // We'll just search the original structure.
        for i in 0..n_new_usize {
            let t = out_types[i] as u8;

            // Find an original atom that matches this element
            let mut label = format!("Element{}", t);
            let mut elem = "Unknown".to_string();

            for j in 0..n_atoms {
                if self.atomic_numbers[j] == t {
                    label = self.labels[j].clone();
                    elem = self.elements[j].clone();
                    break;
                }
            }

            new_state.labels.push(label);
            new_state.elements.push(elem);
            new_state.fract_x.push(out_positions[3 * i]);
            new_state.fract_y.push(out_positions[3 * i + 1]);
            new_state.fract_z.push(out_positions[3 * i + 2]);
            new_state.atomic_numbers.push(t);
        }

        new_state.apply_boundary_mirroring();
        new_state.fractional_to_cartesian();
        new_state.detect_spacegroup();

        Ok(new_state)
    }

    /// Generate a supercell based on a 3x3 expansion matrix (ColMajor).
    pub fn generate_supercell(&self, expansion: &[i32; 9]) -> Result<Self, String> {
        let n_atoms = self.num_atoms();
        if n_atoms == 0 {
            return Err("Cannot generate supercell from empty crystal".to_string());
        }

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

        let mut flat_positions = Vec::with_capacity(n_atoms * 3);
        let mut types = Vec::with_capacity(n_atoms);
        for i in 0..n_atoms {
            flat_positions.push(self.fract_x[i]);
            flat_positions.push(self.fract_y[i]);
            flat_positions.push(self.fract_z[i]);
            types.push(self.atomic_numbers[i] as i32);
        }

        let n_new = unsafe { ffi::get_supercell_size(n_atoms, expansion.as_ptr()) };

        if n_new <= 0 {
            return Err("Invalid supercell size calculation".to_string());
        }

        let n_new_usize = n_new as usize;
        let mut out_lattice = vec![0.0f64; 9];
        let mut out_positions = vec![0.0f64; n_new_usize * 3];
        let mut out_types = vec![0i32; n_new_usize];

        unsafe {
            ffi::build_supercell(
                lattice_col_major.as_ptr(),
                flat_positions.as_ptr(),
                types.as_ptr(),
                n_atoms,
                expansion.as_ptr(),
                out_lattice.as_mut_ptr(),
                out_positions.as_mut_ptr(),
                out_types.as_mut_ptr(),
            );
        }

        let vx = [out_lattice[0], out_lattice[1], out_lattice[2]];
        let vy = [out_lattice[3], out_lattice[4], out_lattice[5]];
        let vz = [out_lattice[6], out_lattice[7], out_lattice[8]];

        let new_a = (vx[0] * vx[0] + vx[1] * vx[1] + vx[2] * vx[2]).sqrt();
        let new_b = (vy[0] * vy[0] + vy[1] * vy[1] + vy[2] * vy[2]).sqrt();
        let new_c = (vz[0] * vz[0] + vz[1] * vz[1] + vz[2] * vz[2]).sqrt();

        let dot_ab = vx[0] * vy[0] + vx[1] * vy[1] + vx[2] * vy[2];
        let dot_bc = vy[0] * vz[0] + vy[1] * vz[1] + vy[2] * vz[2];
        let dot_ca = vz[0] * vx[0] + vz[1] * vx[1] + vz[2] * vx[2];

        let new_gamma = (dot_ab / (new_a * new_b)).acos().to_degrees();
        let new_alpha = (dot_bc / (new_b * new_c)).acos().to_degrees();
        let new_beta = (dot_ca / (new_c * new_a)).acos().to_degrees();

        let mut new_state = CrystalState {
            name: format!("{}_supercell", self.name),
            cell_a: new_a,
            cell_b: new_b,
            cell_c: new_c,
            cell_alpha: new_alpha,
            cell_beta: new_beta,
            cell_gamma: new_gamma,
            spacegroup_hm: "P1".to_string(), // Keep simple, symmetry usually broken
            spacegroup_number: 1,
            labels: Vec::with_capacity(n_new_usize),
            elements: Vec::with_capacity(n_new_usize),
            fract_x: Vec::with_capacity(n_new_usize),
            fract_y: Vec::with_capacity(n_new_usize),
            fract_z: Vec::with_capacity(n_new_usize),
            occupancies: vec![1.0; n_new_usize],
            atomic_numbers: Vec::with_capacity(n_new_usize),
            cart_positions: Vec::new(),
            version: 1,
            bond_analysis: None,
            phonon_data: None,
            active_phonon_mode: None,
            phonon_phase: 0.0,
            intrinsic_sites: n_new_usize,
            selected_atoms: Vec::new(),
        };

        for i in 0..n_new_usize {
            let t = out_types[i] as u8;
            let mut label = format!("Element{}", t);
            let mut elem = "Unknown".to_string();

            for j in 0..n_atoms {
                if self.atomic_numbers[j] == t {
                    label = self.labels[j].clone();
                    elem = self.elements[j].clone();
                    break;
                }
            }

            new_state.labels.push(label);
            new_state.elements.push(elem);
            new_state.fract_x.push(out_positions[3 * i]);
            new_state.fract_y.push(out_positions[3 * i + 1]);
            new_state.fract_z.push(out_positions[3 * i + 2]);
            new_state.atomic_numbers.push(t);
        }

        new_state.apply_boundary_mirroring();
        new_state.fractional_to_cartesian();
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
        let lattice_col_major = self.get_lattice_col_major();

        // Prepare flat positions of intrinsic atoms for MIC overlap check
        let n_intrinsic = self.intrinsic_sites;
        let mut flat_intrinsic = Vec::with_capacity(n_intrinsic * 3);
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

        let label = format!("{}{}", element_symbol, self.num_atoms() + 1);
        self.labels.push(label);
        self.elements.push(element_symbol.to_string());
        self.fract_x.push(fract_pos[0]);
        self.fract_y.push(fract_pos[1]);
        self.fract_z.push(fract_pos[2]);
        self.occupancies.push(1.0);
        self.atomic_numbers.push(atomic_number);
        self.intrinsic_sites += 1;
        self.version += 1;
        self.fractional_to_cartesian();

        Ok(())
    }

    /// Delete atoms by their indices
    pub fn delete_atoms(&mut self, indices: &[usize]) {
        // Sort in descending order and remove duplicates to safely remove by index
        let mut sorted_indices = indices.to_vec();
        sorted_indices.sort_unstable();
        sorted_indices.dedup();

        for &idx in sorted_indices.iter().rev() {
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
        self.version += 1;
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
        self.version += 1;
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

        stats.sort_by(|a, b| a.element_a.cmp(&b.element_a).then(a.element_b.cmp(&b.element_b)));
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
            bond_analysis: None,
            phonon_data: None,
            active_phonon_mode: None,
            phonon_phase: 0.0,
            intrinsic_sites: 2,
        };
        state.fractional_to_cartesian();
        state
    }

    #[test]
    fn test_try_add_atom() {
        let mut c = dummy_crystal();
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
        assert_eq!(c.version, 2, "Version should be incremented");
        assert_eq!(
            c.cart_positions.len(),
            3,
            "Cartesian positions should be updated"
        );
    }

    #[test]
    fn test_delete_atoms() {
        let mut c = dummy_crystal();
        c.delete_atoms(&[0]); // Delete H1
        assert_eq!(c.num_atoms(), 1, "Should have 1 atom remaining");
        assert_eq!(c.labels[0], "O1", "Remaining atom should be O1");
        assert_eq!(c.version, 2, "Version should be incremented");
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
        c.substitute_atoms(&[1], "S", 16); // Substitute O1 with S
        assert_eq!(c.num_atoms(), 2, "Should still have 2 atoms");
        assert_eq!(c.labels[1], "S2", "Label should be S2");
        assert_eq!(c.elements[1], "S", "Element should be S");
        assert_eq!(c.atomic_numbers[1], 16, "Atomic number should be 16");
        assert_eq!(c.version, 2, "Version should be incremented");
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
        
        assert_eq!(analysis.coordination.len(), 2, "Should have 2 coordination shells");
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
        assert!((delta - 0.0).abs() < 1e-10, "Perfect octahedron should have 0 distortion");

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
        assert!((delta_distorted - 0.05).abs() < 1e-10, "Distortion should be exactly 0.05");
    }
}
