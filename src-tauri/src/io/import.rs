//! Structural structure loaders for CrystalCanvas (CIF, XYZ, PDB)
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::crystal_state::CrystalState;
use std::fs;
use std::path::Path;

/// Load a structure from file based on its extension
pub fn load_file(path: &str) -> Result<CrystalState, String> {
    let p = Path::new(path);
    let ext = p
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "cif" => CrystalState::from_cif(path),
        "xyz" => load_xyz(path),
        "pdb" => load_pdb(path),
        _ => Err(format!("Unsupported file extension: {}", ext)),
    }
}

/// Helper to get atomic numbers for common elements
pub(crate) fn get_atomic_number(elem: &str) -> u8 {
    let e = elem.trim().to_uppercase();
    match e.as_str() {
        "H" => 1,
        "HE" => 2,
        "LI" => 3,
        "BE" => 4,
        "B" => 5,
        "C" => 6,
        "N" => 7,
        "O" => 8,
        "F" => 9,
        "NE" => 10,
        "NA" => 11,
        "MG" => 12,
        "AL" => 13,
        "SI" => 14,
        "P" => 15,
        "S" => 16,
        "CL" => 17,
        "AR" => 18,
        "K" => 19,
        "CA" => 20,
        "SC" => 21,
        "TI" => 22,
        "V" => 23,
        "CR" => 24,
        "MN" => 25,
        "FE" => 26,
        "CO" => 27,
        "NI" => 28,
        "CU" => 29,
        "ZN" => 30,
        "GA" => 31,
        "GE" => 32,
        "AS" => 33,
        "SE" => 34,
        "BR" => 35,
        "KR" => 36,
        "RB" => 37,
        "SR" => 38,
        "Y" => 39,
        "ZR" => 40,
        "NB" => 41,
        "MO" => 42,
        "TC" => 43,
        "RU" => 44,
        "RH" => 45,
        "PD" => 46,
        "AG" => 47,
        "CD" => 48,
        "IN" => 49,
        "SN" => 50,
        "SB" => 51,
        "TE" => 52,
        "I" => 53,
        "XE" => 54,
        "CS" => 55,
        "BA" => 56,
        "LA" => 57,
        "CE" => 58,
        "PR" => 59,
        "ND" => 60,
        "PM" => 61,
        "SM" => 62,
        "EU" => 63,
        "GD" => 64,
        "TB" => 65,
        "DY" => 66,
        "HO" => 67,
        "ER" => 68,
        "TM" => 69,
        "YB" => 70,
        "LU" => 71,
        "HF" => 72,
        "TA" => 73,
        "W" => 74,
        "RE" => 75,
        "OS" => 76,
        "IR" => 77,
        "PT" => 78,
        "AU" => 79,
        "HG" => 80,
        "TL" => 81,
        "PB" => 82,
        "BI" => 83,
        "PO" => 84,
        "AT" => 85,
        "RN" => 86,
        "FR" => 87,
        "RA" => 88,
        "AC" => 89,
        "TH" => 90,
        "PA" => 91,
        "U" => 92,
        _ => 0,
    }
}

/// Simple XYZ format parser
fn load_xyz(path: &str) -> Result<CrystalState, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let mut lines = content.lines().map(|l| l.trim()).filter(|l| !l.is_empty());

    let n_atoms_str = lines.next().ok_or("Empty XYZ file")?;
    let _n_atoms: usize = n_atoms_str
        .parse()
        .map_err(|_| "Invalid atom count in XYZ")?;

    let comment = lines.next().unwrap_or("");

    let name = if comment.is_empty() {
        Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Unknown".to_string())
    } else {
        comment.to_string()
    };

    let mut state = CrystalState {
        name,
        ..Default::default()
    };

    let mut cart_pos = Vec::new();
    let mut elems = Vec::new();

    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;
    let mut min_z = f64::MAX;
    let mut max_z = f64::MIN;

    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            let elem = parts[0];
            let x: f64 = parts[1].parse().unwrap_or(0.0);
            let y: f64 = parts[2].parse().unwrap_or(0.0);
            let z: f64 = parts[3].parse().unwrap_or(0.0);

            elems.push(elem.to_string());
            cart_pos.push([x, y, z]);

            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
            min_z = min_z.min(z);
            max_z = max_z.max(z);
        }
    }

    if cart_pos.is_empty() {
        min_x = 0.0;
        max_x = 0.0;
        min_y = 0.0;
        max_y = 0.0;
        min_z = 0.0;
        max_z = 0.0;
    }

    let padding = 10.0;

    let mut dx = (max_x - min_x) + padding;
    let mut dy = (max_y - min_y) + padding;
    let mut dz = (max_z - min_z) + padding;

    dx = dx.max(10.0);
    dy = dy.max(10.0);
    dz = dz.max(10.0);

    state.cell_a = dx;
    state.cell_b = dy;
    state.cell_c = dz;
    state.cell_alpha = 90.0;
    state.cell_beta = 90.0;
    state.cell_gamma = 90.0;
    state.spacegroup_hm = "P1".to_string();
    state.spacegroup_number = 1;
    state.version = 1;

    for i in 0..elems.len() {
        let fx = (cart_pos[i][0] - min_x + padding / 2.0) / dx;
        let fy = (cart_pos[i][1] - min_y + padding / 2.0) / dy;
        let fz = (cart_pos[i][2] - min_z + padding / 2.0) / dz;

        let at_num = get_atomic_number(&elems[i]);

        state.labels.push(format!("{}{}", elems[i], i + 1));
        state.elements.push(elems[i].clone());
        state.fract_x.push(fx);
        state.fract_y.push(fy);
        state.fract_z.push(fz);
        state.occupancies.push(1.0);
        state.atomic_numbers.push(at_num);
    }
    state.fractional_to_cartesian();

    Ok(state)
}

/// Simple PDB format parser (ATOM/HETATM + CRYST1)
fn load_pdb(path: &str) -> Result<CrystalState, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;

    let mut state = CrystalState {
        name: Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Unknown".to_string()),
        version: 1,
        ..Default::default()
    };

    let mut cart_pos = Vec::new();
    let mut elems = Vec::new();

    let mut has_cryst1 = false;

    for line in content.lines() {
        if line.starts_with("CRYST1") && line.len() >= 54 {
            has_cryst1 = true;
            state.cell_a = line[6..15].trim().parse().unwrap_or(10.0);
            state.cell_b = line[15..24].trim().parse().unwrap_or(10.0);
            state.cell_c = line[24..33].trim().parse().unwrap_or(10.0);
            state.cell_alpha = line[33..40].trim().parse().unwrap_or(90.0);
            state.cell_beta = line[40..47].trim().parse().unwrap_or(90.0);
            state.cell_gamma = line[47..54].trim().parse().unwrap_or(90.0);

            if line.len() >= 66 {
                state.spacegroup_hm = line[55..66].trim().to_string();
            } else {
                state.spacegroup_hm = "P 1".to_string();
            }
            state.spacegroup_number = 1;
        } else if (line.starts_with("ATOM  ") || line.starts_with("HETATM")) && line.len() >= 54 {
            let x: f64 = line[30..38].trim().parse().unwrap_or(0.0);
            let y: f64 = line[38..46].trim().parse().unwrap_or(0.0);
            let z: f64 = line[46..54].trim().parse().unwrap_or(0.0);

            let mut elem = "";
            if line.len() >= 78 {
                elem = line[76..78].trim();
            }
            if elem.is_empty() && line.len() >= 16 {
                elem = line[12..16]
                    .trim_start_matches(|c: char| c.is_ascii_digit())
                    .trim();
            }
            if elem.is_empty() {
                elem = "X";
            }

            // Clean up elements that might have trailing chars like "CA" instead of "Ca"
            let elem_lower = elem.to_lowercase();
            let mut formatted_elem = String::new();
            let mut chars = elem_lower.chars();
            if let Some(c) = chars.next() {
                formatted_elem.push(c.to_ascii_uppercase());
            }
            if let Some(c) = chars.next()
                && c.is_ascii_alphabetic()
            {
                formatted_elem.push(c);
            }
            if formatted_elem.is_empty() {
                formatted_elem = "X".to_string();
            }

            elems.push(formatted_elem);
            cart_pos.push([x, y, z]);
        }
    }

    if !has_cryst1 {
        let padding = 10.0;
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        let mut min_z = f64::MAX;
        let mut max_z = f64::MIN;

        for pos in &cart_pos {
            min_x = min_x.min(pos[0]);
            max_x = max_x.max(pos[0]);
            min_y = min_y.min(pos[1]);
            max_y = max_y.max(pos[1]);
            min_z = min_z.min(pos[2]);
            max_z = max_z.max(pos[2]);
        }

        if cart_pos.is_empty() {
            min_x = 0.0;
            max_x = 0.0;
            min_y = 0.0;
            max_y = 0.0;
            min_z = 0.0;
            max_z = 0.0;
        }

        state.cell_a = ((max_x - min_x) + padding).max(10.0);
        state.cell_b = ((max_y - min_y) + padding).max(10.0);
        state.cell_c = ((max_z - min_z) + padding).max(10.0);
        state.cell_alpha = 90.0;
        state.cell_beta = 90.0;
        state.cell_gamma = 90.0;
        state.spacegroup_hm = "P1".to_string();
        state.spacegroup_number = 1;

        for pos in &mut cart_pos {
            pos[0] = pos[0] - min_x + padding / 2.0;
            pos[1] = pos[1] - min_y + padding / 2.0;
            pos[2] = pos[2] - min_z + padding / 2.0;
        }
    }

    // Inverse orthogonalization matrix to convert cartesian to fractional
    let a = state.cell_a;
    let b = state.cell_b;
    let c = state.cell_c;
    let alpha = state.cell_alpha.to_radians();
    let beta = state.cell_beta.to_radians();
    let gamma = state.cell_gamma.to_radians();

    let cos_alpha = alpha.cos();
    let cos_beta = beta.cos();
    let cos_gamma = gamma.cos();
    let sin_gamma = gamma.sin();

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

    let inv_m00 = 1.0 / m00;
    let inv_m11 = 1.0 / m11;
    let inv_m22 = 1.0 / m22;
    let inv_m01 = -m01 / (m00 * m11);
    let inv_m12 = -m12 / (m11 * m22);
    let inv_m02 = (m01 * m12 - m02 * m11) / (m00 * m11 * m22);

    for i in 0..elems.len() {
        let x = cart_pos[i][0];
        let y = cart_pos[i][1];
        let z = cart_pos[i][2];

        let fx = x * inv_m00 + y * inv_m01 + z * inv_m02;
        let fy = y * inv_m11 + z * inv_m12;
        let fz = z * inv_m22;

        let at_num = get_atomic_number(&elems[i]);

        state.labels.push(format!("{}{}", elems[i], i + 1));
        state.elements.push(elems[i].clone());
        state.fract_x.push(fx);
        state.fract_y.push(fy);
        state.fract_z.push(fz);
        state.occupancies.push(1.0);
        state.atomic_numbers.push(at_num);
    }

    state.fractional_to_cartesian();
    Ok(state)
}
