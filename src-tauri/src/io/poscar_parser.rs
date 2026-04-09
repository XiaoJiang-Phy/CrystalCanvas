//! VASP POSCAR/CONTCAR parser for CrystalCanvas
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::crystal_state::CrystalState;
use std::fs;

pub fn parse_poscar(path: &str) -> Result<CrystalState, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read POSCAR: {}", e))?;
    parse_poscar_str(&content)
}

pub fn parse_poscar_str(content: &str) -> Result<CrystalState, String> {
    let mut lines = content.lines().map(|l| l.trim()).filter(|l| !l.is_empty());

    // Line 1: Comment (name)
    let name = lines.next().ok_or("Empty POSCAR file")?.to_string();

    // Line 2: Universal scaling factor
    let scale: f64 = lines
        .next()
        .ok_or("Missing scaling factor")?
        .parse()
        .map_err(|_| "Invalid scaling factor")?;

    // Lines 3-5: Lattice vectors
    let mut lattice = [[0.0; 3]; 3];
    for i in 0..3 {
        let line = lines.next().ok_or(format!("Missing lattice vector {}", i + 1))?;
        let parts: Vec<f64> = line
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        if parts.len() < 3 {
            return Err(format!("Invalid lattice vector line: {}", line));
        }
        lattice[i] = [parts[0] * scale, parts[1] * scale, parts[2] * scale];
    }

    // Line 6 & 7: Chemical species and counts
    // In VASP 5, Line 6 is element names, Line 7 is counts.
    // In VASP 4, Line 6 is counts, and element names are usually in the comment or inferred.
    let line6 = lines.next().ok_or("Missing species/counts line")?;

    let mut elements = Vec::new();
    let counts = if line6.split_whitespace().any(|s| s.chars().any(|c| c.is_alphabetic())) {
        let elements_found = line6.split_whitespace().map(|s| s.to_string()).collect::<Vec<_>>();
        let count_line = lines.next().ok_or("Missing counts line")?;
        elements = elements_found;
        count_line
            .split_whitespace()
            .filter_map(|s| s.parse::<usize>().ok())
            .collect::<Vec<_>>()
    } else {
        let counts_found = line6.split_whitespace().filter_map(|s| s.parse::<usize>().ok()).collect::<Vec<_>>();
        for i in 0..counts_found.len() {
            elements.push(format!("X{}", i + 1));
        }
        counts_found
    };

    if elements.len() != counts.len() {
        return Err("Mismatch between element types and counts".to_string());
    }

    // Next line: Selective dynamics (optional) or Coordinate mode
    let mut mode_line = lines.next().ok_or("Missing coordinate mode line")?.to_lowercase();
    if mode_line.starts_with('s') {
        // Skip selective dynamics line if present
        mode_line = lines.next().ok_or("Missing coordinate mode line")?.to_lowercase();
    }

    let is_cartesian = mode_line.starts_with('c') || mode_line.starts_with('k');

    let mut state = CrystalState {
        name,
        version: 1,
        intrinsic_sites: counts.iter().sum(),
        ..Default::default()
    };

    // Calculate lattice parameters from lattice vectors
    let v1 = lattice[0];
    let v2 = lattice[1];
    let v3 = lattice[2];

    let norm = |v: &[f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let dot = |a: &[f64; 3], b: &[f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

    state.cell_a = norm(&v1);
    state.cell_b = norm(&v2);
    state.cell_c = norm(&v3);

    state.cell_alpha = (dot(&v2, &v3) / (state.cell_b * state.cell_c)).acos().to_degrees();
    state.cell_beta = (dot(&v1, &v3) / (state.cell_a * state.cell_c)).acos().to_degrees();
    state.cell_gamma = (dot(&v1, &v2) / (state.cell_a * state.cell_b)).acos().to_degrees();

    // Parse coordinates
    let mut atom_index = 0;
    for (i, &count) in counts.iter().enumerate() {
        let elem = &elements[i];
        let at_num = crate::io::import::get_atomic_number(elem);

        for _ in 0..count {
            let line = lines.next().ok_or(format!("Missing atom coordinate line for atom {}", atom_index + 1))?;
            let parts: Vec<f64> = line
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if parts.len() < 3 {
                return Err(format!("Invalid coordinate line: {}", line));
            }

            let pos = [parts[0], parts[1], parts[2]];
            
            if is_cartesian {
                // We'll convert cartesian to fractional later
                state.cart_positions.push([(pos[0] * scale) as f32, (pos[1] * scale) as f32, (pos[2] * scale) as f32]);
                state.fract_x.push(0.0); // Placeholder
                state.fract_y.push(0.0);
                state.fract_z.push(0.0);
            } else {
                state.fract_x.push(pos[0]);
                state.fract_y.push(pos[1]);
                state.fract_z.push(pos[2]);
            }

            state.elements.push(elem.to_string());
            state.atomic_numbers.push(at_num);
            state.labels.push(format!("{}{}", elem, atom_index + 1));
            state.occupancies.push(1.0);
            atom_index += 1;
        }
    }

    if is_cartesian {
        // Convert cartesian to fractional
        // (Wait, CrystalState::fractional_to_cartesian is already implemented,
        // but we need cartesian_to_fractional here if input is cartesian)
        // For simplicity, let's just implement the inverse matrix logic here or 
        // rely on a future utility.
        
        let mut cart_state = state.clone();
        cart_state.cartesian_to_fractional();
        state = cart_state;
    } else {
        state.fractional_to_cartesian();
    }

    state.detect_spacegroup();
    Ok(state)
}

impl CrystalState {
    /// Inverts the orthogonalization matrix to convert cartesian to fractional
    pub fn cartesian_to_fractional(&mut self) {
        if self.cart_positions.is_empty() { return; }
        
        let a = self.cell_a;
        let b = self.cell_b;
        let c = self.cell_c;
        let alpha = self.cell_alpha.to_radians();
        let beta = self.cell_beta.to_radians();
        let gamma = self.cell_gamma.to_radians();

        let cos_alpha = alpha.cos();
        let cos_beta = beta.cos();
        let cos_gamma = gamma.cos();
        let sin_gamma = gamma.sin();

        let m00 = a;
        let m01 = b * cos_gamma;
        let m02 = c * cos_beta;
        let m11 = b * sin_gamma;
        let m12 = c * (cos_alpha - cos_beta * cos_gamma) / sin_gamma;
        let m22 = c * ((1.0 - cos_alpha * cos_alpha - cos_beta * cos_beta - cos_gamma * cos_gamma + 2.0 * cos_alpha * cos_beta * cos_gamma).max(0.0).sqrt()) / sin_gamma;

        // Invert the 3x3 upper triangular matrix
        let inv_m00 = 1.0 / m00;
        let inv_m11 = 1.0 / m11;
        let inv_m22 = 1.0 / m22;
        let inv_m01 = -m01 / (m00 * m11);
        let inv_m12 = -m12 / (m11 * m22);
        let inv_m02 = (m01 * m12 - m02 * m11) / (m00 * m11 * m22);

        self.fract_x.clear();
        self.fract_y.clear();
        self.fract_z.clear();

        for pos in &self.cart_positions {
            let x = pos[0] as f64;
            let y = pos[1] as f64;
            let z = pos[2] as f64;

            let fx = x * inv_m00 + y * inv_m01 + z * inv_m02;
            let fy = y * inv_m11 + z * inv_m12;
            let fz = z * inv_m22;

            self.fract_x.push(fx);
            self.fract_y.push(fy);
            self.fract_z.push(fz);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_poscar_si() {
        let content = "Si structure
1.0
5.4307 0.0 0.0
0.0 5.4307 0.0
0.0 0.0 5.4307
Si
2
Direct
0.0 0.0 0.0
0.25 0.25 0.25";

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "{}", content).unwrap();
        let path = file.path().to_str().unwrap();

        let state = parse_poscar(path).expect("Failed to parse silicon POSCAR");
        assert_eq!(state.name, "Si structure");
        assert_eq!(state.elements.len(), 2);
        assert_eq!(state.elements[0], "Si");
        assert_eq!(state.num_atoms(), 2);
        assert!((state.cell_a - 5.4307).abs() < 1e-5);
        assert!((state.fract_x[1] - 0.25).abs() < 1e-5);
    }
}
