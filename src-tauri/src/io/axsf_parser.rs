//! AXSF parser for CrystalCanvas
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::crystal_state::CrystalState;
use crate::phonon::{PhononData, PhononMode};
use std::fs;
use std::path::Path;

pub fn parse_axsf(path: &str) -> Result<(CrystalState, PhononData), String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let mut lines = content.lines().map(|l| l.trim()).filter(|l| !l.is_empty());
    
    let mut _n_steps = 0;
    
    // We expect "ANIMSTEPS N"
    if let Some(first) = lines.next() {
        if first == "CRYSTAL" {
             // maybe ANIMSTEPS is missing or came later?
        } else if first.starts_with("ANIMSTEPS") {
             let parts: Vec<&str> = first.split_whitespace().collect();
             if parts.len() >= 2 {
                 _n_steps = parts[1].parse().unwrap_or(0);
             }
        }
    }

    let mut v1 = [0.0; 3];
    let mut v2 = [0.0; 3];
    let mut v3 = [0.0; 3];

    let mut elems: Vec<String> = Vec::new();
    let mut cart_pos: Vec<[f64; 3]> = Vec::new();
    let mut modes = Vec::new();

    let mut in_primvec = false;
    let mut in_coord = false;
    let mut current_mode_index = 0;
    let mut num_atoms = 0;
    let mut atoms_read = 0;
    let mut current_eigenvectors: Vec<[f64; 3]> = Vec::new();

    let mut vec_idx = 0;

    while let Some(line) = lines.next() {
        if line == "CRYSTAL" {
            continue;
        } else if line == "PRIMVEC" {
            in_primvec = true;
            vec_idx = 0;
            continue;
        } else if line.starts_with("PRIMCOORD") {
            in_primvec = false;
            in_coord = true;
            atoms_read = 0;
            current_eigenvectors = Vec::new();
            
            // Read next line for "num_atoms step"
            if let Some(num_line) = lines.next() {
                 let parts: Vec<&str> = num_line.split_whitespace().collect();
                 if parts.len() >= 1 {
                     num_atoms = parts[0].parse().unwrap_or(0);
                     if elems.is_empty() {
                         elems.reserve(num_atoms);
                         cart_pos.reserve(num_atoms);
                     }
                 }
            }
            continue;
        }

        if in_primvec {
            let parts: Vec<f64> = line.split_whitespace().filter_map(|s| s.parse().ok()).collect();
            if parts.len() >= 3 {
                match vec_idx {
                    0 => v1 = [parts[0], parts[1], parts[2]],
                    1 => v2 = [parts[0], parts[1], parts[2]],
                    2 => v3 = [parts[0], parts[1], parts[2]],
                    _ => {}
                }
                vec_idx += 1;
            }
        } else if in_coord {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 7 { // Element X Y Z dx dy dz
                let elem = parts[0];
                let x: f64 = parts[1].parse().unwrap_or(0.0);
                let y: f64 = parts[2].parse().unwrap_or(0.0);
                let z: f64 = parts[3].parse().unwrap_or(0.0);
                let dx: f64 = parts[4].parse().unwrap_or(0.0);
                let dy: f64 = parts[5].parse().unwrap_or(0.0);
                let dz: f64 = parts[6].parse().unwrap_or(0.0);

                if current_mode_index == 0 {
                    elems.push(elem.to_string());
                    cart_pos.push([x, y, z]);
                }
                current_eigenvectors.push([dx, dy, dz]);
                atoms_read += 1;

                if atoms_read >= num_atoms {
                    modes.push(PhononMode {
                        frequency_cm1: 0.0, // AXSF has no freq info usually
                        is_imaginary: false,
                        q_point: [0.0, 0.0, 0.0],
                        eigenvectors: current_eigenvectors.clone(),
                    });
                    current_mode_index += 1;
                    in_coord = false; 
                }
            }
        }
    }

    if cart_pos.is_empty() {
        return Err("No atomic coordinates found in AXSF file.".to_string());
    }

    let norm = |v: &[f64; 3]| (v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt();
    let dot = |a: &[f64; 3], b: &[f64; 3]| a[0]*b[0] + a[1]*b[1] + a[2]*b[2];

    let a = norm(&v1);
    let b = norm(&v2);
    let c = norm(&v3);

    let alpha = (dot(&v2, &v3) / (b * c)).acos().to_degrees();
    let beta = (dot(&v1, &v3) / (a * c)).acos().to_degrees();
    let gamma = (dot(&v1, &v2) / (a * b)).acos().to_degrees();

    // Reconstruct cell inverse to calculate fract
    let m00 = v1[0]; let m01 = v2[0]; let m02 = v3[0];
    let m10 = v1[1]; let m11 = v2[1]; let m12 = v3[1];
    let m20 = v1[2]; let m21 = v2[2]; let m22 = v3[2];
    
    let det = m00*(m11*m22 - m12*m21) - m01*(m10*m22 - m12*m20) + m02*(m10*m21 - m11*m20);
    let inv_det = if det != 0.0 { 1.0 / det } else { 0.0 };

    let inv00 = (m11*m22 - m12*m21) * inv_det;
    let inv01 = (m02*m21 - m01*m22) * inv_det;
    let inv02 = (m01*m12 - m02*m11) * inv_det;
    let inv10 = (m12*m20 - m10*m22) * inv_det;
    let inv11 = (m00*m22 - m02*m20) * inv_det;
    let inv12 = (m02*m10 - m00*m12) * inv_det;
    let inv20 = (m10*m21 - m11*m20) * inv_det;
    let inv21 = (m01*m20 - m00*m21) * inv_det;
    let inv22 = (m00*m11 - m01*m10) * inv_det;

    let mut fract_x = Vec::new();
    let mut fract_y = Vec::new();
    let mut fract_z = Vec::new();

    for pos in &cart_pos {
        let fx = pos[0]*inv00 + pos[1]*inv01 + pos[2]*inv02;
        let fy = pos[0]*inv10 + pos[1]*inv11 + pos[2]*inv12;
        let fz = pos[0]*inv20 + pos[1]*inv21 + pos[2]*inv22;
        fract_x.push(fx);
        fract_y.push(fy);
        fract_z.push(fz);
    }

    let alpha_rad = alpha.to_radians();
    let beta_rad = beta.to_radians();
    let gamma_rad = gamma.to_radians();

    let cos_alpha = alpha_rad.cos();
    let cos_beta = beta_rad.cos();
    let cos_gamma = gamma_rad.cos();
    let sin_gamma = gamma_rad.sin();

    let std_m00 = a;
    let std_m01 = b * cos_gamma;
    let std_m02 = c * cos_beta;
    let std_m11 = b * sin_gamma;
    let std_m12 = c * (cos_alpha - cos_beta * cos_gamma) / sin_gamma;
    let std_m22 = c
        * ((1.0 - cos_alpha * cos_alpha - cos_beta * cos_beta - cos_gamma * cos_gamma
            + 2.0 * cos_alpha * cos_beta * cos_gamma)
            .max(0.0)
            .sqrt())
        / sin_gamma;

    // Rotate modes eigenvectors to standard orientation
    for mode in &mut modes {
        for ev in &mut mode.eigenvectors {
            let dx = ev[0];
            let dy = ev[1];
            let dz = ev[2];

            // 1. Original cart -> fractional diff
            let dfx = dx*inv00 + dy*inv01 + dz*inv02;
            let dfy = dx*inv10 + dy*inv11 + dz*inv12;
            let dfz = dx*inv20 + dy*inv21 + dz*inv22;

            // 2. Fractional diff -> Standard cart
            let std_dx = std_m00*dfx + std_m01*dfy + std_m02*dfz;
            let std_dy =               std_m11*dfy + std_m12*dfz;
            let std_dz =                             std_m22*dfz;

            ev[0] = std_dx;
            ev[1] = std_dy;
            ev[2] = std_dz;
        }
    }

    let name = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "AXSF_Struct".to_string());

    let mut state = CrystalState {
        name,
        cell_a: a,
        cell_b: b,
        cell_c: c,
        cell_alpha: alpha,
        cell_beta: beta,
        cell_gamma: gamma,
        spacegroup_hm: "P1".to_string(),
        spacegroup_number: 1,
        labels: elems.iter().enumerate().map(|(i, e)| format!("{}{}", e, i+1)).collect(),
        elements: elems.clone(),
        fract_x,
        fract_y,
        fract_z,
        occupancies: vec![1.0; elems.len()],
        atomic_numbers: elems.iter().map(|e| crate::io::import::get_atomic_number(e)).collect(),
        cart_positions: Vec::new(),
        version: 1,
        is_2d: false,
        vacuum_axis: None,
        bond_analysis: None,
        phonon_data: None,
        active_phonon_mode: None,
        phonon_phase: 0.0,
        intrinsic_sites: elems.len(),
        selected_atoms: vec![],
        volumetric_data: None,
        bz_cache: None,
        wannier_overlay: None,
    };

    state.fractional_to_cartesian();
    state.detect_spacegroup();

    let phonon_data = PhononData {
        n_atoms: cart_pos.len(),
        modes,
    };

    Ok((state, phonon_data))
}
