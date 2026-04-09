//! Gaussian .cube parser for CrystalCanvas
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::crystal_state::CrystalState;
use crate::volumetric::{VolumetricData, VolumetricFormat};
use std::fs;

const BOHR_TO_ANGSTROM: f64 = 0.529177249;

pub fn parse_cube(path: &str) -> Result<CrystalState, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let mut lines = content.lines();

    lines.next().ok_or("Empty file")?;
    lines.next().ok_or("Missing second comment line")?;

    let line3 = lines.next().ok_or("Missing atom count line")?;
    let parts3: Vec<&str> = line3.split_whitespace().collect();
    if parts3.len() < 4 {
        return Err("Invalid atom count / origin line".to_string());
    }

    let n_atoms_raw: isize = parts3[0].parse().map_err(|_| "Invalid atom count")?;
    let n_atoms = n_atoms_raw.abs() as usize;

    let mut origin = [
        parts3[1].parse::<f64>().map_err(|_| "Invalid origin X")?,
        parts3[2].parse::<f64>().map_err(|_| "Invalid origin Y")?,
        parts3[3].parse::<f64>().map_err(|_| "Invalid origin Z")?,
    ];

    let mut grid_dims = [0usize; 3];
    let mut lattice_vecs = [[0.0; 3]; 3];
    let mut is_bohr = true;

    for i in 0..3 {
        let line = lines.next().ok_or("Missing voxel axis line")?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            return Err("Invalid voxel axis line".to_string());
        }

        let n_voxels_raw: isize = parts[0].parse().map_err(|_| "Invalid voxel count")?;
        
        if i == 0 && n_voxels_raw < 0 {
            is_bohr = false;
        }

        grid_dims[i] = n_voxels_raw.abs() as usize;
        
        let dx: f64 = parts[1].parse().map_err(|_| "Invalid dx")?;
        let dy: f64 = parts[2].parse().map_err(|_| "Invalid dy")?;
        let dz: f64 = parts[3].parse().map_err(|_| "Invalid dz")?;

        lattice_vecs[i] = [
            grid_dims[i] as f64 * dx,
            grid_dims[i] as f64 * dy,
            grid_dims[i] as f64 * dz,
        ];
    }

    let unit_scale = if is_bohr { BOHR_TO_ANGSTROM } else { 1.0 };

    for i in 0..3 {
        origin[i] *= unit_scale;
        for j in 0..3 {
            lattice_vecs[i][j] *= unit_scale;
        }
    }

    let n_voxels = grid_dims[0] * grid_dims[1] * grid_dims[2];
    if n_voxels > 150 * 150 * 150 {
        return Err(format!("Grid size {}x{}x{} exceeds Phase A limit of 150^3", grid_dims[0], grid_dims[1], grid_dims[2]));
    }

    if grid_dims[0] == 0 || grid_dims[1] == 0 || grid_dims[2] == 0 {
        return Err("Degenerate grid dimensions".to_string());
    }

    let det = lattice_vecs[0][0] * (lattice_vecs[1][1] * lattice_vecs[2][2] - lattice_vecs[1][2] * lattice_vecs[2][1])
            - lattice_vecs[0][1] * (lattice_vecs[1][0] * lattice_vecs[2][2] - lattice_vecs[1][2] * lattice_vecs[2][0])
            + lattice_vecs[0][2] * (lattice_vecs[1][0] * lattice_vecs[2][1] - lattice_vecs[1][1] * lattice_vecs[2][0]);
    if det.abs() < 1e-12 {
        return Err("Degenerate lattice: cell volume is zero".to_string());
    }



    let mut state = CrystalState {
        name: format!("Gaussian Cube: {}", std::path::Path::new(path).file_name().unwrap_or_default().to_string_lossy()),
        version: 1,
        intrinsic_sites: n_atoms,
        ..Default::default()
    };

    let norm = |v: &[f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let dot = |a: &[f64; 3], b: &[f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

    state.cell_a = norm(&lattice_vecs[0]);
    state.cell_b = norm(&lattice_vecs[1]);
    state.cell_c = norm(&lattice_vecs[2]);

    if state.cell_b > 1e-12 && state.cell_c > 1e-12 {
        state.cell_alpha = (dot(&lattice_vecs[1], &lattice_vecs[2]) / (state.cell_b * state.cell_c)).acos().to_degrees();
    } else { state.cell_alpha = 90.0; }
    
    if state.cell_a > 1e-12 && state.cell_c > 1e-12 {
        state.cell_beta = (dot(&lattice_vecs[0], &lattice_vecs[2]) / (state.cell_a * state.cell_c)).acos().to_degrees();
    } else { state.cell_beta = 90.0; }
    
    if state.cell_a > 1e-12 && state.cell_b > 1e-12 {
        state.cell_gamma = (dot(&lattice_vecs[0], &lattice_vecs[1]) / (state.cell_a * state.cell_b)).acos().to_degrees();
    } else { state.cell_gamma = 90.0; }

    // $M^{-1}$: inverse of original lattice for absolute-to-fractional conversion
    let inv_det = 1.0 / det;
    let (ax, ay, az) = (lattice_vecs[0][0], lattice_vecs[0][1], lattice_vecs[0][2]);
    let (bx, by, bz) = (lattice_vecs[1][0], lattice_vecs[1][1], lattice_vecs[1][2]);
    let (cx, cy, cz) = (lattice_vecs[2][0], lattice_vecs[2][1], lattice_vecs[2][2]);
    let inv00 = (by * cz - bz * cy) * inv_det;
    let inv01 = (bz * cx - bx * cz) * inv_det;
    let inv02 = (bx * cy - by * cx) * inv_det;
    let inv10 = (cy * az - cz * ay) * inv_det;
    let inv11 = (cz * ax - cx * az) * inv_det;
    let inv12 = (cx * ay - cy * ax) * inv_det;
    let inv20 = (ay * bz - az * by) * inv_det;
    let inv21 = (az * bx - ax * bz) * inv_det;
    let inv22 = (ax * by - ay * bx) * inv_det;

    for _ in 0..n_atoms {
        let line = lines.next().ok_or("Missing atom line")?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            return Err("Invalid atom line".to_string());
        }

        let at_num: u8 = parts[0].parse().map_err(|_| "Invalid atomic number")?;

        let x: f64 = parts[2].parse().map_err(|_| "Invalid atom X")?;
        let y: f64 = parts[3].parse().map_err(|_| "Invalid atom Y")?;
        let z: f64 = parts[4].parse().map_err(|_| "Invalid atom Z")?;

        let dx = x * unit_scale - origin[0];
        let dy = y * unit_scale - origin[1];
        let dz = z * unit_scale - origin[2];

        state.fract_x.push(inv00 * dx + inv01 * dy + inv02 * dz);
        state.fract_y.push(inv10 * dx + inv11 * dy + inv12 * dz);
        state.fract_z.push(inv20 * dx + inv21 * dy + inv22 * dz);
        state.atomic_numbers.push(at_num);
        let elem = crate::io::import::get_element_symbol(at_num);
        state.labels.push(elem.clone());
        state.elements.push(elem);
        state.occupancies.push(1.0);
    }

    if n_atoms_raw < 0 {
        let _mo_line = lines.next().ok_or("Missing MO line due to negative atom count")?;
    }

    state.fractional_to_cartesian();
    state.detect_spacegroup();

    let mut data = vec![0.0f32; n_voxels];
    let mut data_min = f32::MAX;
    let mut data_max = f32::MIN;
    let mut parsed_count = 0;

    for line in lines {
        for token in line.split_whitespace() {
            if let Ok(val) = token.parse::<f32>() {
                if val < data_min { data_min = val; }
                if val > data_max { data_max = val; }
                
                let iz = parsed_count % grid_dims[2];
                let iy = (parsed_count / grid_dims[2]) % grid_dims[1];
                let ix = parsed_count / (grid_dims[2] * grid_dims[1]);
                let f_idx = ix + iy * grid_dims[0] + iz * grid_dims[0] * grid_dims[1];
                
                if f_idx < n_voxels {
                    data[f_idx] = val;
                }
                
                parsed_count += 1;
                if parsed_count == n_voxels {
                    break;
                }
            }
        }
        if parsed_count >= n_voxels {
            break;
        }
    }

    if parsed_count != n_voxels {
        return Err(format!("Expected {} voxels, but only parsed {}", n_voxels, parsed_count));
    }

    let std_lattice = state.get_lattice_col_major();
    state.volumetric_data = Some(VolumetricData {
        grid_dims,
        lattice: std_lattice,
        data,
        data_min,
        data_max,
        source_format: VolumetricFormat::GaussianCube,
        origin: [0.0, 0.0, 0.0],
    });

    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    const BOHR: f64 = 0.529177249;

    /// $\mathbf{a_i} = N \cdot \Delta r_i \cdot a_0$ cubic cell, 1 H atom at origin.
    /// Grid: $5 \times 5 \times 5$, spacing $0.2\,a_0$.
    /// Origin: $(1.0, 0.0, 0.0)\,a_0$.
    fn make_cube_5x5x5(uniform_val: f32, is_bohr: bool) -> String {
        let n_sign = if is_bohr { 5isize } else { -5isize };
        let origin_x = 1.0_f64;
        let data_rows: String = (0..25).map(|_| {
            (0..5).map(|_| format!(" {:12.5E}", uniform_val)).collect::<Vec<_>>().join("")
                + "\n"
        }).collect();

        format!(
            "H atom cube fixture\nGenerated by CrystalCanvas test suite\n\
             1  {ox:.6}  0.000000  0.000000\n\
             {n}  0.200000  0.000000  0.000000\n\
             {n}  0.000000  0.200000  0.000000\n\
             {n}  0.000000  0.000000  0.200000\n\
             1  0.000000  0.000000  0.000000  0.000000\n\
             {data}",
            ox = origin_x,
            n = n_sign,
            data = data_rows
        )
    }

    /// Fixture with unique per-voxel values encoding C-order position.
    /// Raw value at C-pos $p$ = $p$ (as f32 float).
    /// After F-order reindex: $\text{data}[i_x + i_y N_x + i_z N_x N_y] = i_x N_y N_z + i_y N_z + i_z$
    fn make_cube_5x5x5_indexed() -> String {
        let n = 5usize;
        let total = n * n * n;
        let vals: Vec<String> = (0..total).map(|i| format!(" {:12.5E}", i as f32)).collect();
        let rows: String = vals.chunks(5).map(|c| c.join("") + "\n").collect();
        format!(
            "Indexed cube fixture\nC-to-F reorder test\n\
             1  0.000000  0.000000  0.000000\n\
             5  0.200000  0.000000  0.000000\n\
             5  0.000000  0.200000  0.000000\n\
             5  0.000000  0.000000  0.200000\n\
             1  0.000000  0.000000  0.000000  0.000000\n\
             {rows}",
            rows = rows
        )
    }

    fn write_tmp(content: &str, suffix: &str) -> NamedTempFile {
        let mut f = tempfile::Builder::new().suffix(suffix).tempfile().unwrap();
        write!(f, "{}", content).unwrap();
        f
    }

    #[test]
    fn test_5x5x5_grid_dims() {
        let content = make_cube_5x5x5(1.0, true);
        let tmp = write_tmp(&content, ".cube");
        let state = parse_cube(tmp.path().to_str().unwrap())
            .expect("5x5x5 Bohr fixture must parse");
        let vol = state.volumetric_data.expect("volumetric_data must be Some");
        assert_eq!(vol.grid_dims, [5, 5, 5]);
    }

    #[test]
    fn test_5x5x5_bohr_origin_conversion() {
        // After normalization, origin is always zeroed; original offset baked into fractional coords
        let content = make_cube_5x5x5(1.0, true);
        let tmp = write_tmp(&content, ".cube");
        let state = parse_cube(tmp.path().to_str().unwrap())
            .expect("5x5x5 Bohr fixture must parse");
        let vol = state.volumetric_data.expect("volumetric_data must be Some");
        assert!(vol.origin[0].abs() < 1e-12, "origin must be zeroed after normalization: got {}", vol.origin[0]);
        assert!(vol.origin[1].abs() < 1e-12, "origin_y must be 0");
        assert!(vol.origin[2].abs() < 1e-12, "origin_z must be 0");
    }

    #[test]
    fn test_5x5x5_bohr_lattice_conversion() {
        // Voxel spacing: 0.2 Bohr. Total lattice vector a = 5 * 0.2 * BOHR_TO_ANG
        let content = make_cube_5x5x5(1.0, true);
        let tmp = write_tmp(&content, ".cube");
        let state = parse_cube(tmp.path().to_str().unwrap())
            .expect("5x5x5 Bohr fixture must parse");
        let vol = state.volumetric_data.expect("volumetric_data must be Some");
        let expected_a = 5.0 * 0.2 * BOHR;
        // ColMajor: lattice[0]=a_x, lattice[4]=b_y, lattice[8]=c_z
        assert!((vol.lattice[0] - expected_a).abs() < 1e-9,
            "a_x must be {expected_a:.9} Å: got {:.9}", vol.lattice[0]);
        assert!((vol.lattice[4] - expected_a).abs() < 1e-9,
            "b_y must be {expected_a:.9} Å: got {:.9}", vol.lattice[4]);
        assert!((vol.lattice[8] - expected_a).abs() < 1e-9,
            "c_z must be {expected_a:.9} Å: got {:.9}", vol.lattice[8]);
    }

    #[test]
    fn test_5x5x5_atom_count_and_position() {
        let content = make_cube_5x5x5(1.0, true);
        let tmp = write_tmp(&content, ".cube");
        let state = parse_cube(tmp.path().to_str().unwrap())
            .expect("5x5x5 Bohr fixture must parse");
        assert_eq!(state.elements.len(), 1, "atom count");
        assert_eq!(state.atomic_numbers[0], 1, "H must be Z=1");
        assert!((state.cart_positions[0][0] + BOHR as f32).abs() < 1e-5,
            "H x must be -BOHR Å: got {}", state.cart_positions[0][0]);
    }

    #[test]
    fn test_5x5x5_angstrom_mode_no_conversion() {
        // After normalization, origin zeroed; lattice is standardized PDB matrix
        let content = make_cube_5x5x5(1.0, false);
        let tmp = write_tmp(&content, ".cube");
        let state = parse_cube(tmp.path().to_str().unwrap())
            .expect("5x5x5 Angstrom fixture must parse");
        let vol = state.volumetric_data.expect("volumetric_data must be Some");
        assert!(vol.origin[0].abs() < 1e-12,
            "origin must be zeroed after normalization: got {}", vol.origin[0]);
        let expected_a = 5.0 * 0.2;
        assert!((vol.lattice[0] - expected_a).abs() < 1e-9,
            "a_x in standardized frame must be {expected_a}: got {}", vol.lattice[0]);
    }

    #[test]
    fn test_c_to_fortran_reorder() {
        // Cube C-order raw value at position p = p (f32).
        // After F-order reindex: data[ix + iy*Nx + iz*Nx*Ny] = ix*Ny*Nz + iy*Nz + iz
        let content = make_cube_5x5x5_indexed();
        let tmp = write_tmp(&content, ".cube");
        let state = parse_cube(tmp.path().to_str().unwrap())
            .expect("indexed fixture must parse");
        let vol = state.volumetric_data.expect("volumetric must be Some");
        let nx = 5; let ny = 5; let nz = 5;
        for ix in 0..nx {
            for iy in 0..ny {
                for iz in 0..nz {
                    let expected = (ix * ny * nz + iy * nz + iz) as f32;
                    let f_idx = ix + iy * nx + iz * nx * ny;
                    assert!((vol.data[f_idx] - expected).abs() < 0.5,
                        "data[{ix},{iy},{iz}] = {} ≠ expected {expected}", vol.data[f_idx]);
                }
            }
        }
    }

    #[test]
    fn test_grid_cap_exceeded_returns_err() {
        let content = format!(
            "Oversized\ntest\n1  0.0  0.0  0.0\n151  0.1  0.0  0.0\n151  0.0  0.1  0.0\n151  0.0  0.0  0.1\n1  0.0  0.0  0.0  0.0  0.0000\n 0.0\n"
        );
        let tmp = write_tmp(&content, ".cube");
        let err = parse_cube(tmp.path().to_str().unwrap()).err().expect("oversized must fail");
        assert!(err.contains("150"), "error must cite 150^3 limit: got '{err}'");
    }

    #[test]
    fn test_truncated_data_returns_err() {
        let mut content = make_cube_5x5x5(1.0, true);
        let chop = content.rfind('\n').and_then(|p| content[..p].rfind('\n')).unwrap_or(0);
        content.truncate(chop);
        let tmp = write_tmp(&content, ".cube");
        let err = parse_cube(tmp.path().to_str().unwrap()).err().expect("truncated must fail");
        assert!(err.contains("voxels"), "error must cite voxels: got '{err}'");
    }

    #[test]
    fn test_missing_atom_line_returns_err() {
        let content = "Missing atoms\ntest\n1  0.0  0.0  0.0\n5  0.2  0.0  0.0\n5  0.0  0.2  0.0\n5  0.0  0.0  0.2\n";
        let tmp = write_tmp(content, ".cube");
        let err = parse_cube(tmp.path().to_str().unwrap()).err().expect("missing atom must fail");
        assert!(err.to_lowercase().contains("atom") || err.to_lowercase().contains("voxel"),
            "error must cite atom or voxel: got '{err}'");
    }

    #[test]
    fn test_empty_file_returns_err() {
        let tmp = write_tmp("", ".cube");
        let err = parse_cube(tmp.path().to_str().unwrap()).err().expect("empty file must fail");
        assert!(!err.is_empty(), "must return non-empty error message");
    }
}

