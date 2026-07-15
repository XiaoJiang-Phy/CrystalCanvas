use crate::ipc::{IpcError, IpcResult};
use tauri::{Emitter, State};

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
    log::info!("add_atom: {} at {:?}", element_symbol, fract_pos);
    crate::transaction::with_structural_state_update(
        &app,
        &crystal_state,
        &settings_state,
        &renderer_state,
        &undo_state,
        |cs| {
        let formatted_symbol = crate::llm::router::format_element_symbol(&element_symbol);
        let an = if atomic_number == 0 {
            crate::llm::router::element_to_atomic_number(&formatted_symbol)
        } else {
            atomic_number
        };
        
        if an == 0 {
                return Err(IpcError::invalid_argument(format!(
                    "Invalid element symbol: {}",
                    element_symbol
                )));
        }
        
        cs.try_add_atom(&formatted_symbol, an, fract_pos)
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
    indices: Vec<usize>,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> IpcResult<()> {
    log::info!("delete_atoms: {:?}", indices);
    crate::transaction::with_structural_state_update(
        &app,
        &crystal_state,
        &settings_state,
        &renderer_state,
        &undo_state,
        |cs| {
        cs.delete_atoms(&indices);
        Ok(())
        },
    )
}

#[tauri::command]
pub fn translate_atoms_screen(
    indices: Vec<usize>,
    dx: f32,
    dy: f32,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> IpcResult<()> {
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
    
    let mut cs = crystal_state
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "crystal state"))?;
    let settings = settings_state
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "settings"))?;
    let next_version = cs
        .version
        .checked_add(1)
        .ok_or_else(|| IpcError::from("crystal state version exhausted"))?;
    let rollback = cs
        .translate_atoms_cartesian(&indices, translation)
        .map_err(|_| IpcError::render("unable to allocate atom translation rollback"))?;
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
    cs.version = next_version;
    let version = next_version;
    
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
pub fn substitute_atoms(
    indices: Vec<usize>,
    new_element_symbol: String,
    new_atomic_number: u8,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> IpcResult<()> {
    log::info!("substitute_atoms: {:?} -> {}", indices, new_element_symbol);
    crate::transaction::with_structural_state_update(
        &app,
        &crystal_state,
        &settings_state,
        &renderer_state,
        &undo_state,
        |cs| {
        let formatted_symbol = crate::llm::router::format_element_symbol(&new_element_symbol);
        let mut an = new_atomic_number;
        if an == 0 {
            an = crate::llm::router::element_to_atomic_number(&formatted_symbol);
        }
        
        if an == 0 {
                return Err(IpcError::invalid_argument(format!(
                    "Invalid element symbol: {}",
                    new_element_symbol
                )));
        }
        
        cs.substitute_atoms(&indices, &formatted_symbol, an);
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
    let next_version = cs
        .version
        .checked_add(1)
        .ok_or_else(|| IpcError::from("crystal state version exhausted"))?;
    let mut u_stack = undo_state
        .lock()
        .map_err(|_| IpcError::lock("undo stack lock poisoned"))?;
    let settings = settings_state
        .lock()
        .map_err(|_| IpcError::lock("settings lock poisoned"))?;
    let Some(candidate) = u_stack.undo_candidate_mut() else {
        return Ok(());
    };
    candidate.swap_structural_fields(&mut cs);

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
    cs.version = next_version;
    let version = next_version;
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
    let next_version = cs
        .version
        .checked_add(1)
        .ok_or_else(|| IpcError::from("crystal state version exhausted"))?;
    let mut u_stack = undo_state
        .lock()
        .map_err(|_| IpcError::lock("undo stack lock poisoned"))?;
    let settings = settings_state
        .lock()
        .map_err(|_| IpcError::lock("settings lock poisoned"))?;
    let Some(candidate) = u_stack.redo_candidate_mut() else {
        return Ok(());
    };
    candidate.swap_structural_fields(&mut cs);

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
    cs.version = next_version;
    let version = next_version;
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
