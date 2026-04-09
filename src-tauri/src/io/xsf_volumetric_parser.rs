use crate::crystal_state::CrystalState;
use crate::volumetric::{VolumetricData, VolumetricFormat};
use std::fs;
use std::path::Path;

pub fn parse_xsf_volumetric(path: &str) -> Result<CrystalState, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    
    let mut elems = Vec::new();
    let mut cart_pos = Vec::new();
    let mut v1 = [0.0; 3];
    let mut v2 = [0.0; 3];
    let mut v3 = [0.0; 3];
    let mut has_lattice = false;

    let mut grid_dims = [0usize; 3];
    let mut origin = [0.0f64; 3];
    let mut lattice_vecs = [[0.0f64; 3]; 3];
    let mut data = Vec::new();
    let mut data_min = f32::MAX;
    let mut data_max = f32::MIN;
    let mut has_grid = false;

    let mut lines = content.lines();
    
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed == "PRIMVEC" {
            for i in 0..3 {
                if let Some(v_line) = lines.next() {
                    let parts: Vec<&str> = v_line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let x = parts[0].parse().unwrap_or(0.0);
                        let y = parts[1].parse().unwrap_or(0.0);
                        let z = parts[2].parse().unwrap_or(0.0);
                        match i {
                            0 => v1 = [x, y, z],
                            1 => v2 = [x, y, z],
                            2 => v3 = [x, y, z],
                            _ => {}
                        }
                        if i == 2 {
                            has_lattice = true;
                        }
                    }
                }
            }
        } else if trimmed.starts_with("PRIMCOORD") {
            if let Some(num_line) = lines.next() {
                let parts: Vec<&str> = num_line.split_whitespace().collect();
                if !parts.is_empty() {
                    let mut num_atoms: usize = parts[0].parse().unwrap_or(0);
                    elems.reserve(num_atoms);
                    cart_pos.reserve(num_atoms);
                    while num_atoms > 0 {
                        if let Some(a_line) = lines.next() {
                            let a_parts: Vec<&str> = a_line.split_whitespace().collect();
                            if a_parts.len() >= 4 {
                                let at_num: u8 = a_parts[0].parse().unwrap_or_else(|_| {
                                    crate::io::import::get_atomic_number(a_parts[0])
                                });
                                let sym = crate::io::import::get_element_symbol(at_num);
                                let x = a_parts[1].parse().unwrap_or(0.0);
                                let y = a_parts[2].parse().unwrap_or(0.0);
                                let z = a_parts[3].parse().unwrap_or(0.0);
                                elems.push(sym);
                                cart_pos.push([x, y, z]);
                            } else {
                                return Err("Invalid atom coordinate line".to_string());
                            }
                        }
                        num_atoms -= 1;
                    }
                }
            }
        } else if trimmed.starts_with("BEGIN_DATAGRID_3D") {
            let dim_line = lines.next().ok_or("Missing grid dimensions line")?;
            let dims: Vec<&str> = dim_line.split_whitespace().collect();
            if dims.len() < 3 {
                return Err("Invalid grid dimensions".to_string());
            }
            grid_dims[0] = dims[0].parse().unwrap_or(0);
            grid_dims[1] = dims[1].parse().unwrap_or(0);
            grid_dims[2] = dims[2].parse().unwrap_or(0);

            if grid_dims[0] == 0 || grid_dims[1] == 0 || grid_dims[2] == 0 {
                return Err("Degenerate grid dimensions".to_string());
            }

            let n_voxels = grid_dims[0] * grid_dims[1] * grid_dims[2];
            if n_voxels > 150 * 150 * 150 {
                return Err(format!("Grid size {}x{}x{} exceeds limit of 150^3", grid_dims[0], grid_dims[1], grid_dims[2]));
            }
            
            data.reserve(n_voxels);

            let o_line = lines.next().ok_or("Missing origin line")?;
            let o_parts: Vec<&str> = o_line.split_whitespace().collect();
            if o_parts.len() < 3 {
                return Err("Invalid origin line".to_string());
            }
            origin[0] = o_parts[0].parse().unwrap_or(0.0);
            origin[1] = o_parts[1].parse().unwrap_or(0.0);
            origin[2] = o_parts[2].parse().unwrap_or(0.0);

            for i in 0..3 {
                let l_line = lines.next().ok_or("Missing grid vector line")?;
                let l_parts: Vec<&str> = l_line.split_whitespace().collect();
                if l_parts.len() < 3 {
                    return Err("Invalid grid vector line".to_string());
                }
                lattice_vecs[i][0] = l_parts[0].parse().unwrap_or(0.0);
                lattice_vecs[i][1] = l_parts[1].parse().unwrap_or(0.0);
                lattice_vecs[i][2] = l_parts[2].parse().unwrap_or(0.0);
            }

            while let Some(data_line) = lines.next() {
                let d_trimmed = data_line.trim();
                if d_trimmed == "END_DATAGRID_3D" {
                    break;
                }
                for token in d_trimmed.split_whitespace() {
                    if let Ok(val) = token.parse::<f32>() {
                        if val < data_min { data_min = val; }
                        if val > data_max { data_max = val; }
                        if data.len() < n_voxels {
                            data.push(val);
                        }
                    }
                }
                if data.len() >= n_voxels {
                    break;
                }
            }

            if data.len() != n_voxels {
                return Err(format!("Expected {} voxels, but parsed {}", n_voxels, data.len()));
            }
            
            has_grid = true;
        }
    }

    if !has_grid {
        return Err("No DATAGRID_3D block found".to_string());
    }

    let det = lattice_vecs[0][0] * (lattice_vecs[1][1] * lattice_vecs[2][2] - lattice_vecs[1][2] * lattice_vecs[2][1])
            - lattice_vecs[0][1] * (lattice_vecs[1][0] * lattice_vecs[2][2] - lattice_vecs[1][2] * lattice_vecs[2][0])
            + lattice_vecs[0][2] * (lattice_vecs[1][0] * lattice_vecs[2][1] - lattice_vecs[1][1] * lattice_vecs[2][0]);
    
    if det.abs() < 1e-12 {
        return Err("Degenerate volumetric lattice: cell volume is zero".to_string());
    }



    let mut state = CrystalState {
        name: format!("XSF: {}", Path::new(path).file_name().unwrap_or_default().to_string_lossy()),
        version: 1,
        intrinsic_sites: elems.len(),
        ..Default::default()
    };

    let norm = |v: &[f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let dot = |a: &[f64; 3], b: &[f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

    if has_lattice {
        state.cell_a = norm(&v1);
        state.cell_b = norm(&v2);
        state.cell_c = norm(&v3);
        
        if state.cell_b > 1e-12 && state.cell_c > 1e-12 {
            state.cell_alpha = (dot(&v2, &v3) / (state.cell_b * state.cell_c)).acos().to_degrees();
        } else { state.cell_alpha = 90.0; }
        
        if state.cell_a > 1e-12 && state.cell_c > 1e-12 {
            state.cell_beta = (dot(&v1, &v3) / (state.cell_a * state.cell_c)).acos().to_degrees();
        } else { state.cell_beta = 90.0; }
        
        if state.cell_a > 1e-12 && state.cell_b > 1e-12 {
            state.cell_gamma = (dot(&v1, &v2) / (state.cell_a * state.cell_b)).acos().to_degrees();
        } else { state.cell_gamma = 90.0; }
    } else {
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
    }

    // $M^{-1}$: inverse of grid lattice for absolute-to-fractional conversion
    let inv_det = 1.0 / det;
    let (gax, gay, gaz) = (lattice_vecs[0][0], lattice_vecs[0][1], lattice_vecs[0][2]);
    let (gbx, gby, gbz) = (lattice_vecs[1][0], lattice_vecs[1][1], lattice_vecs[1][2]);
    let (gcx, gcy, gcz) = (lattice_vecs[2][0], lattice_vecs[2][1], lattice_vecs[2][2]);
    let inv00 = (gby * gcz - gbz * gcy) * inv_det;
    let inv01 = (gbz * gcx - gbx * gcz) * inv_det;
    let inv02 = (gbx * gcy - gby * gcx) * inv_det;
    let inv10 = (gcy * gaz - gcz * gay) * inv_det;
    let inv11 = (gcz * gax - gcx * gaz) * inv_det;
    let inv12 = (gcx * gay - gcy * gax) * inv_det;
    let inv20 = (gay * gbz - gaz * gby) * inv_det;
    let inv21 = (gaz * gbx - gax * gbz) * inv_det;
    let inv22 = (gax * gby - gay * gbx) * inv_det;

    if !elems.is_empty() {
        for (i, elem) in elems.into_iter().enumerate() {
            let at_num = crate::io::import::get_atomic_number(&elem);
            let x = cart_pos[i][0];
            let y = cart_pos[i][1];
            let z = cart_pos[i][2];

            let dx = x - origin[0];
            let dy = y - origin[1];
            let dz = z - origin[2];

            state.fract_x.push(inv00 * dx + inv01 * dy + inv02 * dz);
            state.fract_y.push(inv10 * dx + inv11 * dy + inv12 * dz);
            state.fract_z.push(inv20 * dx + inv21 * dy + inv22 * dz);
            state.atomic_numbers.push(at_num);
            state.labels.push(elem.clone());
            state.elements.push(elem);
            state.occupancies.push(1.0);
        }
        state.fractional_to_cartesian();
        state.detect_spacegroup();
    }

    let std_lattice = state.get_lattice_col_major();
    state.volumetric_data = Some(VolumetricData {
        grid_dims,
        lattice: std_lattice,
        data,
        data_min,
        data_max,
        source_format: VolumetricFormat::Xsf,
        origin: [0.0, 0.0, 0.0],
    });

    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// 5×5×5 cubic cell, 1 H at origin.
    /// Grid: $5^3$ voxels, spacing $1.0\,\text{Å}$.
    /// Origin: $(0.5, 0.5, 0.5)\,\text{Å}$.
    fn make_xsf_5x5x5(uniform_val: f32) -> String {
        let vals: Vec<String> = (0..125).map(|_| format!("{:12.5E}", uniform_val)).collect();
        let rows: String = vals.chunks(5)
            .map(|c| " ".to_string() + &c.join(" ") + "\n")
            .collect();
        format!(
            "CRYSTAL\nPRIMVEC\n 5.0 0.0 0.0\n 0.0 5.0 0.0\n 0.0 0.0 5.0\n\
             PRIMCOORD\n 1 1\n 1  0.0 0.0 0.0\n\
             BEGIN_DATAGRID_3D\n 5 5 5\n 0.5 0.5 0.5\n\
             5.0 0.0 0.0\n 0.0 5.0 0.0\n 0.0 0.0 5.0\n{rows}END_DATAGRID_3D\n",
            rows = rows
        )
    }

    /// Fixture with unique per-voxel values encoding sequential XSF order.
    /// XSF native layout is x-fastest; no reorder applied by parser.
    /// After parse: $\text{data}[i] = i$ for $i \in [0, 124]$.
    fn make_xsf_5x5x5_indexed() -> String {
        let vals: Vec<String> = (0..125usize).map(|i| format!("{:12.5E}", i as f32)).collect();
        let rows: String = vals.chunks(5)
            .map(|c| " ".to_string() + &c.join(" ") + "\n")
            .collect();
        format!(
            "CRYSTAL\nPRIMVEC\n 5.0 0.0 0.0\n 0.0 5.0 0.0\n 0.0 0.0 5.0\n\
             PRIMCOORD\n 1 1\n 1  0.0 0.0 0.0\n\
             BEGIN_DATAGRID_3D\n 5 5 5\n 0.5 0.5 0.5\n\
             5.0 0.0 0.0\n 0.0 5.0 0.0\n 0.0 0.0 5.0\n{rows}END_DATAGRID_3D\n",
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
        let content = make_xsf_5x5x5(1.0);
        let tmp = write_tmp(&content, ".xsf");
        let state = parse_xsf_volumetric(tmp.path().to_str().unwrap())
            .expect("5x5x5 XSF fixture must parse");
        let vol = state.volumetric_data.expect("volumetric_data must be Some");
        assert_eq!(vol.grid_dims, [5, 5, 5]);
    }

    #[test]
    fn test_5x5x5_origin() {
        // After normalization, origin is zeroed; offset baked into fractional coords
        let content = make_xsf_5x5x5(1.0);
        let tmp = write_tmp(&content, ".xsf");
        let state = parse_xsf_volumetric(tmp.path().to_str().unwrap())
            .expect("5x5x5 XSF fixture must parse");
        let vol = state.volumetric_data.expect("volumetric_data must be Some");
        assert!(vol.origin[0].abs() < 1e-12, "origin must be zeroed: got {}", vol.origin[0]);
        assert!(vol.origin[1].abs() < 1e-12, "origin must be zeroed: got {}", vol.origin[1]);
        assert!(vol.origin[2].abs() < 1e-12, "origin must be zeroed: got {}", vol.origin[2]);
    }

    #[test]
    fn test_5x5x5_lattice_colmajor() {
        // Grid vecs: (5,0,0),(0,5,0),(0,0,5) → ColMajor diagonal = 5.0
        let content = make_xsf_5x5x5(1.0);
        let tmp = write_tmp(&content, ".xsf");
        let state = parse_xsf_volumetric(tmp.path().to_str().unwrap())
            .expect("5x5x5 XSF fixture must parse");
        let vol = state.volumetric_data.expect("volumetric_data must be Some");
        // ColMajor: lattice[0]=v1_x, lattice[4]=v2_y, lattice[8]=v3_z
        assert!((vol.lattice[0] - 5.0).abs() < 1e-9, "v1_x: got {}", vol.lattice[0]);
        assert!((vol.lattice[4] - 5.0).abs() < 1e-9, "v2_y: got {}", vol.lattice[4]);
        assert!((vol.lattice[8] - 5.0).abs() < 1e-9, "v3_z: got {}", vol.lattice[8]);
        assert!(vol.lattice[1].abs() < 1e-12, "v1_y must be 0");
        assert!(vol.lattice[2].abs() < 1e-12, "v1_z must be 0");
        assert!(vol.lattice[3].abs() < 1e-12, "v2_x must be 0");
    }

    #[test]
    fn test_5x5x5_atom_and_primvec() {
        // PRIMVEC → cell_a/b/c = 5.0, H at origin
        let content = make_xsf_5x5x5(1.0);
        let tmp = write_tmp(&content, ".xsf");
        let state = parse_xsf_volumetric(tmp.path().to_str().unwrap())
            .expect("5x5x5 XSF fixture must parse");
        assert_eq!(state.elements.len(), 1, "atom count");
        assert_eq!(state.atomic_numbers[0], 1, "H must be Z=1");
        assert!((state.cell_a - 5.0).abs() < 1e-9, "cell_a from PRIMVEC: got {}", state.cell_a);
        assert!((state.cell_alpha - 90.0).abs() < 1e-9, "alpha must be 90°");
    }

    #[test]
    fn test_5x5x5_data_sequential() {
        // XSF x-fastest order stored directly; data[i] == i
        let content = make_xsf_5x5x5_indexed();
        let tmp = write_tmp(&content, ".xsf");
        let state = parse_xsf_volumetric(tmp.path().to_str().unwrap())
            .expect("indexed XSF fixture must parse");
        let vol = state.volumetric_data.expect("volumetric must be Some");
        for i in 0..125usize {
            assert!((vol.data[i] - i as f32).abs() < 0.5,
                "data[{i}] = {} ≠ {i}", vol.data[i]);
        }
    }

    #[test]
    fn test_5x5x5_data_min_max() {
        let content = make_xsf_5x5x5_indexed();
        let tmp = write_tmp(&content, ".xsf");
        let state = parse_xsf_volumetric(tmp.path().to_str().unwrap())
            .expect("indexed XSF fixture must parse");
        let vol = state.volumetric_data.expect("volumetric must be Some");
        assert!((vol.data_min - 0.0).abs() < 0.5, "data_min: got {}", vol.data_min);
        assert!((vol.data_max - 124.0).abs() < 0.5, "data_max: got {}", vol.data_max);
    }

    #[test]
    fn test_no_datagrid_returns_err() {
        let content = "CRYSTAL\nPRIMVEC\n 5.0 0.0 0.0\n 0.0 5.0 0.0\n 0.0 0.0 5.0\nPRIMCOORD\n 1 1\n 1  0.0 0.0 0.0\n";
        let tmp = write_tmp(content, ".xsf");
        let err = parse_xsf_volumetric(tmp.path().to_str().unwrap())
            .err().expect("missing DATAGRID_3D must fail");
        assert!(err.to_lowercase().contains("datagrid") || err.to_lowercase().contains("grid"),
            "error must cite missing grid: got '{err}'");
    }

    #[test]
    fn test_grid_cap_exceeded_returns_err() {
        let content = "BEGIN_DATAGRID_3D\n 151 151 151\n 0.0 0.0 0.0\n 1.0 0.0 0.0\n 0.0 1.0 0.0\n 0.0 0.0 1.0\n 0.0\nEND_DATAGRID_3D\n";
        let tmp = write_tmp(content, ".xsf");
        let err = parse_xsf_volumetric(tmp.path().to_str().unwrap())
            .err().expect("oversized grid must fail");
        assert!(err.contains("150"), "error must cite 150^3 limit: got '{err}'");
    }

    #[test]
    fn test_truncated_data_returns_err() {
        let mut content = make_xsf_5x5x5(1.0);
        // Remove END_DATAGRID_3D sentinel and strip two data rows
        let chop = content.rfind("END_DATAGRID_3D").unwrap_or(content.len());
        content.truncate(chop);
        let rows_to_cut = content.rfind('\n')
            .and_then(|p| content[..p].rfind('\n'))
            .unwrap_or(0);
        content.truncate(rows_to_cut);
        let tmp = write_tmp(&content, ".xsf");
        let err = parse_xsf_volumetric(tmp.path().to_str().unwrap())
            .err().expect("truncated data must fail");
        assert!(err.contains("voxels"), "error must cite voxels: got '{err}'");
    }

    #[test]
    fn test_missing_origin_line_returns_err() {
        // File ends after dimension line — parser must not silently accept origin=[0,0,0]
        let content = "BEGIN_DATAGRID_3D\n 5 5 5\n";
        let tmp = write_tmp(content, ".xsf");
        let err = parse_xsf_volumetric(tmp.path().to_str().unwrap())
            .err().expect("missing origin must fail");
        assert!(err.to_lowercase().contains("origin") || err.to_lowercase().contains("missing"),
            "error must cite missing origin: got '{err}'");
    }

    #[test]
    fn test_degenerate_lattice_returns_err() {
        // Grid vecs v2 = v1 → det = 0 → degenerate cell
        let vals: Vec<String> = (0..125).map(|_| "0.0".to_string()).collect();
        let rows: String = vals.chunks(5)
            .map(|c| " ".to_string() + &c.join(" ") + "\n")
            .collect();
        let content = format!(
            "BEGIN_DATAGRID_3D\n 5 5 5\n 0.0 0.0 0.0\n\
             5.0 0.0 0.0\n 5.0 0.0 0.0\n 0.0 0.0 5.0\n{rows}END_DATAGRID_3D\n",
            rows = rows
        );
        let tmp = write_tmp(&content, ".xsf");
        let err = parse_xsf_volumetric(tmp.path().to_str().unwrap())
            .err().expect("degenerate lattice must fail");
        assert!(err.to_lowercase().contains("degenerate") || err.to_lowercase().contains("volume"),
            "error must cite degenerate lattice: got '{err}'");
    }

    #[test]
    fn test_invalid_atom_line_returns_err() {
        // Atom line with only 1 token → fewer than 4 required fields
        let content = "CRYSTAL\nPRIMVEC\n 5.0 0.0 0.0\n 0.0 5.0 0.0\n 0.0 0.0 5.0\nPRIMCOORD\n 1 1\n 1\n";
        let tmp = write_tmp(content, ".xsf");
        let err = parse_xsf_volumetric(tmp.path().to_str().unwrap())
            .err().expect("invalid atom line must fail");
        assert!(err.to_lowercase().contains("atom") || err.to_lowercase().contains("invalid"),
            "error must cite atom: got '{err}'");
    }

    #[test]
    fn test_empty_file_returns_err() {
        let tmp = write_tmp("", ".xsf");
        let err = parse_xsf_volumetric(tmp.path().to_str().unwrap())
            .err().expect("empty file must fail");
        assert!(!err.is_empty(), "must return non-empty error");
    }
}

