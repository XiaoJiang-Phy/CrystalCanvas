//! Core crystal state — Single Source of Truth (SSoT) with SoA layout for physics and rendering
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::ffi;
use serde::Serialize;

/// Error returned when trying to add an overlapping atom
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionError;

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
            labels: Vec::with_capacity(n),
            elements: Vec::with_capacity(n),
            fract_x: Vec::with_capacity(n),
            fract_y: Vec::with_capacity(n),
            fract_z: Vec::with_capacity(n),
            occupancies: Vec::with_capacity(n),
            atomic_numbers: Vec::with_capacity(n),
            cart_positions: Vec::new(),
            version: 1,
        };

        for site in data.sites {
            state.labels.push(site.label);
            state.elements.push(site.element_symbol);
            state.fract_x.push(site.fract_x);
            state.fract_y.push(site.fract_y);
            state.fract_z.push(site.fract_z);
            state.occupancies.push(site.occ);
            state.atomic_numbers.push(site.atomic_number);
        }
        state.apply_boundary_mirroring();
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
        if self.num_atoms() == 0 {
            return;
        }

        // Prepare lattice in col-major
        let alpha = self.cell_alpha.to_radians();
        let beta = self.cell_beta.to_radians();
        let gamma = self.cell_gamma.to_radians();
        let a = self.cell_a;
        let b = self.cell_b;
        let c = self.cell_c;

        let cx = c * beta.cos();
        let cy = c * (alpha.cos() - beta.cos() * gamma.cos()) / gamma.sin();
        let cz = (c * c - cx * cx - cy * cy).max(0.0).sqrt();

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

        let mut flat_positions = Vec::with_capacity(self.num_atoms() * 3);
        let mut types = Vec::with_capacity(self.num_atoms());
        for i in 0..self.num_atoms() {
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
                self.num_atoms(),
                1e-5, // symprec
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
        // Collect lattice and current positions to pass to FFI
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

        let mut flat_positions = Vec::with_capacity(self.num_atoms() * 3);
        for i in 0..self.num_atoms() {
            flat_positions.push(self.fract_x[i]);
            flat_positions.push(self.fract_y[i]);
            flat_positions.push(self.fract_z[i]);
        }

        let is_overlap = unsafe {
            ffi::check_overlap_mic(
                lattice_col_major.as_ptr(),
                flat_positions.as_ptr(),
                self.num_atoms(),
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
}
