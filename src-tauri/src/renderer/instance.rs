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

/// Default covalent radius in Å for display purposes.
/// Scaled down to 0.4× for visual clarity.
pub fn element_radius(atomic_number: u8) -> f32 {
    let covalent = match atomic_number {
        1 => 0.31,
        6 => 0.76,
        7 => 0.71,
        8 => 0.66,
        9 => 0.57,
        11 => 1.66,
        12 => 1.41,
        13 => 1.21,
        14 => 1.11,
        15 => 1.07,
        16 => 1.05,
        17 => 1.02,
        20 => 1.76,
        22 => 1.60,
        26 => 1.52,
        29 => 1.32,
        30 => 1.22,
        79 => 1.36,
        _ => 1.20,
    };
    covalent as f32 * 0.6 // scale for visual display
}

/// Build an array of `AtomInstance` from a `CrystalState`.
/// The state must have `cart_positions` populated (call `fractional_to_cartesian()` first).
pub fn build_instance_data(
    cart_positions: &[[f32; 3]],
    atomic_numbers: &[u8],
    element_symbols: &[String],
) -> Vec<AtomInstance> {
    let n = cart_positions.len();
    let mut instances = Vec::with_capacity(n);
    for i in 0..n {
        instances.push(AtomInstance {
            position: cart_positions[i],
            radius: element_radius(atomic_numbers[i]),
            color: element_color(&element_symbols[i]),
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
                    radius: element_radius(elem),
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
    let m12 = c * (cos_alpha - cos_beta * cos_gamma) / sin_gamma;
    let m22 = c
        * ((1.0 - cos_alpha * cos_alpha - cos_beta * cos_beta - cos_gamma * cos_gamma
            + 2.0 * cos_alpha * cos_beta * cos_gamma)
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
        (o, v_a), (o, v_b), (o, v_c),
        (v_a, ab), (v_b, ab),
        (v_a, ac), (v_c, ac),
        (v_b, bc), (v_c, bc),
        (ab, abc), (ac, abc), (bc, abc),
    ];

    for (v1, v2) in edges {
        lines.push(LineVertex { position: v1.into(), color });
        lines.push(LineVertex { position: v2.into(), color });
    }

    lines
}

/// Build chemical bond lines based on distance.
pub fn build_bond_lines(cs: &crate::crystal_state::CrystalState) -> Vec<LineVertex> {
    let n = cs.cart_positions.len();
    let mut lines = Vec::new();

    for i in 0..n {
        for j in (i + 1)..n {
            let p1 = glam::Vec3::from(cs.cart_positions[i].map(|x| x as f32));
            let p2 = glam::Vec3::from(cs.cart_positions[j].map(|x| x as f32));
            let dist = (p1 - p2).length();

            let r1 = element_radius(cs.atomic_numbers[i]);
            let r2 = element_radius(cs.atomic_numbers[j]);
            
            // Empirical bond threshold: sum of covalent radii * 1.3, minimum 0.5 (to avoid self-bonds or overlaps)
            let max_bond_len = (r1 + r2) / 0.6 * 1.3; // since elements are scaled down by 0.6 for visual radius

            if dist > 0.5 && dist < max_bond_len {
                let color = [0.5, 0.5, 0.5, 0.8]; // Gray bonds
                lines.push(LineVertex { position: p1.into(), color });
                lines.push(LineVertex { position: p2.into(), color });
            }
        }
    }
    
    lines
}
