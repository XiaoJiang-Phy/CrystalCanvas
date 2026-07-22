use crate::ipc::{IpcError, IpcResult};
use tauri::{Emitter, State};

fn resolve_element(element_symbol: &str, supplied_atomic_number: u8) -> IpcResult<(String, u8)> {
    let formatted_symbol = crate::llm::router::format_element_symbol(element_symbol);
    let atomic_number = crate::llm::router::element_to_atomic_number(&formatted_symbol);
    if atomic_number == 0 {
        return Err(IpcError::invalid_argument(format!(
            "invalid element symbol: {}",
            element_symbol
        )));
    }
    if supplied_atomic_number != 0 && supplied_atomic_number != atomic_number {
        return Err(IpcError::invalid_argument(
            "atomic number does not match element symbol",
        ));
    }
    Ok((formatted_symbol, atomic_number))
}

fn restore_failed_atom_drag(
    cs: &crate::crystal_state::CrystalState,
    settings: &crate::settings::AppSettings,
    renderer_state: &State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let atom_scene = crate::wannier::build_atoms_with_ghosts(cs, settings)
        .and_then(crate::renderer::instance::prepare_atom_scene)?;
    let line_scene = crate::renderer::instance::build_line_scene(cs, settings)?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    renderer.commit_atoms(atom_scene);
    renderer.update_lines(&line_scene);
    Ok(())
}

fn validate_atom_drag_collision(
    cs: &crate::crystal_state::CrystalState,
    selected_indices: &[usize],
) -> IpcResult<()> {
    // Keep drag commits on the same minimum-image overlap policy as atom creation.
    const OVERLAP_THRESHOLD_A: f64 = 0.5;

    if selected_indices.len() >= cs.intrinsic_sites {
        return Ok(());
    }
    let remaining = cs
        .intrinsic_sites
        .checked_sub(selected_indices.len())
        .ok_or_else(|| IpcError::invalid_argument("atom drag selection exceeds intrinsic atoms"))?;
    let components = remaining
        .checked_mul(3)
        .ok_or_else(|| IpcError::render("atom drag collision buffer size overflow"))?;
    let mut stationary_positions = Vec::new();
    stationary_positions
        .try_reserve_exact(components)
        .map_err(|_| IpcError::render("unable to allocate atom drag collision buffer"))?;
    for index in 0..cs.intrinsic_sites {
        if selected_indices.binary_search(&index).is_ok() {
            continue;
        }
        stationary_positions.push(cs.fract_x[index]);
        stationary_positions.push(cs.fract_y[index]);
        stationary_positions.push(cs.fract_z[index]);
    }

    let lattice = cs.get_lattice_col_major();
    for &index in selected_indices {
        let position = [cs.fract_x[index], cs.fract_y[index], cs.fract_z[index]];
        let overlaps = unsafe {
            crate::ffi::check_overlap_mic(
                lattice.as_ptr(),
                stationary_positions.as_ptr(),
                remaining,
                position.as_ptr(),
                OVERLAP_THRESHOLD_A,
            )
        };
        if overlaps {
            return Err(IpcError::invalid_argument(
                "atom drag collision detected: atom too close to a stationary atom",
            ));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn update_lattice_params(
    a: f64,
    b: f64,
    c: f64,
    alpha: f64,
    beta: f64,
    gamma: f64,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> IpcResult<()> {
    crate::crystal_state::validate_lattice_parameters(a, b, c, alpha, beta, gamma)
        .map_err(IpcError::invalid_argument)?;
    log::info!(
        "update_lattice_params: a={}, b={}, c={}, alpha={}, beta={}, gamma={}",
        a,
        b,
        c,
        alpha,
        beta,
        gamma
    );
    crate::transaction::with_prepared_state_update(
        &app,
        &crystal_state,
        &settings_state,
        &renderer_state,
        &undo_state,
        |cs| {
            let mut prepared =
                crate::undo::StructuralSnapshot::from_crystal_state(cs).into_crystal_state();
        prepared.cell_a = a;
        prepared.cell_b = b;
        prepared.cell_c = c;
        prepared.cell_alpha = alpha;
        prepared.cell_beta = beta;
        prepared.cell_gamma = gamma;
        prepared.fractional_to_cartesian();
        prepared.detect_spacegroup();
        Ok(prepared)
        },
    )
}

#[tauri::command]
pub fn add_atom(
    element_symbol: String,
    atomic_number: u8,
    fract_pos: [f64; 3],
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> IpcResult<()> {
    crate::crystal_state::validate_fractional_position(fract_pos)
        .map_err(IpcError::invalid_argument)?;
    log::info!("add_atom: {} at {:?}", element_symbol, fract_pos);
    let (formatted_symbol, atomic_number) = resolve_element(&element_symbol, atomic_number)?;
    crate::transaction::with_structural_state_update(
        &app,
        &crystal_state,
        &settings_state,
        &renderer_state,
        &undo_state,
        |cs| {
            crate::crystal_state::validate_atom_request(
                &formatted_symbol,
                atomic_number,
                fract_pos,
                cs.num_atoms(),
            )
            .map_err(IpcError::invalid_argument)?;
            Ok(true)
        },
        |cs| {
            cs.try_add_atom(&formatted_symbol, atomic_number, fract_pos)
                .map_err(|_| {
                    IpcError::invalid_argument(
                        "collision detected: atom too close to existing atoms",
                    )
                })?;
            Ok(())
        },
    )
}

#[tauri::command]
pub fn delete_atoms(
    mut indices: Vec<usize>,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> IpcResult<()> {
    if indices.is_empty() {
        return Err(IpcError::invalid_argument("delete requires at least one atom index"));
    }
    indices.sort_unstable();
    indices.dedup();
    log::info!("delete_atoms: {:?}", indices);
    crate::transaction::with_structural_state_update(
        &app,
        &crystal_state,
        &settings_state,
        &renderer_state,
        &undo_state,
        |cs| {
            if indices.iter().any(|&index| index >= cs.intrinsic_sites) {
                return Err(IpcError::invalid_argument(
                    "delete contains an out-of-range atom index",
                ));
            }
            Ok(true)
        },
        |cs| {
            cs.delete_atoms_sorted_unique(&indices);
            Ok(())
        },
    )
}

#[tauri::command]
pub fn translate_atoms_screen(
    mut indices: Vec<usize>,
    dx: f32,
    dy: f32,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> IpcResult<()> {
    if indices.is_empty() {
        return Err(IpcError::invalid_argument("translation requires at least one atom index"));
    }
    if !dx.is_finite() || !dy.is_finite() {
        return Err(IpcError::invalid_argument("translation delta must be finite"));
    }
    if dx == 0.0 && dy == 0.0 {
        return Ok(());
    }
    let (eye, target, up) = {
        let r = renderer_state
            .lock()
            .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
        (r.camera.eye, r.camera.target, r.camera.up)
    };

    let pan_speed = 0.001 * (eye - target).length();
    let forward = (target - eye).normalize();
    let right = forward.cross(up).normalize();
    let up_dir = right.cross(forward).normalize();
    let translation = right * dx * pan_speed - up_dir * dy * pan_speed;
    if !translation.is_finite() {
        return Err(IpcError::invalid_argument("translation is not finite"));
    }
    if translation.length_squared() == 0.0 {
        return Ok(());
    }

    let mut cs = crystal_state
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "crystal state"))?;
    indices.sort_unstable();
    indices.dedup();
    if indices.iter().any(|&index| index >= cs.intrinsic_sites) {
        return Err(IpcError::invalid_argument(
            "translation contains an out-of-range atom index",
        ));
    }
    let pending_version = crate::transaction::next_version(&cs)?;
    let settings = settings_state
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "settings"))?;
    let rollback = cs
        .translate_atoms_cartesian(&indices, translation)
        .map_err(|_| IpcError::render("unable to allocate atom translation rollback"))?;
    if let Err(error) = cs.validate_structural_invariants() {
        cs.rollback_atom_translation(rollback);
        return Err(IpcError::invalid_argument(error));
    }
    let atom_scene = match crate::wannier::build_atoms_with_ghosts(&cs, &settings)
        .and_then(crate::renderer::instance::prepare_atom_scene)
    {
        Ok(atom_scene) => atom_scene,
        Err(error) => {
            cs.rollback_atom_translation(rollback);
            return Err(error);
        }
    };
    let line_scene = match crate::renderer::instance::build_line_scene(&cs, &settings) {
        Ok(line_scene) => line_scene,
        Err(error) => {
            cs.rollback_atom_translation(rollback);
            return Err(error);
        }
    };
    let mut renderer = match renderer_state.try_lock() {
        Ok(renderer) => renderer,
        Err(error) => {
            cs.rollback_atom_translation(rollback);
            return Err(IpcError::from_try_lock(error, "renderer"));
        }
    };
    renderer.commit_atoms(atom_scene);
    renderer.update_lines(&line_scene);
    let version = crate::transaction::commit_version(&mut cs, pending_version)?;

    drop(renderer);
    drop(settings);
    drop(cs);

    app.emit(
        "state_changed",
        crate::transaction::StateChangedPayload { version },
    )
    .ok();
    Ok(())
}

#[tauri::command]
pub fn begin_atom_drag(
    mut indices: Vec<usize>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<String> {
    if indices.is_empty() {
        return Err(IpcError::invalid_argument(
            "atom drag requires at least one atom index",
        ));
    }
    indices.sort_unstable();
    indices.dedup();

    let cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    if indices.iter().any(|&index| index >= cs.intrinsic_sites) {
        return Err(IpcError::invalid_argument(
            "atom drag contains an out-of-range atom index",
        ));
    }
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    renderer.begin_atom_drag(indices, cs.version)
}

#[tauri::command]
pub fn update_atom_drag(
    session_id: String,
    dx: f32,
    dy: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    if session_id.is_empty() {
        return Err(IpcError::invalid_argument("atom drag session id is empty"));
    }
    if !dx.is_finite() || !dy.is_finite() {
        return Err(IpcError::invalid_argument(
            "atom drag screen delta must be finite",
        ));
    }
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    renderer.update_atom_drag(&session_id, dx, dy)
}

#[tauri::command]
pub fn cancel_atom_drag(
    session_id: String,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    if session_id.is_empty() {
        return Err(IpcError::invalid_argument("atom drag session id is empty"));
    }
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    renderer.cancel_atom_drag(&session_id)
}

#[tauri::command]
pub fn commit_atom_drag(
    session_id: String,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> IpcResult<()> {
    if session_id.is_empty() {
        return Err(IpcError::invalid_argument("atom drag session id is empty"));
    }
    let mut cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let mut undo_stack = undo_state
        .lock()
        .map_err(|_| IpcError::lock("undo stack lock poisoned"))?;
    let settings = settings_state
        .lock()
        .map_err(|_| IpcError::lock("settings lock poisoned"))?;
    let session = {
        let mut renderer = renderer_state
            .lock()
            .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
        renderer.take_atom_drag(&session_id)?
    };
    if cs.version != session.source_version {
        let error = IpcError::busy("atom drag source version conflict");
        restore_failed_atom_drag(&cs, &settings, &renderer_state)?;
        return Err(error);
    }
    if session.translation.length_squared() == 0.0 {
        return Ok(());
    }
    let pending_version = match crate::transaction::next_version(&cs) {
        Ok(pending_version) => pending_version,
        Err(error) => {
            restore_failed_atom_drag(&cs, &settings, &renderer_state)?;
            return Err(error);
        }
    };
    let previous_state = crate::undo::StructuralSnapshot::from_crystal_state(&cs);
    let rollback = match cs.translate_atoms_cartesian(&session.source_indices, session.translation)
    {
        Ok(rollback) => rollback,
        Err(_) => {
            restore_failed_atom_drag(&cs, &settings, &renderer_state)?;
            return Err(IpcError::render("unable to allocate atom drag rollback"));
        }
    };
    if let Err(error) = validate_atom_drag_collision(&cs, &session.source_indices) {
        cs.rollback_atom_translation(rollback);
        restore_failed_atom_drag(&cs, &settings, &renderer_state)?;
        return Err(error);
    }
    if let Err(error) = cs.validate_structural_invariants() {
        cs.rollback_atom_translation(rollback);
        restore_failed_atom_drag(&cs, &settings, &renderer_state)?;
        return Err(IpcError::invalid_argument(error));
    }
    let atom_scene =
        match crate::wannier::build_atoms_with_ghosts_with_overlay(&cs, &settings, None)
            .and_then(crate::renderer::instance::prepare_atom_scene)
        {
            Ok(atom_scene) => atom_scene,
            Err(error) => {
                cs.rollback_atom_translation(rollback);
                restore_failed_atom_drag(&cs, &settings, &renderer_state)?;
                return Err(error);
            }
        };
    let line_scene = match crate::renderer::instance::build_line_scene(&cs, &settings) {
        Ok(line_scene) => line_scene,
        Err(error) => {
            cs.rollback_atom_translation(rollback);
            restore_failed_atom_drag(&cs, &settings, &renderer_state)?;
            return Err(error);
        }
    };
    let mut renderer = match renderer_state.lock() {
        Ok(renderer) => renderer,
        Err(_) => {
            cs.rollback_atom_translation(rollback);
            return Err(IpcError::lock("renderer lock poisoned"));
        }
    };
    let version = match crate::transaction::commit_version(&mut cs, pending_version) {
        Ok(version) => version,
        Err(error) => {
            cs.rollback_atom_translation(rollback);
            return Err(error);
        }
    };
    cs.invalidate_structure_bound_data();
    renderer.clear_structure_bound_overlays();
    renderer.commit_atoms(atom_scene);
    renderer.update_lines(&line_scene);
    undo_stack.push(previous_state);
    let can_undo = undo_stack.can_undo();
    let can_redo = undo_stack.can_redo();
    drop(renderer);
    drop(settings);
    drop(undo_stack);
    drop(cs);
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
    Ok(())
}

#[tauri::command]
pub fn substitute_atoms(
    mut indices: Vec<usize>,
    new_element_symbol: String,
    new_atomic_number: u8,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> IpcResult<()> {
    if indices.is_empty() {
        return Err(IpcError::invalid_argument(
            "substitution requires at least one atom index",
        ));
    }
    indices.sort_unstable();
    indices.dedup();
    log::info!("substitute_atoms: {:?} -> {}", indices, new_element_symbol);
    let (formatted_symbol, atomic_number) = resolve_element(&new_element_symbol, new_atomic_number)?;
    crate::transaction::with_structural_state_update(
        &app,
        &crystal_state,
        &settings_state,
        &renderer_state,
        &undo_state,
        |cs| {
            if indices.iter().any(|&index| index >= cs.intrinsic_sites) {
                return Err(IpcError::invalid_argument(
                    "substitution contains an out-of-range atom index",
                ));
            }
            Ok(indices.iter().any(|&index| {
                cs.elements[index].as_str() != formatted_symbol.as_str()
                    || cs.atomic_numbers[index] != atomic_number
            }))
        },
        |cs| {
            cs.substitute_atoms(&indices, &formatted_symbol, atomic_number);
            Ok(())
        },
    )
}

#[tauri::command]
pub fn update_selection(
    indices: Vec<usize>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    // Selection highlights are purely UI, do not push to undo stack
    let mut cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let settings = settings_state
        .lock()
        .map_err(|_| IpcError::lock("settings lock poisoned"))?;
    if indices.iter().any(|&index| index >= cs.intrinsic_sites) {
        return Err(IpcError::invalid_argument(
            "selection contains an out-of-range atom index",
        ));
    }
    let bond_instances =
        crate::renderer::instance::build_bond_instances(&cs, &settings, &indices)?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    renderer.update_bonds(&bond_instances);
    cs.selected_atoms = indices;

    drop(renderer);
    drop(settings);
    drop(cs);

    Ok(())
}

#[tauri::command]
pub fn undo(
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> IpcResult<()> {
    let mut cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let mut u_stack = undo_state
        .lock()
        .map_err(|_| IpcError::lock("undo stack lock poisoned"))?;
    let settings = settings_state
        .lock()
        .map_err(|_| IpcError::lock("settings lock poisoned"))?;
    let Some(candidate) = u_stack.undo_candidate_mut() else {
        return Ok(());
    };
    let pending_version = crate::transaction::next_version(&cs)?;
    candidate.swap_structural_fields(&mut cs);
    if let Err(error) = cs.validate_structural_invariants() {
        if let Some(candidate) = u_stack.undo_candidate_mut() {
            candidate.swap_structural_fields(&mut cs);
        }
        return Err(IpcError::invalid_argument(error));
    }

    let atom_scene = match crate::wannier::build_atoms_with_ghosts_with_overlay(&cs, &settings, None)
        .and_then(crate::renderer::instance::prepare_atom_scene)
    {
        Ok(atom_scene) => atom_scene,
        Err(error) => {
            if let Some(candidate) = u_stack.undo_candidate_mut() {
                candidate.swap_structural_fields(&mut cs);
            }
            return Err(error);
        }
    };
    let line_scene = match crate::renderer::instance::build_line_scene(&cs, &settings) {
        Ok(line_scene) => line_scene,
        Err(error) => {
            if let Some(candidate) = u_stack.undo_candidate_mut() {
                candidate.swap_structural_fields(&mut cs);
            }
            return Err(error);
        }
    };
    let mut renderer = match renderer_state.lock() {
        Ok(renderer) => renderer,
        Err(_) => {
            if let Some(candidate) = u_stack.undo_candidate_mut() {
                candidate.swap_structural_fields(&mut cs);
            }
            return Err(IpcError::lock("renderer lock poisoned"));
        }
    };
    cs.invalidate_structure_bound_data();
    let committed = u_stack.commit_undo();
    debug_assert!(committed);
    let version = crate::transaction::commit_version(&mut cs, pending_version)?;
    let can_undo = u_stack.can_undo();
    let can_redo = u_stack.can_redo();
    renderer.clear_structure_bound_overlays();
    renderer.commit_atoms(atom_scene);
    renderer.update_lines(&line_scene);

    drop(renderer);
    drop(settings);
    drop(u_stack);
    drop(cs);
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

    Ok(())
}

#[tauri::command]
pub fn redo(
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> IpcResult<()> {
    let mut cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let mut u_stack = undo_state
        .lock()
        .map_err(|_| IpcError::lock("undo stack lock poisoned"))?;
    let settings = settings_state
        .lock()
        .map_err(|_| IpcError::lock("settings lock poisoned"))?;
    let Some(candidate) = u_stack.redo_candidate_mut() else {
        return Ok(());
    };
    let pending_version = crate::transaction::next_version(&cs)?;
    candidate.swap_structural_fields(&mut cs);
    if let Err(error) = cs.validate_structural_invariants() {
        if let Some(candidate) = u_stack.redo_candidate_mut() {
            candidate.swap_structural_fields(&mut cs);
        }
        return Err(IpcError::invalid_argument(error));
    }

    let atom_scene = match crate::wannier::build_atoms_with_ghosts_with_overlay(&cs, &settings, None)
        .and_then(crate::renderer::instance::prepare_atom_scene)
    {
        Ok(atom_scene) => atom_scene,
        Err(error) => {
            if let Some(candidate) = u_stack.redo_candidate_mut() {
                candidate.swap_structural_fields(&mut cs);
            }
            return Err(error);
        }
    };
    let line_scene = match crate::renderer::instance::build_line_scene(&cs, &settings) {
        Ok(line_scene) => line_scene,
        Err(error) => {
            if let Some(candidate) = u_stack.redo_candidate_mut() {
                candidate.swap_structural_fields(&mut cs);
            }
            return Err(error);
        }
    };
    let mut renderer = match renderer_state.lock() {
        Ok(renderer) => renderer,
        Err(_) => {
            if let Some(candidate) = u_stack.redo_candidate_mut() {
                candidate.swap_structural_fields(&mut cs);
            }
            return Err(IpcError::lock("renderer lock poisoned"));
        }
    };
    cs.invalidate_structure_bound_data();
    let committed = u_stack.commit_redo();
    debug_assert!(committed);
    let version = crate::transaction::commit_version(&mut cs, pending_version)?;
    let can_undo = u_stack.can_undo();
    let can_redo = u_stack.can_redo();
    renderer.clear_structure_bound_overlays();
    renderer.commit_atoms(atom_scene);
    renderer.update_lines(&line_scene);

    drop(renderer);
    drop(settings);
    drop(u_stack);
    drop(cs);
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

    Ok(())
}
