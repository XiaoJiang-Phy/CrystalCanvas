//! Tauri IPC commands for interacting with the CrystalCanvas React UI.
//! Commands handle viewport resizing, loading files, and camera state.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use tauri::{Emitter, State};

/// Sent by the React frontend via ResizeObserver when the transparent viewport <div> resizes.
#[tauri::command]
pub fn update_viewport_size(
    width: u32,
    height: u32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    log::info!("update_viewport_size: {}x{}", width, height);
    let mut renderer = renderer_state
        .lock()
        .map_err(|e| format!("Failed to lock renderer: {}", e))?;
    renderer.resize(winit::dpi::PhysicalSize::new(width, height));
    Ok(())
}

#[tauri::command]
pub fn set_camera_projection(
    is_perspective: bool,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    log::info!("set_camera_projection: perspective={}", is_perspective);

    // Lock crystal state FIRST to avoid AB/BA deadlock with restore_unitcell
    let scale = if !is_perspective {
        if let Ok(cs) = crystal_state.lock() {
            let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
            if extent > 0.0 { extent * 1.5 } else { 15.0 }
        } else {
            15.0
        }
    } else {
        15.0
    };

    let mut renderer = renderer_state
        .lock()
        .map_err(|e| format!("Failed to lock renderer: {}", e))?;

    if is_perspective {
        renderer.camera.set_perspective();
    } else {
        renderer.camera.set_orthographic(scale);
    }


    // Sync frontend UI (topbar might already be in sync, but menu and LLM need it)
    #[derive(Clone, serde::Serialize)]
    struct Payload {
        is_perspective: bool,
    }
    let _ = app.emit("view_projection_changed", Payload { is_perspective });

    Ok(())
}

/// Sets visibility flags for unit cell box and bonds.
/// The render loop in `renderer.render()` checks these booleans each frame,
/// so toggling them is sufficient — no geometry rebuild needed.
#[tauri::command]
pub fn set_render_flags(
    show_cell: bool,
    show_bonds: bool,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    log::info!("set_render_flags: cell={}, bonds={}", show_cell, show_bonds);
    let mut renderer = renderer_state
        .lock()
        .map_err(|e| format!("Failed to lock renderer: {}", e))?;
    
    renderer.show_cell = show_cell;
    renderer.show_bonds = show_bonds;
    Ok(())
}

#[tauri::command]
pub fn update_lattice_params(
    a: f64, b: f64, c: f64,
    alpha: f64, beta: f64, gamma: f64,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    log::info!("update_lattice_params: a={}, b={}, c={}, alpha={}, beta={}, gamma={}", a, b, c, alpha, beta, gamma);
    
    let mut cs = crystal_state.lock().map_err(|_| "Failed to lock state")?;
    cs.cell_a = a;
    cs.cell_b = b;
    cs.cell_c = c;
    cs.cell_alpha = alpha;
    cs.cell_beta = beta;
    cs.cell_gamma = gamma;
    
    cs.fractional_to_cartesian();
    cs.detect_spacegroup();
    
    let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
        &settings, &cs.selected_atoms
    );
    
    let mut renderer = renderer_state.lock().map_err(|_| "Renderer lock fail")?;
    renderer.update_atoms(&instances);
    renderer.update_lines(&cs, &settings);
    
    Ok(())
}

/// Load a CIF file into the state.
#[tauri::command]
pub fn load_cif_file(
    path: String,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    base_state: State<'_, BaseCrystalState>,
) -> Result<(), String> {
    log::info!("load_cif_file: {}", path);

    // 1 & 2. Load file (delegating to our format importer)
    let state = crate::io::import::load_file(&path)?;
    log::info!("[load_cif_file] File parsed: {} atoms", state.num_atoms());

    // 4. Update crystal state — must block until lock is available
    {
        let mut cs = crystal_state
            .lock()
            .map_err(|e| format!("Failed to lock crystal state: {}", e))?;
        *cs = state.clone();
        cs.version += 1;
        log::info!("[load_cif_file] Crystal state updated");
    }

    // 5. Store as base state for "Restore Unitcell" functionality
    {
        let mut base = base_state.0.lock().map_err(|e| format!("{}", e))?;
        *base = Some(state.clone());
    }


    // 6. Build instance data for the Renderer
    let settings = settings_state
        .lock()
        .map_err(|e| format!("Failed to lock settings: {}", e))?;

    let instances = crate::renderer::instance::build_instance_data(
        &state.cart_positions,
        &state.atomic_numbers,
        &state.elements,
        &settings, &state.selected_atoms
    );
    log::info!("[load_cif_file] Built {} atom instances", instances.len());


    // 4. Update the renderer — must block until lock is available
    {
        let mut renderer = renderer_state
            .lock()
            .map_err(|e| format!("Failed to lock renderer: {}", e))?;
        renderer.update_atoms(&instances);
        log::info!("[load_cif_file] Atoms uploaded to GPU, count={}", instances.len());
        renderer.update_lines(&state, &settings);
        log::info!("[load_cif_file] Lines updated");

        // Auto-adjust camera distance based on unit cell size
        let extent = state.cell_a.max(state.cell_b).max(state.cell_c) as f32;
        let center = state.unit_cell_center();
        let center_vec = glam::Vec3::from_array(center);
        renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
        renderer.camera.target = center_vec;
        // Optionally update the orthographic scale
        if !renderer.camera.is_perspective {
            renderer.camera.set_orthographic(extent * 1.5);
        }
    }

    Ok(())
}

#[tauri::command]
pub fn add_atom(
    element_symbol: String,
    atomic_number: u8,
    fract_pos: [f64; 3],
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    log::info!("add_atom: {} at {:?}", element_symbol, fract_pos);

    let mut cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;
        
    let formatted_symbol = crate::llm::router::format_element_symbol(&element_symbol);
    let an = if atomic_number == 0 {
        crate::llm::router::element_to_atomic_number(&formatted_symbol)
    } else {
        atomic_number
    };
    
    if an == 0 {
        return Err(format!("Invalid element symbol: {}", element_symbol));
    }
    
    cs.try_add_atom(&formatted_symbol, an, fract_pos)
        .map_err(|_| "Collision detected: atom too close to existing atoms")?;

    let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
        &settings, &cs.selected_atoms
    );
    if let Ok(mut renderer) = renderer_state.lock() {
        renderer.update_atoms(&instances);
        renderer.update_lines(&cs, &settings);
    }

    Ok(())
}

#[tauri::command]
pub fn delete_atoms(
    indices: Vec<usize>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    log::info!("delete_atoms: {:?}", indices);

    let mut cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;
    cs.delete_atoms(&indices);

    let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
        &settings, &cs.selected_atoms
    );
    if let Ok(mut renderer) = renderer_state.lock() {
        renderer.update_atoms(&instances);
        renderer.update_lines(&cs, &settings);
    }

    Ok(())
}

#[tauri::command]
pub fn translate_atoms_screen(
    indices: Vec<usize>,
    dx: f32,
    dy: f32,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    let (eye, target, up) = {
        let r = renderer_state.lock().map_err(|_| "Failed to lock renderer")?;
        (r.camera.eye, r.camera.target, r.camera.up)
    };
    
    // Calculate world space translation vector exactly as camera pan
    // Reduced from 0.002 to 0.001 to achieve 1:1 screen-to-world drag feeling
    let pan_speed = 0.001 * (eye - target).length();
    let forward = (target - eye).normalize();
    let right = forward.cross(up).normalize();
    let up_dir = right.cross(forward).normalize();
    // To make an atom follow the mouse, it must move exactly opposite to the camera's translation.
    // Camera pan: -right * dx + up * dy  (moves camera left/up, so scene appears to move right/down)
    // Atom drag:  +right * dx - up * dy  (moves atom right/down directly)
    let translation = right * dx * pan_speed - up_dir * dy * pan_speed;
    
    // Apply this translation to atoms
    let mut cs = crystal_state.try_lock().map_err(|_| "Failed to lock state")?;
    
    // Inverse orthogonalization to map dx, dy, dz back to fractional coordinates
    let (a, b, c) = (cs.cell_a as f32, cs.cell_b as f32, cs.cell_c as f32);
    let alpha_rad = cs.cell_alpha.to_radians() as f32;
    let beta_rad = cs.cell_beta.to_radians() as f32;
    let gamma_rad = cs.cell_gamma.to_radians() as f32;
    
    let cos_alpha = alpha_rad.cos();
    let cos_beta = beta_rad.cos();
    let cos_gamma = gamma_rad.cos();
    let sin_gamma = gamma_rad.sin();
    
    let m00 = a;
    let m01 = b * cos_gamma;
    let m02 = c * cos_beta;
    let m11 = b * sin_gamma;
    let m12 = c * (cos_alpha - cos_beta * cos_gamma) / sin_gamma;
    let m22 = c * ((1.0 - cos_alpha * cos_alpha - cos_beta * cos_beta - cos_gamma * cos_gamma + 2.0 * cos_alpha * cos_beta * cos_gamma).max(0.0).sqrt()) / sin_gamma;
    
    let d_frac_z = translation.z / m22;
    let d_frac_y = (translation.y - m12 * d_frac_z) / m11;
    let d_frac_x = (translation.x - m01 * d_frac_y - m02 * d_frac_z) / m00;
    
    for &idx in &indices {
        if idx < cs.num_atoms() {
            cs.fract_x[idx] += d_frac_x as f64;
            cs.fract_y[idx] += d_frac_y as f64;
            cs.fract_z[idx] += d_frac_z as f64;
            cs.cart_positions[idx][0] += translation.x;
            cs.cart_positions[idx][1] += translation.y;
            cs.cart_positions[idx][2] += translation.z;
        }
    }
    cs.version += 1;
    
    let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
        &settings, &cs.selected_atoms
    );
    let bond_instances = crate::renderer::instance::build_bond_instances(&cs, &settings, &cs.selected_atoms);
    
    if let Ok(mut renderer) = renderer_state.lock() {
        renderer.update_atoms(&instances);
        renderer.update_bonds(&bond_instances);
    }
    
    Ok(())
}

#[tauri::command]
pub fn substitute_atoms(
    indices: Vec<usize>,
    new_element_symbol: String,
    new_atomic_number: u8,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    log::info!("substitute_atoms: {:?} -> {}", indices, new_element_symbol);

    let mut cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;
        
    let formatted_symbol = crate::llm::router::format_element_symbol(&new_element_symbol);
    let mut an = new_atomic_number;
    if an == 0 {
        an = crate::llm::router::element_to_atomic_number(&formatted_symbol);
    }
    
    if an == 0 {
        return Err(format!("Invalid element symbol: {}", new_element_symbol));
    }
    
    cs.substitute_atoms(&indices, &formatted_symbol, an);

    let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
        &settings, &cs.selected_atoms
    );
    if let Ok(mut renderer) = renderer_state.lock() {
        renderer.update_atoms(&instances);
        renderer.update_lines(&cs, &settings);
    }

    Ok(())
}

#[tauri::command]
pub fn preview_slab(
    miller: [i32; 3],
    layers: i32,
    vacuum_a: f64,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> Result<crate::crystal_state::CrystalState, String> {
    log::info!(
        "preview_slab: miller={:?} layers={} vacuum={}",
        miller,
        layers,
        vacuum_a
    );
    let cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;
    cs.generate_slab(miller, layers, vacuum_a)
}

#[tauri::command]
pub fn preview_supercell(
    expansion: [i32; 9],
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> Result<crate::crystal_state::CrystalState, String> {
    log::info!("preview_supercell: {:?}", expansion);
    let cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;
    cs.generate_supercell(&expansion)
}

/// Apply a supercell expansion to the current crystal, mutating state and updating the renderer.
#[tauri::command]
pub fn apply_supercell(
    matrix: [[i32; 3]; 3],
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    // Flatten the 3x3 matrix into the [i32; 9] format expected by generate_supercell
    let expansion: [i32; 9] = [
        matrix[0][0],
        matrix[0][1],
        matrix[0][2],
        matrix[1][0],
        matrix[1][1],
        matrix[1][2],
        matrix[2][0],
        matrix[2][1],
        matrix[2][2],
    ];
    log::info!("apply_supercell: {:?}", expansion);

    // Single lock scope: generate supercell, replace state, then build render data
    let mut cs = crystal_state
        .lock()
        .map_err(|e| format!("Failed to lock crystal state: {}", e))?;

    let new_state = cs.generate_supercell(&expansion)?;
    *cs = new_state;
    cs.detect_spacegroup();

    let settings = settings_state
        .lock()
        .map_err(|e| format!("Settings lock fail: {}", e))?;

    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
        &settings,
        &cs.selected_atoms,
    );

    let mut renderer = renderer_state
        .lock()
        .map_err(|e| format!("Renderer lock fail: {}", e))?;
    renderer.update_atoms(&instances);
    renderer.update_lines(&cs, &settings);

    // Auto-adjust camera distance for the new structure
    let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
    let center = cs.unit_cell_center();
    let center_vec = glam::Vec3::from_array(center);
    renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
    renderer.camera.target = center_vec;
    if !renderer.camera.is_perspective {
        renderer.camera.set_orthographic(extent * 1.5);
    }
    renderer.update_camera();

    Ok(())
}

/// Apply a slab cut to the current crystal, mutating state and updating the renderer.
#[tauri::command]
pub fn apply_slab(
    miller: [i32; 3],
    layers: i32,
    vacuum_a: f64,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    log::info!(
        "apply_slab: miller={:?} layers={} vacuum={}",
        miller,
        layers,
        vacuum_a
    );

    let mut cs = crystal_state
        .lock()
        .map_err(|e| format!("Failed to lock crystal state: {}", e))?;

    let new_state = cs.generate_slab(miller, layers, vacuum_a)?;
    *cs = new_state;
    cs.detect_spacegroup();

    let settings = settings_state
        .lock()
        .map_err(|e| format!("Settings lock fail: {}", e))?;

    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
        &settings,
        &cs.selected_atoms,
    );

    let mut renderer = renderer_state
        .lock()
        .map_err(|e| format!("Renderer lock fail: {}", e))?;
    renderer.update_atoms(&instances);
    renderer.update_lines(&cs, &settings);

    let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
    let center = cs.unit_cell_center();
    let center_vec = glam::Vec3::from_array(center);
    renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
    renderer.camera.target = center_vec;
    if !renderer.camera.is_perspective {
        renderer.camera.set_orthographic(extent * 1.5);
    }
    renderer.update_camera();

    Ok(())
}

/// Set camera view along a lattice axis or reset the view.
#[tauri::command]
pub fn set_camera_view_axis(
    axis: String,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> Result<(), String> {
    log::info!("set_camera_view_axis: {}", axis);

    let cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock crystal state")?;
    let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
    let dist = extent * 2.5;

    // Compute lattice vectors for axis alignment
    let alpha = (cs.cell_alpha as f32).to_radians();
    let beta = (cs.cell_beta as f32).to_radians();
    let gamma = (cs.cell_gamma as f32).to_radians();
    let a = cs.cell_a as f32;
    let b = cs.cell_b as f32;
    let c = cs.cell_c as f32;

    let cx = c * beta.cos();
    let cy = c * (alpha.cos() - beta.cos() * gamma.cos()) / gamma.sin();
    let cz = (c * c - cx * cx - cy * cy).max(0.0).sqrt();

    let va = glam::Vec3::new(a, 0.0, 0.0);
    let vb = glam::Vec3::new(b * gamma.cos(), b * gamma.sin(), 0.0);
    let vc = glam::Vec3::new(cx, cy, cz);

    let mut renderer = renderer_state
        .try_lock()
        .map_err(|_| "Failed to lock renderer")?;

    let center = cs.unit_cell_center();
    let center_vec = glam::Vec3::from_array(center);
    renderer.camera.target = center_vec;

    match axis.as_str() {
        "a" => {
            let dir = va.normalize();
            renderer.camera.eye = center_vec + dir * dist;
            renderer.camera.up = glam::Vec3::Z;
        }
        "b" => {
            let dir = vb.normalize();
            renderer.camera.eye = center_vec + dir * dist;
            renderer.camera.up = glam::Vec3::Z;
        }
        "c" => {
            let dir = vc.normalize();
            renderer.camera.eye = center_vec + dir * dist;
            renderer.camera.up = glam::Vec3::Y;
        }
        "a_star" => {
            // a* is perpendicular to b-c plane
            let dir = vb.cross(vc).normalize();
            renderer.camera.eye = center_vec + dir * dist;
            renderer.camera.up = glam::Vec3::Z;
        }
        "b_star" => {
            let dir = vc.cross(va).normalize();
            renderer.camera.eye = center_vec + dir * dist;
            renderer.camera.up = glam::Vec3::Z;
        }
        "c_star" => {
            let dir = va.cross(vb).normalize();
            renderer.camera.eye = center_vec + dir * dist;
            renderer.camera.up = glam::Vec3::Y;
        }
        "reset" => {
            renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, dist);
            renderer.camera.up = glam::Vec3::Y;
        }
        _ => {
            return Err(format!("Unknown axis: {}", axis));
        }
    }

    Ok(())
}

#[tauri::command]
pub fn export_file(
    format: String,
    path: String,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> Result<(), String> {
    log::info!("export_file: format={} path={}", format, path);
    let cx = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;
    let fmt = match format.to_uppercase().as_str() {
        "POSCAR" | "VASP" => crate::llm::command::ExportFormat::Poscar,
        "LAMMPS" => crate::llm::command::ExportFormat::Lammps,
        "QE" => crate::llm::command::ExportFormat::Qe,
        _ => return Err(format!("Unsupported format: {}", format)),
    };

    match fmt {
        crate::llm::command::ExportFormat::Poscar => {
            crate::io::export::export_poscar(&cx, &path).map_err(|e| e.to_string())?
        }
        crate::llm::command::ExportFormat::Lammps => {
            crate::io::export::export_lammps_data(&cx, &path).map_err(|e| e.to_string())?
        }
        crate::llm::command::ExportFormat::Qe => {
            crate::io::export::export_qe_input(&cx, &path).map_err(|e| e.to_string())?
        }
    }
    Ok(())
}
// =========================================================================
// Structural Analysis (M10)
// =========================================================================

/// Serializable bond analysis result for the frontend.
#[derive(serde::Serialize)]
pub struct BondAnalysisResult {
    pub bonds: Vec<crate::crystal_state::BondInfo>,
    pub coordination: Vec<crate::crystal_state::CoordinationInfo>,
    pub bond_length_stats: Vec<crate::crystal_state::BondLengthStat>,
    pub distortion_indices: Vec<f64>,
    pub threshold_factor: f64,
}

/// Compute and return bond analysis for the current crystal.
/// This triggers a full recompute of bond connectivity at the given threshold.
#[tauri::command]
pub fn get_bond_analysis(
    threshold_factor: Option<f64>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> Result<BondAnalysisResult, String> {
    let factor = threshold_factor.unwrap_or(1.2);
    let mut cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;

    cs.compute_bond_analysis(factor);

    let analysis = cs
        .bond_analysis
        .as_ref()
        .ok_or_else(|| "Bond analysis not computed".to_string())?;

    let stats = analysis.bond_length_stats(&cs.elements);
    let distortion_indices: Vec<f64> = analysis
        .coordination
        .iter()
        .map(|c| crate::crystal_state::BondAnalysis::distortion_index(c))
        .collect();

    Ok(BondAnalysisResult {
        bonds: analysis.bonds.clone(),
        coordination: analysis.coordination.clone(),
        bond_length_stats: stats,
        distortion_indices,
        threshold_factor: factor,
    })
}

/// Load phonon data from a file (Molden or QE dynmat.dat format).
/// Returns mode summaries for the frontend to display.
#[tauri::command]
pub fn load_phonon(
    path: String,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<Vec<crate::phonon::PhononModeSummary>, String> {
    log::info!("load_phonon: {}", path);
    // 1. Check if the phonon struct loader provides structural info (Molden/QE output)
    let data = crate::phonon::parse_phonon_file(&path)?;
    let summaries = data.summaries();

    // 2. We can try to guess or load structural info from phonon data if our state is empty
    // But dynmat molden/dat doesn't have full cell info, so we just set the phonon data to state
    {
        let mut cs = crystal_state
            .lock()
            .map_err(|_| "Failed to lock state")?;
        
        // Let's ensure the user loaded a structure first.
        if cs.cart_positions.is_empty() {
             return Err("Please load a crystal structure (CIF/XYZ/PDB) before loading Phonon data.".to_string());
        }

        if cs.cart_positions.len() != data.n_atoms {
             log::warn!("Atom count mismatch: struct={} vs phonon={}", cs.cart_positions.len(), data.n_atoms);
             // We allow proceeding but this might truncate eigenvectors
        }

        cs.phonon_data = Some(data);
        cs.active_phonon_mode = None;
        cs.phonon_phase = 0.0;
        
        // Trigger a reset of renderer instances just in case previous states were playing.
        let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
        let instances = crate::renderer::instance::build_instance_data(
            &cs.cart_positions,
            &cs.atomic_numbers,
            &cs.elements,
            &settings, &cs.selected_atoms
        );
        if let Ok(mut renderer) = renderer_state.lock() {
            renderer.update_atoms(&instances);
        }
    }

    Ok(summaries)
}

/// Load Phonon Data using explicit QE scf_in, scf_out, and modes files (Interactive Visualizer style)
#[tauri::command]
pub fn load_phonon_interactive(
    scf_in: String,
    #[allow(unused_variables)] scf_out: String,
    modes: String,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<Vec<crate::phonon::PhononModeSummary>, String> {
    log::info!("load_phonon_interactive: in={}, modes={}", scf_in, modes);
    
    // 1. Load the crystal structure from scf_in now (since its parser is fully robust)
    let mut new_state = crate::io::qe_parser::parse_scf_in(&scf_in)?;
    
    // 2. Parse phonon data
    let data = crate::phonon::parse_phonon_file(&modes)?;
    let summaries = data.summaries();

    if new_state.cart_positions.len() != data.n_atoms {
         log::warn!("Atom count mismatch: struct={} vs phonon={}", new_state.cart_positions.len(), data.n_atoms);
    }

    new_state.phonon_data = Some(data);
    new_state.active_phonon_mode = None;
    new_state.phonon_phase = 0.0;
    
    // Push new state
    {
        let mut cs = crystal_state.lock().map_err(|_| "Failed to lock state")?;
        *cs = new_state;
        
        // Trigger a full renderer update (atoms + unit cell lines + camera)
        let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
        let instances = crate::renderer::instance::build_instance_data(
            &cs.cart_positions,
            &cs.atomic_numbers,
            &cs.elements,
            &settings, &cs.selected_atoms
        );
        
        let mut renderer = renderer_state.lock().map_err(|_| "Renderer lock fail")?;
        renderer.update_atoms(&instances);
        renderer.update_lines(&cs, &settings);
        
        // Auto-adjust camera
        let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
        let center = cs.unit_cell_center();
        let center_vec = glam::Vec3::from_array(center);
        renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
        renderer.camera.target = center_vec;
        if !renderer.camera.is_perspective {
            renderer.camera.set_orthographic(extent * 1.5);
        }
    }

    Ok(summaries)
}

/// Load an AXSF file containing both crystal structure and phonon mode data
#[tauri::command]
pub fn load_axsf_phonon(
    path: String,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<Vec<crate::phonon::PhononModeSummary>, String> {
    log::info!("load_axsf_phonon: {}", path);
    // 1 & 2. Load the crystal structure and phonon data directly from the axsf
    let (mut new_state, data) = crate::io::axsf_parser::parse_axsf(&path)?;
    let summaries = data.summaries();

    new_state.phonon_data = Some(data);
    new_state.active_phonon_mode = None;
    new_state.phonon_phase = 0.0;
    
    // Push new state
    {
        let mut cs = crystal_state.lock().map_err(|_| "Failed to lock state")?;
        *cs = new_state;
        
        let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
        let instances = crate::renderer::instance::build_instance_data(
            &cs.cart_positions,
            &cs.atomic_numbers,
            &cs.elements,
            &settings, &cs.selected_atoms
        );
        
        let mut renderer = renderer_state.lock().map_err(|_| "Renderer lock fail")?;
        renderer.update_atoms(&instances);
        renderer.update_lines(&cs, &settings);
        
        let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
        let center = cs.unit_cell_center();
        let center_vec = glam::Vec3::from_array(center);
        renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
        renderer.camera.target = center_vec;
        if !renderer.camera.is_perspective {
            renderer.camera.set_orthographic(extent * 1.5);
        }
    }

    Ok(summaries)
}

/// Select a phonon mode for animation.
#[tauri::command]
pub fn set_phonon_mode(
    mode_index: Option<usize>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    let mut cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;

    if let Some(idx) = mode_index {
        let n_modes = cs
            .phonon_data
            .as_ref()
            .map_or(0, |d| d.modes.len());
        if idx >= n_modes {
            return Err(format!(
                "Mode index {} out of range (0..{})",
                idx, n_modes
            ));
        }

        // Hide bonds purely for physics visualization mode
        if let Ok(mut renderer) = renderer_state.try_lock() {
            renderer.update_bonds(&[]);
        }
    }

    cs.active_phonon_mode = mode_index;
    cs.phonon_phase = 0.0;
    Ok(())
}

/// Set the animation phase for phonon visualization.
/// phase is in radians [0, 2π].
#[tauri::command]
pub fn set_phonon_phase(
    phase: f64,
    amplitude: Option<f64>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    let mut cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;

    cs.phonon_phase = phase;
    let amp = amplitude.unwrap_or(1.0);

    // If a mode is active, compute displaced positions and update renderer
    if let (Some(mode_idx), Some(phonon_data)) = (cs.active_phonon_mode, &cs.phonon_data) {
        if mode_idx < phonon_data.modes.len() {
            let mode = &phonon_data.modes[mode_idx];
            let sin_phase = phase.sin();
            let n = cs.cart_positions.len().min(mode.eigenvectors.len());

            // Compute displaced positions
            let mut displaced = cs.cart_positions.clone();
            for i in 0..n {
                displaced[i][0] += (amp * mode.eigenvectors[i][0] * sin_phase) as f32;
                displaced[i][1] += (amp * mode.eigenvectors[i][1] * sin_phase) as f32;
                displaced[i][2] += (amp * mode.eigenvectors[i][2] * sin_phase) as f32;
            }

            // Update renderer with displaced positions
            let settings = settings_state
                .lock()
                .map_err(|_| "Settings lock fail")?;
            let instances = crate::renderer::instance::build_instance_data(
                &displaced,
                &cs.atomic_numbers,
                &cs.elements,
                &settings,
                &cs.selected_atoms,
            );
            if let Ok(mut renderer) = renderer_state.lock() {
                renderer.update_atoms(&instances);
            }
        }
    }

    Ok(())
}

// =========================================================================
// LLM AI Tasks
// =========================================================================

pub struct LlmState(pub std::sync::Mutex<Option<crate::llm::provider::ProviderConfig>>);

/// Managed state to store the "base" primitive/standard unit cell before supercell/slab expansions.
pub struct BaseCrystalState(pub std::sync::Mutex<Option<crate::crystal_state::CrystalState>>);


fn get_api_key(provider: &str, provided_key: &str) -> String {
    let clean_provided = provided_key.trim();
    if !clean_provided.is_empty() && clean_provided != "********" && clean_provided != "••••••••" {
        // Save to OS Keychain
        if let Ok(entry) = keyring::Entry::new("CrystalCanvas", provider) {
            let _ = entry.set_password(clean_provided); // Ignore errors if keychain is unavailable
        }
        return clean_provided.to_string();
    }

    // Try to load from keychain
    if let Ok(entry) = keyring::Entry::new("CrystalCanvas", provider)
        && let Ok(pwd) = entry.get_password()
    {
        if !pwd.trim().is_empty() && pwd.trim() != "********" && pwd.trim() != "••••••••" {
            return pwd.trim().to_string();
        }
    }

    // Fallback to .env for development
    dotenvy::dotenv().ok();
    dotenvy::from_path("../.env").ok();
    
    // Case-insensitive env var search
    let target_key = if provider == "claude" {
        "anthropic_api_key".to_string()
    } else {
        format!("{}_api_key", provider.to_lowercase())
    };

    for (k, v) in std::env::vars() {
        if k.to_lowercase() == target_key {
            return v.trim().to_string();
        }
    }

    String::new()
}

#[tauri::command]
pub fn check_api_key_status(provider_type: String) -> Result<bool, String> {
    let key = get_api_key(&provider_type.to_lowercase(), "");
    Ok(!key.is_empty())
}

#[tauri::command]
pub fn llm_configure(
    provider_type: String,
    api_key: String,
    model: String,
    state: State<'_, LlmState>,
) -> Result<(), String> {
    let pt = provider_type.to_lowercase();
    let resolved_key = if pt == "ollama" {
        String::new()
    } else {
        get_api_key(&pt, &api_key)
    };

    let config = match pt.as_str() {
        "openai" => crate::llm::provider::ProviderConfig::OpenAi {
            api_key: resolved_key,
            model,
        },
        "deepseek" => crate::llm::provider::ProviderConfig::DeepSeek {
            api_key: resolved_key,
            model,
        },
        "claude" => crate::llm::provider::ProviderConfig::Claude {
            api_key: resolved_key,
            model,
        },
        "gemini" => crate::llm::provider::ProviderConfig::Gemini {
            api_key: resolved_key,
            model,
        },
        "ollama" => crate::llm::provider::ProviderConfig::Ollama { model },
        _ => return Err(format!("Unknown provider type: {}", provider_type)),
    };
    let mut st = state.0.try_lock().map_err(|_| "Failed to lock LLM state")?;
    *st = Some(config);
    Ok(())
}

#[tauri::command]
pub async fn llm_chat(
    user_message: String,
    selected_indices: Option<Vec<usize>>,
    state: State<'_, LlmState>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> Result<String, String> {
    let config_opt = {
        let st = state.0.try_lock().map_err(|_| "Failed to lock LLM state")?;
        st.clone()
    };

    let config = config_opt
        .ok_or_else(|| "LLM provider is not configured. Please supply an API key.".to_string())?;

    let context = {
        let cs = crystal_state
            .try_lock()
            .map_err(|_| "Failed to lock state")?;
        crate::llm::context::build_crystal_context(&cs, selected_indices.as_deref())
    };

    let messages = crate::llm::prompt::build_messages(&context, &user_message);

    let provider = crate::llm::provider::create_provider(&config);
    provider.chat(&messages).await
}

#[tauri::command]
pub fn llm_execute_command(
    command_json: String,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    // 1. Layer 1: Schema parse validation
    let command: crate::llm::command::CrystalCommand = serde_json::from_str(&command_json)
        .map_err(|e| format!("Schema validation failed: {}", e))?;

    let mut cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;

    // 2. Layer 2: Physics Sandbox validation
    crate::llm::sandbox::validate_command(&command, &cs)
        .map_err(|e| format!("Physics sandbox error: {}", e))?;

    // 3. Layer 3: Execute in Router
    crate::llm::router::execute_command(command, &mut cs)
        .map_err(|e| format!("Command execution failed: {}", e))?;

    // Note: To properly support Undo, we would snapshot here.
    cs.version += 1;

    let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
        &settings,
        &cs.selected_atoms,
    );
    if let Ok(mut renderer) = renderer_state.lock() {
        renderer.update_atoms(&instances);
        renderer.update_lines(&cs, &settings);
    }

    Ok(())
}

#[tauri::command]
pub fn get_crystal_state(
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> Result<crate::crystal_state::CrystalState, String> {
    let cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;
    Ok(cs.clone())
}

/// Rotates the camera orbitally.
#[tauri::command]
pub fn rotate_camera(
    dx: f32,
    dy: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    let mut renderer = renderer_state
        .lock()
        .map_err(|e| format!("Failed to lock renderer: {}", e))?;
    renderer.camera.rotate_around_target(dx, dy);
    Ok(())
}

/// Zooms the camera based on scroll delta.
#[tauri::command]
pub fn zoom_camera(
    delta: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    let mut renderer = renderer_state
        .lock()
        .map_err(|e| format!("Failed to lock renderer: {}", e))?;
    renderer.camera.zoom_towards_target(delta);
    Ok(())
}

/// Pans the camera.
#[tauri::command]
pub fn pan_camera(
    dx: f32,
    dy: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    let mut renderer = renderer_state
        .lock()
        .map_err(|e| format!("Failed to lock renderer: {}", e))?;
    renderer.camera.pan(dx, dy);
    Ok(())
}

/// Resets the camera to default view looking over the crystal.
#[tauri::command]
pub fn reset_camera(
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    let mut renderer = renderer_state
        .lock()
        .map_err(|e| format!("Failed to lock renderer: {}", e))?;

    if let Ok(cs) = crystal_state.lock() {
        let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
        let dist = extent * 2.5;
        let center = cs.unit_cell_center();
        let center_vec = glam::Vec3::from_array(center);
        renderer.camera.target = center_vec;
        renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, dist);
        renderer.camera.orthographic_scale = extent * 1.5;
    } else {
        renderer.camera = crate::renderer::camera::Camera::default_for_crystal();
    }

    Ok(())
}

/// Perform ray-sphere intersection to pick an atom.
#[tauri::command]
pub fn pick_atom(
    x: f32,
    y: f32,
    screen_w: f32,
    screen_h: f32,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<Option<usize>, String> {
    log::info!(
        "pick_atom: window screen_w={} screen_h={} pointer x={} y={}",
        screen_w,
        screen_h,
        x,
        y
    );
    let (camera_eye, view_proj, is_perspective) = {
        let renderer = renderer_state
            .try_lock()
            .map_err(|_| "Failed to lock renderer")?;
        let vp = renderer.camera.build_projection_matrix() * renderer.camera.build_view_matrix();
        (renderer.camera.eye, vp, renderer.camera.is_perspective)
    };

    let inv_vp = view_proj.inverse();

    let nx = (2.0 * x) / screen_w - 1.0;
    let ny = 1.0 - (2.0 * y) / screen_h;

    // Far plane point
    let p_far = inv_vp * glam::Vec4::new(nx, ny, 1.0, 1.0);
    let p_far = p_far.truncate() / p_far.w;

    // Near plane point (only used for Ortho origin)
    let p_near = inv_vp * glam::Vec4::new(nx, ny, 0.0, 1.0);
    let p_near = p_near.truncate() / p_near.w;

    let ray_origin = if is_perspective { camera_eye } else { p_near };
    let ray_dir = (p_far - ray_origin).normalize();

    let cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;

    let mut closest_idx = None;
    let mut min_t = f32::MAX;

    // Use a fixed hit radius for now, scale it up so atoms are easy to click
    let hit_radius_sq = 1.5 * 1.5;

    for (i, pos) in cs.cart_positions.iter().enumerate() {
        let center = glam::Vec3::new(pos[0], pos[1], pos[2]);
        let l = center - ray_origin;
        let tca = l.dot(ray_dir);
        if tca < 0.0 {
            continue;
        } // Behind ray

        let d2 = l.length_squared() - tca * tca;
        if d2 > hit_radius_sq {
            continue;
        } // Ray misses sphere

        let thc = (hit_radius_sq - d2).sqrt();
        let t = tca - thc;

        if t > 0.0 && t < min_t {
            min_t = t;
            closest_idx = Some(i);
        }
    }

    log::info!("pick_atom completed: found closest idx = {:?}", closest_idx);

    Ok(closest_idx)
}

#[tauri::command]
pub fn get_settings(
    settings: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<crate::settings::AppSettings, String> {
    Ok(settings.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
pub fn update_settings(
    app: tauri::AppHandle,
    new_settings: crate::settings::AppSettings,
    settings: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    log::info!("update_settings called");
    
    // 1. Update global settings state
    {
        let mut s = settings.lock().map_err(|e| e.to_string())?;
        *s = new_settings.clone();
    }
    
    // 2. Save to disk
    let _ = new_settings.save(&app).map_err(|e| log::warn!("Failed to save settings: {}", e));

    // 3. Rebuild renderer data
    // Lock State FIRST to avoid AB/BA deadlock with commands that lock renderer then state
    let cs = crystal_state.lock().map_err(|e| format!("State lock: {}", e))?;
    let mut renderer = renderer_state.lock().map_err(|e| format!("Renderer lock: {}", e))?;

    // Update atoms (affects scale and visibility)
    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
        &new_settings,
        &cs.selected_atoms,
    );
    renderer.update_atoms(&instances);

    // Update lines (affects cell box and bonds)
    renderer.update_lines(&cs, &new_settings);
    let bond_instances = crate::renderer::instance::build_bond_instances(&cs, &new_settings, &cs.selected_atoms);
    renderer.update_bonds(&bond_instances);


    Ok(())
}

#[tauri::command]
pub fn update_selection(
    indices: Vec<usize>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    let mut cs = crystal_state.lock().map_err(|_| "Failed to lock state")?;
    cs.selected_atoms = indices;
    let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
        &settings,
        &cs.selected_atoms,
    );
    let bond_instances = crate::renderer::instance::build_bond_instances(&cs, &settings, &cs.selected_atoms);
    if let Ok(mut renderer) = renderer_state.lock() {
        renderer.update_atoms(&instances);
        renderer.update_bonds(&bond_instances);
    }
    Ok(())
}
/// Restore the original unit cell from the base state.
#[tauri::command]
pub fn restore_unitcell(
    base_state: State<'_, BaseCrystalState>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    log::info!("restore_unitcell triggered");

    let base = base_state.0.lock().map_err(|e| format!("Base state lock failed: {}", e))?;
    let Some(original) = base.as_ref() else {
        return Err("No base structure loaded to restore".to_string());
    };

    let mut cs = crystal_state.lock().map_err(|e| format!("Crystal state lock failed: {}", e))?;
    *cs = original.clone();
    cs.version += 1;
    cs.active_phonon_mode = None;
    cs.phonon_phase = 0.0;


    let settings = settings_state.lock().map_err(|e| format!("Settings lock failed: {}", e))?;
    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
        &settings,
        &cs.selected_atoms,
    );

    let mut renderer = renderer_state.lock().map_err(|e| format!("Renderer lock failed: {}", e))?;
    renderer.update_atoms(&instances);
    let bond_instances = crate::renderer::instance::build_bond_instances(&cs, &settings, &cs.selected_atoms);
    renderer.update_bonds(&bond_instances);
    renderer.update_lines(&cs, &settings);

    // Auto-adjust camera to view the restored cell
    let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
    let center = cs.unit_cell_center();
    let center_vec = glam::Vec3::from_array(center);
    renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
    renderer.camera.target = center_vec;
    if !renderer.camera.is_perspective {
        renderer.camera.set_orthographic(extent * 1.5);
    }
    renderer.update_camera();



    Ok(())
}

/// Export the current viewport as a high-resolution image.
/// Renders off-screen at the specified dimensions and saves to the given path.
#[tauri::command]
pub fn export_image(
    path: String,
    width: u32,
    height: u32,
    bg_mode: String,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    log::info!(
        "export_image: {}x{}, bg={}, path={}",
        width,
        height,
        bg_mode,
        path
    );

    let mut renderer = renderer_state
        .lock()
        .map_err(|e| format!("Failed to lock renderer: {}", e))?;

    let rgba_data = renderer.render_offscreen(width, height, &bg_mode)?;

    // Determine output format from file extension
    let path_lower = path.to_lowercase();
    if path_lower.ends_with(".jpg") || path_lower.ends_with(".jpeg") {
        // JPEG does not support transparency — composite onto white if transparent
        let rgb_data: Vec<u8> = if bg_mode == "transparent" {
            rgba_data
                .chunks_exact(4)
                .flat_map(|px| {
                    let a = px[3] as f32 / 255.0;
                    [
                        (px[0] as f32 * a + 255.0 * (1.0 - a)) as u8,
                        (px[1] as f32 * a + 255.0 * (1.0 - a)) as u8,
                        (px[2] as f32 * a + 255.0 * (1.0 - a)) as u8,
                    ]
                })
                .collect()
        } else {
            rgba_data
                .chunks_exact(4)
                .flat_map(|px| [px[0], px[1], px[2]])
                .collect()
        };

        let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_raw(width, height, rgb_data)
                .ok_or_else(|| "Failed to create JPEG image buffer".to_string())?;
        img.save(&path)
            .map_err(|e| format!("Failed to save JPEG: {}", e))?;
    } else {
        // Default: PNG (supports transparency)
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            image::ImageBuffer::from_raw(width, height, rgba_data)
                .ok_or_else(|| "Failed to create PNG image buffer".to_string())?;
        img.save(&path)
            .map_err(|e| format!("Failed to save PNG: {}", e))?;
    }

    log::info!("Image exported successfully to {}", path);
    Ok(())
}
