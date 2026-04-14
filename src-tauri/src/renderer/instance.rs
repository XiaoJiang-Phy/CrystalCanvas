//! Atom instance data for GPU instanced rendering — maps CrystalState to per-atom GPU buffers
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::utils::colors::get_jmol_color;
use bytemuck::{Pod, Zeroable};
use wgpu;

/// Per-atom instance data uploaded to the GPU.
/// Each atom is rendered as an Impostor Sphere billboard quad.
///
/// Memory layout must match the WGSL vertex shader input exactly.
/// Total size: 32 bytes per atom (position:12 + radius:4 + color:16).
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct AtomInstance {
    /// Cartesian position in world space (Å)
    pub position: [f32; 3],
    /// Display radius (Å)
    pub radius: f32,
    /// RGBA color
    pub color: [f32; 4],
}

impl AtomInstance {
    /// Vertex buffer layout descriptor for instanced rendering.
    /// Each instance contributes position, radius, and color to the shader.
    /// Step mode is Instance (not Vertex) — data advances per-instance.
    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        static ATTRIBUTES: &[wgpu::VertexAttribute] = &[
            // @location(0) position: vec3<f32>
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            // @location(1) radius: f32
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32,
            },
            // @location(2) color: vec4<f32>
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<AtomInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: ATTRIBUTES,
        }
    }
}

/// Vertex data for line rendering (unit cell edges, bonds).
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

/// Instance data for rendering thick bonds via instanced cylinders.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct BondInstance {
    pub start: [f32; 3],
    pub radius: f32,      // radius of the bond cylinder
    pub end: [f32; 3],
    pub _pad: f32,        // align to 16 bytes (vec4)
    pub color: [f32; 4],
}

impl BondInstance {
    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        static ATTRIBUTES: &[wgpu::VertexAttribute] = &[
            wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
            wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 16, shader_location: 2, format: wgpu::VertexFormat::Float32x3 },
            wgpu::VertexAttribute { offset: 32, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<BondInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: ATTRIBUTES,
        }
    }
}

impl LineVertex {
    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        static ATTRIBUTES: &[wgpu::VertexAttribute] = &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

/// Default element colors based on CPK convention.
/// Now delegates to the central Jmol table in utils::colors.
pub fn element_color(symbol: &str) -> [f32; 4] {
    get_jmol_color(symbol)
}

/// Empirical covalent radii in Å (Cordero et al., Dalton Trans., 2008).
pub fn covalent_radius(atomic_number: u8) -> f32 {
    // Covalent radii for Z=1..96 (index 0 is dummy for Z=0)
    #[rustfmt::skip]
    const COVALENT_RADII: [f32; 97] = [
        0.50, // 0: dummy
        0.31, // 1: H
        0.28, // 2: He
        1.28, // 3: Li
        0.96, // 4: Be
        0.84, // 5: B
        0.76, // 6: C
        0.71, // 7: N
        0.66, // 8: O
        0.57, // 9: F
        0.58, // 10: Ne
        1.66, // 11: Na
        1.41, // 12: Mg
        1.21, // 13: Al
        1.11, // 14: Si
        1.07, // 15: P
        1.05, // 16: S
        1.02, // 17: Cl
        1.06, // 18: Ar
        2.03, // 19: K
        1.76, // 20: Ca
        1.70, // 21: Sc
        1.36, // 22: Ti
        1.53, // 23: V
        1.39, // 24: Cr
        1.39, // 25: Mn (low spin)
        1.32, // 26: Fe (low spin)
        1.26, // 27: Co (low spin)
        1.24, // 28: Ni
        1.32, // 29: Cu
        1.22, // 30: Zn
        1.22, // 31: Ga
        1.20, // 32: Ge
        1.19, // 33: As
        1.20, // 34: Se
        1.20, // 35: Br
        1.16, // 36: Kr
        2.20, // 37: Rb
        1.95, // 38: Sr
        1.90, // 39: Y
        1.75, // 40: Zr
        1.64, // 41: Nb
        1.54, // 42: Mo
        1.47, // 43: Tc
        1.46, // 44: Ru
        1.42, // 45: Rh
        1.39, // 46: Pd
        1.45, // 47: Ag
        1.44, // 48: Cd
        1.42, // 49: In
        1.39, // 50: Sn
        1.39, // 51: Sb
        1.38, // 52: Te
        1.39, // 53: I
        1.40, // 54: Xe
        2.44, // 55: Cs
        2.15, // 56: Ba
        2.07, // 57: La
        2.04, // 58: Ce
        2.03, // 59: Pr
        2.01, // 60: Nd
        1.99, // 61: Pm
        1.98, // 62: Sm
        1.98, // 63: Eu
        1.96, // 64: Gd
        1.94, // 65: Tb
        1.92, // 66: Dy
        1.92, // 67: Ho
        1.89, // 68: Er
        1.90, // 69: Tm
        1.87, // 70: Yb
        1.87, // 71: Lu
        1.75, // 72: Hf
        1.70, // 73: Ta
        1.62, // 74: W
        1.51, // 75: Re
        1.44, // 76: Os
        1.41, // 77: Ir
        1.36, // 78: Pt
        1.36, // 79: Au
        1.32, // 80: Hg
        1.45, // 81: Tl
        1.46, // 82: Pb
        1.48, // 83: Bi
        1.40, // 84: Po
        1.50, // 85: At
        1.50, // 86: Rn
        2.60, // 87: Fr
        2.21, // 88: Ra
        2.15, // 89: Ac
        2.06, // 90: Th
        2.00, // 91: Pa
        1.96, // 92: U
        1.90, // 93: Np
        1.87, // 94: Pu
        1.80, // 95: Am
        1.69, // 96: Cm
    ];

    let idx = (atomic_number as usize).min(COVALENT_RADII.len() - 1);
    COVALENT_RADII[idx]
}

pub fn element_radius(atomic_number: u8, scale_factor: f32) -> f32 {
    let r = covalent_radius(atomic_number);
    // Map covalent radii [0.6, 2.0] into a narrower visual range [0.3, 0.45]
    (0.25 + r * 0.1) * scale_factor
}

/// Helper to identify typical transition metals and post-transition metals.
/// Used to prevent drawing artificial metal-metal bonds (e.g. Fe-Fe) in metallic/ionic grids.
pub fn is_metal(z: u8) -> bool {
    // Basic heuristic: Transition metals, Lanthanides, Actinides, and some post-transition.
    (z >= 3 && z <= 4) || (z >= 11 && z <= 13) ||
    (z >= 19 && z <= 31) || (z >= 37 && z <= 50) ||
    (z >= 55 && z <= 83) || (z >= 87 && z <= 103)
}

/// Build an array of `AtomInstance` from a `CrystalState`.
/// The state must have `cart_positions` populated (call `fractional_to_cartesian()` first).
pub fn build_instance_data(
    cart_positions: &[[f32; 3]],
    atomic_numbers: &[u8],
    element_symbols: &[String],
    settings: &crate::settings::AppSettings,
    selected_atoms: &[usize],
) -> Vec<AtomInstance> {
    let n = cart_positions.len();
    let mut instances = Vec::with_capacity(n);
    for i in 0..n {
        let mut color = element_color(&element_symbols[i]);
        if let Some(custom_color) = settings.custom_atom_colors.get(&element_symbols[i]) {
            color = *custom_color;
        }
        instances.push(AtomInstance {
            position: cart_positions[i],
            radius: {
                let mut r = element_radius(atomic_numbers[i], settings.atom_scale);
                if selected_atoms.contains(&i) {
                    r *= 1.2;
                }
                r
            },
            color: if selected_atoms.contains(&i) {
                // Highlight: mix with bright cyan
                [color[0] * 0.4, color[1] * 0.8 + 0.4, color[2] * 0.8 + 0.8, 1.0]
            } else {
                color
            },
        });
    }
    instances
}

/// Build test instances: atoms arranged in a 3D grid with varying elements.
/// Useful for the render_demo binary.
pub fn build_test_instances(
    count_x: usize,
    count_y: usize,
    count_z: usize,
    spacing: f32,
) -> Vec<AtomInstance> {
    let test_elements: &[u8] = &[11, 17, 8, 26, 29, 79, 14, 22]; // Na, Cl, O, Fe, Cu, Au, Si, Ti
    let mut instances = Vec::with_capacity(count_x * count_y * count_z);

    // Center the grid at origin
    let offset_x = (count_x as f32 - 1.0) * spacing * 0.5;
    let offset_y = (count_y as f32 - 1.0) * spacing * 0.5;
    let offset_z = (count_z as f32 - 1.0) * spacing * 0.5;

    for ix in 0..count_x {
        for iy in 0..count_y {
            for iz in 0..count_z {
                let idx = (ix + iy * count_x + iz * count_x * count_y) % test_elements.len();
                let elem = test_elements[idx];
                instances.push(AtomInstance {
                    position: [
                        ix as f32 * spacing - offset_x,
                        iy as f32 * spacing - offset_y,
                        iz as f32 * spacing - offset_z,
                    ],
                    radius: element_radius(elem, 1.0),
                    color: element_color(match elem {
                        11 => "Na",
                        17 => "Cl",
                        8 => "O",
                        26 => "Fe",
                        29 => "Cu",
                        79 => "Au",
                        14 => "Si",
                        22 => "Ti",
                        _ => "H",
                    }),
                });
            }
        }
    }
    instances
}

/// Build unit cell bounding box lines.
pub fn build_cell_lines(cs: &crate::crystal_state::CrystalState) -> Vec<LineVertex> {
    let mut lines = Vec::with_capacity(24);
    let color = [0.8, 0.8, 0.8, 0.8]; // Light gray

    let a = cs.cell_a;
    let b = cs.cell_b;
    let c = cs.cell_c;
    let alpha_rad = cs.cell_alpha.to_radians();
    let beta_rad = cs.cell_beta.to_radians();
    let gamma_rad = cs.cell_gamma.to_radians();

    let cos_alpha = alpha_rad.cos();
    let cos_beta = beta_rad.cos();
    let cos_gamma = gamma_rad.cos();
    let sin_gamma = gamma_rad.sin();

    let m00 = a;
    let m01 = b * cos_gamma;
    let m02 = c * cos_beta;
    let m11 = b * sin_gamma;
    if sin_gamma.abs() < 1e-6 {
        log::warn!(
            "build_cell_lines: sin_gamma is near zero, cell angles are likely invalid. a={} b={} c={} alpha={} beta={} gamma={}",
            a,
            b,
            c,
            cs.cell_alpha,
            cs.cell_beta,
            cs.cell_gamma
        );
        return Vec::new();
    }

    let m12 = c * (cos_alpha - cos_beta * cos_gamma) / sin_gamma;
    let m22 = c
        * ((1.0 - cos_alpha * cos_alpha - cos_beta * cos_beta - cos_gamma * cos_gamma
            + 2.0 * cos_alpha * cos_beta * cos_gamma)
            .max(0.0) // Defensive square root check
            .sqrt())
        / sin_gamma;

    let o = glam::Vec3::ZERO;
    let v_a = glam::Vec3::new(m00 as f32, 0.0, 0.0);
    let v_b = glam::Vec3::new(m01 as f32, m11 as f32, 0.0);
    let v_c = glam::Vec3::new(m02 as f32, m12 as f32, m22 as f32);

    let ab = v_a + v_b;
    let ac = v_a + v_c;
    let bc = v_b + v_c;
    let abc = v_a + v_b + v_c;

    let edges = [
        (o, v_a),
        (o, v_b),
        (o, v_c),
        (v_a, ab),
        (v_b, ab),
        (v_a, ac),
        (v_c, ac),
        (v_b, bc),
        (v_c, bc),
        (ab, abc),
        (ac, abc),
        (bc, abc),
    ];

    for (v1, v2) in edges {
        lines.push(LineVertex {
            position: v1.into(),
            color,
        });
        lines.push(LineVertex {
            position: v2.into(),
            color,
        });
    }

    lines
}

/// Build lines for Measurement overlays (distance, angles).
pub fn build_measurement_lines(cs: &crate::crystal_state::CrystalState) -> Vec<LineVertex> {
    let mut lines = Vec::new();
    let color = [1.0, 0.4, 0.0, 0.9]; // Orange, alpha=0.9 triggers stipple effect in shader

    let n_atoms = cs.cart_positions.len();

    for m in &cs.measurements {
        match m.kind {
            crate::crystal_state::MeasurementKind::Distance => {
                if m.indices.len() == 2 && m.indices.iter().all(|&i| i < n_atoms) {
                    let p1 = cs.cart_positions[m.indices[0]];
                    let p2 = cs.cart_positions[m.indices[1]];
                    lines.push(LineVertex { position: p1, color });
                    lines.push(LineVertex { position: p2, color });
                }
            }
            crate::crystal_state::MeasurementKind::Angle => {
                if m.indices.len() == 3 && m.indices.iter().all(|&i| i < n_atoms) {
                    // Lines from center (index 1) to both ends (index 0, index 2)
                    let p0 = cs.cart_positions[m.indices[0]];
                    let p1 = cs.cart_positions[m.indices[1]];
                    let p2 = cs.cart_positions[m.indices[2]];
                    lines.push(LineVertex { position: p1, color });
                    lines.push(LineVertex { position: p0, color });
                    lines.push(LineVertex { position: p1, color });
                    lines.push(LineVertex { position: p2, color });
                }
            }
            crate::crystal_state::MeasurementKind::Dihedral => {
                if m.indices.len() == 4 && m.indices.iter().all(|&i| i < n_atoms) {
                    // Lines connecting the four atoms in sequence
                    let p0 = cs.cart_positions[m.indices[0]];
                    let p1 = cs.cart_positions[m.indices[1]];
                    let p2 = cs.cart_positions[m.indices[2]];
                    let p3 = cs.cart_positions[m.indices[3]];
                    lines.push(LineVertex { position: p0, color });
                    lines.push(LineVertex { position: p1, color });
                    lines.push(LineVertex { position: p1, color });
                    lines.push(LineVertex { position: p2, color });
                    lines.push(LineVertex { position: p2, color });
                    lines.push(LineVertex { position: p3, color });
                }
            }
        }
    }
    
    lines
}

/// Build chemical bond instances based on distance, for thick cylinder rendering.
pub fn build_bond_instances(
    cs: &crate::crystal_state::CrystalState,
    settings: &crate::settings::AppSettings,
    selected_atoms: &[usize],
) -> Vec<BondInstance> {
    let n = cs.cart_positions.len();
    let mut instances = Vec::new();

    for i in 0..n {
        for j in (i + 1)..n {
            let p1 = glam::Vec3::from(cs.cart_positions[i]);
            let p2 = glam::Vec3::from(cs.cart_positions[j]);
            let dist = (p1 - p2).length();

            let r1 = covalent_radius(cs.atomic_numbers[i]);
            let r2 = covalent_radius(cs.atomic_numbers[j]);

            let max_bond_len = r1 + r2 + settings.bond_tolerance;

            // Only draw visual bonds if they are actually physically proximal in the standard cell.
            // This prevents ultra-long MIC wrap-around lines taking up the entire screen.
            if dist > 0.5 && dist < max_bond_len {
                // Heuristic to prevent arbitrary metal-metal bonds in ionic/ceramic views:
                // If both are typical transition metals and of the same element, they shouldn't form a bond.
                let is_metal_i = crate::renderer::instance::is_metal(cs.atomic_numbers[i]);
                if is_metal_i && cs.atomic_numbers[i] == cs.atomic_numbers[j] {
                    continue; // Skip identical metal-metal bonds like Fe-Fe
                }

                instances.push(BondInstance {
                    start: p1.into(),
                    radius: settings.bond_radius,
                    end: p2.into(),
                    _pad: 0.0,
                    color: {
                    if selected_atoms.contains(&i) || selected_atoms.contains(&j) {
                        [settings.bond_color[0] * 0.5, settings.bond_color[1] * 0.9 + 0.3, settings.bond_color[2] * 0.9 + 0.5, 1.0]
                    } else {
                        settings.bond_color
                    }
                },
                });
            }
        }
    }

    instances
}

/// Build instances for Wannier hoppings using the bond cylinder shader.
/// Google Material Design 500-level palette for per-orbital hopping colors.
/// Cycle modulo if num_wann exceeds palette size.
const ORBITAL_PALETTE: [[f32; 4]; 10] = [
    [0.259, 0.522, 0.957, 0.90], // Google Blue   #4285F4
    [0.918, 0.263, 0.208, 0.90], // Google Red    #EA4335
    [0.984, 0.737, 0.020, 0.90], // Google Yellow #FBBC05
    [0.204, 0.659, 0.325, 0.90], // Google Green  #34A853
    [0.671, 0.329, 0.804, 0.90], // Purple 500    #AB47BC
    [0.000, 0.737, 0.831, 0.90], // Cyan 500      #00BCD4
    [1.000, 0.341, 0.133, 0.90], // Deep Orange   #FF5722
    [0.247, 0.318, 0.710, 0.90], // Indigo 500    #3F51B5
    [0.545, 0.765, 0.290, 0.90], // Light Green   #8BC34A
    [0.914, 0.118, 0.388, 0.90], // Pink 500      #E91E63
];

pub fn build_hopping_instances(
    hoppings: &[crate::wannier::VisibleHopping],
    t_max: f64,
) -> Vec<BondInstance> {
    let mut instances = Vec::with_capacity(hoppings.len());
    let safe_t_max = if t_max.abs() < 1e-12 { 1.0 } else { t_max.abs() };

    for h in hoppings {
        let frac = (h.magnitude / safe_t_max).min(1.0) as f32;
        let radius = 0.02 + 0.06 * frac;
        let color = ORBITAL_PALETTE[h.orb_m % ORBITAL_PALETTE.len()];

        instances.push(BondInstance {
            start: h.start_cart,
            radius,
            end: h.end_cart,
            _pad: 0.0,
            color,
        });
    }

    instances
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wannier::VisibleHopping;

    #[test]
    fn test_hopping_instance_orbital_color() {
        let hoppings = vec![
            VisibleHopping {
                start_cart: [0.0, 0.0, 0.0],
                end_cart: [1.0, 0.0, 0.0],
                magnitude: 2.0,
                sign: 1.0,
                orb_m: 0,
                dest_atom: 1,
                r_vec: [1, 0, 0],
            },
            VisibleHopping {
                start_cart: [0.0, 0.0, 0.0],
                end_cart: [0.0, 1.0, 0.0],
                magnitude: 1.0,
                sign: -1.0,
                orb_m: 1,
                dest_atom: 0,
                r_vec: [0, 1, 0],
            },
        ];

        let instances = build_hopping_instances(&hoppings, 4.0);
        assert_eq!(instances.len(), 2);

        // orb_m=0 → Google Blue #4285F4
        assert_eq!(instances[0].color, ORBITAL_PALETTE[0]);
        // orb_m=1 → Google Red #EA4335
        assert_eq!(instances[1].color, ORBITAL_PALETTE[1]);

        // frac = 2.0/4.0 = 0.5 → radius 0.02 + 0.03 = 0.05
        assert!((instances[0].radius - 0.05).abs() < 1e-4);
    }
}
