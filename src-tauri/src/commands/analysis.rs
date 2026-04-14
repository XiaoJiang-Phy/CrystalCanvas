use tauri::{Emitter, State};

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
    app: tauri::AppHandle,
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
        let instances = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
        if let Ok(mut renderer) = renderer_state.lock() {
            renderer.update_atoms(&instances);
        }
    }

    app.emit("state_changed", ()).ok();

    Ok(summaries)
}

/// Load Phonon Data using explicit QE scf_in, scf_out, and modes files (Interactive Visualizer style)
#[tauri::command]
pub fn load_phonon_interactive(
    scf_in: String,
    #[allow(unused_variables)] scf_out: String,
    modes: String,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
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
    
    let can_undo;
    let can_redo;
    {
        let mut cs = crystal_state.lock().map_err(|_| "Failed to lock state")?;
        
        let mut u_stack = undo_state.lock().map_err(|e| format!("{}", e))?;
        u_stack.push(crate::undo::LightweightState::from_crystal_state(&cs));
        can_undo = u_stack.can_undo();
        can_redo = u_stack.can_redo();
        drop(u_stack);
        
        *cs = new_state;
        
        // Trigger a full renderer update (atoms + unit cell lines + camera)
        let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
        let instances = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
        
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

    app.emit("state_changed", ()).ok();
    app.emit("undo_stack_changed", crate::transaction::UndoStackPayload { can_undo, can_redo }).ok();

    Ok(summaries)
}

/// Load an AXSF file containing both crystal structure and phonon mode data
#[tauri::command]
pub fn load_axsf_phonon(
    path: String,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> Result<Vec<crate::phonon::PhononModeSummary>, String> {
    log::info!("load_axsf_phonon: {}", path);
    // 1 & 2. Load the crystal structure and phonon data directly from the axsf
    let (mut new_state, data) = crate::io::axsf_parser::parse_axsf(&path)?;
    let summaries = data.summaries();

    new_state.phonon_data = Some(data);
    new_state.active_phonon_mode = None;
    new_state.phonon_phase = 0.0;
    
    let can_undo;
    let can_redo;
    {
        let mut cs = crystal_state.lock().map_err(|_| "Failed to lock state")?;
        let mut u_stack = undo_state.lock().map_err(|e| format!("{}", e))?;
        u_stack.push(crate::undo::LightweightState::from_crystal_state(&cs));
        can_undo = u_stack.can_undo();
        can_redo = u_stack.can_redo();
        drop(u_stack);
        *cs = new_state;
        
        let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
        let instances = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
        
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

    app.emit("state_changed", ()).ok();
    app.emit("undo_stack_changed", crate::transaction::UndoStackPayload { can_undo, can_redo }).ok();

    Ok(summaries)
}

/// Select a phonon mode for animation.
#[tauri::command]
pub fn set_phonon_mode(
    mode_index: Option<usize>,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    let mut cs = crystal_state
        .lock()
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
        if let Ok(mut renderer) = renderer_state.lock() {
            renderer.update_bonds(&[]);
        }
    }

    cs.active_phonon_mode = mode_index;
    cs.phonon_phase = 0.0;
    app.emit("state_changed", ()).ok();
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
        .lock()
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
            let instances = crate::wannier::build_atoms_with_ghosts_displaced(&cs, &displaced, &settings);
            if let Ok(mut renderer) = renderer_state.lock() {
                renderer.update_atoms(&instances);
            }
        }
    }

    Ok(())
}
