use tauri::{Emitter, State};

use super::{BaseCrystalState, VolumetricInfo};
use crate::ipc::{ExportFileFormat, ExportImageBackground, IpcEnumInput, IpcError, IpcResult};
use crate::renderer::renderer::{PublicationBackground, PublicationRenderConfig};

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
    state
        .validate_structural_invariants()
        .map_err(IpcError::parse)?;
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
    let atom_scene = crate::renderer::instance::prepare_atom_scene(
        crate::wannier::build_atoms_with_ghosts(&state, &settings)?,
    )?;
    let line_scene = crate::renderer::instance::build_line_scene(&state, &settings)?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|e| IpcError::lock(format!("Failed to lock renderer: {}", e)))?;
    let pending_version = crate::transaction::next_version(&cs)?;
    let previous_state = crate::undo::StructuralSnapshot::from_crystal_state(&cs);

    let prepared_volumetric = state
        .volumetric_data
        .as_ref()
        .map(|vol| renderer.prepare_volumetric(vol))
        .transpose()
        .map_err(|_| IpcError::render("GPU out of memory while preparing volumetric grid"))?;

    renderer.clear_structure_bound_overlays();
    renderer.commit_atoms(atom_scene);
    renderer.update_lines(&line_scene);

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
    let version = crate::transaction::stamp_version(&mut state, pending_version);
    *cs = state;
    u_stack.push(previous_state);
    let can_undo = u_stack.can_undo();
    let can_redo = u_stack.can_redo();

    drop(renderer);
    drop(settings);
    drop(u_stack);
    drop(cs);
    drop(base);

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
        ExportFileFormat::Poscar | ExportFileFormat::Vasp => {
            crate::llm::command::ExportFormat::Poscar
        }
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
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let bg_mode = bg_mode.parse("bgMode")?;
    let publication_background = match bg_mode {
        ExportImageBackground::Transparent => PublicationBackground::Transparent,
        ExportImageBackground::White => PublicationBackground::White,
        ExportImageBackground::Black => PublicationBackground::Black,
        ExportImageBackground::Default => PublicationBackground::Current,
    };
    let primary_path = std::path::Path::new(&path);
    let raster_format = crate::export_recipe::validate_publication_raster_targets(primary_path)
        .map_err(IpcError::invalid_argument)?;
    if matches!(bg_mode, ExportImageBackground::Transparent) && raster_format == "jpeg" {
        log::info!("Transparent JPEG request will be composited onto white");
    }
    log::info!(
        "export_image: {}x{}, bg={}, path={}",
        width,
        height,
        bg_mode.as_str(),
        path
    );

    let crystal = crystal_state
        .lock()
        .map_err(|e| IpcError::lock(format!("Failed to lock crystal state: {}", e)))?;
    let settings = settings_state
        .lock()
        .map_err(|e| IpcError::lock(format!("Failed to lock settings: {}", e)))?;
    let renderer = renderer_state
        .lock()
        .map_err(|e| IpcError::lock(format!("Failed to lock renderer: {}", e)))?;

    let recipe = crate::export_recipe::PublicationRasterRecipe::from_current_scene(
        &crystal,
        &settings,
        &renderer,
        width,
        height,
        bg_mode.as_str(),
        raster_format,
    )
    .map_err(IpcError::invalid_argument)?;
    drop(settings);
    drop(crystal);

    let publication_config: PublicationRenderConfig = renderer
        .publication_render_config(
            &recipe.rendering.publication_admission,
            publication_background,
        )
        .map_err(IpcError::render)?;
    let publication_result = renderer
        .render_offscreen(&publication_config)
        .map_err(IpcError::render)?;
    if publication_result.dimensions() != (width, height)
        || !publication_result.is_premultiplied_alpha()
    {
        return Err(IpcError::render(
            "publication render result does not match the requested output contract",
        ));
    }
    let rgba_data = publication_result.into_rgba();

    drop(renderer);

    let recipe_path =
        crate::export_recipe::write_publication_raster_pair(primary_path, rgba_data, recipe)
            .map_err(IpcError::io)?;

    log::info!(
        "Image and export recipe written successfully to {} and {}",
        path,
        recipe_path.display()
    );
    Ok(())
}

#[tauri::command]
pub fn write_text_file(path: String, content: String) -> IpcResult<()> {
    std::fs::write(&path, &content)
        .map_err(|e| IpcError::io(format!("Failed to write {}: {}", path, e)))
}
