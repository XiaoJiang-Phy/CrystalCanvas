//! Quantum ESPRESSO parser for CrystalCanvas
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::crystal_state::CrystalState;
use std::fs;
use std::path::Path;

const BOHR_TO_ANGSTROM: f64 = 0.529177210903;

pub fn parse_scf_out(path: &str) -> Result<CrystalState, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let mut lines = content.lines().map(|l| l.trim());
    
    let mut alat_bohr = 0.0;
    let mut v1 = [0.0; 3];
    let mut v2 = [0.0; 3];
    let mut v3 = [0.0; 3];
    let mut has_axes = false;

    let mut elems: Vec<String> = Vec::new();
    let mut fracts: Vec<[f64; 3]> = Vec::new();

    while let Some(line) = lines.next() {
        if line.starts_with("lattice parameter (alat)") {
            if let Some(eq) = line.find('=') {
                if let Some(end) = line.find("a.u.") {
                    if eq + 1 < end {
                        alat_bohr = line[eq+1..end].trim().parse().unwrap_or(0.0);
                    }
                }
            }
        } else if line.starts_with("crystal axes: (cart. coord. in units of alat)") {
            // Use rfind to skip the `a(1)` prefix and match the LAST pair of parens
            // e.g. "a(1) = (  -0.500000   0.000000   0.500000 )"
            let parse_vec = |l: &str| -> [f64; 3] {
                if let Some(end) = l.rfind(')') {
                    // Find the matching '(' by searching backwards from end
                    if let Some(start) = l[..end].rfind('(') {
                        let parts: Vec<f64> = l[start+1..end].split_whitespace().filter_map(|s| s.parse().ok()).collect();
                        if parts.len() >= 3 {
                            return [parts[0], parts[1], parts[2]];
                        }
                    }
                }
                [0.0, 0.0, 0.0]
            };
            v1 = parse_vec(lines.next().unwrap_or(""));
            v2 = parse_vec(lines.next().unwrap_or(""));
            v3 = parse_vec(lines.next().unwrap_or(""));
            has_axes = true;
        } else if line.starts_with("site n.     atom                  positions (cryst. coord.)") {
            // Read until empty line
            while let Some(pos_line) = lines.next() {
                if pos_line.is_empty() {
                    break;
                }
                // e.g. "1           Ce  tau(   1) = (  0.0000000  0.0000000  0.0000000  )"
                let parts: Vec<&str> = pos_line.split_whitespace().collect();
                if parts.len() >= 9 {
                    let elem = parts[1];
                    // The coordinates are inside parens
                    let mut nums = Vec::new();
                    if let Some(start) = pos_line.find("= (") {
                        if let Some(end) = pos_line[start..].find(')') {
                            let coords_str = &pos_line[start+3 .. start+end];
                            nums = coords_str.split_whitespace().filter_map(|s| s.parse::<f64>().ok()).collect();
                        }
                    }
                    if nums.len() >= 3 {
                        elems.push(elem.to_string());
                        fracts.push([nums[0], nums[1], nums[2]]);
                    }
                }
            }
        }
    }

    if alat_bohr == 0.0 || !has_axes || elems.is_empty() {
        return Err("Could not find necessary structural data in scf.out".to_string());
    }

    let lat = alat_bohr * BOHR_TO_ANGSTROM;
    
    // Scale vectors to Angstroms
    let v1_a = [v1[0]*lat, v1[1]*lat, v1[2]*lat];
    let v2_a = [v2[0]*lat, v2[1]*lat, v2[2]*lat];
    let v3_a = [v3[0]*lat, v3[1]*lat, v3[2]*lat];

    let norm = |v: &[f64; 3]| (v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt();
    let dot = |a: &[f64; 3], b: &[f64; 3]| a[0]*b[0] + a[1]*b[1] + a[2]*b[2];

    let a = norm(&v1_a);
    let b = norm(&v2_a);
    let c = norm(&v3_a);

    let alpha = (dot(&v2_a, &v3_a) / (b * c)).acos().to_degrees();
    let beta = (dot(&v1_a, &v3_a) / (a * c)).acos().to_degrees();
    let gamma = (dot(&v1_a, &v2_a) / (a * b)).acos().to_degrees();

    let name = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "QE_SCF".to_string());

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
        fract_x: fracts.iter().map(|f| f[0]).collect(),
        fract_y: fracts.iter().map(|f| f[1]).collect(),
        fract_z: fracts.iter().map(|f| f[2]).collect(),
        occupancies: vec![1.0; elems.len()],
        // Simplistic atomic number mapping
        atomic_numbers: elems.iter().map(|e| crate::io::import::get_atomic_number(e)).collect(),
        cart_positions: Vec::new(),
        version: 1,
        bond_analysis: None,
        phonon_data: None,
        active_phonon_mode: None,
        phonon_phase: 0.0,
        intrinsic_sites: elems.len(),
        selected_atoms: vec![],
    };

    state.fractional_to_cartesian();
    state.detect_spacegroup();
    Ok(state)
}
