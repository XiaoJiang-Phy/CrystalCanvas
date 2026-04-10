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
        BravaisType::TetragonalBodyCentered => {
            // Ref: SC2010 Table 6, requires a, c from conventional cell
            let a_len = norm(&lat[0]);
            let c_conv = c_len; // c of conventional = c of body-centered primitive

            if c_conv < a_len {
                // BCT1: $\eta = (1 + c^2/a^2)/4$
                let eta = (1.0 + c_conv * c_conv / (a_len * a_len)) / 4.0;
                KPath {
                    points: pts! {
                        "Γ" => [0.0, 0.0, 0.0], "M" => [-0.5, 0.5, 0.5],
                        "N" => [0.0, 0.5, 0.0], "P" => [0.25, 0.25, 0.25],
                        "X" => [0.0, 0.0, 0.5], "Z" => [eta, eta, -eta],
                        "Z_1" => [-eta, 1.0 - eta, eta],
                    },
                    path_segments: path! { "Γ", "X", "M", "Γ", "Z", "P", "N", "Z_1", "M"; "X", "P" },
                }
            } else {
                // BCT2: $\eta = (1 + a^2/c^2)/4$, $\zeta = a^2/(2c^2)$
                let eta = (1.0 + a_len * a_len / (c_conv * c_conv)) / 4.0;
                let zeta = a_len * a_len / (2.0 * c_conv * c_conv);
                KPath {
                    points: pts! {
                        "Γ" => [0.0, 0.0, 0.0], "N" => [0.0, 0.5, 0.0],
                        "P" => [0.25, 0.25, 0.25],
                        "Σ" => [-eta, eta, eta], "Σ_1" => [eta, 1.0 - eta, -eta],
                        "X" => [0.0, 0.0, 0.5],
                        "Y" => [-zeta, zeta, 0.5], "Y_1" => [0.5, 0.5, -zeta],
                        "Z" => [0.5, 0.5, -0.5],
                    },
                    path_segments: path! { "Γ", "X", "Y", "Σ", "Γ", "Z", "Σ_1", "N", "P", "Y_1", "Z"; "X", "P" },
                }
            }
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
        BravaisType::OrthorhombicFaceCentered => {
            // Ref: SC2010 Table 8-10 (ORCF1/2/3)
            let a_len = norm(&lat[0]);
            let inv_a2 = 1.0 / (a_len * a_len);
            let inv_b2 = 1.0 / (b_len * b_len);
            let inv_c2 = 1.0 / (c_len * c_len);

            if (inv_a2 - inv_b2 - inv_c2).abs() < 1e-10 {
                // ORCF3: $1/a^2 = 1/b^2 + 1/c^2$
                let zeta = (1.0 + a_len * a_len / (b_len * b_len) - a_len * a_len / (c_len * c_len)) / 4.0;
                let eta = (1.0 + a_len * a_len / (b_len * b_len) + a_len * a_len / (c_len * c_len)) / 4.0;
                KPath {
                    points: pts! {
                        "Γ" => [0.0, 0.0, 0.0], "A" => [0.5, 0.5 + zeta, zeta],
                        "A_1" => [0.5, 0.5 - zeta, 1.0 - zeta],
                        "L" => [0.5, 0.5, 0.5], "T" => [1.0, 0.5, 0.5],
                        "X" => [0.0, eta, eta], "X_1" => [1.0, 1.0 - eta, 1.0 - eta],
                        "Y" => [0.5, 0.0, 0.5], "Z" => [0.5, 0.5, 0.0],
                    },
                    path_segments: path! { "Γ", "Y", "T", "Z", "Γ", "X", "A_1", "Y"; "X", "A", "Z"; "L", "Γ" },
                }
            } else if inv_a2 > inv_b2 + inv_c2 {
                // ORCF1: $1/a^2 > 1/b^2 + 1/c^2$
                let zeta = (1.0 + a_len * a_len / (b_len * b_len) - a_len * a_len / (c_len * c_len)) / 4.0;
                let eta = (1.0 + a_len * a_len / (b_len * b_len) + a_len * a_len / (c_len * c_len)) / 4.0;
                KPath {
                    points: pts! {
                        "Γ" => [0.0, 0.0, 0.0], "A" => [0.5, 0.5 + zeta, zeta],
                        "A_1" => [0.5, 0.5 - zeta, 1.0 - zeta],
                        "L" => [0.5, 0.5, 0.5], "T" => [1.0, 0.5, 0.5],
                        "X" => [0.0, eta, eta], "X_1" => [1.0, 1.0 - eta, 1.0 - eta],
                        "Y" => [0.5, 0.0, 0.5], "Z" => [0.5, 0.5, 0.0],
                    },
                    path_segments: path! { "Γ", "Y", "T", "Z", "Γ", "X", "A_1", "Y"; "T", "X_1"; "X", "A", "Z"; "L", "Γ" },
                }
            } else {
                // ORCF2: $1/a^2 < 1/b^2 + 1/c^2$
                let eta = (1.0 + a_len * a_len / (b_len * b_len) - a_len * a_len / (c_len * c_len)) / 4.0;
                let delta = (1.0 + b_len * b_len / (a_len * a_len) - b_len * b_len / (c_len * c_len)) / 4.0;
                let phi = (1.0 + c_len * c_len / (b_len * b_len) - c_len * c_len / (a_len * a_len)) / 4.0;
                KPath {
                    points: pts! {
                        "Γ" => [0.0, 0.0, 0.0],
                        "C" => [0.5, 0.5 - eta, 1.0 - eta], "C_1" => [0.5, 0.5 + eta, eta],
                        "D" => [0.5 - delta, 0.5, 1.0 - delta], "D_1" => [0.5 + delta, 0.5, delta],
                        "L" => [0.5, 0.5, 0.5],
                        "H" => [1.0 - phi, 0.5 - phi, 0.5], "H_1" => [phi, 0.5 + phi, 0.5],
                        "X" => [0.0, 0.5, 0.5], "Y" => [0.5, 0.0, 0.5], "Z" => [0.5, 0.5, 0.0],
                    },
                    path_segments: path! { "Γ", "Y", "C", "D", "X", "Γ", "Z", "D_1", "H", "C"; "C_1", "Z"; "X", "H_1"; "H", "Y"; "L", "Γ" },
                }
            }
        },
        BravaisType::OrthorhombicBodyCentered => {
            // Ref: SC2010 Table 7 (ORCI)
            let a_len = norm(&lat[0]);
            let a2 = a_len * a_len;
            let b2 = b_len * b_len;
            let c2 = c_len * c_len;

            let zeta = (1.0 + a2 / c2) / 4.0;
            let eta = (1.0 + b2 / c2) / 4.0;
            let delta = (b2 - a2) / (4.0 * c2);
            let mu = (a2 + b2) / (4.0 * c2);
            KPath {
                points: pts! {
                    "Γ" => [0.0, 0.0, 0.0],
                    "L" => [-mu, mu, 0.5 - delta], "L_1" => [mu, -mu, 0.5 + delta],
                    "L_2" => [0.5 - delta, 0.5 + delta, -mu],
                    "R" => [0.0, 0.5, 0.0], "S" => [0.5, 0.0, 0.0], "T" => [0.0, 0.0, 0.5],
                    "W" => [0.25, 0.25, 0.25],
                    "X" => [-zeta, zeta, zeta], "X_1" => [zeta, 1.0 - zeta, -zeta],
                    "Y" => [eta, -eta, eta], "Y_1" => [1.0 - eta, eta, -eta],
                    "Z" => [0.5, 0.5, -0.5],
                },
                path_segments: path! { "Γ", "X", "L", "T", "W", "R", "X_1", "Z", "Γ", "Y", "S", "W"; "L_1", "Y"; "Y_1", "Z" },
            }
        },
        BravaisType::OrthorhombicBaseCentered => {
            // Ref: SC2010 Table 11 (ORCC), $\zeta = (1 + a^2/b^2)/4$
            let a_len = norm(&lat[0]);
            let zeta = (1.0 + a_len * a_len / (b_len * b_len)) / 4.0;
            KPath {
                points: pts! {
                    "Γ" => [0.0, 0.0, 0.0], "A" => [zeta, zeta, 0.5],
                    "A_1" => [-zeta, 1.0 - zeta, 0.5],
                    "R" => [0.0, 0.5, 0.5], "S" => [0.0, 0.5, 0.0],
                    "T" => [-0.5, 0.5, 0.5], "X" => [zeta, zeta, 0.0],
                    "X_1" => [-zeta, 1.0 - zeta, 0.0],
                    "Y" => [-0.5, 0.5, 0.0], "Z" => [0.0, 0.0, 0.5],
                },
                path_segments: path! { "Γ", "X", "S", "R", "A", "Z", "Γ", "Y", "X_1", "A_1", "T", "Y"; "Γ", "S" },
            }
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
        BravaisType::MonoclinicPrimitive => {
            // Ref: SC2010 Table 13 (MCLC simplified as mP)
            // Unique axis: b, non-right angle α (between b and c)
            let sin_alpha = (1.0 - alpha_cos * alpha_cos).sqrt();

            let eta = (1.0 - b_len * alpha_cos / c_len) / (2.0 * sin_alpha * sin_alpha);
            let nu  = 0.5 - eta * c_len * alpha_cos / b_len;
            KPath {
                points: pts! {
                    "Γ" => [0.0, 0.0, 0.0],
                    "A" => [0.5, 0.5, 0.0], "C" => [0.0, 0.5, 0.5],
                    "D" => [0.5, 0.0, 0.5], "D_1" => [0.5, 0.0, -0.5],
                    "E" => [0.5, 0.5, 0.5],
                    "H" => [0.0, eta, 1.0 - nu], "H_1" => [0.0, 1.0 - eta, nu],
                    "H_2" => [0.0, eta, -nu],
                    "M" => [0.5, eta, 1.0 - nu], "M_1" => [0.5, 1.0 - eta, nu],
                    "M_2" => [0.5, eta, -nu],
                    "X" => [0.0, 0.5, 0.0], "Y" => [0.0, 0.0, 0.5],
                    "Y_1" => [0.0, 0.0, -0.5],
                    "Z" => [0.5, 0.0, 0.0],
                },
                path_segments: path! { "Γ", "Z", "D", "C", "A", "E", "D_1", "Y_1", "Γ", "X", "H_1"; "M", "D"; "Z", "M"; "A", "M_1"; "X", "H" },
            }
        },
        BravaisType::MonoclinicBaseCentered => {
            // Ref: SC2010 Table 14 (MCLC)
            // Simplified parametric version for base-centered monoclinic
            let a_len = norm(&lat[0]);
            let sin_alpha = (1.0 - alpha_cos * alpha_cos).sqrt();

            let zeta = (2.0 - b_len * alpha_cos / c_len) / (4.0 * sin_alpha * sin_alpha);
            let eta = 0.5 + 2.0 * zeta * c_len * alpha_cos / b_len;
            let psi = 0.75 - a_len * a_len / (4.0 * b_len * b_len * sin_alpha * sin_alpha);
            let phi = psi + (0.75 - psi) * b_len * alpha_cos / c_len;
            KPath {
                points: pts! {
                    "Γ" => [0.0, 0.0, 0.0],
                    "N" => [0.5, 0.0, 0.0], "N_1" => [0.0, -0.5, 0.0],
                    "F" => [1.0 - zeta, 1.0 - zeta, 1.0 - eta],
                    "F_1" => [zeta, zeta, eta], "F_2" => [-zeta, -zeta, 1.0 - eta],
                    "I" => [phi, 1.0 - phi, 0.5], "I_1" => [1.0 - phi, phi - 1.0, 0.5],
                    "L" => [0.5, 0.5, 0.5],
                    "M" => [0.5, 0.0, 0.5],
                    "X" => [1.0 - psi, psi - 1.0, 0.0], "X_1" => [psi, 1.0 - psi, 0.0],
                    "X_2" => [psi - 1.0, -psi, 0.0],
                    "Y" => [0.5, 0.5, 0.0], "Y_1" => [-0.5, -0.5, 0.0],
                    "Z" => [0.0, 0.0, 0.5],
                },
                path_segments: path! { "Γ", "Y", "F", "L", "I"; "I_1", "Z", "Γ", "X", "X_1"; "N", "Γ", "M" },
            }
        },
        BravaisType::Triclinic => {
            // Ref: SC2010 Table 15
            // aP2: all reciprocal angles < 90°; aP1: otherwise
            // For simplicity, use aP2 path (more common for standardized cells)
            KPath {
                points: pts! {
                    "Γ" => [0.0, 0.0, 0.0], "L" => [0.5, 0.5, 0.0],
                    "M" => [0.0, 0.5, 0.5], "N" => [0.5, 0.0, 0.5],
                    "R" => [0.5, 0.5, 0.5], "X" => [0.5, 0.0, 0.0],
                    "Y" => [0.0, 0.5, 0.0], "Z" => [0.0, 0.0, 0.5],
                },
                path_segments: path! { "X", "Γ", "Y"; "L", "Γ", "Z"; "N", "Γ", "M"; "R", "Γ" },
            }
        },
        BravaisType::Unknown => KPath {
            points: pts! { "Γ" => [0.0, 0.0, 0.0] },
            path_segments: path! { "Γ" },
        }
    }
}
