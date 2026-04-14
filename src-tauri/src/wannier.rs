// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::io::wannier_hr_parser::WannierHrData;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct VisibleHopping {
    /// Cartesian position of orbital m in cell 0
    pub start_cart: [f32; 3],
    /// Cartesian position of orbital n in cell R
    pub end_cart: [f32; 3],
    /// |t| for radius scaling
    pub magnitude: f64,
    /// Principal component sign (+1.0 or -1.0) for color mapping
    pub sign: f32,
    /// Source orbital index for per-orbital color mapping
    pub orb_m: usize,
    /// Destination orbital (atom) index
    pub dest_atom: usize,
    /// Translation vector R for the destination
    pub r_vec: [i32; 3],
}

#[derive(Clone, Debug, Serialize)]
pub struct WannierOverlay {
    pub hr_data: WannierHrData,
    pub t_min_threshold: f64,
    pub show_onsite: bool,
    pub active_r_shells: Vec<bool>,
    pub active_orbitals: Vec<bool>,
    pub visible_hoppings: Vec<VisibleHopping>,
    #[serde(skip)]
    r_shell_map: std::collections::HashMap<[i32; 3], usize>,
}

impl WannierOverlay {
    pub fn new(hr_data: WannierHrData, lattice_col_major: &[f64; 9], atom_positions: &[[f32; 3]]) -> Result<Self, String> {
        let n_shells = hr_data.r_shells.len();
        let mut overlay = Self {
            t_min_threshold: 0.01,
            show_onsite: false,
            active_r_shells: vec![true; n_shells],
            active_orbitals: vec![true; hr_data.num_wann],
            visible_hoppings: Vec::with_capacity(hr_data.hoppings.len()),
            r_shell_map: hr_data.r_shells.iter().enumerate().map(|(i, &r)| (r, i)).collect(),
            hr_data,
        };
        overlay.filter_and_rebuild(lattice_col_major, atom_positions)?;
        Ok(overlay)
    }

    pub fn filter_and_rebuild(&mut self, lattice_col_major: &[f64; 9], atom_positions: &[[f32; 3]]) -> Result<(), String> {
        if atom_positions.len() < self.hr_data.num_wann {
            return Err(format!("Atom positions array size ({}) is smaller than num_wann ({})", atom_positions.len(), self.hr_data.num_wann));
        }

        self.visible_hoppings.clear();

        for hopping in &self.hr_data.hoppings {
            // Hide on-site energies (R=0, m=n) by default
            if !self.show_onsite && hopping.m == hopping.n && hopping.r_vec == [0, 0, 0] {
                continue;
            }

            let t_mag = hopping.magnitude;
            if t_mag < self.t_min_threshold {
                continue;
            }

            if !self.active_orbitals[hopping.m] || !self.active_orbitals[hopping.n] {
                continue;
            }

            let shell_idx = self.r_shell_map.get(&hopping.r_vec).copied()
                .ok_or_else(|| "Hopping R-vec not found in r_shell_map".to_string())?;

            if !self.active_r_shells[shell_idx] {
                continue;
            }

            let start_cart = atom_positions[hopping.m];
            let pos_n = atom_positions[hopping.n];

            let rx = hopping.r_vec[0] as f64;
            let ry = hopping.r_vec[1] as f64;
            let rz = hopping.r_vec[2] as f64;

            // Apply unit cell translation: T = R * lattice (col-major)
            let tx = rx * lattice_col_major[0] + ry * lattice_col_major[3] + rz * lattice_col_major[6];
            let ty = rx * lattice_col_major[1] + ry * lattice_col_major[4] + rz * lattice_col_major[7];
            let tz = rx * lattice_col_major[2] + ry * lattice_col_major[5] + rz * lattice_col_major[8];

            let end_cart = [
                pos_n[0] + tx as f32,
                pos_n[1] + ty as f32,
                pos_n[2] + tz as f32,
            ];

            self.visible_hoppings.push(VisibleHopping {
                start_cart,
                end_cart,
                magnitude: t_mag,
                sign: if hopping.re.abs() >= hopping.im.abs() {
                    if hopping.re >= 0.0 { 1.0 } else { -1.0 }
                } else {
                    if hopping.im >= 0.0 { 1.0 } else { -1.0 }
                },
                orb_m: hopping.m,
                dest_atom: hopping.n,
                r_vec: hopping.r_vec,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::wannier_hr_parser::parse_wannier_hr;

    fn get_graphene_hr_path() -> String {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .join("tests/fixtures/graphene_hr.dat")
            .to_string_lossy().into_owned()
    }

    const LATTICE: [f64; 9] = [
        2.46, 0.0, 0.0,
        -1.23, 2.13, 0.0,
        0.0, 0.0, 10.0
    ];
    
    const ATOMS: [[f32; 3]; 2] = [
        [0.0, 0.0, 0.0],
        [1.23, 0.71, 0.0],
    ];

    #[test]
    fn test_wannier_filter_magnitude() {
        let hr_data = parse_wannier_hr(&get_graphene_hr_path()).unwrap();
        let mut overlay = WannierOverlay::new(hr_data, &LATTICE, &ATOMS).unwrap();
        
        // Total 7 hoppings. 1 is on-site -> 6 inter-site default visible.
        assert_eq!(overlay.visible_hoppings.len(), 6);
        
        // Filter out everything but nearest neighbor (~2.7)
        overlay.t_min_threshold = 2.0;
        overlay.filter_and_rebuild(&LATTICE, &ATOMS).unwrap();
        
        // Graphene has 3 nearest neighbors for each carbon atom
        assert_eq!(overlay.visible_hoppings.len(), 3);
    }
    
    #[test]
    fn test_wannier_filter_r_shell() {
        let hr_data = parse_wannier_hr(&get_graphene_hr_path()).unwrap();
        let mut overlay = WannierOverlay::new(hr_data, &LATTICE, &ATOMS).unwrap();
        
        // Turn off R-shell 1 (which refers to one of the length-1 R-shells, e.g. R=[1,0,0])
        overlay.active_r_shells[1] = false;
        overlay.filter_and_rebuild(&LATTICE, &ATOMS).unwrap();
        
        // Out of the 6 visible hoppings, disabling one length-1 R-vector removes exactly 1 hopping.
        assert_eq!(overlay.visible_hoppings.len(), 5);
        
        // Turn everything back on, change orbital filter
        overlay.active_r_shells[1] = true;
        overlay.active_orbitals[0] = false;
        overlay.filter_and_rebuild(&LATTICE, &ATOMS).unwrap();
        assert_eq!(overlay.visible_hoppings.len(), 0);
    }

    #[test]
    fn test_wannier_overlay_cartesian() {
        let hr_data = parse_wannier_hr(&get_graphene_hr_path()).unwrap();
        let overlay = WannierOverlay::new(hr_data, &LATTICE, &ATOMS).unwrap();
        
        // Find hopping with R = [1, 0, 0] which translates x by +2.46
        let h = overlay.visible_hoppings.iter()
            .find(|x| x.end_cart[0] > 2.0 && x.magnitude > 2.0)
            .unwrap();
        
        // Atom 0 is at [0,0,0], so start_cart is [0,0,0]
        assert_eq!(h.start_cart, [0.0, 0.0, 0.0]);
        // R=[1,0,0] means translation by `a` = 2.46. 
        // We defined atom_n = m = 0, so end_cart = [2.46, 0.0, 0.0]
        assert!((h.end_cart[0] - 2.46).abs() < 1e-4);
        assert!((h.end_cart[1] - 0.0).abs() < 1e-4);
    }
}

pub fn build_atoms_with_ghosts(
    cs: &crate::crystal_state::CrystalState,
    settings: &crate::settings::AppSettings,
) -> Vec<crate::renderer::instance::AtomInstance> {
    build_atoms_with_ghosts_displaced(cs, &cs.cart_positions, settings)
}

pub fn build_atoms_with_ghosts_displaced(
    cs: &crate::crystal_state::CrystalState,
    cart_positions: &[[f32; 3]],
    settings: &crate::settings::AppSettings,
) -> Vec<crate::renderer::instance::AtomInstance> {
    let mut instances = crate::renderer::instance::build_instance_data(
        cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
        &cs.occupancies,
        settings,
        &cs.selected_atoms,
    );

    if let Some(overlay) = &cs.wannier_overlay {
        let mut ghosts = std::collections::HashSet::new();
        for h in &overlay.visible_hoppings {
            if h.r_vec != [0, 0, 0] {
                ghosts.insert((h.dest_atom, h.r_vec));
            }
        }
        
        let lattice_col_major = cs.get_lattice_col_major();
        for (atom_idx, r_vec) in &ghosts {
            let pos = cart_positions[*atom_idx];
            let rx = r_vec[0] as f64;
            let ry = r_vec[1] as f64;
            let rz = r_vec[2] as f64;
            let tx = rx * lattice_col_major[0] + ry * lattice_col_major[3] + rz * lattice_col_major[6];
            let ty = rx * lattice_col_major[1] + ry * lattice_col_major[4] + rz * lattice_col_major[7];
            let tz = rx * lattice_col_major[2] + ry * lattice_col_major[5] + rz * lattice_col_major[8];
            
            if *atom_idx < instances.len() {
                let mut inst = instances[*atom_idx].clone();
                inst.position = [
                    pos[0] + tx as f32,
                    pos[1] + ty as f32,
                    pos[2] + tz as f32,
                ];
                
                inst.radius *= 0.50;
                inst.color[3] = 0.40;
                inst.color[0] *= 0.8;
                inst.color[1] *= 0.8;
                inst.color[2] *= 0.8;

                instances.push(inst);
            }
        }
    }
    
    instances
}
