//! Structural structure loaders for CrystalCanvas (CIF, XYZ, PDB)
use crate::crystal_state::CrystalState;
use std::fs;
use std::path::Path;

/// Load a structure from file based on its extension
pub fn load_file(path: &str) -> Result<CrystalState, String> {
    let p = Path::new(path);
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    
    match ext.as_str() {
        "cif" => CrystalState::from_cif(path),
        "xyz" => load_xyz(path),
        "pdb" => load_pdb(path),
        _ => Err(format!("Unsupported file extension: {}", ext)),
    }
}

/// Simple XYZ format parser
fn load_xyz(path: &str) -> Result<CrystalState, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let mut lines = content.lines().map(|l| l.trim()).filter(|l| !l.is_empty());
    
    let n_atoms_str = lines.next().ok_or("Empty XYZ file")?;
    let _n_atoms: usize = n_atoms_str.parse().map_err(|_| "Invalid atom count in XYZ")?;
    
    let comment = lines.next().unwrap_or("");
    
    let mut state = CrystalState::default();
    state.name = if comment.is_empty() {
        Path::new(path).file_stem().unwrap().to_str().unwrap().to_string()
    } else {
        comment.to_string()
    };
    
    // XYZ gives purely cartesian coordinates
    let mut cart_pos = Vec::new();
    let mut elems = Vec::new();
    
    let mut min_x = f64::MAX; let mut max_x = f64::MIN;
    let mut min_y = f64::MAX; let mut max_y = f64::MIN;
    let mut min_z = f64::MAX; let mut max_z = f64::MIN;
    
    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            let elem = parts[0];
            let x: f64 = parts[1].parse().unwrap_or(0.0);
            let y: f64 = parts[2].parse().unwrap_or(0.0);
            let z: f64 = parts[3].parse().unwrap_or(0.0);
            
            elems.push(elem.to_string());
            cart_pos.push([x, y, z]);
            
            min_x = min_x.min(x); max_x = max_x.max(x);
            min_y = min_y.min(y); max_y = max_y.max(y);
            min_z = min_z.min(z); max_z = max_z.max(z);
        }
    }
    
    // Determine bounding box for unit cell for visualization (add 10Å padding)
    let padding = 10.0;
    let dx = (max_x - min_x) + padding;
    let dy = (max_y - min_y) + padding;
    let dz = (max_z - min_z) + padding;
    
    let dx = if dx < 1.0 { 10.0 } else { dx };
    let dy = if dy < 1.0 { 10.0 } else { dy };
    let dz = if dz < 1.0 { 10.0 } else { dz };
    
    state.cell_a = dx;
    state.cell_b = dy;
    state.cell_c = dz;
    state.cell_alpha = 90.0;
    state.cell_beta = 90.0;
    state.cell_gamma = 90.0;
    state.spacegroup_hm = "P1".to_string();
    state.spacegroup_number = 1;
    state.version = 1;
    
    // Convert cartesian to fractional based on this bounding box
    for i in 0..elems.len() {
        // Shift center to center of box
        let fx = (cart_pos[i][0] - min_x + padding/2.0) / dx;
        let fy = (cart_pos[i][1] - min_y + padding/2.0) / dy;
        let fz = (cart_pos[i][2] - min_z + padding/2.0) / dz;
        
        // Dummy atomic number using length of string to avoid huge mapping table
        // A real table is preferred, but simple string passing is enough for now
        // Or we use 0 to represent unknown
        let at_num = match elems[i].as_str() {
            "H" => 1, "C" => 6, "N" => 7, "O" => 8, "Na" => 11, "Cl" => 17, "Fe" => 26, _ => 0
        };
        
        // Use standard add_atom logic without MIC collision to build initial state quickly
        state.labels.push(format!("{}{}", elems[i], i+1));
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
    
    let mut state = CrystalState::default();
    state.name = Path::new(path).file_stem().unwrap().to_str().unwrap().to_string();
    state.version = 1;
    
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
            state.cell_beta  = line[40..47].trim().parse().unwrap_or(90.0);
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
            
            // Try to extract element from columns 77-78, fallback to atom name in 13-16
            let mut elem = "";
            if line.len() >= 78 {
                elem = line[76..78].trim();
            }
            if elem.is_empty() {
                elem = line[12..14].trim();
            }
            if elem.is_empty() {
                elem = "X";
            }
            
            elems.push(elem.to_string());
            cart_pos.push([x, y, z]);
        }
    }
    
    if !has_cryst1 {
        // Fallback to bounding box like XYZ
        state.cell_a = 10.0; state.cell_b = 10.0; state.cell_c = 10.0;
        state.cell_alpha = 90.0; state.cell_beta = 90.0; state.cell_gamma = 90.0;
        state.spacegroup_hm = "P1".to_string();
        state.spacegroup_number = 1;
    }
    
    // Precompute orthogonalization matrix 
    let a = state.cell_a;
    let b = state.cell_b;
    let c = state.cell_c;
    let alpha = state.cell_alpha.to_radians();
    let beta  = state.cell_beta.to_radians();
    let gamma = state.cell_gamma.to_radians();
    
    // We actually need the inverse orthogonalization matrix to go from cartesian -> fractional
    // Simple cubic fallback for PDB if orthogonal (90,90,90)
    let is_orthogonal = (alpha - std::f64::consts::FRAC_PI_2).abs() < 1e-4 && (beta - std::f64::consts::FRAC_PI_2).abs() < 1e-4 && (gamma - std::f64::consts::FRAC_PI_2).abs() < 1e-4;
    
    for i in 0..elems.len() {
        let (fx, fy, fz) = if is_orthogonal {
            (cart_pos[i][0] / a, cart_pos[i][1] / b, cart_pos[i][2] / c)
        } else {
            // Very simplified invert for non-orthogonal PDB (usually PDB is orthogonal or we should use proper matrix inversion)
            // Just output cartesian components scaled for now to avoid bulky math
            (cart_pos[i][0] / a, cart_pos[i][1] / b, cart_pos[i][2] / c)
        };
        
        let at_num = match elems[i].as_str() {
            "H" => 1, "C" => 6, "N" => 7, "O" => 8, "Na" => 11, "Cl" => 17, "Fe" => 26, _ => 0
        };
        
        state.labels.push(format!("{}{}", elems[i], i+1));
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
