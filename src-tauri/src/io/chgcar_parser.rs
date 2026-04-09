//! VASP CHGCAR/LOCPOT parser for CrystalCanvas
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::crystal_state::CrystalState;
use crate::volumetric::{VolumetricData, VolumetricFormat};
use std::fs;

pub fn parse_chgcar(path: &str) -> Result<CrystalState, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let mut state = crate::io::poscar_parser::parse_poscar_str(&content)?;

    let path_lower = path.to_lowercase();
    let format = if path_lower.ends_with("locpot") || path_lower.contains("locpot") {
        VolumetricFormat::VaspLocpot
    } else {
        VolumetricFormat::VaspChgcar
    };

    let mut lines = content.lines();

    lines.next().ok_or("Empty file")?;

    let scale: f64 = lines.next()
        .ok_or("Missing scale")?
        .trim()
        .parse()
        .map_err(|_| "Invalid scale")?;

    let mut lattice_vecs = [[0.0; 3]; 3];
    for i in 0..3 {
        let line = lines.next().ok_or("Missing lattice")?;
        let parts: Vec<f64> = line.split_whitespace().filter_map(|s| s.parse().ok()).collect();
        if parts.len() < 3 { return Err("Invalid lattice line".to_string()); }
        lattice_vecs[i] = [parts[0] * scale, parts[1] * scale, parts[2] * scale];
    }
    
    let line6 = lines.next().ok_or("Missing species/counts line")?;
    let counts: Vec<usize> = if line6.split_whitespace().any(|s| s.chars().any(|c| c.is_alphabetic())) {
        let count_line = lines.next().ok_or("Missing counts line")?;
        count_line.split_whitespace().filter_map(|s| s.parse().ok()).collect()
    } else {
        line6.split_whitespace().filter_map(|s| s.parse().ok()).collect()
    };
    
    let total_atoms: usize = counts.iter().sum();

    let mode_line = lines.next().ok_or("Missing coordinate mode")?.trim().to_lowercase();
    if mode_line.starts_with('s') {
        let _ = lines.next().ok_or("Missing coordinate mode")?;
    }
    
    let mut atoms_skipped = 0;
    while atoms_skipped < total_atoms {
        let line = lines.next().ok_or("Missing atom coordinate")?;
        if line.trim().is_empty() { continue; }
        atoms_skipped += 1;
    }

    let mut grid_dims = [0usize; 3];
    let mut found_grid = false;
    
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() == 3 {
            if let (Ok(nx), Ok(ny), Ok(nz)) = (
                parts[0].parse::<usize>(),
                parts[1].parse::<usize>(),
                parts[2].parse::<usize>()
            ) {
                grid_dims = [nx, ny, nz];
                found_grid = true;
                break;
            }
        }
    }

    if !found_grid {
        return Err("Could not find NGX NGY NGZ grid dimensions in file".to_string());
    }

    let n_voxels = grid_dims[0] * grid_dims[1] * grid_dims[2];
    
    if n_voxels > 150 * 150 * 150 {
        return Err(format!("Grid size {}x{}x{} exceeds Phase A limit of 150^3", grid_dims[0], grid_dims[1], grid_dims[2]));
    }

    let det = lattice_vecs[0][0] * (lattice_vecs[1][1] * lattice_vecs[2][2] - lattice_vecs[1][2] * lattice_vecs[2][1])
            - lattice_vecs[0][1] * (lattice_vecs[1][0] * lattice_vecs[2][2] - lattice_vecs[1][2] * lattice_vecs[2][0])
            + lattice_vecs[0][2] * (lattice_vecs[1][0] * lattice_vecs[2][1] - lattice_vecs[1][1] * lattice_vecs[2][0]);
    let v_cell = det.abs();

    if v_cell < 1e-12 {
        return Err("Degenerate lattice: cell volume is zero".to_string());
    }

    let normalization = if matches!(format, VolumetricFormat::VaspChgcar) {
        (1.0 / v_cell) as f32
    } else {
        1.0
    };

    let mut data = Vec::with_capacity(n_voxels);
    let mut data_min = f32::MAX;
    let mut data_max = f32::MIN;

    for line in lines {
        for token in line.split_whitespace() {
            if let Ok(mut val) = token.parse::<f32>() {
                val *= normalization;
                if val < data_min { data_min = val; }
                if val > data_max { data_max = val; }
                data.push(val);
                if data.len() == n_voxels {
                    break;
                }
            }
        }
        if data.len() >= n_voxels {
            break;
        }
    }

    if data.len() != n_voxels {
        return Err(format!("Expected {} voxels, but only parsed {}", n_voxels, data.len()));
    }

    state.volumetric_data = Some(VolumetricData {
        grid_dims,
        lattice: state.get_lattice_col_major(),
        data,
        data_min,
        data_max,
        source_format: format,
        origin: [0.0, 0.0, 0.0],
    });

    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// $\mathbf{a} = 5.4307\,\text{\AA}$ cubic cell, 1 Si atom, uniform charge density.
    /// ColMajor lattice determinant: $V_{\text{cell}} = 5.4307^3 \approx 160.1\,\text{\AA}^3$
    fn make_chgcar_10x10x10(raw_value: f64, scale: f64) -> String {
        let n = 10;
        let data_line: String = std::iter::repeat(format!("  {:.6E}", raw_value))
            .take(5)
            .collect::<Vec<_>>()
            .join("");
        let rows: String = std::iter::repeat(data_line + "\n")
            .take((n * n * n + 4) / 5)
            .collect();
        format!(
            "Si test\n\
             {scale:.6}\n\
             5.430700  0.000000  0.000000\n\
             0.000000  5.430700  0.000000\n\
             0.000000  0.000000  5.430700\n\
             Si\n\
             1\n\
             Direct\n\
             0.000000  0.000000  0.000000\n\
             \n\
             {n}  {n}  {n}\n\
             {rows}",
            scale = scale,
            n = n,
            rows = rows
        )
    }

    fn write_tmp(content: &str, suffix: &str) -> NamedTempFile {
        let mut f = tempfile::Builder::new()
            .suffix(suffix)
            .tempfile()
            .unwrap();
        write!(f, "{}", content).unwrap();
        f
    }

    #[test]
    fn test_10x10x10_grid_dims() {
        let content = make_chgcar_10x10x10(1.0, 1.0);
        let tmp = write_tmp(&content, "CHGCAR");
        let state = parse_chgcar(tmp.path().to_str().unwrap())
            .expect("10x10x10 fixture must parse");
        let vol = state.volumetric_data.expect("volumetric_data must be Some");
        assert_eq!(vol.grid_dims, [10, 10, 10]);
    }

    #[test]
    fn test_10x10x10_lattice_matches_poscar_header() {
        let content = make_chgcar_10x10x10(1.0, 1.0);
        let tmp = write_tmp(&content, "CHGCAR");
        let state = parse_chgcar(tmp.path().to_str().unwrap())
            .expect("10x10x10 fixture must parse");
        let vol = state.volumetric_data.expect("volumetric_data must be Some");
        // ColMajor layout: a_x = lattice[0], b_x = lattice[3], c_x = lattice[6]
        assert!((vol.lattice[0] - 5.4307).abs() < 1e-4, "a_x mismatch: {}", vol.lattice[0]);
        assert!((vol.lattice[4] - 5.4307).abs() < 1e-4, "b_y mismatch: {}", vol.lattice[4]);
        assert!((vol.lattice[8] - 5.4307).abs() < 1e-4, "c_z mismatch: {}", vol.lattice[8]);
        assert!(vol.lattice[1].abs() < 1e-10, "off-diagonal a_y must be 0");
        assert!(vol.lattice[2].abs() < 1e-10, "off-diagonal a_z must be 0");
    }

    #[test]
    fn test_10x10x10_normalization_integral() {
        // Raw CHGCAR value = V_cell per voxel, so after normalization each voxel = 1.0 e/Ų.
        // Integral = sum_i rho(r_i) * dV = sum_i (raw/V_cell) * (V_cell/N) = raw
        let raw_value = 160.098_f64;
        let content = make_chgcar_10x10x10(raw_value, 1.0);
        let tmp = write_tmp(&content, "CHGCAR");
        let state = parse_chgcar(tmp.path().to_str().unwrap())
            .expect("10x10x10 fixture must parse");
        let vol = state.volumetric_data.expect("volumetric_data must be Some");
        let v_cell = 5.4307_f64.powi(3);
        let n_voxels = 1000_f64;
        // sum rho_i * dV = sum (raw/V_cell) * (V_cell/N) = raw
        let integral: f64 = vol.data.iter().map(|&v| v as f64).sum::<f64>() * (v_cell / n_voxels);
        assert!(
            (integral - raw_value).abs() / raw_value < 1e-3,
            "normalization integral = {integral:.4}, expected ≈ {raw_value:.4}"
        );
    }

    #[test]
    fn test_truncated_data_block_returns_err() {
        let mut content = make_chgcar_10x10x10(1.0, 1.0);
        // Chop the last 300 values (30% of 1000-voxel grid declared, only 700 present)
        let chop_at = content.rfind('\n').unwrap_or(content.len());
        let chop_at2 = content[..chop_at].rfind('\n').unwrap_or(chop_at);
        content.truncate(chop_at2);
        let tmp = write_tmp(&content, "CHGCAR");
        let result = parse_chgcar(tmp.path().to_str().unwrap());
        let err = result.err().expect("truncated data must return Err");
        assert!(err.contains("voxels"), "error must cite voxel count: got '{err}'");
    }

    #[test]
    fn test_grid_cap_exceeded_returns_err() {
        // Declare 151×151×151 = 3,442,951 > 150³ = 3,375,000
        let content = format!(
            "Oversized\n1.0\n5.0  0.0  0.0\n0.0  5.0  0.0\n0.0  0.0  5.0\nX\n1\nDirect\n0.0 0.0 0.0\n\n151 151 151\n 0.0\n"
        );
        let tmp = write_tmp(&content, "CHGCAR");
        let result = parse_chgcar(tmp.path().to_str().unwrap());
        let err = result.err().expect("oversized grid must be rejected");
        assert!(err.contains("150"), "error must cite the 150^3 limit: got '{err}'");
    }

    #[test]
    fn test_degenerate_lattice_vcell_guard_logic() {
        // spglib segfaults on coplanar/zero lattice vectors — this test validates
        // the v_cell < 1e-12 guard formula in isolation, without invoking parse_chgcar.
        // Coplanar: a = b = c = (1, 0, 0) \u2192 det = 0
        let l = [[1.0_f64, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
        let det = l[0][0] * (l[1][1] * l[2][2] - l[1][2] * l[2][1])
                - l[0][1] * (l[1][0] * l[2][2] - l[1][2] * l[2][0])
                + l[0][2] * (l[1][0] * l[2][1] - l[1][1] * l[2][0]);
        assert!(det.abs() < 1e-12, "coplanar lattice det must be < 1e-12: got {det}");
    }

    #[test]
    fn test_locpot_skips_normalization() {
        let content = make_chgcar_10x10x10(1.0, 1.0);
        let tmp = tempfile::Builder::new()
            .suffix("LOCPOT")
            .tempfile()
            .unwrap();
        write!(tmp.as_file(), "{}", content).unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let state = parse_chgcar(&path).expect("LOCPOT must parse");
        drop(tmp);
        let vol = state.volumetric_data.expect("volumetric_data must be Some");
        // For LOCPOT: normalization = 1.0, raw value = 1.0 → stored = 1.0
        assert!((vol.data[0] - 1.0_f32).abs() < 1e-5,
            "LOCPOT values must not be divided by V_cell: got {}", vol.data[0]);
        assert!(matches!(vol.source_format, crate::volumetric::VolumetricFormat::VaspLocpot));
    }

    #[test]
    fn test_scale_factor_applies_to_lattice() {
        // Scale = 2.0 → actual lattice vectors are 2.0 × 5.4307 = 10.8614 Å
        let content = make_chgcar_10x10x10(1.0, 2.0);
        let tmp = write_tmp(&content, "CHGCAR");
        let state = parse_chgcar(tmp.path().to_str().unwrap())
            .expect("scaled fixture must parse");
        let vol = state.volumetric_data.expect("volumetric_data must be Some");
        assert!((vol.lattice[0] - 5.4307 * 2.0).abs() < 1e-4,
            "scaled a_x must be 10.8614, got {}", vol.lattice[0]);
    }

    #[test]
    fn test_missing_grid_line_returns_err() {
        // Omit the "10 10 10" grid line — parser must not find it and return Err
        let content = format!(
            "No grid\n1.0\n5.0 0.0 0.0\n0.0 5.0 0.0\n0.0 0.0 5.0\nX\n1\nDirect\n0.0 0.0 0.0\n"
        );
        let tmp = write_tmp(&content, "CHGCAR");
        let result = parse_chgcar(tmp.path().to_str().unwrap());
        let err = result.err().expect("missing grid dims must return Err");
        assert!(err.contains("NGX"), "error must cite NGX NGY NGZ: got '{err}'");
    }
}

