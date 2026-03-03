//! Tauri IPC commands for interacting with the CrystalCanvas React UI.
//! Commands handle viewport resizing, loading files, and camera state.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

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

/// Sets visibility flags for unit cell box and bonds.
#[tauri::command]
pub fn set_render_flags(
    show_cell: bool,
    show_bonds: bool,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.show_cell = show_cell;
        renderer.show_bonds = show_bonds;
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
        &state.elements,
    );

    // 4. Update the renderer
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.update_atoms(&instances);
        renderer.update_lines(&state);

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
) -> Result<(), String> {
    log::info!("add_atom: {} at {:?}", element_symbol, fract_pos);

    let mut cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;
    cs.try_add_atom(&element_symbol, atomic_number, fract_pos)
        .map_err(|_| "Collision detected: atom too close to existing atoms")?;

    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
    );
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.update_atoms(&instances);
        renderer.update_lines(&cs);
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

    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
    );
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.update_atoms(&instances);
        renderer.update_lines(&cs);
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

    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
    );
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.update_atoms(&instances);
        renderer.update_lines(&cs);
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

    let new_state = {
        let cs = crystal_state
            .try_lock()
            .map_err(|_| "Failed to lock state")?;
        cs.generate_supercell(&expansion)?
    };

    // Replace the crystal state
    {
        let mut cs = crystal_state
            .try_lock()
            .map_err(|_| "Failed to lock state")?;
        *cs = new_state;
        cs.detect_spacegroup();
    }

    // Update renderer
    let cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;
    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
    );
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.update_atoms(&instances);
        renderer.update_lines(&cs);

        // Auto-adjust camera distance for the new structure
        let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
        let center = cs.unit_cell_center();
        let center_vec = glam::Vec3::from_array(center);
        renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
        renderer.camera.target = center_vec;
        if !renderer.camera.is_perspective {
            renderer.camera.set_orthographic(extent * 1.5);
        }
    }

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
) -> Result<(), String> {
    log::info!(
        "apply_slab: miller={:?} layers={} vacuum={}",
        miller,
        layers,
        vacuum_a
    );

    let new_state = {
        let cs = crystal_state
            .try_lock()
            .map_err(|_| "Failed to lock state")?;
        cs.generate_slab(miller, layers, vacuum_a)?
    };

    // Replace the crystal state
    {
        let mut cs = crystal_state
            .try_lock()
            .map_err(|_| "Failed to lock state")?;
        *cs = new_state;
        cs.detect_spacegroup();
    }

    // Update renderer
    let cs = crystal_state
        .try_lock()
        .map_err(|_| "Failed to lock state")?;
    let instances = crate::renderer::instance::build_instance_data(
        &cs.cart_positions,
        &cs.atomic_numbers,
        &cs.elements,
    );
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.update_atoms(&instances);
        renderer.update_lines(&cs);

        let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
        let center = cs.unit_cell_center();
        let center_vec = glam::Vec3::from_array(center);
        renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
        renderer.camera.target = center_vec;
        if !renderer.camera.is_perspective {
            renderer.camera.set_orthographic(extent * 1.5);
        }
    }

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
// LLM AI Tasks
// =========================================================================

pub struct LlmState(pub std::sync::Mutex<Option<crate::llm::provider::ProviderConfig>>);

fn get_api_key(provider: &str, provided_key: &str) -> String {
    if !provided_key.trim().is_empty() && provided_key != "********" {
        // Save to OS Keychain
        if let Ok(entry) = keyring::Entry::new("CrystalCanvas", provider) {
            let _ = entry.set_password(provided_key); // Ignore errors if keychain is unavailable
        }
        return provided_key.to_string();
    }

    // Try to load from keychain
    if let Ok(entry) = keyring::Entry::new("CrystalCanvas", provider)
        && let Ok(pwd) = entry.get_password()
    {
        return pwd;
    }

    // Fallback to .env for development
    dotenvy::dotenv().ok();
    dotenvy::from_path("../.env").ok();

    let env_key = match provider {
        "openai" => "OPENAI_API_KEY",
        "deepseek" => "DEEPSEEK_API_KEY",
        "claude" => "ANTHROPIC_API_KEY",
        "gemini" => "GEMINI_API_KEY",
        _ => "",
    };

    std::env::var(env_key)
        .or_else(|_| std::env::var(format!("{}_API_KEY", provider.to_uppercase())))
        .unwrap_or_default()
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

    // 4. Trigger rendering update
    let cart_positions = cs.cart_positions.clone();
    let atomic_numbers = cs.atomic_numbers.clone();
    let elements = cs.elements.clone();

    // Release the lock early so we don't hold it over the renderer update if it's slow
    drop(cs);

    let instances =
        crate::renderer::instance::build_instance_data(&cart_positions, &atomic_numbers, &elements);
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.update_atoms(&instances);
        // We technically need `cs` to build lines, but we just dropped it!
        // We shouldn't drop `cs` if we use it for lines since we just computed state.
        // Wait, the fastest way is to skip re-locking and just not drop it. Since we already refactored commands, let's lock it again.
    }
    let cs = crystal_state.try_lock().map_err(|_| "Failed")?;
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.update_lines(&cs);
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
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.camera.rotate_around_target(dx, dy);
    }
    Ok(())
}

/// Zooms the camera based on scroll delta.
#[tauri::command]
pub fn zoom_camera(
    delta: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.camera.zoom_towards_target(delta);
    }
    Ok(())
}

/// Pans the camera.
#[tauri::command]
pub fn pan_camera(
    dx: f32,
    dy: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    if let Ok(mut renderer) = renderer_state.try_lock() {
        renderer.camera.pan(dx, dy);
    }
    Ok(())
}

/// Resets the camera to default view looking over the crystal.
#[tauri::command]
pub fn reset_camera(
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    if let Ok(mut renderer) = renderer_state.try_lock() {
        if let Ok(cs) = crystal_state.try_lock() {
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
