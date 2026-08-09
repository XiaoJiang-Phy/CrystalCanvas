use tauri::{Emitter, State};

use super::VolumetricInfo;
use crate::ipc::{
    IpcEnumInput, IpcError, IpcResult, IsosurfaceSignMode, VolumeColormap, VolumeRenderMode,
};

fn initial_isovalue(data_min: f32, data_max: f32) -> Option<f32> {
    let bound = data_min.abs().max(data_max.abs());
    if !data_min.is_finite() || !data_max.is_finite() || bound <= 0.0 {
        return None;
    }
    if data_min < 0.0 {
        return Some(bound * 0.1);
    }
    let candidate = data_max * 0.1;
    Some(if candidate < data_min {
        data_min + (data_max - data_min) * 0.1
    } else {
        candidate
    })
}

fn commit_active_field_update(
    state: &mut crate::crystal_state::CrystalState,
    renderer: &mut crate::renderer::renderer::Renderer,
    update: impl FnOnce(&mut crate::volumetric::FieldLayer) -> Result<(), IpcError>,
) -> IpcResult<()> {
    let mut candidate = state.field_scene.clone();
    let layer = candidate
        .active_layer_mut()
        .ok_or_else(|| IpcError::invalid_argument("no active field layer"))?;
    update(layer)?;
    layer
        .presentation_settings
        .validate()
        .map_err(IpcError::invalid_argument)?;
    let prepared = renderer
        .prepare_field_layer(layer)
        .map_err(|_| IpcError::render("field presentation exceeds the available GPU budget"))?;
    renderer
        .commit_field_layer(prepared, layer.id, layer.revision)
        .map_err(|_| IpcError::render("stale field presentation preparation"))?;
    state.field_scene = candidate;
    Ok(())
}

#[tauri::command]
pub fn load_volumetric_file(
    path: String,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> IpcResult<VolumetricInfo> {
    log::info!("load_volumetric_file: {}", path);

    let extension = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let filename = std::path::Path::new(&path)
        .file_name()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mut new_state = match extension.as_str() {
        "chgcar" | "locpot" => {
            crate::io::chgcar_parser::parse_chgcar(&path).map_err(IpcError::parse)?
        }
        "cube" => crate::io::cube_parser::parse_cube(&path).map_err(IpcError::parse)?,
        "xsf" => crate::io::xsf_volumetric_parser::parse_xsf_volumetric(&path)
            .map_err(IpcError::parse)?,
        _ => {
            if filename.starts_with("chgcar")
                || filename.starts_with("locpot")
                || filename.starts_with("aeccar")
            {
                crate::io::chgcar_parser::parse_chgcar(&path).map_err(IpcError::parse)?
            } else {
                return Err(IpcError::invalid_argument(format!(
                    "unsupported volumetric format: ext='{}', file='{}'",
                    extension, filename
                )));
            }
        }
    };
    new_state
        .validate_structural_invariants()
        .map_err(IpcError::parse)?;
    let source_sha256 =
        crate::volumetric::source_artifact_sha256(&path).map_err(IpcError::parse)?;

    let admitted = new_state
        .admit_volumetric_import(filename.clone(), source_sha256)
        .map_err(IpcError::invalid_argument)?;
    let info = VolumetricInfo {
        grid_dims: admitted.grid_dims,
        data_min: admitted.data_min,
        data_max: admitted.data_max,
        format: extension,
    };
    if initial_isovalue(info.data_min, info.data_max).is_none() {
        return Err(IpcError::invalid_argument(
            "volumetric scalar range is not usable",
        ));
    }

    let mut cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let settings = settings_state
        .lock()
        .map_err(|_| IpcError::lock("settings lock poisoned"))?;
    let atom_scene = crate::renderer::instance::prepare_atom_scene(
        crate::wannier::build_atoms_with_ghosts(&new_state, &settings)?,
    )?;
    let line_scene = crate::renderer::instance::build_line_scene(&new_state, &settings)?;
    let extent = new_state.cell_a.max(new_state.cell_b).max(new_state.cell_c) as f32;
    let center = new_state.unit_cell_center();
    let mut r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    let pending_version = crate::transaction::next_version(&cs)?;

    let active_layer = new_state
        .field_scene
        .active_layer()
        .ok_or_else(|| IpcError::render("new field layer is missing"))?;
    let prepared_volumetric = r
        .prepare_field_layer(active_layer)
        .map_err(|_| IpcError::render("GPU out of memory while preparing volumetric grid"))?;

    r.clear_non_field_structure_bound_overlays();
    r.commit_atoms(atom_scene);
    r.update_lines(&line_scene);

    let center_vec = glam::Vec3::from_array(center);
    r.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
    r.camera.target = center_vec;
    if !r.camera.is_perspective {
        r.camera.set_orthographic(extent * 1.5);
    }
    r.update_camera();
    r.commit_replacement_field_layer(
        prepared_volumetric,
        admitted.layer_id,
        admitted.layer_revision,
    )
    .map_err(|_| IpcError::render("stale field layer preparation"))?;

    new_state.volumetric_data = None;
    let field_payload = FieldSceneChangedPayload::from_scene(&new_state.field_scene);
    let version = crate::transaction::stamp_version(&mut new_state, pending_version);
    *cs = new_state;

    drop(r);
    drop(settings);
    drop(cs);

    app.emit(
        "state_changed",
        crate::transaction::StateChangedPayload { version },
    )
    .ok();

    let _ = app.emit("volumetric_loaded", &info);
    let _ = app.emit("field_scene_changed", field_payload);

    Ok(info)
}

#[tauri::command]
pub fn set_isovalue(
    value: f32,
    layer_id: u64,
    expected_revision: u64,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let mut cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    require_field_revision(&cs.field_scene, expected_revision)?;
    let layer = cs
        .field_scene
        .active_layer()
        .filter(|layer| layer.id == layer_id)
        .ok_or_else(|| IpcError::invalid_argument("active field layer is stale"))?;
    let mut render_settings = layer.render_settings;
    render_settings.isovalue = value;
    render_settings.positive_isovalue = value;
    render_settings.negative_isovalue = value;
    let mut r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    r.update_field_render_settings(layer, render_settings)
        .map_err(|_| IpcError::render("isosurface exceeds the available GPU vertex budget"))?;

    // Sync volume clip threshold + density cutoff (Both mode)
    let is_both = matches!(
        r.volume_render_mode,
        crate::renderer::renderer::RendererVolumeMode::Both
    );
    r.with_active_volume_pipeline(|volume, queue| {
        if is_both {
            volume.set_clip_threshold(queue, value.abs());
            volume.set_density_cutoff(queue, value.abs());
        } else {
            volume.set_clip_threshold(queue, 0.0);
        }
    });
    if let Some(layer) = cs.field_scene.active_layer_mut() {
        layer.render_settings.isovalue = value;
        layer.render_settings.positive_isovalue = value;
        layer.render_settings.negative_isovalue = value;
    }
    Ok(())
}

/// Set independently selected positive and negative isosurface thresholds for
/// one revision-checked active layer.  The legacy `set_isovalue` command keeps
/// its linked-threshold behavior for existing callers.
#[tauri::command]
pub fn set_signed_isovalues(
    positive_value: f32,
    negative_value: f32,
    layer_id: u64,
    expected_revision: u64,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    if !positive_value.is_finite()
        || !negative_value.is_finite()
        || positive_value <= 0.0
        || negative_value <= 0.0
    {
        return Err(IpcError::invalid_argument(
            "signed isovalues must be finite and positive",
        ));
    }
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    require_field_revision(&state.field_scene, expected_revision)?;
    let layer = state
        .field_scene
        .active_layer()
        .filter(|layer| layer.id == layer_id)
        .ok_or_else(|| IpcError::invalid_argument("active field layer is stale"))?;
    if positive_value > layer.data_max || negative_value > -layer.data_min {
        return Err(IpcError::invalid_argument(
            "signed isovalue exceeds its source scalar range",
        ));
    }
    let mut settings = layer.render_settings;
    settings.positive_isovalue = positive_value;
    settings.negative_isovalue = negative_value;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    renderer
        .update_field_render_settings(layer, settings)
        .map_err(|_| IpcError::render("isosurface exceeds the available GPU vertex budget"))?;
    if let Some(layer) = state.field_scene.active_layer_mut() {
        layer.render_settings.positive_isovalue = positive_value;
        layer.render_settings.negative_isovalue = negative_value;
    }
    Ok(())
}

#[tauri::command]
pub fn set_isosurface_color(
    color: [f32; 4],
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let mut r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    let r_mut = &mut *r;
    if let Some(iso) = &mut r_mut.active_field_layer_pipeline {
        iso.set_color(&r_mut.gpu.queue, color);
    }
    if let Some(layer) = state.field_scene.active_layer_mut() {
        layer.render_settings.color = color;
    }
    Ok(())
}

#[tauri::command]
pub fn set_isosurface_colors(
    positive_color: [f32; 4],
    negative_color: [f32; 4],
    layer_id: u64,
    expected_revision: u64,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let valid_color = |color: [f32; 4]| {
        color
            .iter()
            .all(|component| component.is_finite() && (0.0..=1.0).contains(component))
    };
    if !valid_color(positive_color) || !valid_color(negative_color) {
        return Err(IpcError::invalid_argument(
            "isosurface colors must contain finite normalized components",
        ));
    }
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    require_field_revision(&state.field_scene, expected_revision)?;
    let layer = state
        .field_scene
        .active_layer_mut()
        .filter(|layer| layer.id == layer_id)
        .ok_or_else(|| IpcError::invalid_argument("active field layer is stale"))?;
    let opacity = layer.render_settings.opacity;
    let positive_color = [
        positive_color[0],
        positive_color[1],
        positive_color[2],
        opacity,
    ];
    let negative_color = [
        negative_color[0],
        negative_color[1],
        negative_color[2],
        opacity,
    ];
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    if !renderer.set_active_isosurface_colors(positive_color, negative_color) {
        return Err(IpcError::render("active field renderer is unavailable"));
    }
    layer.render_settings.color = positive_color;
    layer.render_settings.color_negative = negative_color;
    Ok(())
}

#[tauri::command]
pub fn set_isosurface_opacity(
    opacity: f32,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let opacity = opacity.clamp(0.0, 1.0);
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let mut r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    r.set_isosurface_opacity(opacity);
    if let Some(layer) = state.field_scene.active_layer_mut() {
        layer.render_settings.opacity = opacity;
        layer.render_settings.color[3] = opacity;
        layer.render_settings.color_negative[3] = opacity;
    }
    Ok(())
}

#[tauri::command]
pub fn set_isosurface_sign_mode(
    mode: IpcEnumInput<IsosurfaceSignMode>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<()> {
    let mode = mode.parse("mode")?;
    let sign_mode: u32 = match mode {
        IsosurfaceSignMode::Positive => 0,
        IsosurfaceSignMode::Negative => 1,
        IsosurfaceSignMode::Both => 2,
    };
    let field_sign_mode = match mode {
        IsosurfaceSignMode::Positive => crate::volumetric::FieldSignMode::Positive,
        IsosurfaceSignMode::Negative => crate::volumetric::FieldSignMode::Negative,
        IsosurfaceSignMode::Both => crate::volumetric::FieldSignMode::Both,
    };

    let mut cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let Some(layer) = cs.field_scene.active_layer() else {
        return Ok(());
    };
    let mut render_settings = layer.render_settings;
    render_settings.sign_mode = field_sign_mode;

    let mut r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    r.update_field_render_settings(layer, render_settings)
        .map_err(|_| IpcError::render("isosurface exceeds the available GPU vertex budget"))?;

    // Sync volume signed mapping with sign_mode
    let use_signed = sign_mode == 2;
    r.with_active_volume_pipeline(|volume, queue| volume.set_signed_mapping(queue, use_signed));
    if let Some(layer) = cs.field_scene.active_layer_mut() {
        layer.render_settings.sign_mode = field_sign_mode;
    }

    Ok(())
}

#[tauri::command]
pub fn set_volume_render_mode(
    mode: IpcEnumInput<VolumeRenderMode>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let mode = mode.parse("mode")?;
    let new_mode = match mode {
        VolumeRenderMode::Isosurface => crate::renderer::renderer::RendererVolumeMode::Isosurface,
        VolumeRenderMode::Volume => crate::renderer::renderer::RendererVolumeMode::Volume,
        VolumeRenderMode::Both => crate::renderer::renderer::RendererVolumeMode::Both,
    };
    let field_render_mode = match mode {
        VolumeRenderMode::Isosurface => crate::volumetric::FieldRenderMode::Isosurface,
        VolumeRenderMode::Volume => crate::volumetric::FieldRenderMode::Volume,
        VolumeRenderMode::Both => crate::volumetric::FieldRenderMode::Both,
    };
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let mut r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    if !r.set_active_field_render_mode(field_render_mode) {
        return Err(IpcError::render("active field renderer is unavailable"));
    }

    // Sync volume clip threshold + density cutoff with current isovalue
    let iso_threshold = r
        .active_field_layer_pipeline
        .as_ref()
        .map_or(0.0, |iso| iso.cur_threshold.abs());
    let (clip, cutoff) = match new_mode {
        crate::renderer::renderer::RendererVolumeMode::Both => (iso_threshold, iso_threshold),
        _ => (0.0, 0.0),
    };
    r.with_active_volume_pipeline(|volume, queue| {
        volume.set_clip_threshold(queue, clip);
        volume.set_density_cutoff(queue, cutoff);
    });
    if let Some(layer) = state.field_scene.active_layer_mut() {
        layer.render_settings.render_mode = field_render_mode;
    }
    Ok(())
}

#[tauri::command]
pub fn set_volume_opacity_range(
    min: f32,
    max: f32,
    opacity_scale: f32,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    if !min.is_finite() || !max.is_finite() || min >= max || !opacity_scale.is_finite() {
        return Err(IpcError::invalid_argument(
            "volume display range is invalid",
        ));
    }
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    commit_active_field_update(&mut state, &mut renderer, |layer| {
        layer.presentation_settings.display_range = Some([min, max]);
        layer.presentation_settings.opacity_scale = opacity_scale.clamp(0.0, 10.0);
        Ok(())
    })
}

#[tauri::command]
pub fn set_volume_density_cutoff(
    cutoff: f32,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    if !cutoff.is_finite() || cutoff < 0.0 {
        return Err(IpcError::invalid_argument(
            "volume density cutoff is invalid",
        ));
    }
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    commit_active_field_update(&mut state, &mut renderer, |layer| {
        layer.presentation_settings.density_cutoff = cutoff;
        Ok(())
    })
}

#[tauri::command]
pub fn set_volume_colormap(
    mode: IpcEnumInput<VolumeColormap>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<()> {
    let mode = mode.parse("mode")?;
    let colormap_mode: u32 = match mode {
        VolumeColormap::Grayscale => 1,
        VolumeColormap::Inferno => 2,
        VolumeColormap::Plasma => 3,
        VolumeColormap::Coolwarm => 4,
        VolumeColormap::Hot => 5,
        VolumeColormap::Magma => 6,
        VolumeColormap::Cividis => 7,
        VolumeColormap::Turbo => 8,
        VolumeColormap::Rdylbu => 9,
        VolumeColormap::Viridis => 0,
    };

    let mut cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let mut r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    r.active_colormap_mode = colormap_mode;
    r.with_active_volume_pipeline(|volume, queue| volume.set_colormap(queue, colormap_mode));

    if let Some(layer) = cs.field_scene.active_layer_mut() {
        layer.render_settings.colormap_mode = colormap_mode;
    }
    Ok(())
}

#[tauri::command]
pub fn get_volumetric_info(
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<Option<VolumetricInfo>> {
    let cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    if let Some(vol) = cs.field_scene.active_layer() {
        Ok(Some(VolumetricInfo {
            grid_dims: vol.grid_dims,
            data_min: vol.data_min,
            data_max: vol.data_max,
            format: "field".to_string(),
        }))
    } else {
        Ok(None)
    }
}

#[derive(Clone, serde::Serialize)]
pub struct FieldLayerInfo {
    pub id: u64,
    pub revision: u64,
    pub label: String,
    pub grid_dims: [usize; 3],
    pub data_min: f32,
    pub data_max: f32,
    pub source_sha256: String,
    pub normalized_sha256: String,
    pub lineage: Option<Vec<crate::volumetric::FieldLineageTerm>>,
    pub visible: bool,
    pub isovalue: f32,
    pub opacity: f32,
    pub color: [f32; 4],
    pub color_negative: [f32; 4],
    pub opacity_scale: f32,
    pub sign_mode: crate::volumetric::FieldSignMode,
    pub render_mode: crate::volumetric::FieldRenderMode,
    pub colormap_mode: u32,
}

#[derive(Clone, serde::Serialize)]
pub struct FieldSceneInfo {
    pub revision: u64,
    pub active_layer_id: Option<u64>,
    pub layers: Vec<FieldLayerInfo>,
}

#[derive(Clone, serde::Serialize)]
pub struct FieldSceneChangedPayload {
    pub revision: u64,
    pub active_layer_id: Option<u64>,
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldLinearCombinationTerm {
    pub layer_id: u64,
    pub coefficient: f64,
}

impl FieldSceneInfo {
    fn from_scene(scene: &crate::volumetric::FieldScene) -> Self {
        Self {
            revision: scene.revision,
            active_layer_id: scene.active_layer,
            layers: scene
                .layers
                .iter()
                .map(|layer| FieldLayerInfo {
                    id: layer.id,
                    revision: layer.revision,
                    label: layer.label.clone(),
                    grid_dims: layer.grid_dims,
                    data_min: layer.data_min,
                    data_max: layer.data_max,
                    source_sha256: layer.source_sha256.clone(),
                    normalized_sha256: layer.normalized_sha256.clone(),
                    lineage: layer.lineage.clone(),
                    visible: layer.render_settings.visible,
                    isovalue: layer.render_settings.isovalue,
                    opacity: layer.render_settings.opacity,
                    color: layer.render_settings.color,
                    color_negative: layer.render_settings.color_negative,
                    opacity_scale: layer.presentation_settings.opacity_scale,
                    sign_mode: layer.render_settings.sign_mode,
                    render_mode: layer.render_settings.render_mode,
                    colormap_mode: layer.render_settings.colormap_mode,
                })
                .collect(),
        }
    }
}

impl FieldSceneChangedPayload {
    pub(crate) fn from_scene(scene: &crate::volumetric::FieldScene) -> Self {
        Self {
            revision: scene.revision,
            active_layer_id: scene.active_layer,
        }
    }
}

fn require_field_revision(
    scene: &crate::volumetric::FieldScene,
    expected_revision: u64,
) -> IpcResult<()> {
    if scene.revision == expected_revision {
        Ok(())
    } else {
        Err(IpcError::invalid_argument("field scene revision is stale"))
    }
}

fn require_field_structure_mapping(
    volumetric: &crate::volumetric::VolumetricData,
    state: &crate::crystal_state::CrystalState,
) -> IpcResult<()> {
    const LATTICE_TOLERANCE_ANGSTROM: f64 = 1.0e-5;
    let structure_lattice = state.get_lattice_col_major();
    let matches =
        volumetric
            .lattice
            .iter()
            .zip(structure_lattice.iter())
            .all(|(field, structure)| {
                let scale = field.abs().max(structure.abs()).max(1.0);
                (field - structure).abs() <= LATTICE_TOLERANCE_ANGSTROM * scale
            });
    matches.then_some(()).ok_or_else(|| {
        IpcError::invalid_argument("field lattice does not map to the current structure")
    })
}

fn apply_explicit_scalar_unit(
    volumetric: &mut crate::volumetric::VolumetricData,
    scalar_unit: Option<&str>,
) -> IpcResult<()> {
    let Some(scalar_unit) = scalar_unit else {
        return Ok(());
    };
    if volumetric.scalar_metadata.metadata_declared {
        return Ok(());
    }
    let scalar_unit = match scalar_unit {
        "electron_per_cubic_angstrom" => crate::volumetric::ScalarUnit::ElectronPerCubicAngstrom,
        "electron_per_bohr_cubed" => crate::volumetric::ScalarUnit::ElectronPerBohrCubed,
        _ => {
            return Err(IpcError::invalid_argument(
                "unsupported explicit scalar unit",
            ));
        }
    };
    volumetric.scalar_metadata = crate::volumetric::FieldSourceMetadata {
        scalar_unit,
        scalar_unit_scale: 1.0,
        normalization: crate::volumetric::FieldNormalization::Raw,
        metadata_declared: true,
        source_origin_angstrom: volumetric.scalar_metadata.source_origin_angstrom,
    };
    Ok(())
}

#[tauri::command]
pub fn add_field_layer(
    path: String,
    scalar_unit: Option<String>,
    expected_revision: u64,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<FieldSceneInfo> {
    let source_sha256 =
        crate::volumetric::source_artifact_sha256(&path).map_err(IpcError::parse)?;
    let source_name = std::path::Path::new(&path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let extension = std::path::Path::new(&path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let parsed = match if extension.is_empty() {
        source_name.as_str()
    } else {
        extension.as_str()
    } {
        "cube" => crate::io::cube_parser::parse_cube(&path),
        "xsf" => crate::io::xsf_volumetric_parser::parse_xsf_volumetric(&path),
        "chgcar" | "locpot" => crate::io::chgcar_parser::parse_chgcar(&path),
        _ => return Err(IpcError::invalid_argument("unsupported field-layer format")),
    }
    .map_err(IpcError::parse)?;
    let label = std::path::Path::new(&path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("field")
        .to_owned();
    let mut volumetric = parsed
        .volumetric_data
        .ok_or_else(|| IpcError::parse("no volumetric data found in file"))?;
    apply_explicit_scalar_unit(&mut volumetric, scalar_unit.as_deref())?;
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    require_field_revision(&state.field_scene, expected_revision)?;
    require_field_structure_mapping(&volumetric, &state)?;
    let mut prepared_scene = state.field_scene.clone();
    let layer = prepared_scene
        .add_layer_from_source(label, volumetric, source_sha256)
        .map_err(IpcError::invalid_argument)?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    let prepared = renderer
        .prepare_field_layer(layer)
        .map_err(|_| IpcError::render("GPU out of memory while preparing field layer"))?;
    let layer_id = layer.id;
    let layer_revision = layer.revision;
    renderer
        .commit_field_layer(prepared, layer_id, layer_revision)
        .map_err(|_| IpcError::render("stale field layer preparation"))?;
    state.field_scene = prepared_scene;
    let info = FieldSceneInfo::from_scene(&state.field_scene);
    let payload = FieldSceneChangedPayload::from_scene(&state.field_scene);
    drop(renderer);
    drop(state);
    app.emit("field_scene_changed", payload).ok();
    Ok(info)
}

#[tauri::command]
pub fn remove_field_layer(
    layer_id: u64,
    expected_revision: u64,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<FieldSceneInfo> {
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    require_field_revision(&state.field_scene, expected_revision)?;
    let mut prepared_scene = state.field_scene.clone();
    prepared_scene
        .remove_layer(layer_id)
        .map_err(IpcError::invalid_argument)?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    if let Some(layer) = prepared_scene.active_layer() {
        let prepared = renderer
            .prepare_field_layer(layer)
            .map_err(|_| IpcError::render("GPU out of memory while preparing field layer"))?;
        renderer.remove_field_layer_resources(layer_id);
        renderer
            .commit_field_layer(prepared, layer.id, layer.revision)
            .map_err(|_| IpcError::render("stale field layer preparation"))?;
    } else {
        renderer.clear_volumetric();
    }
    state.field_scene = prepared_scene;
    let info = FieldSceneInfo::from_scene(&state.field_scene);
    let payload = FieldSceneChangedPayload::from_scene(&state.field_scene);
    drop(renderer);
    drop(state);
    app.emit("field_scene_changed", payload).ok();
    Ok(info)
}

#[tauri::command]
pub fn set_field_layer_visibility(
    layer_id: u64,
    visible: bool,
    expected_revision: u64,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<FieldSceneInfo> {
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    require_field_revision(&state.field_scene, expected_revision)?;
    if !state
        .field_scene
        .layers
        .iter()
        .any(|layer| layer.id == layer_id)
    {
        return Err(IpcError::invalid_argument("field layer does not exist"));
    }
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    if !renderer.set_field_layer_visibility(layer_id, visible) {
        return Err(IpcError::render("field layer renderer resource is stale"));
    }
    state
        .field_scene
        .set_layer_visibility(layer_id, visible)
        .map_err(IpcError::invalid_argument)?;
    let info = FieldSceneInfo::from_scene(&state.field_scene);
    let payload = FieldSceneChangedPayload::from_scene(&state.field_scene);
    drop(state);
    app.emit("field_scene_changed", payload).ok();
    Ok(info)
}

#[tauri::command]
pub fn set_field_layer_presentation(
    layer_id: u64,
    presentation_settings: crate::renderer::field_scene::FieldPresentationSettings,
    expected_revision: u64,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<FieldSceneInfo> {
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    require_field_revision(&state.field_scene, expected_revision)?;
    let mut prepared_scene = state.field_scene.clone();
    prepared_scene
        .set_layer_presentation(layer_id, presentation_settings)
        .map_err(IpcError::invalid_argument)?;
    let layer = prepared_scene
        .layers
        .iter()
        .find(|layer| layer.id == layer_id)
        .ok_or_else(|| IpcError::invalid_argument("field layer does not exist"))?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    let prepared = renderer
        .prepare_field_layer(layer)
        .map_err(|_| IpcError::render("field presentation exceeds the available GPU budget"))?;
    renderer
        .commit_field_layer(prepared, layer.id, layer.revision)
        .map_err(|_| IpcError::render("stale field presentation preparation"))?;
    state.field_scene = prepared_scene;
    let info = FieldSceneInfo::from_scene(&state.field_scene);
    let payload = FieldSceneChangedPayload::from_scene(&state.field_scene);
    drop(renderer);
    drop(state);
    app.emit("field_scene_changed", payload).ok();
    Ok(info)
}

#[tauri::command]
pub fn rename_field_layer(
    layer_id: u64,
    label: String,
    expected_revision: u64,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<FieldSceneInfo> {
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    require_field_revision(&state.field_scene, expected_revision)?;
    state
        .field_scene
        .rename_layer(layer_id, label)
        .map_err(IpcError::invalid_argument)?;
    let info = FieldSceneInfo::from_scene(&state.field_scene);
    let payload = FieldSceneChangedPayload::from_scene(&state.field_scene);
    drop(state);
    app.emit("field_scene_changed", payload).ok();
    Ok(info)
}

#[tauri::command]
pub fn reorder_field_layer(
    layer_id: u64,
    target_index: usize,
    expected_revision: u64,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<FieldSceneInfo> {
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    require_field_revision(&state.field_scene, expected_revision)?;
    state
        .field_scene
        .reorder_layer(layer_id, target_index)
        .map_err(IpcError::invalid_argument)?;
    let info = FieldSceneInfo::from_scene(&state.field_scene);
    let payload = FieldSceneChangedPayload::from_scene(&state.field_scene);
    drop(state);
    app.emit("field_scene_changed", payload).ok();
    Ok(info)
}

#[tauri::command]
pub fn select_active_field_layer(
    layer_id: u64,
    expected_revision: u64,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<FieldSceneInfo> {
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    require_field_revision(&state.field_scene, expected_revision)?;
    let mut prepared_scene = state.field_scene.clone();
    prepared_scene
        .select_active(layer_id)
        .map_err(IpcError::invalid_argument)?;
    let layer = prepared_scene
        .active_layer()
        .ok_or_else(|| IpcError::render("active field layer is missing"))?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    let prepared = renderer
        .prepare_field_layer(layer)
        .map_err(|_| IpcError::render("GPU out of memory while preparing field layer"))?;
    renderer
        .commit_field_layer(prepared, layer.id, layer.revision)
        .map_err(|_| IpcError::render("stale field layer preparation"))?;
    state.field_scene = prepared_scene;
    let info = FieldSceneInfo::from_scene(&state.field_scene);
    let payload = FieldSceneChangedPayload::from_scene(&state.field_scene);
    drop(renderer);
    drop(state);
    app.emit("field_scene_changed", payload).ok();
    Ok(info)
}

#[tauri::command]
pub fn combine_field_layers(
    terms: Vec<FieldLinearCombinationTerm>,
    output_label: String,
    expected_revision: u64,
    app: tauri::AppHandle,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<FieldSceneInfo> {
    let mut state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    require_field_revision(&state.field_scene, expected_revision)?;
    let mut prepared_scene = state.field_scene.clone();
    if terms.len() > crate::volumetric::MAX_LINEAR_COMBINATION_TERMS {
        return Err(IpcError::invalid_argument(
            "linear combination term count is invalid",
        ));
    }
    let mut linear_terms = Vec::new();
    linear_terms
        .try_reserve_exact(terms.len())
        .map_err(|_| IpcError::invalid_argument("unable to reserve linear-combination terms"))?;
    for term in terms {
        linear_terms.push((term.layer_id, term.coefficient));
    }
    let layer = prepared_scene
        .combine(&linear_terms, output_label)
        .map_err(IpcError::invalid_argument)?;
    let layer_id = layer.id;
    let layer_revision = layer.revision;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    let prepared = renderer
        .prepare_field_layer(layer)
        .map_err(|_| IpcError::render("GPU out of memory while prepare field layer"))?;
    renderer
        .commit_field_layer(prepared, layer_id, layer_revision)
        .map_err(|_| IpcError::render("stale field layer commit"))?;
    state.field_scene = prepared_scene;
    let info = FieldSceneInfo::from_scene(&state.field_scene);
    let payload = FieldSceneChangedPayload::from_scene(&state.field_scene);
    drop(renderer);
    drop(state);
    app.emit("field_scene_changed", payload).ok();
    Ok(info)
}

#[tauri::command]
pub fn get_field_scene_info(
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<FieldSceneInfo> {
    let state = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    Ok(FieldSceneInfo::from_scene(&state.field_scene))
}
