use tauri::{Emitter, State};

use super::LlmState;
use crate::ipc::{CameraAxis, IpcEnumInput, IpcError, IpcResult, LlmProvider};

/// Sent by the React frontend via ResizeObserver when the transparent viewport <div> resizes.
#[tauri::command]
pub fn update_viewport_size(
    width: u32,
    height: u32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    log::info!("update_viewport_size: {}x{}", width, height);
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    renderer.resize(winit::dpi::PhysicalSize::new(width, height));
    Ok(())
}

#[tauri::command]
pub fn set_camera_projection(
    is_perspective: bool,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    log::info!("set_camera_projection: perspective={}", is_perspective);

    // Lock crystal state FIRST to avoid AB/BA deadlock with restore_unitcell
    let scale = if !is_perspective {
        let cs = crystal_state
            .lock()
            .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
        let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
        if extent > 0.0 { extent * 1.5 } else { 15.0 }
    } else {
        15.0
    };

    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;

    if is_perspective {
        renderer.camera.set_perspective();
    } else {
        renderer.camera.set_orthographic(scale);
    }

    // Sync frontend UI (topbar might already be in sync, but menu and LLM need it)
    #[derive(Clone, serde::Serialize)]
    struct Payload {
        is_perspective: bool,
    }
    let _ = app.emit("view_projection_changed", Payload { is_perspective });

    Ok(())
}

/// Sets visibility flags for unit cell box and bonds.
/// The render loop in `renderer.render()` checks these booleans each frame,
/// so toggling them is sufficient — no geometry rebuild needed.
#[tauri::command]
pub fn set_render_flags(
    show_cell: bool,
    show_bonds: bool,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    log::info!("set_render_flags: cell={}, bonds={}", show_cell, show_bonds);
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    
    renderer.show_cell = show_cell;
    renderer.show_bonds = show_bonds;
    Ok(())
}

/// Set camera view along a lattice axis or reset the view.
#[tauri::command]
pub fn set_camera_view_axis(
    axis: IpcEnumInput<CameraAxis>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<()> {
    let axis = axis.parse("axis")?;
    log::info!("set_camera_view_axis: {:?}", axis);

    let cs = crystal_state
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "crystal state"))?;
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
        .map_err(|error| IpcError::from_try_lock(error, "renderer"))?;

    let center = cs.unit_cell_center();
    let center_vec = glam::Vec3::from_array(center);
    renderer.camera.target = center_vec;

    match axis {
        CameraAxis::A => {
            let dir = va.normalize();
            renderer.camera.eye = center_vec + dir * dist;
            renderer.camera.up = glam::Vec3::Z;
        }
        CameraAxis::B => {
            let dir = vb.normalize();
            renderer.camera.eye = center_vec + dir * dist;
            renderer.camera.up = glam::Vec3::Z;
        }
        CameraAxis::C => {
            let dir = vc.normalize();
            renderer.camera.eye = center_vec + dir * dist;
            renderer.camera.up = glam::Vec3::Y;
        }
        CameraAxis::AStar => {
            // a* is perpendicular to b-c plane
            let dir = vb.cross(vc).normalize();
            renderer.camera.eye = center_vec + dir * dist;
            renderer.camera.up = glam::Vec3::Z;
        }
        CameraAxis::BStar => {
            let dir = vc.cross(va).normalize();
            renderer.camera.eye = center_vec + dir * dist;
            renderer.camera.up = glam::Vec3::Z;
        }
        CameraAxis::CStar => {
            let dir = va.cross(vb).normalize();
            renderer.camera.eye = center_vec + dir * dist;
            renderer.camera.up = glam::Vec3::Y;
        }
        CameraAxis::Reset => {
            renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, dist);
            renderer.camera.up = glam::Vec3::Y;
        }
    }

    Ok(())
}

fn get_api_key(provider: &str, provided_key: &str) -> String {
    let clean_provided = provided_key.trim();
    if !clean_provided.is_empty() && clean_provided != "********" && clean_provided != "••••••••"
    {
        // Save to OS Keychain
        if let Ok(entry) = keyring::Entry::new("CrystalCanvas", provider) {
            let _ = entry.set_password(clean_provided); // Ignore errors if keychain is unavailable
        }
        return clean_provided.to_string();
    }

    // Try to load from keychain
    if let Ok(entry) = keyring::Entry::new("CrystalCanvas", provider) {
        if let Ok(pwd) = entry.get_password() {
            if !pwd.trim().is_empty() && pwd.trim() != "********" && pwd.trim() != "••••••••"
            {
                return pwd.trim().to_string();
            }
        }
    }

    // Fallback to .env for development
    dotenvy::dotenv().ok();
    dotenvy::from_path("../.env").ok();
    
    // Case-insensitive env var search
    let target_key = if provider == "claude" {
        "anthropic_api_key".to_string()
    } else {
        format!("{}_api_key", provider.to_lowercase())
    };

    for (k, v) in std::env::vars() {
        if k.to_lowercase() == target_key {
            return v.trim().to_string();
        }
    }

    String::new()
}

#[tauri::command]
pub fn check_api_key_status(provider_type: IpcEnumInput<LlmProvider>) -> IpcResult<bool> {
    let provider_type = provider_type.parse("providerType")?;
    let key = get_api_key(provider_type.as_str(), "");
    Ok(!key.is_empty())
}

#[tauri::command]
pub fn llm_configure(
    provider_type: IpcEnumInput<LlmProvider>,
    api_key: String,
    model: String,
    state: State<'_, LlmState>,
) -> IpcResult<()> {
    let provider_type = provider_type.parse("providerType")?;
    let provider_name = provider_type.as_str();
    let resolved_key = if matches!(provider_type, LlmProvider::Ollama) {
        String::new()
    } else {
        get_api_key(provider_name, &api_key)
    };

    let config = match provider_type {
        LlmProvider::Openai => crate::llm::provider::ProviderConfig::OpenAi {
            api_key: resolved_key,
            model,
        },
        LlmProvider::Deepseek => crate::llm::provider::ProviderConfig::DeepSeek {
            api_key: resolved_key,
            model,
        },
        LlmProvider::Claude => crate::llm::provider::ProviderConfig::Claude {
            api_key: resolved_key,
            model,
        },
        LlmProvider::Gemini => crate::llm::provider::ProviderConfig::Gemini {
            api_key: resolved_key,
            model,
        },
        LlmProvider::Ollama => crate::llm::provider::ProviderConfig::Ollama { model },
    };
    let mut st = state
        .0
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "LLM state"))?;
    *st = Some(config);
    Ok(())
}

#[tauri::command]
pub async fn llm_chat(
    user_message: String,
    selected_indices: Option<Vec<usize>>,
    state: State<'_, LlmState>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<String> {
    let config_opt = {
        let st = state
            .0
            .try_lock()
            .map_err(|error| IpcError::from_try_lock(error, "LLM state"))?;
        st.clone()
    };

    let config =
        config_opt.ok_or_else(|| IpcError::invalid_argument("LLM provider is not configured"))?;

    let context = {
        let cs = crystal_state
            .try_lock()
            .map_err(|error| IpcError::from_try_lock(error, "crystal state"))?;
        crate::llm::context::build_crystal_context(&cs, selected_indices.as_deref())
    };

    let messages = crate::llm::prompt::build_messages(&context, &user_message);

    let provider = crate::llm::provider::create_provider(&config);
    provider.chat(&messages).await.map_err(IpcError::io)
}

#[tauri::command]
pub fn llm_execute_command(
    command_json: String,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> IpcResult<()> {
    // 1. Layer 1: Schema parse validation
    let command: crate::llm::command::CrystalCommand = serde_json::from_str(&command_json)
        .map_err(|e| IpcError::parse(format!("schema validation failed: {}", e)))?;

    let dry_run_state = {
        let cs = crystal_state
            .lock()
            .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
        crate::undo::StructuralSnapshot::from_crystal_state(&cs).into_crystal_state()
    };

    // 2. Layer 2: Physics Sandbox validation
    crate::llm::sandbox::validate_command(&command, &dry_run_state).map_err(|e| {
        IpcError::invalid_argument(format!("physics sandbox rejected command: {}", e))
    })?;

    if matches!(&command, crate::llm::command::CrystalCommand::ExportFile(_)) {
        let mut cs = crystal_state
            .lock()
            .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
        return crate::llm::router::execute_command(command, &mut cs)
            .map_err(|e| IpcError::invalid_argument(format!("command execution failed: {}", e)));
    }

    crate::transaction::with_prepared_state_update_and_refit(
        &app,
        &crystal_state,
        &settings_state,
        &renderer_state,
        &undo_state,
        |cs| {
            let mut prepared =
                crate::undo::StructuralSnapshot::from_crystal_state(cs).into_crystal_state();
            crate::llm::router::execute_command(command, &mut prepared).map_err(|e| {
                IpcError::invalid_argument(format!("command execution failed: {}", e))
            })?;
        Ok(prepared)
        },
    )
}

#[tauri::command]
pub fn get_crystal_state(
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<crate::crystal_state::CrystalState> {
    let cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    Ok(cs.clone())
}

/// Rotates the camera orbitally.
#[tauri::command]
pub fn rotate_camera(
    dx: f32,
    dy: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
        
    if renderer.show_bz {
        // Rotation disabled in BZ view — labels use a fixed camera projection
    } else {
        renderer.camera.rotate_around_target(dx, dy);
    }
    Ok(())
}

/// Zooms the camera based on scroll delta.
#[tauri::command]
pub fn zoom_camera(
    delta: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
        
    if renderer.show_bz {
        let crate::renderer::renderer::Renderer {
            ref gpu,
            ref mut bz_viewport,
            ..
        } = *renderer;
        if let Some(bz_vp) = bz_viewport {
            bz_vp.camera.zoom_towards_target(delta);
            bz_vp.update_camera(&gpu.queue);
        }
    } else {
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
) -> IpcResult<()> {
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
        
    if renderer.show_bz {
        let crate::renderer::renderer::Renderer {
            ref gpu,
            ref mut bz_viewport,
            ..
        } = *renderer;
        if let Some(bz_vp) = bz_viewport {
            bz_vp.camera.pan(dx, dy);
            bz_vp.update_camera(&gpu.queue);
        }
    } else {
        renderer.camera.pan(dx, dy);
    }
    Ok(())
}

/// Resets the camera to default view looking over the crystal.
#[tauri::command]
pub fn reset_camera(
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;

    let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
    let dist = extent * 2.5;
    let center = cs.unit_cell_center();
    let center_vec = glam::Vec3::from_array(center);
    renderer.camera.target = center_vec;
    renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, dist);
    renderer.camera.orthographic_scale = extent * 1.5;

    Ok(())
}

/// Perform ray-sphere intersection to pick an atom.
#[tauri::command]
pub fn pick_atom(
    x: f32,
    y: f32,
    screen_w: f32,
    screen_h: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<Option<usize>> {
    log::info!(
        "pick_atom: window screen_w={} screen_h={} pointer x={} y={}",
        screen_w,
        screen_h,
        x,
        y
    );
    let (camera_eye, view_proj, is_perspective, pick_scene) = {
        let renderer = renderer_state
            .try_lock()
            .map_err(|error| IpcError::from_try_lock(error, "renderer"))?;
        let vp = renderer.camera.build_projection_matrix() * renderer.camera.build_view_matrix();
        (
            renderer.camera.eye,
            vp,
            renderer.camera.is_perspective,
            renderer.pick_scene_snapshot(),
        )
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

    let closest_idx = crate::renderer::ray_picking::ray_pick(
        &pick_scene,
        &crate::renderer::ray_picking::Ray {
            origin: ray_origin.to_array(),
            direction: ray_dir.to_array(),
        },
    )
    .map(|hit| hit.index);

    let renderer = renderer_state
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "renderer"))?;
    let current_view_proj =
        renderer.camera.build_projection_matrix() * renderer.camera.build_view_matrix();
    if !renderer.is_pick_scene_current(&pick_scene) || current_view_proj != view_proj {
        return Err(IpcError::busy("pick scene changed during picking"));
    }

    log::info!("pick_atom completed: found closest idx = {:?}", closest_idx);

    Ok(closest_idx)
}

#[tauri::command]
pub fn get_settings(
    settings: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> IpcResult<crate::settings::AppSettings> {
    Ok(settings
        .lock()
        .map_err(|_| IpcError::lock("settings lock poisoned"))?
        .clone())
}

#[tauri::command]
pub fn update_settings(
    app: tauri::AppHandle,
    new_settings: crate::settings::AppSettings,
    settings: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    log::info!("update_settings called");
    let cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let mut current_settings = settings
        .lock()
        .map_err(|_| IpcError::lock("settings lock poisoned"))?;
    let atom_scene = crate::renderer::instance::prepare_atom_scene(
        crate::wannier::build_atoms_with_ghosts(&cs, &new_settings)?,
    )?;
    let line_scene = crate::renderer::instance::build_line_scene(&cs, &new_settings)?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    renderer.commit_atoms(atom_scene);
    renderer.update_lines(&line_scene);
    *current_settings = new_settings.clone();

    drop(renderer);
    drop(current_settings);
    drop(cs);

    let _ = new_settings
        .save(&app)
        .map_err(|e| log::warn!("Failed to save settings: {}", e));

    Ok(())
}
