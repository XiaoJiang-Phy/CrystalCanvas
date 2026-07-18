use tauri::{Emitter, State};

use super::VolumetricInfo;
use crate::ipc::{
    IpcEnumInput, IpcError, IpcResult, IsosurfaceSignMode, VolumeColormap, VolumeRenderMode,
};

fn smoothstep_f32(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + t * (b[0] - a[0]),
        a[1] + t * (b[1] - a[1]),
        a[2] + t * (b[2] - a[2]),
    ]
}

fn colormap_3pt(c0: [f32; 3], c1: [f32; 3], c2: [f32; 3], t: f32) -> [f32; 3] {
    let s1 = smoothstep_f32(0.0, 0.5, t);
    let m = lerp3(c0, c1, s1);
    let s2 = smoothstep_f32(0.5, 1.0, t);
    lerp3(m, c2, s2)
}

fn colormap_sample(mode: u32, t: f32) -> [f32; 3] {
    match mode {
        1 => {
            let v = 0.2 + 0.8 * t;
            [v, v, v]
        }
        2 => colormap_3pt(
            [0.0002, 0.0016, 0.0139],
            [0.8651, 0.3165, 0.2261],
            [0.9882, 0.9984, 0.6449],
            t,
        ),
        3 => colormap_3pt(
            [0.0504, 0.0298, 0.5280],
            [0.7981, 0.2239, 0.4471],
            [0.9400, 0.9752, 0.1313],
            t,
        ),
        4 => {
            let blue = [0.2298_f32, 0.2987, 0.7537];
            let white = [0.9647_f32, 0.9647, 0.9647];
            let red = [0.7059_f32, 0.0156, 0.1502];
            if t < 0.5 {
                lerp3(blue, white, smoothstep_f32(0.0, 0.5, t))
            } else {
                lerp3(white, red, smoothstep_f32(0.5, 1.0, t))
            }
        }
        5 => {
            let r = (t * 2.5).clamp(0.0, 1.0);
            let g = ((t - 0.4) * 2.5).clamp(0.0, 1.0);
            let b = ((t - 0.7) * 3.33).clamp(0.0, 1.0);
            [r, g, b]
        }
        6 => colormap_3pt(
            [0.0015, 0.0005, 0.0139],
            [0.7107, 0.0221, 0.3264],
            [0.9873, 0.9913, 0.7494],
            t,
        ),
        7 => colormap_3pt(
            [0.0, 0.1262, 0.3015],
            [0.5529, 0.5529, 0.5059],
            [0.9955, 0.9110, 0.1459],
            t,
        ),
        8 => {
            let c0 = [0.1900_f32, 0.0718, 0.2322];
            let c1 = [0.1602_f32, 0.7346, 0.9398];
            let c2 = [0.9445_f32, 0.8530, 0.1094];
            let c3 = [0.4796_f32, 0.0158, 0.0106];
            if t < 0.33 {
                lerp3(c0, c1, smoothstep_f32(0.0, 0.33, t))
            } else if t < 0.66 {
                lerp3(c1, c2, smoothstep_f32(0.33, 0.66, t))
            } else {
                lerp3(c2, c3, smoothstep_f32(0.66, 1.0, t))
        }
        }
        9 => {
            let red = [0.6471_f32, 0.0, 0.1490];
            let yellow = [1.0_f32, 1.0, 0.749];
            let blue = [0.1922_f32, 0.2118, 0.5843];
            if t < 0.5 {
                lerp3(red, yellow, smoothstep_f32(0.0, 0.5, t))
            } else {
                lerp3(yellow, blue, smoothstep_f32(0.5, 1.0, t))
        }
        }
        _ => colormap_3pt(
            [0.2777273, 0.00540734, 0.33409981],
            [0.10509304, 0.59800696, 0.55836266],
            [0.99320573, 0.90615594, 0.143936],
            t,
        ),
    }
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
    
    let vol_data = new_state
        .volumetric_data
        .take()
        .ok_or_else(|| IpcError::parse("no volumetric data found in file"))?;
    
    let info = VolumetricInfo {
        grid_dims: vol_data.grid_dims,
        data_min: vol_data.data_min,
        data_max: vol_data.data_max,
        format: extension,
    };
    
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

    let prepared_volumetric = r
        .prepare_volumetric(&vol_data)
        .map_err(|_| IpcError::render("GPU out of memory while preparing volumetric grid"))?;

    r.clear_structure_bound_overlays();
    r.commit_atoms(atom_scene);
    r.update_lines(&line_scene);

    let center_vec = glam::Vec3::from_array(center);
    r.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
    r.camera.target = center_vec;
    if !r.camera.is_perspective {
        r.camera.set_orthographic(extent * 1.5);
    }
    r.update_camera();
    r.commit_volumetric(prepared_volumetric);

    let has_negative = vol_data.data_min < -0.01 * vol_data.data_max.abs();
    r.active_colormap_mode = if has_negative { 4 } else { 0 };
    if has_negative {
        if let Some(vol) = &r.volume_raycast_pipeline {
            vol.set_signed_mapping(&r.gpu.queue, true);
            vol.set_colormap(&r.gpu.queue, 4);
        }
        log::info!(
            "Signed volumetric data detected (min={:.3e}). Enabled signed mapping + Coolwarm.",
            vol_data.data_min
        );
    }

    new_state.volumetric_data = Some(vol_data);
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
    
    Ok(info)
}

#[tauri::command]
pub fn set_isovalue(
    value: f32,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let (grid_dims, data_range) = {
        let cs = crystal_state
            .lock()
            .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
        cs.volumetric_data
            .as_ref()
            .map(|v| {
            let abs_max = v.data_min.abs().max(v.data_max.abs());
            (v.grid_dims, abs_max)
            })
            .unzip()
    };

    if let Some(dims) = grid_dims {
        let mut r = renderer_state
            .lock()
            .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
        r.update_isovalue(dims, value);

        // Auto-sync isosurface color with volume colormap.
        // Must match volume_raycast.wgsl sqrt-stretched signed mapping:
        // $t = 0.5 \pm 0.5\sqrt{|v/v_{\max}|}$
        if let Some(abs_max) = data_range {
            let norm = (value.abs() / abs_max.max(1e-10)).clamp(0.0, 1.0);
            let stretched = norm.sqrt();
            let t_pos = 0.5 + 0.5 * stretched;     // positive lobe → upper half
            let t_neg = 0.5 - 0.5 * stretched;     // negative lobe → lower half
            let color_pos = colormap_sample(r.active_colormap_mode, t_pos);
            let color_neg = colormap_sample(r.active_colormap_mode, t_neg);
            let r_mut = &mut *r;
            if let Some(iso) = &mut r_mut.isosurface_pipeline {
                let alpha = iso.cur_color[3];
                iso.set_color(
                    &r_mut.gpu.queue,
                    [color_pos[0], color_pos[1], color_pos[2], alpha],
                );
                iso.set_color_negative(
                    &r_mut.gpu.queue,
                    [color_neg[0], color_neg[1], color_neg[2], alpha],
                );
            }
        }

        // Sync volume clip threshold + density cutoff (Both mode)
        let is_both = matches!(
            r.volume_render_mode,
            crate::renderer::renderer::RendererVolumeMode::Both
        );
        if let Some(vol) = &r.volume_raycast_pipeline {
            if is_both {
                vol.set_clip_threshold(&r.gpu.queue, value.abs());
                vol.set_density_cutoff(&r.gpu.queue, value.abs());
            } else {
                vol.set_clip_threshold(&r.gpu.queue, 0.0);
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn set_isosurface_color(
    color: [f32; 4],
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let mut r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    let r_mut = &mut *r;
    if let Some(iso) = &mut r_mut.isosurface_pipeline {
        iso.set_color(&r_mut.gpu.queue, color);
    }
    Ok(())
}

#[tauri::command]
pub fn set_isosurface_opacity(
    opacity: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let mut r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    r.set_isosurface_opacity(opacity);
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

    let grid_dims = {
        let cs = crystal_state
            .lock()
            .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
        cs.volumetric_data.as_ref().map(|v| v.grid_dims)
    };

    let r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    if let Some(iso) = &r.isosurface_pipeline {
        iso.set_sign_mode(&r.gpu.queue, sign_mode);
    }

    if let Some(_dims) = grid_dims {
        let dispatch = r.isosurface_dispatch_size;
        if let Some(iso) = &r.isosurface_pipeline {
            let mut encoder =
                r.gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Sign Mode Re-dispatch"),
            });
            iso.dispatch_compute(&mut encoder, dispatch);
            r.gpu.queue.submit(std::iter::once(encoder.finish()));
        }
    }

    // Sync volume signed mapping with sign_mode
    let use_signed = sign_mode == 2;
    if let Some(vol) = &r.volume_raycast_pipeline {
        vol.set_signed_mapping(&r.gpu.queue, use_signed);
    }

    Ok(())
}

#[tauri::command]
pub fn set_volume_render_mode(
    mode: IpcEnumInput<VolumeRenderMode>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let mode = mode.parse("mode")?;
    let mut r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    let new_mode = match mode {
        VolumeRenderMode::Isosurface => crate::renderer::renderer::RendererVolumeMode::Isosurface,
        VolumeRenderMode::Volume => crate::renderer::renderer::RendererVolumeMode::Volume,
        VolumeRenderMode::Both => crate::renderer::renderer::RendererVolumeMode::Both,
    };
    r.volume_render_mode = new_mode;

    // Sync volume clip threshold + density cutoff with current isovalue
    let iso_threshold = r
        .isosurface_pipeline
        .as_ref()
        .map_or(0.0, |iso| iso.cur_threshold.abs());
    let (clip, cutoff) = match new_mode {
        crate::renderer::renderer::RendererVolumeMode::Both => (iso_threshold, iso_threshold),
        _ => (0.0, 0.0),
    };
    if let Some(vol) = &r.volume_raycast_pipeline {
        vol.set_clip_threshold(&r.gpu.queue, clip);
        vol.set_density_cutoff(&r.gpu.queue, cutoff);
    }
    Ok(())
}

#[tauri::command]
pub fn set_volume_opacity_range(
    min: f32,
    max: f32,
    opacity_scale: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let mut r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    let r_mut = &mut *r;
    if let Some(vol) = &mut r_mut.volume_raycast_pipeline {
        // clamp scale
        vol.update_transfer_function(
            &r_mut.gpu.queue,
            [min, max],
            opacity_scale.max(0.01).min(10.0),
        );
    }
    Ok(())
}

#[tauri::command]
pub fn set_volume_density_cutoff(
    cutoff: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    if let Some(vol) = &r.volume_raycast_pipeline {
        vol.set_density_cutoff(&r.gpu.queue, cutoff.max(0.0));
    }
    Ok(())
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

    let iso_sync = {
        let cs = crystal_state
            .lock()
            .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
        cs.volumetric_data.as_ref().map(|v| {
            let abs_max = v.data_min.abs().max(v.data_max.abs());
            abs_max
        })
    };

    let mut r = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    r.active_colormap_mode = colormap_mode;
    if let Some(vol) = &r.volume_raycast_pipeline {
        vol.set_colormap(&r.gpu.queue, colormap_mode);
    }

    // Re-sync isosurface color with the new colormap
    // Matches volume_raycast.wgsl sqrt-stretched signed mapping
    if let Some(abs_max) = iso_sync {
        let r_mut = &mut *r;
        if let Some(iso) = &mut r_mut.isosurface_pipeline {
            let cur_threshold = iso.cur_threshold;
            let norm = (cur_threshold.abs() / abs_max.max(1e-10)).clamp(0.0, 1.0);
            let stretched = norm.sqrt();
            let t_pos = 0.5 + 0.5 * stretched;
            let t_neg = 0.5 - 0.5 * stretched;
            let color_pos = colormap_sample(colormap_mode, t_pos);
            let color_neg = colormap_sample(colormap_mode, t_neg);
            let alpha = iso.cur_color[3];
            iso.set_color(
                &r_mut.gpu.queue,
                [color_pos[0], color_pos[1], color_pos[2], alpha],
            );
            iso.set_color_negative(
                &r_mut.gpu.queue,
                [color_neg[0], color_neg[1], color_neg[2], alpha],
            );
        }
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
    if let Some(vol) = &cs.volumetric_data {
        let fmt_str = match vol.source_format {
            crate::volumetric::VolumetricFormat::VaspChgcar => "chgcar",
            crate::volumetric::VolumetricFormat::VaspLocpot => "locpot",
            crate::volumetric::VolumetricFormat::GaussianCube => "cube",
            crate::volumetric::VolumetricFormat::Xsf => "xsf",
        };
        Ok(Some(VolumetricInfo {
            grid_dims: vol.grid_dims,
            data_min: vol.data_min,
            data_max: vol.data_max,
            format: fmt_str.to_string(),
        }))
    } else {
        Ok(None)
    }
}
