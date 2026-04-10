// [Overview: Brillouin Zone construction in pure Rust using Wigner-Seitz half-plane intersection.]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BravaisType {
    CubicPrimitive,
    CubicFaceCentered,
    CubicBodyCentered,
    TetragonalPrimitive,
    TetragonalBodyCentered,
    OrthorhombicPrimitive,
    OrthorhombicBaseCentered,
    OrthorhombicBodyCentered,
    OrthorhombicFaceCentered,
    Hexagonal,
    Rhombohedral,
    MonoclinicPrimitive,
    MonoclinicBaseCentered,
    Triclinic,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrillouinZone {
    pub recip_lattice: [[f64; 3]; 3],
    pub vertices: Vec<[f64; 3]>,
    pub edges: Vec<[usize; 2]>,
    pub faces: Vec<Vec<usize>>,
    pub bravais_type: BravaisType,
}

#[derive(Debug, Clone)]
struct Plane {
    normal: [f64; 3],
    d: f64, // p dot n - d <= 0 is inside
}

#[derive(Debug, Clone)]
struct Polygon {
    vertices: Vec<[f64; 3]>,
}

impl BrillouinZone {
    pub fn new(real_lattice: [[f64; 3]; 3], bravais_type: BravaisType) -> Self {
        let recip = Self::compute_reciprocal_lattice(real_lattice);
        let planes = Self::generate_bisecting_planes(&recip);
        let (vertices, edges, faces) = Self::wigner_seitz_cut(&planes, &recip);
        
        Self {
            recip_lattice: recip,
            vertices,
            edges,
            faces,
            bravais_type,
        }
    }

    fn compute_reciprocal_lattice(a: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
        let cross = |u: [f64; 3], v: [f64; 3]| -> [f64; 3] {
            [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ]
        };
        let dot = |u: [f64; 3], v: [f64; 3]| -> f64 {
            u[0] * v[0] + u[1] * v[1] + u[2] * v[2]
        };

        let b1 = cross(a[1], a[2]);
        let b2 = cross(a[2], a[0]);
        let b3 = cross(a[0], a[1]);

        let v = dot(a[0], b1);
        if v.abs() < 1e-12 {
            return [[0.0; 3]; 3];
        }
        let f = 2.0 * std::f64::consts::PI / v;

        [
            [b1[0] * f, b1[1] * f, b1[2] * f],
            [b2[0] * f, b2[1] * f, b2[2] * f],
            [b3[0] * f, b3[1] * f, b3[2] * f],
        ]
    }

    fn generate_bisecting_planes(b: &[[f64; 3]; 3]) -> Vec<Plane> {
        let mut planes = Vec::new();
        for i in -2..=2 {
            for j in -2..=2 {
                for k in -2..=2 {
                    if i == 0 && j == 0 && k == 0 {
                        continue;
                    }
                    let g = [
                        i as f64 * b[0][0] + j as f64 * b[1][0] + k as f64 * b[2][0],
                        i as f64 * b[0][1] + j as f64 * b[1][1] + k as f64 * b[2][1],
                        i as f64 * b[0][2] + j as f64 * b[1][2] + k as f64 * b[2][2],
                    ];
                    let g_mag_sq = g[0] * g[0] + g[1] * g[1] + g[2] * g[2];
                    if g_mag_sq > 1e-6 {
                        let g_mag = g_mag_sq.sqrt();
                        let normal = [g[0] / g_mag, g[1] / g_mag, g[2] / g_mag];
                        let d = g_mag / 2.0;
                        planes.push(Plane { normal, d });
                    }
                }
            }
        }
        planes.sort_by(|a, b| a.d.partial_cmp(&b.d).unwrap_or(std::cmp::Ordering::Equal));
        planes
    }

    fn wigner_seitz_cut(planes: &[Plane], recip: &[[f64; 3]; 3]) -> (Vec<[f64; 3]>, Vec<[usize; 2]>, Vec<Vec<usize>>) {
        let mut max_b = 0.0_f64;
        for r in recip {
            let m = (r[0]*r[0] + r[1]*r[1] + r[2]*r[2]).sqrt();
            if m > max_b { max_b = m; }
        }
        let size = (max_b * 10.0).max(10.0);
        let mut faces_poly = vec![
            Polygon { vertices: vec![[-size, -size, size], [size, -size, size], [size, size, size], [-size, size, size]] }, // +z
            Polygon { vertices: vec![[-size, size, -size], [size, size, -size], [size, -size, -size], [-size, -size, -size]] }, // -z
            Polygon { vertices: vec![[size, -size, -size], [size, size, -size], [size, size, size], [size, -size, size]] }, // +x
            Polygon { vertices: vec![[-size, -size, size], [-size, size, size], [-size, size, -size], [-size, -size, -size]] }, // -x
            Polygon { vertices: vec![[-size, size, size], [size, size, size], [size, size, -size], [-size, size, -size]] }, // +y
            Polygon { vertices: vec![[-size, -size, -size], [size, -size, -size], [size, -size, size], [-size, -size, size]] }, // -y
        ];

        let dot = |u: &[f64; 3], v: &[f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];

        for plane in planes {
            let mut next_faces = Vec::new();
            let mut new_face_vertices = Vec::new();

            for poly in &faces_poly {
                let mut clipped_vertices = Vec::new();
                let v = &poly.vertices;
                let n = v.len();
                if n == 0 { continue; }

                let mut dists = Vec::with_capacity(n);
                for pt in v {
                    dists.push(dot(pt, &plane.normal) - plane.d);
                }

                for i in 0..n {
                    let curr = i;
                    let next = (i + 1) % n;
                    
                    let d_curr = dists[curr];
                    let d_next = dists[next];

                    let curr_in = d_curr <= 1e-7;
                    let next_in = d_next <= 1e-7;

                    if curr_in {
                        clipped_vertices.push(v[curr]);
                    }
                    
                    if curr_in != next_in {
                        let t = d_curr / (d_curr - d_next);
                        let pt = [
                            v[curr][0] + t * (v[next][0] - v[curr][0]),
                            v[curr][1] + t * (v[next][1] - v[curr][1]),
                            v[curr][2] + t * (v[next][2] - v[curr][2]),
                        ];
                        clipped_vertices.push(pt);
                        new_face_vertices.push(pt);
                    }
                }
                
                if clipped_vertices.len() >= 3 {
                    next_faces.push(Polygon { vertices: clipped_vertices });
                }
            }

            if new_face_vertices.len() >= 3 {
                let mut valid_new_face = Vec::new();
                for pt in &new_face_vertices {
                    let mut is_dup = false;
                    for valid_pt in &valid_new_face {
                        let dx = pt[0] - valid_pt[0];
                        let dy = pt[1] - valid_pt[1];
                        let dz = pt[2] - valid_pt[2];
                        if dx*dx + dy*dy + dz*dz < 1e-12 {
                            is_dup = true;
                            break;
                        }
                    }
                    if !is_dup {
                        valid_new_face.push(*pt);
                    }
                }
                
                if valid_new_face.len() >= 3 {
                    let mut centroid = [0.0, 0.0, 0.0];
                    for pt in &valid_new_face {
                        centroid[0] += pt[0];
                        centroid[1] += pt[1];
                        centroid[2] += pt[2];
                    }
                    let k = 1.0 / valid_new_face.len() as f64;
                    centroid[0] *= k;
                    centroid[1] *= k;
                    centroid[2] *= k;

                    let v0 = [
                        valid_new_face[0][0] - centroid[0],
                        valid_new_face[0][1] - centroid[1],
                        valid_new_face[0][2] - centroid[2],
                    ];
                    let mut t1 = v0;
                    let t1_mag = (t1[0]*t1[0] + t1[1]*t1[1] + t1[2]*t1[2]).sqrt();
                    if t1_mag > 1e-8 {
                        t1[0] /= t1_mag; t1[1] /= t1_mag; t1[2] /= t1_mag;
                    }
                    
                    let mut t2 = [
                        plane.normal[1]*t1[2] - plane.normal[2]*t1[1],
                        plane.normal[2]*t1[0] - plane.normal[0]*t1[2],
                        plane.normal[0]*t1[1] - plane.normal[1]*t1[0],
                    ];
                    let t2_mag = (t2[0]*t2[0] + t2[1]*t2[1] + t2[2]*t2[2]).sqrt();
                    if t2_mag > 1e-8 {
                        t2[0] /= t2_mag; t2[1] /= t2_mag; t2[2] /= t2_mag;
                    }

                    valid_new_face.sort_by(|a, b| {
                        let va = [a[0] - centroid[0], a[1] - centroid[1], a[2] - centroid[2]];
                        let vb = [b[0] - centroid[0], b[1] - centroid[1], b[2] - centroid[2]];
                        let angle_a = f64::atan2(dot(&va, &t2), dot(&va, &t1));
                        let angle_b = f64::atan2(dot(&vb, &t2), dot(&vb, &t1));
                        angle_a.partial_cmp(&angle_b).unwrap_or(std::cmp::Ordering::Equal)
                    });

                    let e1 = [
                        valid_new_face[1][0] - valid_new_face[0][0],
                        valid_new_face[1][1] - valid_new_face[0][1],
                        valid_new_face[1][2] - valid_new_face[0][2],
                    ];
                    let e2 = [
                        valid_new_face[2][0] - valid_new_face[1][0],
                        valid_new_face[2][1] - valid_new_face[1][1],
                        valid_new_face[2][2] - valid_new_face[1][2],
                    ];
                    let n_cross = [
                        e1[1]*e2[2] - e1[2]*e2[1],
                        e1[2]*e2[0] - e1[0]*e2[2],
                        e1[0]*e2[1] - e1[1]*e2[0],
                    ];
                    if dot(&n_cross, &plane.normal) < 0.0 {
                         valid_new_face.reverse();
                    }

                    next_faces.push(Polygon { vertices: valid_new_face });
                }
            }
            faces_poly = next_faces;
        }

        let mut final_vertices: Vec<[f64; 3]> = Vec::new();
        let mut final_edges: Vec<[usize; 2]> = Vec::new();
        let mut final_faces: Vec<Vec<usize>> = Vec::new();

        let get_or_add_vertex = |v_query: [f64; 3], verts: &mut Vec<[f64; 3]>| -> usize {
            for (i, v) in verts.iter().enumerate() {
                let dx = v_query[0] - v[0];
                let dy = v_query[1] - v[1];
                let dz = v_query[2] - v[2];
                if dx*dx + dy*dy + dz*dz < 1e-10 {
                    return i;
                }
            }
            verts.push(v_query);
            verts.len() - 1
        };

        for poly in &faces_poly {
            let mut face_idxs = Vec::new();
            for v in &poly.vertices {
                face_idxs.push(get_or_add_vertex(*v, &mut final_vertices));
            }
            
            if face_idxs.len() >= 3 {
                for i in 0..face_idxs.len() {
                    let v1 = face_idxs[i];
                    let v2 = face_idxs[(i + 1) % face_idxs.len()];
                    let min_v = std::cmp::min(v1, v2);
                    let max_v = std::cmp::max(v1, v2);
                    let edge = [min_v, max_v];
                    if !final_edges.contains(&edge) {
                        final_edges.push(edge);
                    }
                }
                final_faces.push(face_idxs);
            }
        }

        (final_vertices, final_edges, final_faces)
    }
}
