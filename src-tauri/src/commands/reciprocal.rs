use std::fmt::Write as _;

use tauri::State;

use crate::crystal_state::BrillouinZoneCache;
use crate::ipc::{IpcError, IpcResult};

const KPATH_NPOINTS_MIN: u32 = 5;
const KPATH_NPOINTS_MAX: u32 = 100;
const PHYSICAL_EPSILON: f64 = 1e-6;
const VIEWPORT_DIMENSION_MIN: f32 = 1.0;
const VIEWPORT_DIMENSION_MAX: f32 = 32_768.0;

fn validate_finite_f32(name: &str, value: f32) -> IpcResult<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(IpcError::invalid_argument(format!("{name} must be finite")))
    }
}

fn validate_viewport_dimension(name: &str, value: f32) -> IpcResult<()> {
    validate_finite_f32(name, value)?;
    if (VIEWPORT_DIMENSION_MIN..=VIEWPORT_DIMENSION_MAX).contains(&value) {
        Ok(())
    } else {
        Err(IpcError::invalid_argument(format!(
            "{name} must be between 1 and 32768",
        )))
    }
}

fn validate_viewport_dimensions(width: f32, height: f32) -> IpcResult<f32> {
    validate_viewport_dimension("width", width)?;
    validate_viewport_dimension("height", height)?;
    let aspect = width / height;
    if aspect.is_finite() && aspect > 0.0 {
        Ok(aspect)
    } else {
        Err(IpcError::invalid_argument(
            "viewport aspect ratio must be positive and finite",
        ))
    }
}

fn validate_screen_coordinate(x: f32, y: f32) -> IpcResult<()> {
    if x.is_finite() && y.is_finite() {
        Ok(())
    } else {
        Err(IpcError::from(
            "projected BZ label coordinate is non-finite",
        ))
    }
}

fn validate_kpath_npoints(npoints: u32) -> IpcResult<()> {
    if (KPATH_NPOINTS_MIN..=KPATH_NPOINTS_MAX).contains(&npoints) {
        Ok(())
    } else {
        Err(IpcError::invalid_argument(
            "npoints must be between 5 and 100",
        ))
    }
}

fn validate_lattice_3d(lattice: &[[f64; 3]; 3]) -> IpcResult<()> {
    if !lattice
        .iter()
        .flatten()
        .all(|component| component.is_finite())
    {
        return Err(IpcError::invalid_argument(
            "lattice components must be finite",
        ));
    }

    let norm = |vector: &[f64; 3]| {
        (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt()
    };
    let lengths = [norm(&lattice[0]), norm(&lattice[1]), norm(&lattice[2])];
    if lengths.iter().any(|length| *length <= PHYSICAL_EPSILON) {
        return Err(IpcError::invalid_argument(
            "lattice vectors must have non-zero length",
        ));
    }

    let determinant = lattice[0][0]
        * (lattice[1][1] * lattice[2][2] - lattice[1][2] * lattice[2][1])
        - lattice[0][1] * (lattice[1][0] * lattice[2][2] - lattice[1][2] * lattice[2][0])
        + lattice[0][2] * (lattice[1][0] * lattice[2][1] - lattice[1][1] * lattice[2][0]);
    let normalized_volume = determinant.abs() / (lengths[0] * lengths[1] * lengths[2]);
    if !normalized_volume.is_finite() || normalized_volume <= PHYSICAL_EPSILON {
        return Err(IpcError::invalid_argument("lattice is degenerate"));
    }
    Ok(())
}

fn inplane_lattice_vectors(
    lattice: &[[f64; 3]; 3],
    vacuum_axis: usize,
) -> IpcResult<([f64; 3], [f64; 3])> {
    match vacuum_axis {
        0 => Ok((lattice[1], lattice[2])),
        1 => Ok((lattice[0], lattice[2])),
        2 => Ok((lattice[0], lattice[1])),
        _ => Err(IpcError::invalid_argument("vacuum axis must be 0, 1, or 2")),
    }
}

fn remap_2d_kpoint_for_export(kpoint: [f64; 3], vacuum_axis: usize) -> IpcResult<[f64; 3]> {
    match vacuum_axis {
        0 => Ok([0.0, kpoint[0], kpoint[1]]),
        1 => Ok([kpoint[0], 0.0, kpoint[1]]),
        2 => Ok([kpoint[0], kpoint[1], 0.0]),
        _ => Err(IpcError::invalid_argument("vacuum axis must be 0, 1, or 2")),
    }
}

fn planar_lattice_parameters(a: [f64; 3], b: [f64; 3]) -> IpcResult<(f64, f64, f64)> {
    if !a
        .iter()
        .chain(b.iter())
        .all(|component| component.is_finite())
    {
        return Err(IpcError::invalid_argument(
            "in-plane lattice components must be finite",
        ));
    }

    let a_len = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    let b_len = (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt();
    if a_len <= PHYSICAL_EPSILON || b_len <= PHYSICAL_EPSILON {
        return Err(IpcError::invalid_argument(
            "in-plane lattice vectors must have non-zero length",
        ));
    }

    let denominator = a_len * b_len;
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let area = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    let normalized_area = area / denominator;
    if !normalized_area.is_finite() || normalized_area <= PHYSICAL_EPSILON {
        return Err(IpcError::invalid_argument("in-plane lattice is degenerate"));
    }

    let cos_gamma = ((a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / denominator).clamp(-1.0, 1.0);
    let gamma_deg = cos_gamma.acos().to_degrees();
    if !gamma_deg.is_finite() {
        return Err(IpcError::invalid_argument(
            "in-plane lattice angle must be finite",
        ));
    }
    Ok((a_len, b_len, gamma_deg))
}

fn validate_kpath_length(length: f64) -> IpcResult<()> {
    if length.is_finite() && length > PHYSICAL_EPSILON {
        Ok(())
    } else {
        Err(IpcError::invalid_argument(
            "K-path lengths must be positive and finite",
        ))
    }
}

fn calculate_kpath_sample_count(npoints: u32, segment_len: f64, avg_len: f64) -> IpcResult<usize> {
    validate_kpath_npoints(npoints)?;
    validate_kpath_length(segment_len)?;
    validate_kpath_length(avg_len)?;
    let sample_count = (f64::from(npoints) * segment_len / avg_len)
        .round()
        .max(2.0);
    if !sample_count.is_finite() || sample_count > usize::MAX as f64 {
        return Err(IpcError::invalid_argument(
            "K-path sample count is out of range",
        ));
    }
    Ok(sample_count as usize)
}

fn validate_bz_geometry(bz: &crate::brillouin_zone::BrillouinZone) -> IpcResult<()> {
    let reciprocal_finite = bz
        .recip_lattice
        .iter()
        .flatten()
        .all(|component| component.is_finite());
    let vertices_finite = bz
        .vertices
        .iter()
        .flatten()
        .all(|component| component.is_finite());
    let vertex_count = bz.vertices.len();
    let edges_valid = !bz.edges.is_empty()
        && bz
            .edges
            .iter()
            .all(|edge| edge[0] < vertex_count && edge[1] < vertex_count && edge[0] != edge[1]);
    let faces_valid = (bz.is_2d || !bz.faces.is_empty())
        && bz.faces.iter().all(|face| {
            face.len() >= 3
                && face.iter().enumerate().all(|(index, vertex)| {
                    *vertex < vertex_count && !face[..index].contains(vertex)
                })
        });
    let topology_valid = vertex_count >= 3 && edges_valid && faces_valid;
    if reciprocal_finite && vertices_finite && topology_valid {
        Ok(())
    } else {
        Err(IpcError::invalid_argument(
            "Brillouin zone geometry is empty or non-finite",
        ))
    }
}

fn validate_kpath_geometry(kpath: &crate::kpath::KPath) -> IpcResult<()> {
    let points_valid = !kpath.points.is_empty()
        && kpath.points.iter().enumerate().all(|(index, point)| {
            !point.label.is_empty()
                && point
                    .coord_frac
                    .iter()
                    .all(|component| component.is_finite())
                && !kpath.points[..index]
                    .iter()
                    .any(|previous| previous.label == point.label)
        });
    let segments_valid = !kpath.path_segments.is_empty()
        && kpath.path_segments.iter().all(|segment| {
            segment.len() >= 2
                && segment.iter().all(|label| {
                    !label.is_empty() && kpath.points.iter().any(|point| point.label == *label)
                })
        });
    if points_valid && segments_valid {
        Ok(())
    } else {
        Err(IpcError::invalid_argument("K-path geometry is invalid"))
    }
}

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
) -> IpcResult<BzInfoResponse> {
    let mut cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let sg = cs.spacegroup_number;
    let lattice_col = cs.get_lattice_col_major();
    // BrillouinZone::new expects row vectors [ [x,y,z], [x,y,z], [x,y,z] ]
    let lat = [
        [lattice_col[0], lattice_col[1], lattice_col[2]],
        [lattice_col[3], lattice_col[4], lattice_col[5]],
        [lattice_col[6], lattice_col[7], lattice_col[8]],
    ];
    validate_lattice_3d(&lat)?;
    let previous_dimensionality = (cs.is_2d, cs.vacuum_axis);
    cs.detect_2d();

    let computation = (|| -> IpcResult<(
        crate::brillouin_zone::BrillouinZone,
        crate::kpath::KPath,
        String,
    )> {
        let result = if cs.is_2d {
            let vac_axis = cs.vacuum_axis.unwrap_or(2);
            let (a_proj, b_proj) = inplane_lattice_vectors(&lat, vac_axis)?;

            let (a_len, b_len, gamma_deg) = planar_lattice_parameters(a_proj, b_proj)?;
            let bz_type_2d =
                crate::kpath_2d::identify_bravais_2d(a_len, b_len, gamma_deg, sg);
            let mut bz =
                crate::brillouin_zone::BrillouinZone::new_2d(a_proj, b_proj, vac_axis);
            bz.bravais_type = crate::brillouin_zone::BravaisType::Unknown;
            let kpath = crate::kpath_2d::get_kpath_2d(bz_type_2d, a_len, b_len, vac_axis);
            (bz, kpath, format!("2D {:?}", bz_type_2d))
        } else {
            let bz_type = crate::kpath::identify_bravais_type(sg);
            let bz = crate::brillouin_zone::BrillouinZone::new(lat, bz_type);
            let kpath = crate::kpath::get_kpath(bz_type, &lat);
            (bz, kpath, format!("{:?}", bz_type))
        };
        validate_bz_geometry(&result.0)?;
        validate_kpath_geometry(&result.1)?;
        Ok(result)
    })();
    let (bz, kpath, bravais_type_str) = match computation {
        Ok(result) => result,
        Err(error) => {
            cs.is_2d = previous_dimensionality.0;
            cs.vacuum_axis = previous_dimensionality.1;
            return Err(error);
        }
    };

    let response = BzInfoResponse {
        bravais_type: bravais_type_str,
        spacegroup: sg,
        vertices_count: bz.vertices.len(),
        edges_count: bz.edges.len(),
        faces_count: bz.faces.len(),
        is_2d: bz.is_2d,
    };

    let mut renderer = match renderer_state.lock() {
        Ok(renderer) => renderer,
        Err(_) => {
            cs.is_2d = previous_dimensionality.0;
            cs.vacuum_axis = previous_dimensionality.1;
            return Err(IpcError::lock("renderer state lock poisoned"));
        }
    };
    renderer.update_bz_data(Some((&bz, &kpath)));

    cs.bz_cache = Some(BrillouinZoneCache {
        bz,
        kpath,
        source_version: cs.version,
        vacuum_axis: cs.vacuum_axis,
    });

    Ok(response)
}

#[tauri::command]
pub fn toggle_bz_display(
    show: bool,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<()> {
    if !show {
        let mut renderer = renderer_state
            .lock()
            .map_err(|_| IpcError::lock("renderer state lock poisoned"))?;
        renderer.show_bz = false;
        return Ok(());
    }

    let cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let cache = cs
        .bz_cache
        .as_ref()
        .ok_or_else(|| IpcError::invalid_argument("Brillouin zone not computed yet"))?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer state lock poisoned"))?;
    renderer.update_bz_data(Some((&cache.bz, &cache.kpath)));
    Ok(())
}

#[tauri::command]
pub fn get_kpath_info(
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<KPathInfoResponse> {
    let cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    if let Some(cache) = &cs.bz_cache {
        let points = cache
            .kpath
            .points
            .iter()
            .map(|p| KPathPointUi {
                label: p.label.clone(),
                coord_frac: p.coord_frac,
            })
            .collect();

        Ok(KPathInfoResponse {
            points,
            segments: cache.kpath.path_segments.clone(),
        })
    } else {
        Err(IpcError::invalid_argument(
            "Brillouin zone not computed yet",
        ))
    }
}

#[tauri::command]
pub fn set_bz_scale(
    scale: f32,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    validate_finite_f32("scale", scale)?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer state lock poisoned"))?;
    renderer.bz_scale = scale.clamp(0.15, 1.0);
    Ok(())
}

#[tauri::command]
pub fn generate_kpath_text(
    npoints: u32,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<KPathTextResponse> {
    let cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let cache = cs
        .bz_cache
        .as_ref()
        .ok_or_else(|| IpcError::invalid_argument("Brillouin zone not computed yet"))?;
    generate_kpath_text_from_cache(cache, cs.version, npoints)
}

fn generate_kpath_text_from_cache(
    cache: &BrillouinZoneCache,
    current_version: u32,
    npoints: u32,
) -> IpcResult<KPathTextResponse> {
    validate_kpath_npoints(npoints)?;
    if cache.source_version != current_version {
        return Err(IpcError::invalid_argument(
            "Brillouin zone cache is stale; recompute before exporting",
        ));
    }
    let bz = &cache.bz;
    let kpath = &cache.kpath;

    let mut pt_map = std::collections::HashMap::with_capacity(kpath.points.len());
    for point in &kpath.points {
        pt_map.insert(point.label.as_str(), &point.coord_frac);
    }

    let is_2d = bz.is_2d;
    let export_vacuum_axis = if is_2d {
        Some(
            cache
                .vacuum_axis
                .ok_or_else(|| IpcError::from("2D BZ cache has no vacuum axis"))?,
        )
    } else {
        None
    };

    let rl = &bz.recip_lattice; // $\mathbf{b}_i$ rows

    // $\mathbf{k}_{\text{cart}} = k_1 \mathbf{b}_1 + k_2 \mathbf{b}_2 + k_3 \mathbf{b}_3$
    let frac_to_cart = |f: &[f64; 3]| -> [f64; 3] {
        [
            f[0] * rl[0][0] + f[1] * rl[1][0] + f[2] * rl[2][0],
            f[0] * rl[0][1] + f[1] * rl[1][1] + f[2] * rl[2][1],
            f[0] * rl[0][2] + f[1] * rl[1][2] + f[2] * rl[2][2],
        ]
    };

    let cart_dist = |a: &[f64; 3], b: &[f64; 3]| -> f64 {
        let ca = frac_to_cart(a);
        let cb = frac_to_cart(b);
        ((ca[0] - cb[0]).powi(2) + (ca[1] - cb[1]).powi(2) + (ca[2] - cb[2]).powi(2)).sqrt()
    };

    // Collect all segment pairs with their Cartesian lengths
    struct SegPair<'a> {
        c0: [f64; 3],
        c1: [f64; 3],
        l0: &'a str,
        l1: &'a str,
        len: f64,
        is_seg_end: bool,
        sample_count: usize,
    }
    let pair_capacity = kpath
        .path_segments
        .iter()
        .try_fold(0_usize, |count, segment| {
            count.checked_add(segment.len().saturating_sub(1))
        })
        .ok_or_else(|| IpcError::from("K-path segment count overflow"))?;
    let mut pairs: Vec<SegPair<'_>> = Vec::with_capacity(pair_capacity);
    let mut total_len = 0.0_f64;

    for seg in &kpath.path_segments {
        for (pi, pair) in seg.windows(2).enumerate() {
            let c0 = *pt_map
                .get(pair[0].as_str())
                .ok_or_else(|| IpcError::from(format!("Missing k-path point: {}", pair[0])))?;
            let c1 = *pt_map
                .get(pair[1].as_str())
                .ok_or_else(|| IpcError::from(format!("Missing k-path point: {}", pair[1])))?;
            let d = cart_dist(c0, c1);
            validate_kpath_length(d)?;
            pairs.push(SegPair {
                c0: *c0,
                c1: *c1,
                l0: pair[0].as_str(),
                l1: pair[1].as_str(),
                len: d,
                is_seg_end: pi == seg.len() - 2,
                sample_count: 0,
            });
            total_len += d;
            if !total_len.is_finite() {
                return Err(IpcError::invalid_argument(
                    "K-path total length must be finite",
                ));
            }
        }
    }

    validate_kpath_length(total_len)?;

    let avg_len = total_len / pairs.len() as f64;
    validate_kpath_length(avg_len)?;

    let mut total_samples = 0_usize;
    for pair in &mut pairs {
        let sample_count = calculate_kpath_sample_count(npoints, pair.len, avg_len)?;
        pair.sample_count = sample_count;
        let emitted_count = if pair.is_seg_end {
            sample_count
        } else {
            sample_count
                .checked_sub(1)
                .ok_or_else(|| IpcError::from("K-path sample count underflow"))?
        };
        total_samples = total_samples
            .checked_add(emitted_count)
            .ok_or_else(|| IpcError::from("K-path sample count overflow"))?;
    }

    // QE: K_POINTS {crystal}
    let qe_capacity = total_samples
        .checked_mul(80)
        .and_then(|capacity| capacity.checked_add(64))
        .ok_or_else(|| IpcError::from("QE K-path output size overflow"))?;
    let mut qe_text = String::new();
    qe_text
        .try_reserve(qe_capacity)
        .map_err(|_| IpcError::from("Unable to reserve QE K-path output"))?;
    write!(qe_text, "K_POINTS {{crystal}}\n{total_samples}\n")
        .map_err(|_| IpcError::from("Unable to format QE K-path output"))?;
    for sp in &pairs {
        let seg_n = sp.sample_count;
        let end = if sp.is_seg_end { seg_n } else { seg_n - 1 };
        for i in 0..end {
            let t = if seg_n > 1 {
                i as f64 / (seg_n - 1) as f64
            } else {
                0.0
            };
            let k = [
                sp.c0[0] + t * (sp.c1[0] - sp.c0[0]),
                sp.c0[1] + t * (sp.c1[1] - sp.c0[1]),
                sp.c0[2] + t * (sp.c1[2] - sp.c0[2]),
            ];
            let k = match export_vacuum_axis {
                Some(vacuum_axis) => remap_2d_kpoint_for_export(k, vacuum_axis)?,
                None => k,
            };
            writeln!(qe_text, "  {:.10}  {:.10}  {:.10}  1.0", k[0], k[1], k[2])
                .map_err(|_| IpcError::from("Unable to format QE K-path point"))?;
        }
    }

    // VASP: explicit KPOINTS with uniform spacing
    let vasp_capacity = total_samples
        .checked_mul(96)
        .and_then(|capacity| capacity.checked_add(96))
        .ok_or_else(|| IpcError::from("VASP K-path output size overflow"))?;
    let mut vasp_text = String::new();
    vasp_text
        .try_reserve(vasp_capacity)
        .map_err(|_| IpcError::from("Unable to reserve VASP K-path output"))?;
    write!(
        vasp_text,
        "k-points for band structure (uniform spacing)\n{total_samples}\nReciprocal lattice"
    )
    .map_err(|_| IpcError::from("Unable to format VASP K-path header"))?;
    for sp in &pairs {
        let seg_n = sp.sample_count;
        let end = if sp.is_seg_end { seg_n } else { seg_n - 1 };
        for i in 0..end {
            let t = if seg_n > 1 {
                i as f64 / (seg_n - 1) as f64
            } else {
                0.0
            };
            let k = [
                sp.c0[0] + t * (sp.c1[0] - sp.c0[0]),
                sp.c0[1] + t * (sp.c1[1] - sp.c0[1]),
                sp.c0[2] + t * (sp.c1[2] - sp.c0[2]),
            ];
            let k = match export_vacuum_axis {
                Some(vacuum_axis) => remap_2d_kpoint_for_export(k, vacuum_axis)?,
                None => k,
            };
            write!(
                vasp_text,
                "\n  {:.10}  {:.10}  {:.10}  1.0",
                k[0], k[1], k[2]
            )
            .map_err(|_| IpcError::from("Unable to format VASP K-path point"))?;
            if i == 0 {
                write!(vasp_text, "  ! {}", sp.l0)
                    .map_err(|_| IpcError::from("Unable to format VASP K-path label"))?;
            } else if i == end - 1 && sp.is_seg_end {
                write!(vasp_text, "  ! {}", sp.l1)
                    .map_err(|_| IpcError::from("Unable to format VASP K-path label"))?;
            }
        }
    }

    Ok(KPathTextResponse {
        qe: qe_text,
        vasp: vasp_text,
    })
}

#[tauri::command]
pub fn get_bz_label_positions(
    width: f32,
    height: f32,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
) -> IpcResult<Vec<BzLabelPos>> {
    let aspect = validate_viewport_dimensions(width, height)?;
    let cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let cache = cs
        .bz_cache
        .as_ref()
        .ok_or_else(|| IpcError::invalid_argument("Brillouin zone not computed yet"))?;
    let bz = &cache.bz;
    let kpath = &cache.kpath;

    // Reconstruct camera state identical to BzSubViewport::update_bz
    let mut max_r = 0.0_f64;
    for v in &bz.vertices {
        let r = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if r > max_r {
            max_r = r;
        }
    }
    let mut max_b = 0.0_f64;
    for row in &bz.recip_lattice {
        let m = (row[0] * row[0] + row[1] * row[1] + row[2] * row[2]).sqrt();
        if m > max_b {
            max_b = m;
        }
    }
    if max_r < 1e-6 {
        max_r = max_b;
    }
    if !max_r.is_finite() || !max_b.is_finite() || max_r <= PHYSICAL_EPSILON {
        return Err(IpcError::from(
            "cached Brillouin zone has invalid spatial extent",
        ));
    }
    let fit_scale = (max_r * 2.8) as f32;
    if !fit_scale.is_finite() {
        return Err(IpcError::from(
            "cached Brillouin zone extent exceeds viewport range",
        ));
    }
    let ortho_scale = fit_scale.max(1.0);
    let dist = fit_scale * 2.0;

    let mut eye = glam::Vec3::new(dist * 0.4, dist * 0.3, dist);
    let target = glam::Vec3::ZERO;
    let mut up = glam::Vec3::Y;

    if bz.is_2d {
        let (eye_direction, plane_up) = crate::renderer::bz_renderer::camera_axes_2d(bz)
            .ok_or_else(|| IpcError::from("cached 2D BZ camera frame is invalid"))?;
        eye = eye_direction * dist;
        up = plane_up;
    }

    let view = glam::Mat4::look_at_rh(eye, target, up);
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
        if !clip.x.is_finite() || !clip.y.is_finite() || !clip.w.is_finite() {
            return Err(IpcError::from("BZ label projection is non-finite"));
        }
        if clip.w.abs() < 1e-6 {
            continue;
        }

        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;
        let x = (ndc_x + 1.0) / 2.0 * width;
        let y = (1.0 - ndc_y) / 2.0 * height;
        validate_screen_coordinate(x, y)?;

        labels.push(BzLabelPos {
            label: kp.label.clone(),
            x,
            y,
        });
    }

    Ok(labels)
}

#[cfg(test)]
mod tests {
    use super::{
        calculate_kpath_sample_count, generate_kpath_text_from_cache, inplane_lattice_vectors,
        planar_lattice_parameters, remap_2d_kpoint_for_export, validate_bz_geometry,
        validate_finite_f32, validate_kpath_geometry, validate_kpath_length,
        validate_kpath_npoints, validate_lattice_3d, validate_screen_coordinate,
        validate_viewport_dimension, validate_viewport_dimensions,
    };
    use crate::crystal_state::BrillouinZoneCache;

    #[test]
    fn finite_scalar_validation_rejects_non_finite_values() {
        assert!(validate_finite_f32("scale", 0.5).is_ok());
        assert!(validate_finite_f32("scale", f32::NAN).is_err());
        assert!(validate_finite_f32("scale", f32::INFINITY).is_err());
    }

    #[test]
    fn viewport_dimension_validation_requires_positive_finite_values() {
        assert!(validate_viewport_dimension("width", 640.0).is_ok());
        assert!(validate_viewport_dimension("width", 0.0).is_err());
        assert!(validate_viewport_dimension("width", 0.5).is_err());
        assert!(validate_viewport_dimension("width", -1.0).is_err());
        assert!(validate_viewport_dimension("width", f32::NAN).is_err());
        assert!(validate_viewport_dimension("width", 32_769.0).is_err());
        assert!(validate_viewport_dimensions(1920.0, 1080.0).is_ok());
        assert!(validate_viewport_dimensions(f32::MAX, f32::MIN_POSITIVE).is_err());
        assert!(validate_screen_coordinate(100.0, 200.0).is_ok());
        assert!(validate_screen_coordinate(f32::NAN, 200.0).is_err());
    }

    #[test]
    fn kpath_npoints_validation_enforces_resource_bounds() {
        assert!(validate_kpath_npoints(5).is_ok());
        assert!(validate_kpath_npoints(100).is_ok());
        assert!(validate_kpath_npoints(4).is_err());
        assert!(validate_kpath_npoints(101).is_err());
        assert!(validate_kpath_npoints(u32::MAX).is_err());
    }

    #[test]
    fn reciprocal_lattice_validation_rejects_degenerate_geometry() {
        let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert!(validate_lattice_3d(&identity).is_ok());

        let coplanar = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]];
        assert!(validate_lattice_3d(&coplanar).is_err());

        let non_finite = [[f64::NAN, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert!(validate_lattice_3d(&non_finite).is_err());
    }

    #[test]
    fn planar_lattice_parameters_reject_degenerate_projections() {
        let (_, _, gamma_deg) = planar_lattice_parameters([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
            .expect("orthogonal lattice must be valid");
        assert!((gamma_deg - 90.0).abs() < 1e-6);
        assert!(planar_lattice_parameters([1.0, 0.0, 0.0], [2.0, 0.0, 0.0]).is_err());
        assert!(planar_lattice_parameters([f64::NAN, 0.0, 0.0], [0.0, 1.0, 0.0]).is_err());
    }

    #[test]
    fn inplane_lattice_selection_preserves_skew_3d_vectors() {
        let lattice = [[2.0, 0.0, 0.0], [0.5, 8.0, 0.0], [0.25, 1.0, 3.0]];
        assert_eq!(
            inplane_lattice_vectors(&lattice, 1).unwrap(),
            (lattice[0], lattice[2])
        );
        assert!(inplane_lattice_vectors(&lattice, 3).is_err());
    }

    #[test]
    fn kpoint_export_mapping_restores_original_reciprocal_axis_order() {
        let internal = [0.25, 0.75, 0.0];

        assert_eq!(
            remap_2d_kpoint_for_export(internal, 0).unwrap(),
            [0.0, 0.25, 0.75]
        );
        assert_eq!(
            remap_2d_kpoint_for_export(internal, 1).unwrap(),
            [0.25, 0.0, 0.75]
        );
        assert_eq!(
            remap_2d_kpoint_for_export(internal, 2).unwrap(),
            [0.25, 0.75, 0.0]
        );
        assert!(remap_2d_kpoint_for_export(internal, 3).is_err());
    }

    #[test]
    fn kpath_export_uses_cache_axis_and_rejects_stale_snapshot() {
        let make_cache = |vacuum_axis| BrillouinZoneCache {
            bz: crate::brillouin_zone::BrillouinZone {
                recip_lattice: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                edges: vec![[0, 1], [1, 2], [2, 0]],
                faces: vec![vec![0, 1, 2]],
                bravais_type: crate::brillouin_zone::BravaisType::Unknown,
                is_2d: true,
            },
            kpath: crate::kpath::KPath {
                points: vec![
                    crate::kpath::HighSymmetryPoint {
                        label: "Γ".into(),
                        coord_frac: [0.0, 0.0, 0.0],
                    },
                    crate::kpath::HighSymmetryPoint {
                        label: "X".into(),
                        coord_frac: [0.25, 0.75, 0.0],
                    },
                ],
                path_segments: vec![vec!["Γ".into(), "X".into()]],
            },
            source_version: 7,
            vacuum_axis: Some(vacuum_axis),
        };

        for (vacuum_axis, expected) in [
            (0, "0.0000000000  0.2500000000  0.7500000000"),
            (1, "0.2500000000  0.0000000000  0.7500000000"),
            (2, "0.2500000000  0.7500000000  0.0000000000"),
        ] {
            let cache = make_cache(vacuum_axis);
            let output = generate_kpath_text_from_cache(&cache, 7, 5).unwrap();
            assert!(output.qe.contains(expected));
            assert!(output.vasp.contains(expected));
            assert!(generate_kpath_text_from_cache(&cache, 8, 5).is_err());
        }
    }

    #[test]
    fn kpath_length_and_sample_count_reject_non_finite_values() {
        assert!(validate_kpath_length(0.5).is_ok());
        assert!(validate_kpath_length(0.0).is_err());
        assert!(validate_kpath_length(f64::NAN).is_err());
        assert!(validate_kpath_length(f64::INFINITY).is_err());
        assert_eq!(calculate_kpath_sample_count(20, 0.5, 0.5).unwrap(), 20);
        assert!(calculate_kpath_sample_count(20, f64::NAN, 0.5).is_err());
        assert!(calculate_kpath_sample_count(20, 0.5, 0.0).is_err());
    }

    #[test]
    fn bz_geometry_validation_rejects_invalid_topology_indices() {
        let mut bz = crate::brillouin_zone::BrillouinZone {
            recip_lattice: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            edges: vec![[0, 1], [1, 2], [2, 0]],
            faces: vec![vec![0, 1, 2]],
            bravais_type: crate::brillouin_zone::BravaisType::Unknown,
            is_2d: false,
        };
        assert!(validate_bz_geometry(&bz).is_ok());
        bz.edges[0] = [0, 3];
        assert!(validate_bz_geometry(&bz).is_err());
        bz.edges[0] = [0, 1];
        bz.faces[0] = vec![0, 1, 3];
        assert!(validate_bz_geometry(&bz).is_err());
        bz.faces[0] = vec![0, 1];
        assert!(validate_bz_geometry(&bz).is_err());
    }

    #[test]
    fn kpath_geometry_validation_rejects_duplicate_and_missing_labels() {
        let mut kpath = crate::kpath::KPath {
            points: vec![
                crate::kpath::HighSymmetryPoint {
                    label: "Γ".into(),
                    coord_frac: [0.0, 0.0, 0.0],
                },
                crate::kpath::HighSymmetryPoint {
                    label: "X".into(),
                    coord_frac: [0.5, 0.0, 0.0],
                },
            ],
            path_segments: vec![vec!["Γ".into(), "X".into()]],
        };
        assert!(validate_kpath_geometry(&kpath).is_ok());
        kpath.path_segments[0][1] = "M".into();
        assert!(validate_kpath_geometry(&kpath).is_err());
        kpath.path_segments[0][1] = "X".into();
        kpath.points[1].label = "Γ".into();
        assert!(validate_kpath_geometry(&kpath).is_err());
    }
}
