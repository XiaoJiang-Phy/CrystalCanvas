use tauri::{Emitter, State};

use super::{BaseCrystalState, VolumetricInfo};
use crate::ipc::{ExportFileFormat, ExportImageBackground, IpcEnumInput, IpcError, IpcResult};
use crate::renderer::publication_look::{PublicationLookProfile, PublicationLookProfileId};
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

    let admitted_field = if state.volumetric_data.is_some() {
        let source_sha256 =
            crate::volumetric::source_artifact_sha256(&path).map_err(IpcError::parse)?;
        Some(
            state
                .admit_volumetric_import(
                    std::path::Path::new(&path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("field")
                        .to_owned(),
                    source_sha256,
                )
                .map_err(IpcError::invalid_argument)?,
        )
    } else {
        None
    };
    let vol_info = admitted_field.map(|field| {
        let extension = std::path::Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        VolumetricInfo {
            grid_dims: field.grid_dims,
            data_min: field.data_min,
            data_max: field.data_max,
            format: extension,
        }
    });

    let mut base_snapshot = state.clone();
    base_snapshot.field_scene = Default::default();

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
        .field_scene
        .active_layer()
        .map(|layer| {
            renderer
                .prepare_field_layer(layer)
                .map(|prepared| (prepared, layer.id, layer.revision))
        })
        .transpose()
        .map_err(|_| IpcError::render("GPU out of memory while preparing volumetric grid"))?;

    renderer.clear_non_field_structure_bound_overlays();
    renderer.commit_atoms(atom_scene);
    renderer.update_lines(&line_scene);

    let center_vec = glam::Vec3::from_array(center);
    renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
    renderer.camera.target = center_vec;
    if !renderer.camera.is_perspective {
        renderer.camera.set_orthographic(extent * 1.5);
    }

    if let Some((prepared, layer_id, layer_revision)) = prepared_volumetric {
        renderer
            .commit_field_layer(prepared, layer_id, layer_revision)
            .map_err(|_| IpcError::render("stale field layer preparation"))?;
    } else {
        renderer.clear_volumetric();
    }
    renderer.update_camera();

    let field_payload = super::volumetric::FieldSceneChangedPayload::from_scene(&state.field_scene);
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
    let _ = app.emit("field_scene_changed", field_payload);

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
    publication_profile: Option<IpcEnumInput<PublicationLookProfileId>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let bg_mode = bg_mode.parse("bgMode")?;
    let profile_id = publication_profile
        .map(|profile| profile.parse("publicationProfile"))
        .transpose()?
        .unwrap_or(PublicationLookProfileId::ScientificGloss);
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

    let look_profile = PublicationLookProfile::for_id(profile_id).map_err(IpcError::render)?;
    let (recipe, publication_bond_instances) =
        crate::export_recipe::PublicationRasterRecipe::from_current_scene(
            &crystal,
            &settings,
            &renderer,
            look_profile,
            width,
            height,
            bg_mode.as_str(),
            raster_format,
        )
        .map_err(IpcError::invalid_argument)?;
    let publication_background = match recipe.output.effective_background.as_str() {
        "transparent" => PublicationBackground::Transparent,
        "white" => PublicationBackground::White,
        "black" => PublicationBackground::Black,
        "default" => PublicationBackground::Current,
        _ => {
            return Err(IpcError::render(
                "publication recipe selected an unsupported effective background",
            ));
        }
    };
    drop(settings);
    drop(crystal);

    let publication_config: PublicationRenderConfig = renderer
        .publication_render_config_with_profile(
            &recipe.rendering.publication_admission,
            publication_background,
            look_profile,
            publication_bond_instances,
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

/// Export the admitted structure scene as a one-way Blender-compatible GLB.
#[tauri::command]
pub fn export_blender_scene(
    path: String,
    publication_profile: IpcEnumInput<PublicationLookProfileId>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let profile_id = publication_profile.parse("publicationProfile")?;
    let primary_path = std::path::Path::new(&path);
    crate::export_recipe::validate_publication_glb_targets(primary_path)
        .map_err(IpcError::invalid_argument)?;
    let crystal = crystal_state
        .lock()
        .map_err(|error| IpcError::lock(format!("Failed to lock crystal state: {error}")))?;
    let settings = settings_state
        .lock()
        .map_err(|error| IpcError::lock(format!("Failed to lock settings: {error}")))?;
    let renderer = renderer_state
        .lock()
        .map_err(|error| IpcError::lock(format!("Failed to lock renderer: {error}")))?;
    let look_profile = PublicationLookProfile::for_id(profile_id).map_err(IpcError::render)?;
    let structure = crate::scene_export::build_publication_scene_snapshot(
        &crystal,
        &settings,
        &renderer,
        look_profile,
    )
    .map_err(|error| IpcError::render(error.message))?;
    let frozen_crystal = crystal.clone();
    drop(renderer);
    drop(settings);
    drop(crystal);
    let scene = crate::scene_export::build_publication_field_scene_from_snapshot(
        &frozen_crystal,
        structure,
    )
    .map_err(|error| IpcError::render(error.message))?;
    let recipe =
        crate::export_recipe::PublicationGlbRecipe::from_field_scene(&frozen_crystal, &scene)
            .map_err(IpcError::invalid_argument)?;
    let artifact = crate::blender_export::build_blender_glb_field_scene(&scene, &recipe.export_id)
        .map_err(|error| IpcError::render(error.message))?;
    let mut recipe = recipe;
    recipe.semantic_inventory = artifact.semantic_inventory;
    crate::export_recipe::write_publication_glb_pair(primary_path, &artifact.bytes, recipe)
        .map_err(IpcError::io)?;
    Ok(())
}

#[tauri::command]
pub fn write_text_file(path: String, content: String) -> IpcResult<()> {
    std::fs::write(&path, &content)
        .map_err(|e| IpcError::io(format!("Failed to write {}: {}", path, e)))
}
