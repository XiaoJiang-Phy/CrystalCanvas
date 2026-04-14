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
        measurements: Vec::new(),
    };

    state.fractional_to_cartesian();
    state.detect_spacegroup();
    Ok(state)
}

pub fn parse_scf_in(path: &str) -> Result<CrystalState, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let mut lines = content.lines().map(|l| l.trim());

    let mut ibrav = 0;
    let mut celldm = [0.0; 7]; // 1-indexed for convenience
    let mut a_abc = [0.0; 3];
    let mut cos_abc = [0.0; 3];
    let mut nat = 0;
    
    let mut v1 = [0.0; 3];
    let mut v2 = [0.0; 3];
    let mut v3 = [0.0; 3];
    let mut has_cell_params = false;
    let mut cell_params_unit = "alat";

    let mut elems = Vec::new();
    let mut positions = Vec::new();
    let mut pos_unit = "alat";

    while let Some(line) = lines.next() {
        let l_lower = line.to_lowercase();
        if l_lower.contains("&system") {
            // Parse &SYSTEM block until /
            while let Some(sys_line) = lines.next() {
                let sl = sys_line.to_lowercase();
                if sl.contains('/') { break; }
                
                // Simple key=value parser
                for part in sl.split(',') {
                    let kv: Vec<&str> = part.split('=').map(|s| s.trim()).collect();
                    if kv.len() >= 2 {
                        let key = kv[0];
                        let val_str = kv[1].split_whitespace().next().unwrap_or("");
                        let val: f64 = val_str.parse().unwrap_or(0.0);

                        if key == "ibrav" { ibrav = val as i32; }
                        else if key == "nat" { nat = val as usize; }
                        else if key.starts_with("celldm(") {
                            if let Some(idx_str) = key.strip_prefix("celldm(").and_then(|s| s.strip_suffix(')')) {
                                if let Ok(idx) = idx_str.parse::<usize>() {
                                    if idx >= 1 && idx <= 6 { celldm[idx] = val; }
                                }
                            }
                        }
                        else if key == "a" { a_abc[0] = val; }
                        else if key == "b" { a_abc[1] = val; }
                        else if key == "c" { a_abc[2] = val; }
                        else if key == "cosab" { cos_abc[0] = val; }
                        else if key == "cosac" { cos_abc[1] = val; }
                        else if key == "cosbc" { cos_abc[2] = val; }
                    }
                }
            }
        } else if l_lower.starts_with("cell_parameters") {
            has_cell_params = true;
            if l_lower.contains("bohr") { cell_params_unit = "bohr"; }
            else if l_lower.contains("angstrom") { cell_params_unit = "angstrom"; }
            else { cell_params_unit = "alat"; }

            let parse_vec = |l: &str| -> [f64; 3] {
                let p: Vec<f64> = l.split_whitespace().filter_map(|s| s.parse().ok()).collect();
                if p.len() >= 3 { [p[0], p[1], p[2]] } else { [0.0, 0.0, 0.0] }
            };
            v1 = parse_vec(lines.next().unwrap_or(""));
            v2 = parse_vec(lines.next().unwrap_or(""));
            v3 = parse_vec(lines.next().unwrap_or(""));
        } else if l_lower.starts_with("atomic_positions") {
            if l_lower.contains("crystal") { pos_unit = "crystal"; }
            else if l_lower.contains("bohr") { pos_unit = "bohr"; }
            else if l_lower.contains("angstrom") { pos_unit = "angstrom"; }
            else { pos_unit = "alat"; }

            for _ in 0..nat {
                if let Some(pos_line) = lines.next() {
                    let p: Vec<&str> = pos_line.split_whitespace().collect();
                    if p.len() >= 4 {
                        elems.push(p[0].to_string());
                        positions.push([
                            p[1].parse().unwrap_or(0.0),
                            p[2].parse().unwrap_or(0.0),
                            p[3].parse().unwrap_or(0.0)
                        ]);
                    }
                }
            }
        }
    }

    // Resolve alat
    let mut alat = celldm[1];
    if alat == 0.0 { alat = a_abc[0] / BOHR_TO_ANGSTROM; }
    if alat == 0.0 && has_cell_params && cell_params_unit == "alat" {
        // If cell_parameters are in alat, we expect alat to be set. 
        // If not, it's often 1.0 or the first lattice vector length.
        alat = 1.0; 
    }

    let scale = if cell_params_unit == "bohr" || cell_params_unit == "alat" {
        alat * BOHR_TO_ANGSTROM
    } else {
        1.0 // angstrom
    };

    let lattice_vectors = if has_cell_params {
        [
            [v1[0] * scale, v1[1] * scale, v1[2] * scale],
            [v2[0] * scale, v2[1] * scale, v2[2] * scale],
            [v3[0] * scale, v3[1] * scale, v3[2] * scale],
        ]
    } else if ibrav != 0 {
        let side = if celldm[1] != 0.0 { celldm[1] * BOHR_TO_ANGSTROM } else { a_abc[0] };
        match ibrav {
            1 => [ // Simple Cubic
                [side, 0.0, 0.0],
                [0.0, side, 0.0],
                [0.0, 0.0, side],
            ],
            2 => [ // Face-Centered Cubic (FCC)
                [-side/2.0, 0.0, side/2.0],
                [0.0, side/2.0, side/2.0],
                [-side/2.0, side/2.0, 0.0],
            ],
            3 => [ // Body-Centered Cubic (BCC)
                [side/2.0, side/2.0, side/2.0],
                [-side/2.0, side/2.0, side/2.0],
                [-side/2.0, -side/2.0, side/2.0],
            ],
            4 => { // Hexagonal
                let c = if celldm[3] != 0.0 { celldm[1] * celldm[3] * BOHR_TO_ANGSTROM } else { a_abc[2] };
                [
                    [side, 0.0, 0.0],
                    [-side/2.0, side * 3.0f64.sqrt() / 2.0, 0.0],
                    [0.0, 0.0, c],
                ]
            },
            _ => return Err(format!("Unsupported ibrav={}. Please use ibrav=0 and CELL_PARAMETERS.", ibrav)),
        }
    } else {
        return Err("No cell parameters or supported ibrav found.".to_string());
    };

    let norm = |v: &[f64; 3]| (v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt();
    let dot = |a: &[f64; 3], b: &[f64; 3]| a[0]*b[0] + a[1]*b[1] + a[2]*b[2];

    let a = norm(&lattice_vectors[0]);
    let b = norm(&lattice_vectors[1]);
    let c = norm(&lattice_vectors[2]);

    let alpha = (dot(&lattice_vectors[1], &lattice_vectors[2]) / (b * c)).acos().to_degrees();
    let beta = (dot(&lattice_vectors[0], &lattice_vectors[2]) / (a * c)).acos().to_degrees();
    let gamma = (dot(&lattice_vectors[0], &lattice_vectors[1]) / (a * b)).acos().to_degrees();

    let name = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "QE_IN".to_string());

    let mut state = CrystalState {
        name,
        cell_a: a, cell_b: b, cell_c: c,
        cell_alpha: alpha, cell_beta: beta, cell_gamma: gamma,
        elements: elems.clone(),
        labels: elems.iter().enumerate().map(|(i, e)| format!("{}{}", e, i+1)).collect(),
        occupancies: vec![1.0; elems.len()],
        atomic_numbers: elems.iter().map(|e| crate::io::import::get_atomic_number(e)).collect(),
        version: 1,
        is_2d: false,
        vacuum_axis: None,
        intrinsic_sites: elems.len(),
        ..Default::default()
    };

    // Convert positions
    if pos_unit == "crystal" {
        state.fract_x = positions.iter().map(|p| p[0]).collect();
        state.fract_y = positions.iter().map(|p| p[1]).collect();
        state.fract_z = positions.iter().map(|p| p[2]).collect();
        state.fractional_to_cartesian();
    } else {
        let p_scale = match pos_unit {
            "bohr" => BOHR_TO_ANGSTROM,
            "alat" => alat * BOHR_TO_ANGSTROM,
            _ => 1.0, // angstrom
        };
        for p in positions {
            state.cart_positions.push([(p[0] * p_scale) as f32, (p[1] * p_scale) as f32, (p[2] * p_scale) as f32]);
        }
        state.cartesian_to_fractional();
    }

    state.detect_spacegroup();
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_scf_in_basic() {
        let content = "&CONTROL\n  prefix='si'\n/\n&SYSTEM\n  ibrav=1, A=5.43, nat=2, ntyp=1\n/\nATOMIC_POSITIONS (crystal)\nSi 0.0 0.0 0.0\nSi 0.25 0.25 0.25";
        
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "{}", content).unwrap();
        let path = file.path().to_str().unwrap();

        let state = parse_scf_in(path).expect("Failed to parse basic QE input");
        assert_eq!(state.num_atoms(), 2);
        assert!((state.cell_a - 5.43).abs() < 1e-5);
        assert_eq!(state.elements[0], "Si");
    }

    #[test]
    fn test_parse_scf_in_ceo() {
        // Run test assuming working dir is src-tauri
        let path = "../tests/data/CeO/scf.in"; 
        if std::path::Path::new(path).exists() {
            let state = parse_scf_in(path).expect("Failed to parse CeO scf.in");
            assert_eq!(state.num_atoms(), 3);
            assert_eq!(state.elements.clone(), vec!["Ce", "O", "O"]);
            assert!(state.cell_a - 3.8686 > -1e-4);
        }
    }
}
