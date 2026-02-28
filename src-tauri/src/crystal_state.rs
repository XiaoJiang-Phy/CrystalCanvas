//! Core crystal state — Single Source of Truth (SSoT) with SoA layout for physics and rendering

use crate::ffi;

/// The central crystal structure state, holding all atom data in SoA layout.
/// - f64 fields for physics calculations (fractional coords)
/// - f32 fields for GPU rendering (Cartesian coords, populated on demand)
#[allow(dead_code)]
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
}

impl CrystalState {
    /// Construct from FFI data returned by the C++ parser.
    pub fn from_ffi(data: ffi::FfiCrystalData) -> Self {
        let n = data.sites.len();
        let mut state = CrystalState {
            name: data.name,
            cell_a: data.a,
            cell_b: data.b,
            cell_c: data.c,
            cell_alpha: data.alpha,
            cell_beta: data.beta,
            cell_gamma: data.gamma,
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

        state
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
        let m22 = c * ((1.0 - cos_alpha * cos_alpha - cos_beta * cos_beta
            - cos_gamma * cos_gamma
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
}
