use tauri::State;

#[derive(serde::Serialize)]
pub struct WannierInfo {
    pub num_wann: usize,
    pub r_shells: Vec<[i32; 3]>,
    pub t_max: f64,
}

#[tauri::command]
pub fn load_wannier_hr(
    path: String,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<WannierInfo, String> {
    log::info!("load_wannier_hr: {}", path);
    let hr_data = crate::io::wannier_hr_parser::parse_wannier_hr(&path)?;
    let mut cs = crystal_state.lock().map_err(|e| e.to_string())?;

    if cs.num_atoms() < hr_data.num_wann {
        return Err(format!("Crystal structure has {} atoms, but Wannier data has {} orbitals", cs.num_atoms(), hr_data.num_wann));
    }

    let lattice_col_major = cs.get_lattice_col_major();
    // WannierOverlay::new naturally populates visible_hoppings with defaults
    let overlay = crate::wannier::WannierOverlay::new(hr_data, &lattice_col_major, &cs.cart_positions)?;
    
    let mut renderer = renderer_state.lock().map_err(|e| e.to_string())?;
    let instances = crate::renderer::instance::build_hopping_instances(&overlay.visible_hoppings, overlay.hr_data.t_max);
    renderer.update_hoppings(&instances);
    renderer.show_hoppings = true;
    renderer.show_bonds = false;

    // Extract WannierInfo before moving overlay into cs
    let num_wann = overlay.hr_data.num_wann;
    let r_shells = overlay.hr_data.r_shells.clone();
    let t_max = overlay.hr_data.t_max;

    cs.wannier_overlay = Some(overlay);

    if let Ok(settings) = settings_state.lock() {
        let atoms = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
        renderer.update_atoms(&atoms);
    }

    Ok(WannierInfo {
        num_wann,
        r_shells,
        t_max,
    })
}

#[tauri::command]
pub fn set_wannier_t_min(
    t_min: f64,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    log::info!("set_wannier_t_min: {}", t_min);
    let mut cs = crystal_state.lock().map_err(|e| e.to_string())?;
    
    if let Some(mut overlay) = cs.wannier_overlay.take() {
        overlay.t_min_threshold = t_min;
        let lattice_col_major = cs.get_lattice_col_major();
        overlay.filter_and_rebuild(&lattice_col_major, &cs.cart_positions)?;
        
        let mut renderer = renderer_state.lock().map_err(|e| e.to_string())?;
        let instances = crate::renderer::instance::build_hopping_instances(&overlay.visible_hoppings, overlay.hr_data.t_max);
        renderer.update_hoppings(&instances);
        
        cs.wannier_overlay = Some(overlay);
        
        if let Ok(settings) = settings_state.lock() {
            let atoms = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
            renderer.update_atoms(&atoms);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn set_wannier_r_shell(
    shell_idx: usize,
    active: bool,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    log::info!("set_wannier_r_shell: {} -> {}", shell_idx, active);
    let mut cs = crystal_state.lock().map_err(|e| e.to_string())?;
    
    if let Some(mut overlay) = cs.wannier_overlay.take() {
        if shell_idx < overlay.active_r_shells.len() {
            overlay.active_r_shells[shell_idx] = active;
            let lattice_col_major = cs.get_lattice_col_major();
            overlay.filter_and_rebuild(&lattice_col_major, &cs.cart_positions)?;
            
            let mut renderer = renderer_state.lock().map_err(|e| e.to_string())?;
            let instances = crate::renderer::instance::build_hopping_instances(&overlay.visible_hoppings, overlay.hr_data.t_max);
            renderer.update_hoppings(&instances);
            
            cs.wannier_overlay = Some(overlay);
            
            if let Ok(settings) = settings_state.lock() {
                let atoms = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
                renderer.update_atoms(&atoms);
            }
        } else {
            cs.wannier_overlay = Some(overlay);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn set_wannier_orbital(
    orb_idx: usize,
    active: bool,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    log::info!("set_wannier_orbital: {} -> {}", orb_idx, active);
    let mut cs = crystal_state.lock().map_err(|e| e.to_string())?;
    
    if let Some(mut overlay) = cs.wannier_overlay.take() {
        if orb_idx < overlay.active_orbitals.len() {
            overlay.active_orbitals[orb_idx] = active;
            let lattice_col_major = cs.get_lattice_col_major();
            overlay.filter_and_rebuild(&lattice_col_major, &cs.cart_positions)?;
            
            let mut renderer = renderer_state.lock().map_err(|e| e.to_string())?;
            let instances = crate::renderer::instance::build_hopping_instances(&overlay.visible_hoppings, overlay.hr_data.t_max);
            renderer.update_hoppings(&instances);
            
            cs.wannier_overlay = Some(overlay);
            
            if let Ok(settings) = settings_state.lock() {
                let atoms = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
                renderer.update_atoms(&atoms);
            }
        } else {
            cs.wannier_overlay = Some(overlay);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn toggle_wannier_onsite(
    show: bool,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    log::info!("toggle_wannier_onsite: {}", show);
    let mut cs = crystal_state.lock().map_err(|e| e.to_string())?;
    
    if let Some(mut overlay) = cs.wannier_overlay.take() {
        overlay.show_onsite = show;
        let lattice_col_major = cs.get_lattice_col_major();
        overlay.filter_and_rebuild(&lattice_col_major, &cs.cart_positions)?;
        
        let mut renderer = renderer_state.lock().map_err(|e| e.to_string())?;
        let instances = crate::renderer::instance::build_hopping_instances(&overlay.visible_hoppings, overlay.hr_data.t_max);
        renderer.update_hoppings(&instances);
        
        cs.wannier_overlay = Some(overlay);
        
        if let Ok(settings) = settings_state.lock() {
            let atoms = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
            renderer.update_atoms(&atoms);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn toggle_hopping_display(
    show: bool,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    log::info!("toggle_hopping_display: {}", show);
    let mut renderer = renderer_state.lock().map_err(|e| e.to_string())?;
    renderer.show_hoppings = show;
    Ok(())
}

#[tauri::command]
pub fn clear_wannier(
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> Result<(), String> {
    log::info!("clear_wannier");
    let mut cs = crystal_state.lock().map_err(|e| e.to_string())?;
    cs.wannier_overlay = None;
    
    let mut renderer = renderer_state.lock().map_err(|e| e.to_string())?;
    renderer.update_hoppings(&[]);
    
    if let Ok(settings) = settings_state.lock() {
        let atoms = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
        renderer.update_atoms(&atoms);
    }
    
    renderer.show_hoppings = false;
    renderer.show_bonds = true;
    Ok(())
}
