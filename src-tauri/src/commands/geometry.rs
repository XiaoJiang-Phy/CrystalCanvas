use tauri::State;
use super::BaseCrystalState;
use crate::undo::UndoStack;
use crate::transaction::{with_state_read_try, with_state_update_and_refit};

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
    with_state_read_try(&crystal_state, |cs| {
        cs.generate_slab(miller, layers, vacuum_a)
    })
}

#[tauri::command]
pub fn preview_supercell(
    expansion: [i32; 9],
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> Result<crate::crystal_state::CrystalState, String> {
    log::info!("preview_supercell: {:?}", expansion);
    with_state_read_try(&crystal_state, |cs| {
        cs.generate_supercell(&expansion)
    })
}

/// Apply a supercell expansion to the current crystal, mutating state and updating the renderer.
#[tauri::command]
pub fn apply_supercell(
    matrix: [[i32; 3]; 3],
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<UndoStack>>,
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

    with_state_update_and_refit(&app, &crystal_state, &settings_state, &renderer_state, &undo_state, |cs| {
        let new_state = cs.generate_supercell(&expansion)?;
        *cs = new_state;
        cs.detect_spacegroup();
        Ok(())
    })
}

/// Apply a slab cut to the current crystal, mutating state and updating the renderer.
#[tauri::command]
pub fn apply_slab(
    miller: [i32; 3],
    layers: i32,
    vacuum_a: f64,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<UndoStack>>,
) -> Result<(), String> {
    log::info!(
        "apply_slab: miller={:?} layers={} vacuum={}",
        miller,
        layers,
        vacuum_a
    );

    with_state_update_and_refit(&app, &crystal_state, &settings_state, &renderer_state, &undo_state, |cs| {
        let new_state = cs.generate_slab(miller, layers, vacuum_a)?;
        *cs = new_state;
        cs.detect_spacegroup();
        Ok(())
    })
}

/// Apply Niggli reduction to the current crystal.
#[tauri::command]
pub fn apply_niggli_reduce(
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<UndoStack>>,
) -> Result<(), String> {
    log::info!("apply_niggli_reduce");
    
    with_state_update_and_refit(&app, &crystal_state, &settings_state, &renderer_state, &undo_state, |cs| {
        cs.niggli_reduce()?;
        Ok(())
    })
}

/// Apply cell standardization (primitive or conventional).
#[tauri::command]
pub fn apply_cell_standardize(
    to_primitive: bool,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<UndoStack>>,
) -> Result<(), String> {
    log::info!("apply_cell_standardize: to_primitive={}", to_primitive);
    
    with_state_update_and_refit(&app, &crystal_state, &settings_state, &renderer_state, &undo_state, |cs| {
        if to_primitive {
            cs.to_primitive()?;
        } else {
            cs.to_conventional()?;
        }
        Ok(())
    })
}

/// Shift slab termination to expose a different surface layer.
#[tauri::command]
pub fn shift_termination(
    target_layer_idx: i32,
    layer_tolerance_a: Option<f64>,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<UndoStack>>,
) -> Result<i32, String> {
    let tolerance = layer_tolerance_a.unwrap_or(0.3);
    log::info!(
        "shift_termination: layer_idx={} tolerance={}",
        target_layer_idx, tolerance
    );

    let mut return_layers = 0;
    
    crate::transaction::with_state_update(&app, &crystal_state, &settings_state, &renderer_state, &undo_state, |cs| {
        return_layers = cs.shift_termination(target_layer_idx, tolerance)?;
        Ok(())
    })?;

    Ok(return_layers)
}

/// Restore the original unit cell from the base state.
#[tauri::command]
pub fn restore_unitcell(
    app: tauri::AppHandle,
    base_state: State<'_, BaseCrystalState>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<UndoStack>>,
) -> Result<(), String> {
    log::info!("restore_unitcell triggered");

    let base = base_state.0.lock().map_err(|e| format!("Base state lock failed: {}", e))?;
    let Some(original) = base.as_ref() else {
        return Err("No base structure loaded to restore".to_string());
    };
    
    let orig_clone = original.clone();
    drop(base); // Drop lock early to avoid deadlocks

    with_state_update_and_refit(&app, &crystal_state, &settings_state, &renderer_state, &undo_state, |cs| {
        *cs = orig_clone;
        cs.active_phonon_mode = None;
        cs.phonon_phase = 0.0;
        Ok(())
    })
}
