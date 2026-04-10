//! 2D K-Path Generator for CrystalCanvas
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::kpath::{HighSymmetryPoint, KPath};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BravaisType2D {
    Square,
    Hexagonal,
    Rectangular,
    CenteredRectangular,
    Oblique,
}

/// Returns true if the spacegroup is an orthorhombic base-centered group (C, A).
/// Ref: SC2010 Table 11 (ORCC) representing CenteredRectangular surface cases.
fn sg_is_centered_ortho(sg: i32) -> bool {
    let base_centered = [20, 21, 35, 36, 37, 38, 39, 40, 41, 63, 64, 65, 66, 67, 68];
    base_centered.contains(&sg)
}

/// Identify the 2D Bravais lattice from IN-PLANE lattice parameters.
/// 
/// Parameters `a`, `b`, and `gamma_deg` MUST be obtained from the projected 2D
/// primitive lattice (e.g., via `CrystalState::get_inplane_lattice`), NOT from
/// the 3D bulk parameters directly (unless the vacuum axis has been strictly decoupled).
pub fn identify_bravais_2d(a: f64, b: f64, gamma_deg: f64, spacegroup: i32) -> BravaisType2D {
    if a < 1e-12 || b < 1e-12 {
        return BravaisType2D::Oblique; // Degenerate cell protection
    }
    
    let a_b_diff = (a - b).abs() / a;
    let gamma_diff_90 = (gamma_deg - 90.0).abs();
    let gamma_diff_120 = (gamma_deg - 120.0).abs();
    let gamma_diff_60 = (gamma_deg - 60.0).abs();

    if a_b_diff < 0.01 && gamma_diff_90 < 1.0 {
        BravaisType2D::Square
    } else if a_b_diff < 0.01 && (gamma_diff_120 < 1.0 || gamma_diff_60 < 1.0) {
        BravaisType2D::Hexagonal
    } else if gamma_diff_90 < 1.0 {
        if sg_is_centered_ortho(spacegroup) {
            BravaisType2D::CenteredRectangular
        } else {
            BravaisType2D::Rectangular
        }
    } else {
        BravaisType2D::Oblique
    }
}

fn map_2d_to_3d(k1: f64, k2: f64, _vacuum_axis: usize) -> [f64; 3] {
    // recip_lattice is always ordered as [b1, b2, vacuum_unit],
    // so fractional coords are always (k1, k2, 0) in that basis.
    [k1, k2, 0.0]
}


macro_rules! pts2d {
    ($vac:expr, $($name:expr => [$x:expr, $y:expr]),* $(,)?) => {
        vec![
            $(
                HighSymmetryPoint {
                    label: $name.to_string(),
                    coord_frac: map_2d_to_3d($x, $y, $vac),
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

/// Retrieve the high-symmetry Brillouin zone path for a 2D system.
/// 
/// Parameters `a` and `b` MUST be the norms of the in-plane lattice vectors.
/// `vacuum_axis` identifies the out-of-plane dimension (0=x, 1=y, 2=z) where
/// out-of-plane components will safely project to 0.0.
pub fn get_kpath_2d(btype: BravaisType2D, a: f64, b: f64, vacuum_axis: usize) -> KPath {
    match btype {
        BravaisType2D::Square => KPath {
            points: pts2d! { vacuum_axis,
                "Γ" => [0.0, 0.0], "X" => [0.5, 0.0], "M" => [0.5, 0.5]
            },
            path_segments: path! { "Γ", "X", "M", "Γ" },
        },
        BravaisType2D::Hexagonal => KPath {
            points: pts2d! { vacuum_axis,
                "Γ" => [0.0, 0.0], "M" => [0.5, 0.0], "K" => [1.0/3.0, 1.0/3.0]
            },
            path_segments: path! { "Γ", "M", "K", "Γ" },
        },
        BravaisType2D::Rectangular => KPath {
            points: pts2d! { vacuum_axis,
                "Γ" => [0.0, 0.0], "X" => [0.5, 0.0], "S" => [0.5, 0.5], "Y" => [0.0, 0.5]
            },
            path_segments: path! { "Γ", "X", "S", "Y", "Γ" },
        },
        BravaisType2D::CenteredRectangular => {
            let zeta = (1.0 + (a * a) / (b * b)) / 4.0;
            KPath {
                points: pts2d! { vacuum_axis,
                    "Γ" => [0.0, 0.0], "X" => [0.5, 0.0], "S_0" => [zeta, zeta], "Y" => [0.0, 0.5]
                },
                path_segments: path! { "Γ", "X", "S_0", "Y", "Γ" },
            }
        },
        BravaisType2D::Oblique => KPath {
            points: pts2d! { vacuum_axis,
                "Γ" => [0.0, 0.0], "X" => [0.5, 0.0], "C" => [0.5, 0.5], "Y" => [0.0, 0.5]
            },
            path_segments: path! { "Γ", "X", "C", "Y", "Γ" },
        },
    }
}
