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
) -> Result<(), String> {
    log::info!("load_cif_file: {}", path);
    // 1. Parse CIF via C++ FFI
    let out = crate::ffi::bridge::ffi::parse_cif_file(&path).map_err(|e| e.to_string())?;

    // 2. Build Crystal State and convert fractional to cartesian
    let mut state = crate::crystal_state::CrystalState::from_ffi(out);
    state.fractional_to_cartesian();

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
