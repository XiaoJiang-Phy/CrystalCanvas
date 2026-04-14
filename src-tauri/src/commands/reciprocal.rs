use tauri::State;

#[derive(serde::Serialize)]
pub struct BzInfoResponse {
    pub bravais_type: String,
    pub spacegroup: i32,
    pub vertices_count: usize,
    pub edges_count: usize,
    pub faces_count: usize,
    pub is_2d: bool,
}

#[derive(serde::Serialize)]
pub struct KPathPointUi {
    pub label: String,
    pub coord_frac: [f64; 3],
}

#[derive(serde::Serialize)]
pub struct KPathInfoResponse {
    pub points: Vec<KPathPointUi>,
    pub segments: Vec<Vec<String>>,
}

#[derive(serde::Serialize)]
pub struct KPathTextResponse {
    pub qe: String,
    pub vasp: String,
}

#[derive(serde::Serialize)]
pub struct BzLabelPos {
    pub label: String,
    pub x: f32,
    pub y: f32,
}

#[tauri::command]
pub fn compute_brillouin_zone(
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<BzInfoResponse, String> {
    let mut cs = crystal_state.lock().map_err(|e| e.to_string())?;
    cs.detect_2d(); // Update 2D heuristics based on current geometry
    
    let sg = cs.spacegroup_number;
    
    let lattice_col = cs.get_lattice_col_major();
    // BrillouinZone::new expects row vectors [ [x,y,z], [x,y,z], [x,y,z] ]
    let lat = [
        [lattice_col[0], lattice_col[1], lattice_col[2]],
        [lattice_col[3], lattice_col[4], lattice_col[5]],
        [lattice_col[6], lattice_col[7], lattice_col[8]],
    ];
    
    let (bz, kpath, bravais_type_str) = if cs.is_2d {
        let (a_proj, b_proj) = cs.get_inplane_lattice();
        let vac_axis = cs.vacuum_axis.unwrap_or(2);
        
        // Calculate in-plane lattice parameters for 2D classification
        let a_len = (a_proj[0]*a_proj[0] + a_proj[1]*a_proj[1]).sqrt();
        let b_len = (b_proj[0]*b_proj[0] + b_proj[1]*b_proj[1]).sqrt();
        let dot = a_proj[0]*b_proj[0] + a_proj[1]*b_proj[1];
        let gamma_deg = (dot / (a_len * b_len)).acos().to_degrees();

        let bz_type_2d = crate::kpath_2d::identify_bravais_2d(a_len, b_len, gamma_deg, sg);
        
        // Build 2D Wigner-Seitz cell
        let mut bz = crate::brillouin_zone::BrillouinZone::new_2d(a_proj, b_proj, vac_axis);
        bz.bravais_type = crate::brillouin_zone::BravaisType::Unknown; // Enums mismatch, frontend displays string
        
        let kpath = crate::kpath_2d::get_kpath_2d(bz_type_2d, a_len, b_len, vac_axis);
        (bz, kpath, format!("2D {:?}", bz_type_2d))
    } else {
        let bz_type = crate::kpath::identify_bravais_type(sg);
        let bz = crate::brillouin_zone::BrillouinZone::new(lat, bz_type);
        let kpath = crate::kpath::get_kpath(bz_type, &lat);
        (bz, kpath, format!("{:?}", bz_type))
    };
    
    let response = BzInfoResponse {
        bravais_type: bravais_type_str,
        spacegroup: sg,
        vertices_count: bz.vertices.len(),
        edges_count: bz.edges.len(),
        faces_count: bz.faces.len(),
        is_2d: bz.is_2d,
    };
    
    if let Ok(mut renderer) = renderer_state.lock() {
        renderer.update_bz_data(Some((&bz, &kpath)));
    }
    
    cs.bz_cache = Some((bz, kpath));
    
    Ok(response)
}

#[tauri::command]
pub fn toggle_bz_display(
    show: bool,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> Result<(), String> {
    let cs = crystal_state.lock().map_err(|e| e.to_string())?;
    let mut renderer = renderer_state.lock().map_err(|e| e.to_string())?;
    
    if show {
        if let Some((bz, kpath)) = &cs.bz_cache {
            renderer.update_bz_data(Some((bz, kpath)));
        }
    } else {
        renderer.show_bz = false;
    }
    Ok(())
}

#[tauri::command]
pub fn get_kpath_info(
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> Result<KPathInfoResponse, String> {
    let cs = crystal_state.lock().map_err(|e| e.to_string())?;
    if let Some((_, kpath)) = &cs.bz_cache {
        let points = kpath.points.iter().map(|p| KPathPointUi {
            label: p.label.clone(),
            coord_frac: p.coord_frac,
        }).collect();
        
        Ok(KPathInfoResponse {
            points,
            segments: kpath.path_segments.clone(),
        })
    } else {
        Err("Brillouin Zone not computed yet".into())
    }
}

#[tauri::command]
pub fn set_bz_scale(
    scale: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> Result<(), String> {
    let mut renderer = renderer_state.lock().map_err(|e| e.to_string())?;
    renderer.bz_scale = scale.clamp(0.15, 1.0);
    Ok(())
}

#[tauri::command]
pub fn generate_kpath_text(
    npoints: u32,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> Result<KPathTextResponse, String> {
    let cs = crystal_state.lock().map_err(|e| e.to_string())?;
    let (bz, kpath) = cs.bz_cache.as_ref().ok_or("No BZ data computed")?;

    let pt_map: std::collections::HashMap<&str, &[f64; 3]> = kpath
        .points.iter()
        .map(|p| (p.label.as_str(), &p.coord_frac))
        .collect();

    let is_2d = bz.is_2d;

    let rl = &bz.recip_lattice; // $\mathbf{b}_i$ rows

    // $\mathbf{k}_{\text{cart}} = k_1 \mathbf{b}_1 + k_2 \mathbf{b}_2 + k_3 \mathbf{b}_3$
    let frac_to_cart = |f: &[f64; 3]| -> [f64; 3] {
        [
            f[0]*rl[0][0] + f[1]*rl[1][0] + f[2]*rl[2][0],
            f[0]*rl[0][1] + f[1]*rl[1][1] + f[2]*rl[2][1],
            f[0]*rl[0][2] + f[1]*rl[1][2] + f[2]*rl[2][2],
        ]
    };

    let cart_dist = |a: &[f64; 3], b: &[f64; 3]| -> f64 {
        let ca = frac_to_cart(a);
        let cb = frac_to_cart(b);
        ((ca[0]-cb[0]).powi(2) + (ca[1]-cb[1]).powi(2) + (ca[2]-cb[2]).powi(2)).sqrt()
    };

    // Collect all segment pairs with their Cartesian lengths
    struct SegPair { c0: [f64; 3], c1: [f64; 3], l0: String, l1: String, len: f64, is_seg_end: bool }
    let mut pairs: Vec<SegPair> = Vec::new();
    let mut total_len = 0.0_f64;

    for seg in &kpath.path_segments {
        for (pi, pair) in seg.windows(2).enumerate() {
            let c0 = *pt_map.get(pair[0].as_str()).ok_or(format!("Missing: {}", pair[0]))?;
            let c1 = *pt_map.get(pair[1].as_str()).ok_or(format!("Missing: {}", pair[1]))?;
            let d = cart_dist(c0, c1);
            pairs.push(SegPair {
                c0: *c0, c1: *c1,
                l0: pair[0].clone(), l1: pair[1].clone(),
                len: d,
                is_seg_end: pi == seg.len() - 2,
            });
            total_len += d;
        }
    }

    if total_len < 1e-12 { return Err("Degenerate k-path (zero length)".into()); }

    let npts_per_seg = npoints.max(5) as f64;
    let avg_len = total_len / pairs.len() as f64;

    let calc_seg_n = |seg_len: f64| -> usize {
        (npts_per_seg * seg_len / avg_len).round().max(2.0) as usize
    };

    // QE: K_POINTS {crystal}
    let mut qe_kpts: Vec<String> = Vec::new();
    for sp in &pairs {
        let seg_n = calc_seg_n(sp.len);
        let end = if sp.is_seg_end { seg_n } else { seg_n - 1 };
        for i in 0..end {
            let t = if seg_n > 1 { i as f64 / (seg_n - 1) as f64 } else { 0.0 };
            let mut k = [
                sp.c0[0] + t * (sp.c1[0] - sp.c0[0]),
                sp.c0[1] + t * (sp.c1[1] - sp.c0[1]),
                sp.c0[2] + t * (sp.c1[2] - sp.c0[2]),
            ];
            if is_2d {
                k[2] = 0.0;
            }
            qe_kpts.push(format!("  {:.10}  {:.10}  {:.10}  1.0", k[0], k[1], k[2]));
        }
    }
    let qe_text = format!("K_POINTS {{crystal}}\n{}\n{}\n", qe_kpts.len(), qe_kpts.join("\n"));

    // VASP: explicit KPOINTS with uniform spacing
    let mut vasp_kpts: Vec<String> = Vec::new();
    vasp_kpts.push("k-points for band structure (uniform spacing)".into());
    vasp_kpts.push("PLACEHOLDER_COUNT".into());
    vasp_kpts.push("Reciprocal lattice".into());
    let mut kcount = 0_usize;
    for sp in &pairs {
        let seg_n = calc_seg_n(sp.len);
        let end = if sp.is_seg_end { seg_n } else { seg_n - 1 };
        for i in 0..end {
            let t = if seg_n > 1 { i as f64 / (seg_n - 1) as f64 } else { 0.0 };
            let mut k = [
                sp.c0[0] + t * (sp.c1[0] - sp.c0[0]),
                sp.c0[1] + t * (sp.c1[1] - sp.c0[1]),
                sp.c0[2] + t * (sp.c1[2] - sp.c0[2]),
            ];
            if is_2d {
                k[2] = 0.0;
            }
            let comment = if i == 0 { format!("  ! {}", sp.l0) }
                          else if i == end - 1 && sp.is_seg_end { format!("  ! {}", sp.l1) }
                          else { String::new() };
            vasp_kpts.push(format!("  {:.10}  {:.10}  {:.10}  1.0{}", k[0], k[1], k[2], comment));
            kcount += 1;
        }
    }
    vasp_kpts[1] = format!("{}", kcount);
    let vasp_text = vasp_kpts.join("\n");

    Ok(KPathTextResponse { qe: qe_text, vasp: vasp_text })
}

#[tauri::command]
pub fn get_bz_label_positions(
    width: f32,
    height: f32,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> Result<Vec<BzLabelPos>, String> {
    let cs = crystal_state.lock().map_err(|e| e.to_string())?;
    let (bz, kpath) = cs.bz_cache.as_ref().ok_or("No BZ data")?;

    // Reconstruct camera state identical to BzSubViewport::update_bz
    let mut max_r = 0.0_f64;
    for v in &bz.vertices {
        let r = (v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt();
        if r > max_r { max_r = r; }
    }
    let mut max_b = 0.0_f64;
    for row in &bz.recip_lattice {
        let m = (row[0]*row[0] + row[1]*row[1] + row[2]*row[2]).sqrt();
        if m > max_b { max_b = m; }
    }
    if max_r < 1e-6 { max_r = max_b; }
    let fit_scale = (max_r * 2.8) as f32;
    let ortho_scale = fit_scale.max(1.0);
    let dist = fit_scale * 2.0;

    let mut eye = glam::Vec3::new(dist * 0.4, dist * 0.3, dist);
    let target = glam::Vec3::ZERO;
    let mut up = glam::Vec3::Y;
    
    if bz.is_2d {
        let mut vac_axis = 2;
        if bz.recip_lattice[2][0].abs() > 0.5 { vac_axis = 0; }
        else if bz.recip_lattice[2][1].abs() > 0.5 { vac_axis = 1; }
        
        let mut eye_arr = [0.0; 3];
        eye_arr[vac_axis] = -dist;
        eye = glam::Vec3::from_array(eye_arr);
        
        let up_axis = (vac_axis + 2) % 3;
        let mut up_arr = [0.0; 3];
        up_arr[up_axis] = 1.0;
        up = glam::Vec3::from_array(up_arr);
    }

    let view = glam::Mat4::look_at_rh(eye, target, up);
    let aspect = width / height;
    let hw = ortho_scale * aspect / 2.0;
    let hh = ortho_scale / 2.0;
    let proj = glam::Mat4::orthographic_rh(-hw, hw, -hh, hh, 0.1, 200.0);
    let vp = proj * view;

    let mut labels = Vec::with_capacity(kpath.points.len());
    for kp in &kpath.points {
        let mut c = [0.0_f64; 3];
        for j in 0..3 {
            c[j] = kp.coord_frac[0] * bz.recip_lattice[0][j]
                 + kp.coord_frac[1] * bz.recip_lattice[1][j]
                 + kp.coord_frac[2] * bz.recip_lattice[2][j];
        }

        let pos = glam::Vec4::new(c[0] as f32, c[1] as f32, c[2] as f32, 1.0);
        let clip = vp * pos;
        if clip.w.abs() < 1e-6 { continue; }

        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;

        labels.push(BzLabelPos {
            label: kp.label.clone(),
            x: (ndc_x + 1.0) / 2.0 * width,
            y: (1.0 - ndc_y) / 2.0 * height,
        });
    }

    Ok(labels)
}
