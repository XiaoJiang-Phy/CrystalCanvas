//! Tauri IPC commands for interacting with the CrystalCanvas React UI.
//! Commands handle viewport resizing, loading files, and camera state.

use tauri::State;

/// Sent by the React frontend via ResizeObserver when the transparent viewport <div> resizes.
#[tauri::command]
pub fn update_viewport_size(
    width: u32,
    height: u32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    log::info!("update_viewport_size: {}x{}", width, height);
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.resize(winit::dpi::PhysicalSize::new(width, height));
    }
    Ok(())
}

/// Sets the camera projection mode.
#[tauri::command]
pub fn set_camera_projection(
    is_perspective: bool,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    log::info!("set_camera_projection: perspective={}", is_perspective);
    if let Ok(mut renderer) = renderer_state.try_lock() {
        if is_perspective {
            renderer.camera.set_perspective();
        } else {
            renderer.camera.set_orthographic(30.0); // Assuming 30.0 orthographic scale for now
        }
    }
    Ok(())
}

/// Load a CIF file into the state.
#[tauri::command]
pub fn load_cif_file(
    path: String,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> Result<(), String> {
    log::info!("load_cif_file: {}", path);

    // 1 & 2. Load file (delegating to our format importer)
    let state = crate::io::import::load_file(&path)?;

    if let Ok(mut cs) = crystal_state.try_lock() {
        *cs = state.clone();
    }

    // 3. Build instance data for the Renderer
    let instances = crate::renderer::instance::build_instance_data(
        &state.cart_positions,
        &state.atomic_numbers,
    );

    // 4. Update the renderer
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.update_atoms(&instances);

        // Auto-adjust camera distance based on unit cell size
        let extent = state.cell_a.max(state.cell_b).max(state.cell_c) as f32;
        renderer.camera.eye = glam::Vec3::new(0.0, 0.0, extent * 2.0);
        renderer.camera.target = glam::Vec3::ZERO;
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
) -> Result<(), String> {
    log::info!("add_atom: {} at {:?}", element_symbol, fract_pos);

    let mut cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;
    cs.try_add_atom(&element_symbol, atomic_number, fract_pos)
        .map_err(|_| "Collision detected: atom too close to existing atoms")?;

    let instances =
        crate::renderer::instance::build_instance_data(&cs.cart_positions, &cs.atomic_numbers);
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.update_atoms(&instances);
    }

    Ok(())
}

#[tauri::command]
pub fn delete_atoms(
    indices: Vec<usize>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    log::info!("delete_atoms: {:?}", indices);

    let mut cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;
    cs.delete_atoms(&indices);

    let instances =
        crate::renderer::instance::build_instance_data(&cs.cart_positions, &cs.atomic_numbers);
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.update_atoms(&instances);
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
) -> Result<(), String> {
    log::info!("substitute_atoms: {:?} -> {}", indices, new_element_symbol);

    let mut cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;
    cs.substitute_atoms(&indices, &new_element_symbol, new_atomic_number);

    let instances =
        crate::renderer::instance::build_instance_data(&cs.cart_positions, &cs.atomic_numbers);
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.update_atoms(&instances);
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
