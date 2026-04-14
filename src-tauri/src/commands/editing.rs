use tauri::{Emitter, State};

#[tauri::command]
pub fn update_lattice_params(
    a: f64, b: f64, c: f64,
    alpha: f64, beta: f64, gamma: f64,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> Result<(), String> {
    log::info!("update_lattice_params: a={}, b={}, c={}, alpha={}, beta={}, gamma={}", a, b, c, alpha, beta, gamma);
    crate::transaction::with_state_update(&app, &crystal_state, &settings_state, &renderer_state, &undo_state, |cs| {
        cs.cell_a = a;
        cs.cell_b = b;
        cs.cell_c = c;
        cs.cell_alpha = alpha;
        cs.cell_beta = beta;
        cs.cell_gamma = gamma;
        cs.fractional_to_cartesian();
        cs.detect_spacegroup();
        Ok(())
    })
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
) -> Result<(), String> {
    log::info!("add_atom: {} at {:?}", element_symbol, fract_pos);
    crate::transaction::with_state_update(&app, &crystal_state, &settings_state, &renderer_state, &undo_state, |cs| {
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
        Ok(())
    })
}

#[tauri::command]
pub fn delete_atoms(
    indices: Vec<usize>,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> Result<(), String> {
    log::info!("delete_atoms: {:?}", indices);
    crate::transaction::with_state_update(&app, &crystal_state, &settings_state, &renderer_state, &undo_state, |cs| {
        cs.delete_atoms(&indices);
        Ok(())
    })
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
) -> Result<(), String> {
    let (eye, target, up) = {
        let r = renderer_state.lock().map_err(|_| "Failed to lock renderer")?;
        (r.camera.eye, r.camera.target, r.camera.up)
    };
    
    let pan_speed = 0.001 * (eye - target).length();
    let forward = (target - eye).normalize();
    let right = forward.cross(up).normalize();
    let up_dir = right.cross(forward).normalize();
    let translation = right * dx * pan_speed - up_dir * dy * pan_speed;
    
    let mut cs = crystal_state.try_lock().map_err(|_| "State currently in use")?;
    cs.translate_atoms_cartesian(&indices, translation);
    cs.version += 1;
    
    let settings = settings_state.try_lock().map_err(|_| "Settings in use")?;
    let instances = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
    
    let mut renderer = renderer_state.try_lock().map_err(|_| "Renderer in use")?;
    renderer.update_atoms(&instances);
    renderer.update_lines(&cs, &settings);
    if !indices.is_empty() {
        let bond_instances = crate::renderer::instance::build_bond_instances(&cs, &settings, &cs.selected_atoms);
        renderer.update_bonds(&bond_instances);
    }
    
    drop(renderer);
    drop(settings);
    drop(cs);

    app.emit("state_changed", ()).ok();
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
) -> Result<(), String> {
    log::info!("substitute_atoms: {:?} -> {}", indices, new_element_symbol);
    crate::transaction::with_state_update(&app, &crystal_state, &settings_state, &renderer_state, &undo_state, |cs| {
        let formatted_symbol = crate::llm::router::format_element_symbol(&new_element_symbol);
        let mut an = new_atomic_number;
        if an == 0 {
            an = crate::llm::router::element_to_atomic_number(&formatted_symbol);
        }
        
        if an == 0 {
            return Err(format!("Invalid element symbol: {}", new_element_symbol));
        }
        
        cs.substitute_atoms(&indices, &formatted_symbol, an);
        Ok(())
    })
}

#[tauri::command]
pub fn update_selection(
    indices: Vec<usize>,
    app: tauri::AppHandle,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    // Selection highlights are purely UI, do not push to undo stack
    let mut cs = crystal_state.lock().map_err(|_| "Failed to lock state")?;
    cs.selected_atoms = indices.clone();
    
    let settings = settings_state.lock().map_err(|_| "Failed to lock settings")?;
    let bond_instances = crate::renderer::instance::build_bond_instances(&cs, &settings, &indices);
    
    let mut renderer = renderer_state.lock().map_err(|_| "Failed to lock renderer")?;
    renderer.update_bonds(&bond_instances);
    
    drop(renderer);
    drop(settings);
    drop(cs);

    app.emit("state_changed", ()).ok();
    Ok(())
}

#[tauri::command]
pub fn undo(
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> Result<(), String> {
    let mut cs = crystal_state.lock().map_err(|_| "Failed to lock state")?;
    let mut u_stack = undo_state.lock().map_err(|e| format!("Undo stack locked: {}", e))?;
    
    // Attempt undo
    if let Some(prev_state) = u_stack.undo(crate::undo::LightweightState::from_crystal_state(&cs)) {
        // Restore properties from prev_state to cs
        cs.name = prev_state.name;
        cs.spacegroup_hm = prev_state.spacegroup_hm;
        cs.spacegroup_number = prev_state.spacegroup_number;
        cs.is_2d = prev_state.is_2d;
        cs.vacuum_axis = prev_state.vacuum_axis;
        cs.intrinsic_sites = prev_state.intrinsic_sites;
        
        cs.cell_a = prev_state.cell_a;
        cs.cell_b = prev_state.cell_b;
        cs.cell_c = prev_state.cell_c;
        cs.cell_alpha = prev_state.cell_alpha;
        cs.cell_beta = prev_state.cell_beta;
        cs.cell_gamma = prev_state.cell_gamma;
        
        cs.labels = prev_state.labels;
        cs.elements = prev_state.elements;
        cs.fract_x = prev_state.fract_x;
        cs.fract_y = prev_state.fract_y;
        cs.fract_z = prev_state.fract_z;
        cs.occupancies = prev_state.occupancies;
        cs.atomic_numbers = prev_state.atomic_numbers;
        cs.cart_positions = prev_state.cart_positions;
        cs.selected_atoms = prev_state.selected_atoms;
        
        cs.version += 1;
        let can_undo = u_stack.can_undo();
        let can_redo = u_stack.can_redo();
        drop(u_stack); // Drop early
        
        let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
        let instances = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
        let bond_instances = crate::renderer::instance::build_bond_instances(&cs, &settings, &cs.selected_atoms);
        let mut renderer = renderer_state.lock().map_err(|_| "Renderer lock fail")?;
        renderer.update_atoms(&instances);
        renderer.update_lines(&cs, &settings);
        renderer.update_bonds(&bond_instances);
        
        drop(renderer);
        drop(settings);
        drop(cs);
        app.emit("state_changed", ()).ok();
        app.emit("undo_stack_changed", crate::transaction::UndoStackPayload { can_undo, can_redo }).ok();
    }
    
    Ok(())
}

#[tauri::command]
pub fn redo(
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> Result<(), String> {
    let mut cs = crystal_state.lock().map_err(|_| "Failed to lock state")?;
    let mut u_stack = undo_state.lock().map_err(|e| format!("Undo stack locked: {}", e))?;
    
    // Attempt redo
    if let Some(next_state) = u_stack.redo(crate::undo::LightweightState::from_crystal_state(&cs)) {
        cs.name = next_state.name;
        cs.spacegroup_hm = next_state.spacegroup_hm;
        cs.spacegroup_number = next_state.spacegroup_number;
        cs.is_2d = next_state.is_2d;
        cs.vacuum_axis = next_state.vacuum_axis;
        cs.intrinsic_sites = next_state.intrinsic_sites;
        
        cs.cell_a = next_state.cell_a;
        cs.cell_b = next_state.cell_b;
        cs.cell_c = next_state.cell_c;
        cs.cell_alpha = next_state.cell_alpha;
        cs.cell_beta = next_state.cell_beta;
        cs.cell_gamma = next_state.cell_gamma;
        
        cs.labels = next_state.labels;
        cs.elements = next_state.elements;
        cs.fract_x = next_state.fract_x;
        cs.fract_y = next_state.fract_y;
        cs.fract_z = next_state.fract_z;
        cs.occupancies = next_state.occupancies;
        cs.atomic_numbers = next_state.atomic_numbers;
        cs.cart_positions = next_state.cart_positions;
        cs.selected_atoms = next_state.selected_atoms;
        
        cs.version += 1;
        let can_undo = u_stack.can_undo();
        let can_redo = u_stack.can_redo();
        drop(u_stack); // Drop early
        
        let settings = settings_state.lock().map_err(|_| "Settings lock fail")?;
        let instances = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
        let bond_instances = crate::renderer::instance::build_bond_instances(&cs, &settings, &cs.selected_atoms);
        let mut renderer = renderer_state.lock().map_err(|_| "Renderer lock fail")?;
        renderer.update_atoms(&instances);
        renderer.update_lines(&cs, &settings);
        renderer.update_bonds(&bond_instances);
        
        drop(renderer);
        drop(settings);
        drop(cs);
        app.emit("state_changed", ()).ok();
        app.emit("undo_stack_changed", crate::transaction::UndoStackPayload { can_undo, can_redo }).ok();
    }
    
    Ok(())
}
