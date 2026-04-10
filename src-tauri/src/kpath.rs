// [Overview: High-Symmetry k-Point Database (Setyawan-Curtarolo 2010)]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Serialize, Deserialize};
use crate::brillouin_zone::BravaisType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighSymmetryPoint {
    pub label: String,
    pub coord_frac: [f64; 3], // In primitive reciprocal basis
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KPath {
    pub points: Vec<HighSymmetryPoint>,
    pub path_segments: Vec<Vec<String>>,
}

/// Identifies the Bravais type from the spacegroup.
pub fn identify_bravais_type(sg_num: i32) -> BravaisType {
    if (195..=230).contains(&sg_num) {
        if [195, 198, 200, 201, 205, 207, 208, 212, 213, 215, 218, 221, 222, 223, 224].contains(&sg_num) {
            BravaisType::CubicPrimitive
        } else if [196, 202, 203, 209, 216, 219, 225, 226, 227, 228].contains(&sg_num) {
            BravaisType::CubicFaceCentered
        } else {
            BravaisType::CubicBodyCentered
        }
    } else if (168..=194).contains(&sg_num) {
        BravaisType::Hexagonal
    } else if (143..=167).contains(&sg_num) {
        if [146, 148, 155, 160, 161, 166, 167].contains(&sg_num) {
            BravaisType::Rhombohedral
        } else {
            BravaisType::Hexagonal
        }
    } else if (75..=142).contains(&sg_num) {
        let body_centered = [79, 80, 82, 87, 88, 97, 98, 107, 108, 109, 110, 119, 120, 121, 122, 139, 140, 141, 142];
        if body_centered.contains(&sg_num) {
            BravaisType::TetragonalBodyCentered
        } else {
            BravaisType::TetragonalPrimitive
        }
    } else if (16..=74).contains(&sg_num) {
        let face_centered = [22, 42, 43, 69, 70];
        let body_centered = [23, 24, 44, 45, 46, 71, 72, 73, 74];
        let base_centered = [20, 21, 35, 36, 37, 38, 39, 40, 41, 63, 64, 65, 66, 67, 68];
        
        if face_centered.contains(&sg_num) {
            BravaisType::OrthorhombicFaceCentered
        } else if body_centered.contains(&sg_num) {
            BravaisType::OrthorhombicBodyCentered
        } else if base_centered.contains(&sg_num) {
            BravaisType::OrthorhombicBaseCentered
        } else {
            BravaisType::OrthorhombicPrimitive
        }
    } else if (3..=15).contains(&sg_num) {
        let base_centered = [5, 8, 9, 12, 15];
        if base_centered.contains(&sg_num) {
            BravaisType::MonoclinicBaseCentered
        } else {
            BravaisType::MonoclinicPrimitive
        }
    } else if (1..=2).contains(&sg_num) {
        BravaisType::Triclinic
    } else {
        BravaisType::Unknown
    }
}

macro_rules! pts {
    ($($name:expr => [$x:expr, $y:expr, $z:expr]),* $(,)?) => {
        vec![
            $(
                HighSymmetryPoint {
                    label: $name.to_string(),
                    coord_frac: [$x, $y, $z],
                }
            ),*
        ]
    };
}

macro_rules! path {
    ($($($pt:expr),+);+ $(;)?) => {
        vec![
            $(
                vec![ $( $pt.to_string() ),+ ]
            ),+
        ]
    };
}

pub fn get_kpath(bravais_type: BravaisType, lat: &[[f64; 3]; 3]) -> KPath {
    let norm = |v: &[f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let dot = |u: &[f64; 3], v: &[f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];

    let b_len = norm(&lat[1]);
    let c_len = norm(&lat[2]);
    let alpha_cos = dot(&lat[1], &lat[2]) / (b_len * c_len);

    match bravais_type {
        BravaisType::CubicPrimitive => KPath {
            points: pts! {
                "Γ" => [0.0, 0.0, 0.0], "M" => [0.5, 0.5, 0.0],
                "R" => [0.5, 0.5, 0.5], "X" => [0.0, 0.5, 0.0],
            },
            path_segments: path! { "Γ", "X", "M", "Γ", "R", "X"; "M", "R" },
        },
        BravaisType::CubicFaceCentered => KPath {
            points: pts! {
                "Γ" => [0.0, 0.0, 0.0], "K" => [0.375, 0.375, 0.75],
                "L" => [0.5, 0.5, 0.5], "U" => [0.625, 0.25, 0.625],
                "W" => [0.5, 0.25, 0.75], "X" => [0.5, 0.0, 0.5],
            },
            path_segments: path! { "Γ", "X", "W", "K", "Γ", "L", "U", "W", "L", "K"; "U", "X" },
        },
        BravaisType::CubicBodyCentered => KPath {
            points: pts! {
                "Γ" => [0.0, 0.0, 0.0], "H" => [0.5, -0.5, 0.5],
                "P" => [0.25, 0.25, 0.25], "N" => [0.0, 0.0, 0.5],
            },
            path_segments: path! { "Γ", "H", "N", "Γ", "P", "H"; "P", "N" },
        },
        BravaisType::TetragonalPrimitive => KPath {
            points: pts! {
                "Γ" => [0.0, 0.0, 0.0], "A" => [0.5, 0.5, 0.5],
                "M" => [0.5, 0.5, 0.0], "R" => [0.0, 0.5, 0.5],
                "X" => [0.0, 0.5, 0.0], "Z" => [0.0, 0.0, 0.5],
            },
            path_segments: path! { "Γ", "X", "M", "Γ", "Z", "R", "A", "Z"; "X", "R"; "M", "A" },
        },
        BravaisType::TetragonalBodyCentered => KPath {
            points: pts! {
                "Γ" => [0.0, 0.0, 0.0], "N" => [0.0, 0.5, 0.0],
                "P" => [0.25, 0.25, 0.25], "X" => [0.0, 0.0, 0.5],
                "Z" => [-0.5, 0.5, 0.5],
            },
            path_segments: path! { "Γ", "X", "P", "N", "Γ", "Z" },
        },
        BravaisType::OrthorhombicPrimitive => KPath {
            points: pts! {
                "Γ" => [0.0, 0.0, 0.0], "R" => [0.5, 0.5, 0.5],
                "S" => [0.5, 0.5, 0.0], "T" => [0.0, 0.5, 0.5],
                "U" => [0.5, 0.0, 0.5], "X" => [0.5, 0.0, 0.0],
                "Y" => [0.0, 0.5, 0.0], "Z" => [0.0, 0.0, 0.5],
            },
            path_segments: path! { "Γ", "X", "S", "Y", "Γ", "Z", "U", "R", "T", "Z"; "Y", "T"; "U", "X"; "S", "R" },
        },
        BravaisType::OrthorhombicFaceCentered => KPath {
            points: pts! {
                "Γ" => [0.0, 0.0, 0.0], "A" => [0.5, 0.5, 0.0], 
                "T" => [1.0, 0.5, 0.5], "X" => [0.0, 0.5, 0.5],
                "Y" => [0.5, 0.0, 0.5], "Z" => [0.5, 0.5, 0.0],
            },
            path_segments: path! { "Γ", "Y", "T", "Z", "Γ", "X", "A", "Y"; "T", "X"; "X", "Z" },
        },
        BravaisType::OrthorhombicBodyCentered => KPath {
            points: pts! {
                "Γ" => [0.0, 0.0, 0.0], "L" => [0.5, 0.5, 0.5],
                "R" => [0.0, 0.5, 0.0], "S" => [0.5, 0.0, 0.0],
                "T" => [0.0, 0.0, 0.5], "W" => [0.25, 0.25, 0.25],
                "X" => [-0.5, 0.5, 0.5], "Y" => [0.5, -0.5, 0.5],
                "Z" => [0.5, 0.5, -0.5],
            },
            path_segments: path! { "Γ", "X", "L", "T", "W", "R", "X"; "Z", "Γ", "Y", "S", "W"; "L", "Y"; "Y", "Z" },
        },
        BravaisType::OrthorhombicBaseCentered => KPath {
            points: pts! {
                "Γ" => [0.0, 0.0, 0.0], "R" => [0.0, 0.5, 0.5],
                "S" => [0.0, 0.5, 0.0], "T" => [-0.5, 0.5, 0.5],
                "Y" => [0.5, 0.5, 0.0], "Z" => [0.0, 0.0, 0.5],
            },
            path_segments: path! { "Γ", "Y", "T", "Z", "Γ", "S", "R", "Z"; "T", "R"; "Y", "S" },
        },
        BravaisType::Hexagonal => KPath {
            points: pts! {
                "Γ" => [0.0, 0.0, 0.0], "A" => [0.0, 0.0, 0.5],
                "H" => [1.0/3.0, 1.0/3.0, 0.5], "K" => [1.0/3.0, 1.0/3.0, 0.0],
                "L" => [0.5, 0.0, 0.5], "M" => [0.5, 0.0, 0.0],
            },
            path_segments: path! { "Γ", "M", "K", "Γ", "A", "L", "H", "A"; "L", "M"; "K", "H" },
        },
        BravaisType::Rhombohedral => {
            if alpha_cos > 0.0 {
                // hR1
                let eta = (1.0 + 4.0 * alpha_cos) / (2.0 + 4.0 * alpha_cos);
                let nu = 0.75 - eta / 2.0;
                KPath {
                    points: pts! {
                        "Γ" => [0.0, 0.0, 0.0], "B" => [eta, 0.5, 1.0 - eta], "B_1" => [0.5, 1.0 - eta, eta - 1.0],
                        "F" => [0.5, 0.5, 0.0], "L" => [0.5, 0.0, 0.0], "L_1" => [0.0, 0.0, -0.5],
                        "P" => [eta, nu, nu], "P_1" => [1.0 - nu, 1.0 - nu, 1.0 - eta],
                        "P_2" => [nu, nu, eta], "Q" => [1.0 - nu, nu, 0.0], "X" => [nu, 0.0, -nu],
                        "Z" => [0.5, 0.5, 0.5],
                    },
                    path_segments: path! { "Γ", "L", "B_1"; "B", "Z", "Γ", "X", "Q", "F", "P_1", "Z"; "L", "P" },
                }
            } else {
                // hR2
                let eta = 1.0 / (2.0 * ((1.0 - alpha_cos) / (1.0 + alpha_cos)));
                let nu = 0.75 - eta / 2.0;
                KPath {
                    points: pts! {
                        "Γ" => [0.0, 0.0, 0.0], "F" => [0.5, -0.5, 0.0], "L" => [0.5, 0.0, 0.0],
                        "P" => [1.0 - nu, -nu, 1.0 - nu], "P_1" => [nu, nu - 1.0, nu - 1.0],
                        "Q" => [eta, eta, eta], "Q_1" => [1.0 - eta, -eta, -eta],
                        "Z" => [0.5, -0.5, 0.5],
                    },
                    path_segments: path! { "Γ", "P", "Z", "Q", "Γ", "F", "P_1", "Q_1", "L", "Z" },
                }
            }
        },
        BravaisType::MonoclinicPrimitive => KPath {
            points: pts! {
                "Γ" => [0.0, 0.0, 0.0], "A" => [0.5, 0.5, 0.0],
                "C" => [0.0, 0.5, 0.5], "D" => [0.5, 0.0, 0.5],
                "D1" => [0.5, 0.0, -0.5], "E" => [0.5, 0.5, 0.5],
                "Y" => [0.0, 0.5, 0.0], "Z" => [0.0, 0.0, 0.5],
            },
            // HACK: Full parametric SC2010 formulas for monoclinic deferred due to equation complexity
            path_segments: path! { "Γ", "Y", "C", "Z", "Γ" }, 
        },
        BravaisType::MonoclinicBaseCentered => KPath {
            points: pts! {
                "Γ" => [0.0, 0.0, 0.0], "A" => [0.5, 0.5, 0.0],
                "M" => [0.0, 0.5, 0.5], "V" => [0.5, 0.0, 0.5],
                "L" => [0.5, 0.5, 0.5], "Y" => [0.0, 0.5, 0.0],
            },
            // HACK: Full parametric SC2010 formulas for monoclinic deferred due to equation complexity
            path_segments: path! { "Γ", "Y", "M", "A", "Γ", "L", "V", "Γ" }, 
        },
        BravaisType::Triclinic => KPath {
            points: pts! {
                "Γ" => [0.0, 0.0, 0.0], "L" => [0.5, 0.5, 0.0],
                "M" => [0.0, 0.5, 0.5], "N" => [0.5, 0.0, 0.5],
                "R" => [0.5, 0.5, 0.5], "X" => [0.5, 0.0, 0.0],
                "Y" => [0.0, 0.5, 0.0], "Z" => [0.0, 0.0, 0.5],
            },
            path_segments: path! { "X", "Γ", "Y", "L", "Γ", "Z", "N", "Γ", "M", "R", "Γ" },
        },
        BravaisType::Unknown => KPath {
            points: pts! { "Γ" => [0.0, 0.0, 0.0] },
            path_segments: path! { "Γ" },
        }
    }
}
