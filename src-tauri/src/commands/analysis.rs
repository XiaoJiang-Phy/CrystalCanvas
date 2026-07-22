use crate::ipc::{IpcError, IpcResult};
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
) -> IpcResult<BondAnalysisResult> {
    let factor = threshold_factor.unwrap_or(1.2);
    if !factor.is_finite() || factor <= 0.0 {
        return Err(IpcError::invalid_argument(
            "bond threshold factor must be finite and positive",
        ));
    }
    let mut cs = crystal_state
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "crystal state"))?;

    cs.compute_bond_analysis(factor);

    let analysis = cs
        .bond_analysis
        .as_ref()
        .ok_or_else(|| IpcError::from("bond analysis was not computed"))?;

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
) -> IpcResult<Vec<crate::phonon::PhononModeSummary>> {
    log::info!("load_phonon: {}", path);
    // 1. Check if the phonon struct loader provides structural info (Molden/QE output)
    let data = crate::phonon::parse_phonon_file(&path).map_err(IpcError::parse)?;
    let summaries = data.summaries();

    // 2. We can try to guess or load structural info from phonon data if our state is empty
    // But dynmat molden/dat doesn't have full cell info, so we just set the phonon data to state
    {
        let mut cs = crystal_state
            .lock()
            .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
        let settings = settings_state
            .lock()
            .map_err(|_| IpcError::lock("settings lock poisoned"))?;
        if cs.cart_positions.is_empty() {
            return Err(IpcError::invalid_argument(
                "load a crystal structure before loading phonon data",
            ));
        }

        if cs.intrinsic_sites != data.n_atoms {
            return Err(IpcError::invalid_argument(format!(
                "phonon atom count {} does not match crystal atom count {}",
                data.n_atoms, cs.intrinsic_sites
            )));
        }

        let pending_version = crate::transaction::next_version(&cs)?;
        let atom_scene = crate::renderer::instance::prepare_atom_scene(
            crate::wannier::build_atoms_with_ghosts(&cs, &settings)?,
        )?;
        let mut renderer = renderer_state
            .lock()
            .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
        cs.phonon_data = Some(data);
        cs.active_phonon_mode = None;
        cs.phonon_phase = 0.0;
        let version = crate::transaction::commit_version(&mut cs, pending_version)?;
        renderer.commit_atoms(atom_scene);

        drop(renderer);
        drop(settings);
        drop(cs);
        app.emit(
            "state_changed",
            crate::transaction::StateChangedPayload { version },
        )
        .ok();
    }

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
) -> IpcResult<Vec<crate::phonon::PhononModeSummary>> {
    log::info!("load_phonon_interactive: in={}, modes={}", scf_in, modes);

    // 1. Load the crystal structure from scf_in now (since its parser is fully robust)
    let mut new_state = crate::io::qe_parser::parse_scf_in(&scf_in).map_err(IpcError::parse)?;
    new_state
        .validate_structural_invariants()
        .map_err(IpcError::parse)?;

    // 2. Parse phonon data
    let data = crate::phonon::parse_phonon_file(&modes).map_err(IpcError::parse)?;
    if new_state.intrinsic_sites != data.n_atoms {
        return Err(IpcError::invalid_argument(format!(
            "phonon atom count {} does not match structure atom count {}",
            data.n_atoms, new_state.intrinsic_sites
        )));
    }
    let summaries = data.summaries();

    new_state.phonon_data = Some(data);
    new_state.active_phonon_mode = None;
    new_state.phonon_phase = 0.0;

    let can_undo;
    let can_redo;
    let version;
    {
        let mut cs = crystal_state
            .lock()
            .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;

        let mut u_stack = undo_state
            .lock()
            .map_err(|_| IpcError::lock("undo stack lock poisoned"))?;
        let settings = settings_state
            .lock()
            .map_err(|_| IpcError::lock("settings lock poisoned"))?;
        let pending_version = crate::transaction::next_version(&cs)?;
        let previous_state = crate::undo::StructuralSnapshot::from_crystal_state(&cs);
        let atom_scene = crate::renderer::instance::prepare_atom_scene(
            crate::wannier::build_atoms_with_ghosts(&new_state, &settings)?,
        )?;
        let line_scene = crate::renderer::instance::build_line_scene(&new_state, &settings)?;
        let mut renderer = renderer_state
            .lock()
            .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
        renderer.clear_structure_bound_overlays();
        let committed_version = crate::transaction::stamp_version(&mut new_state, pending_version);
        *cs = new_state;
        renderer.commit_atoms(atom_scene);
        renderer.update_lines(&line_scene);

        // Auto-adjust camera
        let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
        let center = cs.unit_cell_center();
        let center_vec = glam::Vec3::from_array(center);
        renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
        renderer.camera.target = center_vec;
        if !renderer.camera.is_perspective {
            renderer.camera.set_orthographic(extent * 1.5);
        }
        u_stack.push(previous_state);
        can_undo = u_stack.can_undo();
        can_redo = u_stack.can_redo();
        version = committed_version;
    }

    app.emit(
        "state_changed",
        crate::transaction::StateChangedPayload { version },
    )
    .ok();
    app.emit(
        "undo_stack_changed",
        crate::transaction::UndoStackPayload { can_undo, can_redo },
    )
    .ok();

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
) -> IpcResult<Vec<crate::phonon::PhononModeSummary>> {
    log::info!("load_axsf_phonon: {}", path);
    // 1 & 2. Load the crystal structure and phonon data directly from the axsf
    let (mut new_state, data) =
        crate::io::axsf_parser::parse_axsf(&path).map_err(IpcError::parse)?;
    new_state
        .validate_structural_invariants()
        .map_err(IpcError::parse)?;
    let summaries = data.summaries();

    new_state.phonon_data = Some(data);
    new_state.active_phonon_mode = None;
    new_state.phonon_phase = 0.0;

    let can_undo;
    let can_redo;
    let version;
    {
        let mut cs = crystal_state
            .lock()
            .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
        let mut u_stack = undo_state
            .lock()
            .map_err(|_| IpcError::lock("undo stack lock poisoned"))?;
        let settings = settings_state
            .lock()
            .map_err(|_| IpcError::lock("settings lock poisoned"))?;
        let pending_version = crate::transaction::next_version(&cs)?;
        let previous_state = crate::undo::StructuralSnapshot::from_crystal_state(&cs);
        let atom_scene = crate::renderer::instance::prepare_atom_scene(
            crate::wannier::build_atoms_with_ghosts(&new_state, &settings)?,
        )?;
        let line_scene = crate::renderer::instance::build_line_scene(&new_state, &settings)?;
        let mut renderer = renderer_state
            .lock()
            .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
        renderer.clear_structure_bound_overlays();
        let committed_version = crate::transaction::stamp_version(&mut new_state, pending_version);
        *cs = new_state;
        renderer.commit_atoms(atom_scene);
        renderer.update_lines(&line_scene);

        let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
        let center = cs.unit_cell_center();
        let center_vec = glam::Vec3::from_array(center);
        renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
        renderer.camera.target = center_vec;
        if !renderer.camera.is_perspective {
            renderer.camera.set_orthographic(extent * 1.5);
        }
        u_stack.push(previous_state);
        can_undo = u_stack.can_undo();
        can_redo = u_stack.can_redo();
        version = committed_version;
    }

    app.emit(
        "state_changed",
        crate::transaction::StateChangedPayload { version },
    )
    .ok();
    app.emit(
        "undo_stack_changed",
        crate::transaction::UndoStackPayload { can_undo, can_redo },
    )
    .ok();

    Ok(summaries)
}

/// Select a phonon mode for animation.
#[tauri::command]
pub fn set_phonon_mode(
    mode_index: Option<usize>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    phonon_frame_wake: State<'_, crate::commands::PhononFrameWake>,
) -> IpcResult<()> {
    let mut cs = crystal_state
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "crystal state"))?;

    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    if let Some(idx) = mode_index {
        let phonon_data = cs.phonon_data.as_ref().ok_or_else(|| {
            IpcError::invalid_argument("load phonon data before selecting a mode")
        })?;
        let n_modes = phonon_data.modes.len();
        if idx >= n_modes {
            return Err(IpcError::invalid_argument(format!(
                "Mode index {} out of range (0..{})",
                idx, n_modes
            )));
        }

        renderer.set_phonon_mode(Some(&phonon_data.modes[idx]))?;
        renderer.update_bonds(&[]);
    } else {
        renderer.set_phonon_mode(None)?;
    }
    phonon_frame_wake.stop();

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
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let display_scale = amplitude.unwrap_or(1.0);
    if !phase.is_finite() || !display_scale.is_finite() {
        return Err(IpcError::invalid_argument(
            "phonon phase and display scale must be finite",
        ));
    }
    let mut renderer = renderer_state
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "renderer"))?;
    renderer.set_phonon_phase(phase, display_scale)
}

/// Set phonon display scale without changing the renderer-owned playback phase.
#[tauri::command]
pub fn set_phonon_display_scale(
    display_scale: f64,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let mut renderer = renderer_state
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "renderer"))?;
    renderer.set_phonon_display_scale(display_scale)
}

/// Start or stop renderer-owned phonon presentation playback.
#[tauri::command]
pub fn set_phonon_playing(
    playing: bool,
    app: tauri::AppHandle,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    phonon_frame_wake: State<'_, crate::commands::PhononFrameWake>,
) -> IpcResult<()> {
    let mut renderer = renderer_state
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "renderer"))?;
    let was_playing = renderer.phonon_is_playing();
    renderer.set_phonon_playing(playing)?;

    if playing {
        if !was_playing {
            if let Err(error) = phonon_frame_wake.start(app) {
                let _ = renderer.set_phonon_playing(false);
                return Err(error);
            }
        }
    } else {
        phonon_frame_wake.stop();
    }
    Ok(())
}

#[tauri::command]
pub fn add_measurement(
    indices: Vec<usize>,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> IpcResult<crate::crystal_state::MeasurementOverlay> {
    let mut measurement = None;
    crate::transaction::with_state_update(
        &app,
        &crystal_state,
        &settings_state,
        &renderer_state,
        &undo_state,
        |_| Ok(true),
        |cs| {
            measurement = Some(
                cs.add_measurement(&indices)
                    .map_err(IpcError::invalid_argument)?,
            );
            Ok(())
        },
    )?;

    measurement.ok_or_else(|| IpcError::from("measurement transaction returned no result"))
}

#[tauri::command]
pub fn clear_measurements(
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> IpcResult<()> {
    crate::transaction::with_state_update(
        &app,
        &crystal_state,
        &settings_state,
        &renderer_state,
        &undo_state,
        |cs| Ok(!cs.measurements.is_empty()),
        |cs| {
            cs.clear_measurements();
            Ok(())
        },
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_measurements(
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<Vec<crate::crystal_state::MeasurementOverlay>> {
    let cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    Ok(cs.measurements.clone())
}

#[derive(serde::Serialize)]
pub struct MeasurementLabelPos {
    pub label: String,
    pub x: f32,
    pub y: f32,
}

#[tauri::command]
pub fn get_measurement_labels_screen(
    width: f32,
    height: f32,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<Vec<MeasurementLabelPos>> {
    let cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;

    let vp_matrix = renderer.camera.build_view_projection_matrix();

    let mut positions = Vec::new();

    for m in &cs.measurements {
        let pos_3d = glam::Vec3::from_array([
            m.label_position[0],
            m.label_position[1],
            m.label_position[2],
        ]);

        let clip = vp_matrix * pos_3d.extend(1.0);

        // Z-clipping check
        if clip.w > 0.0 {
            let ndc_x = clip.x / clip.w;
            let ndc_y = clip.y / clip.w;

            // Render only if roughly within viewport
            if ndc_x >= -1.2 && ndc_x <= 1.2 && ndc_y >= -1.2 && ndc_y <= 1.2 {
                let text = match m.kind {
                    crate::crystal_state::MeasurementKind::Distance => format!("{:.3} Å", m.value),
                    _ => format!("{:.1}°", m.value),
                };
                positions.push(MeasurementLabelPos {
                    label: text,
                    // Convert NDC to screen Space (Y flips)
                    x: (ndc_x + 1.0) / 2.0 * width,
                    y: (1.0 - ndc_y) / 2.0 * height,
                });
            }
        }
    }

    Ok(positions)
}
