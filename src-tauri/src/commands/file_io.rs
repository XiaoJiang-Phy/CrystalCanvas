use tauri::{Emitter, State};

use super::{BaseCrystalState, VolumetricInfo};

/// Load a CIF file into the state.
#[tauri::command]
pub fn load_cif_file(
    path: String,
    app: tauri::AppHandle,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    base_state: State<'_, BaseCrystalState>,
    undo_state: State<'_, std::sync::Mutex<crate::undo::UndoStack>>,
) -> Result<(), String> {
    log::info!("load_cif_file: {}", path);

    // 1 & 2. Load file (delegating to our format importer)
    let mut state = crate::io::import::load_file(&path)?;
    log::info!("[load_cif_file] File parsed: {} atoms", state.num_atoms());

    let vol_data = state.volumetric_data.take();
    let vol_info = vol_data.as_ref().map(|v| {
        let extension = std::path::Path::new(&path).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        VolumetricInfo {
            grid_dims: v.grid_dims,
            data_min: v.data_min,
            data_max: v.data_max,
            format: extension,
        }
    });

    {
        let mut base = base_state.0.lock().map_err(|e| format!("{}", e))?;
        *base = Some(state.clone()); // Clonned without heavy volumetric buffer
    }

    state.volumetric_data = vol_data;

    let extent = state.cell_a.max(state.cell_b).max(state.cell_c) as f32;
    let center = state.unit_cell_center();

    let settings = settings_state.lock().map_err(|e| format!("Failed to lock settings: {}", e))?;
    let instances = crate::wannier::build_atoms_with_ghosts(&state, &settings);

    {
        let mut renderer = renderer_state.lock().map_err(|e| format!("Failed to lock renderer: {}", e))?;
        renderer.update_atoms(&instances);
        renderer.update_lines(&state, &settings);

        let center_vec = glam::Vec3::from_array(center);
        renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
        renderer.camera.target = center_vec;
        if !renderer.camera.is_perspective {
            renderer.camera.set_orthographic(extent * 1.5);
        }

        if let Some(vol) = &state.volumetric_data {
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                renderer.upload_volumetric(vol);
            }));
            if res.is_err() {
                log::error!("GPU OOM: Failed to create volumetric pipelines. File too large.");
                renderer.clear_volumetric();
            }
        } else {
            renderer.clear_volumetric();
        }
        renderer.update_camera();
    }
    drop(settings);

    let can_undo;
    let can_redo;
    {
        let mut cs = crystal_state.lock().map_err(|e| format!("Failed to lock crystal state: {}", e))?;
        // Snapshot the old state before overriding
        let mut u_stack = undo_state.lock().map_err(|e| format!("Failed to lock undo state: {}", e))?;
        u_stack.push(crate::undo::LightweightState::from_crystal_state(&cs));
        can_undo = u_stack.can_undo();
        can_redo = u_stack.can_redo();
        drop(u_stack);

        *cs = state;
        cs.version += 1;
    }

    app.emit("state_changed", ()).ok();
    app.emit("undo_stack_changed", crate::transaction::UndoStackPayload { can_undo, can_redo }).ok();
    if let Some(info) = vol_info {
        let _ = app.emit("volumetric_loaded", &info);
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

/// Export the current viewport as a high-resolution image.
/// Renders off-screen at the specified dimensions and saves to the given path.
#[tauri::command]
pub fn export_image(
    path: String,
    width: u32,
    height: u32,
    bg_mode: String,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    log::info!(
        "export_image: {}x{}, bg={}, path={}",
        width,
        height,
        bg_mode,
        path
    );

    let mut renderer = renderer_state
        .lock()
        .map_err(|e| format!("Failed to lock renderer: {}", e))?;

    let rgba_data = renderer.render_offscreen(width, height, &bg_mode)?;

    // Determine output format from file extension
    let path_lower = path.to_lowercase();
    if path_lower.ends_with(".jpg") || path_lower.ends_with(".jpeg") {
        // JPEG does not support transparency — composite onto white if transparent
        let rgb_data: Vec<u8> = if bg_mode == "transparent" {
            rgba_data
                .chunks_exact(4)
                .flat_map(|px| {
                    let a = px[3] as f32 / 255.0;
                    [
                        (px[0] as f32 * a + 255.0 * (1.0 - a)) as u8,
                        (px[1] as f32 * a + 255.0 * (1.0 - a)) as u8,
                        (px[2] as f32 * a + 255.0 * (1.0 - a)) as u8,
                    ]
                })
                .collect()
        } else {
            rgba_data
                .chunks_exact(4)
                .flat_map(|px| [px[0], px[1], px[2]])
                .collect()
        };

        let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_raw(width, height, rgb_data)
                .ok_or_else(|| "Failed to create JPEG image buffer".to_string())?;
        img.save(&path)
            .map_err(|e| format!("Failed to save JPEG: {}", e))?;
    } else {
        // Default: PNG (supports transparency)
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            image::ImageBuffer::from_raw(width, height, rgba_data)
                .ok_or_else(|| "Failed to create PNG image buffer".to_string())?;
        img.save(&path)
            .map_err(|e| format!("Failed to save PNG: {}", e))?;
    }

    log::info!("Image exported successfully to {}", path);
    Ok(())
}

#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, &content).map_err(|e| format!("Failed to write {}: {}", path, e))
}
