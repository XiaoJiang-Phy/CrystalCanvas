use tauri::{Emitter, State};

use super::{BaseCrystalState, VolumetricInfo};
use crate::ipc::{ExportFileFormat, ExportImageBackground, IpcEnumInput, IpcError, IpcResult};

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
) -> IpcResult<()> {
    log::info!("load_cif_file: {}", path);

    // 1 & 2. Load file (delegating to our format importer)
    let mut state = crate::io::import::load_file(&path).map_err(IpcError::parse)?;
    log::info!("[load_cif_file] File parsed: {} atoms", state.num_atoms());

    let vol_data = state.volumetric_data.take();
    let vol_info = vol_data.as_ref().map(|v| {
        let extension = std::path::Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        VolumetricInfo {
            grid_dims: v.grid_dims,
            data_min: v.data_min,
            data_max: v.data_max,
            format: extension,
        }
    });

    let base_snapshot = state.clone();
    state.volumetric_data = vol_data;

    let extent = state.cell_a.max(state.cell_b).max(state.cell_c) as f32;
    let center = state.unit_cell_center();
    let mut base = base_state
        .0
        .lock()
        .map_err(|e| IpcError::lock(e.to_string()))?;
    let mut cs = crystal_state
        .lock()
        .map_err(|e| IpcError::lock(format!("Failed to lock crystal state: {}", e)))?;
    let mut u_stack = undo_state
        .lock()
        .map_err(|e| IpcError::lock(format!("Failed to lock undo state: {}", e)))?;
    let settings = settings_state
        .lock()
        .map_err(|e| IpcError::lock(format!("Failed to lock settings: {}", e)))?;
    let instances = crate::wannier::build_atoms_with_ghosts(&state, &settings);
    let mut renderer = renderer_state
        .lock()
        .map_err(|e| IpcError::lock(format!("Failed to lock renderer: {}", e)))?;
    let next_version = cs.version.checked_add(1)
        .ok_or_else(|| IpcError::from("crystal state version exhausted"))?;
    let previous_state = crate::undo::StructuralSnapshot::from_crystal_state(&cs);

    let prepared_volumetric = state
        .volumetric_data
        .as_ref()
        .map(|vol| renderer.prepare_volumetric(vol))
        .transpose()
        .map_err(|_| IpcError::render("GPU out of memory while preparing volumetric grid"))?;

    renderer.clear_structure_bound_overlays();
    renderer.update_atoms(&instances);
    renderer.update_lines(&state, &settings);

    let center_vec = glam::Vec3::from_array(center);
    renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
    renderer.camera.target = center_vec;
    if !renderer.camera.is_perspective {
        renderer.camera.set_orthographic(extent * 1.5);
    }

    if let Some(prepared) = prepared_volumetric {
        renderer.commit_volumetric(prepared);
    }
    renderer.update_camera();

    *base = Some(base_snapshot);
    *cs = state;
    cs.version = next_version;
    u_stack.push(previous_state);
    let can_undo = u_stack.can_undo();
    let can_redo = u_stack.can_redo();
    let version = next_version;

    drop(renderer);
    drop(settings);
    drop(u_stack);
    drop(cs);
    drop(base);

    app.emit("state_changed", crate::transaction::StateChangedPayload { version }).ok();
    app.emit(
        "undo_stack_changed",
        crate::transaction::UndoStackPayload { can_undo, can_redo },
    )
    .ok();
    if let Some(info) = vol_info {
        let _ = app.emit("volumetric_loaded", &info);
    }

    Ok(())
}

#[tauri::command]
pub fn export_file(
    format: IpcEnumInput<ExportFileFormat>,
    path: String,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<()> {
    let format = format.parse("format")?;
    log::info!("export_file: format={:?} path={}", format, path);
    let cx = crystal_state
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "crystal state"))?;
    let fmt = match format {
        ExportFileFormat::Poscar | ExportFileFormat::Vasp => crate::llm::command::ExportFormat::Poscar,
        ExportFileFormat::Lammps => crate::llm::command::ExportFormat::Lammps,
        ExportFileFormat::Qe => crate::llm::command::ExportFormat::Qe,
    };

    match fmt {
        crate::llm::command::ExportFormat::Poscar => {
            crate::io::export::export_poscar(&cx, &path).map_err(|e| IpcError::io(e.to_string()))?
        }
        crate::llm::command::ExportFormat::Lammps => {
            crate::io::export::export_lammps_data(&cx, &path)
                .map_err(|e| IpcError::io(e.to_string()))?
        }
        crate::llm::command::ExportFormat::Qe => crate::io::export::export_qe_input(&cx, &path)
            .map_err(|e| IpcError::io(e.to_string()))?,
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
    bg_mode: IpcEnumInput<ExportImageBackground>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let bg_mode = bg_mode.parse("bgMode")?;
    log::info!(
        "export_image: {}x{}, bg={}, path={}",
        width,
        height,
        bg_mode.as_str(),
        path
    );

    let mut renderer = renderer_state
        .lock()
        .map_err(|e| IpcError::lock(format!("Failed to lock renderer: {}", e)))?;

    let rgba_data = renderer
        .render_offscreen(width, height, bg_mode.as_str())
        .map_err(IpcError::render)?;

    // Determine output format from file extension
    let path_lower = path.to_lowercase();
    if path_lower.ends_with(".jpg") || path_lower.ends_with(".jpeg") {
        // JPEG does not support transparency — composite onto white if transparent
        let rgb_data: Vec<u8> = if matches!(bg_mode, ExportImageBackground::Transparent) {
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
                .ok_or_else(|| IpcError::render("Failed to create JPEG image buffer"))?;
        img.save(&path)
            .map_err(|e| IpcError::io(format!("Failed to save JPEG: {}", e)))?;
    } else {
        // Default: PNG (supports transparency)
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            image::ImageBuffer::from_raw(width, height, rgba_data)
                .ok_or_else(|| IpcError::render("Failed to create PNG image buffer"))?;
        img.save(&path)
            .map_err(|e| IpcError::io(format!("Failed to save PNG: {}", e)))?;
    }

    log::info!("Image exported successfully to {}", path);
    Ok(())
}

#[tauri::command]
pub fn write_text_file(path: String, content: String) -> IpcResult<()> {
    std::fs::write(&path, &content)
        .map_err(|e| IpcError::io(format!("Failed to write {}: {}", path, e)))
}
