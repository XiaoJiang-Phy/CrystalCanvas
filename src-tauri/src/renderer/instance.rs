//! Atom instance data for GPU instanced rendering — maps CrystalState to per-atom GPU buffers

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

/// Default element colors based on CPK convention.
/// Returns RGBA as [f32; 4]. Alpha is always 1.0.
pub fn element_color(atomic_number: u8) -> [f32; 4] {
    match atomic_number {
        1 => [1.0, 1.0, 1.0, 1.0],       // H  — white
        6 => [0.3, 0.3, 0.3, 1.0],        // C  — dark gray
        7 => [0.2, 0.2, 0.9, 1.0],        // N  — blue
        8 => [0.9, 0.1, 0.1, 1.0],        // O  — red
        9 => [0.0, 0.9, 0.2, 1.0],        // F  — green
        11 => [0.6, 0.3, 0.9, 1.0],       // Na — purple
        12 => [0.0, 0.6, 0.0, 1.0],       // Mg — dark green
        13 => [0.7, 0.7, 0.8, 1.0],       // Al — silver
        14 => [0.5, 0.5, 0.6, 1.0],       // Si — gray
        15 => [0.9, 0.5, 0.0, 1.0],       // P  — orange
        16 => [0.9, 0.8, 0.0, 1.0],       // S  — yellow
        17 => [0.0, 0.9, 0.0, 1.0],       // Cl — green
        20 => [0.4, 0.8, 0.4, 1.0],       // Ca — light green
        22 => [0.6, 0.6, 0.7, 1.0],       // Ti — titanium gray
        26 => [0.7, 0.4, 0.1, 1.0],       // Fe — rust orange
        29 => [0.8, 0.5, 0.2, 1.0],       // Cu — copper
        30 => [0.5, 0.5, 0.7, 1.0],       // Zn — blue-gray
        79 => [0.9, 0.8, 0.0, 1.0],       // Au — gold
        _ => [0.6, 0.4, 0.7, 1.0],        // Default — lavender
    }
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
) -> Vec<AtomInstance> {
    let n = cart_positions.len();
    let mut instances = Vec::with_capacity(n);
    for i in 0..n {
        instances.push(AtomInstance {
            position: cart_positions[i],
            radius: element_radius(atomic_numbers[i]),
            color: element_color(atomic_numbers[i]),
        });
    }
    instances
}

/// Build test instances: atoms arranged in a 3D grid with varying elements.
/// Useful for the render_demo binary.
pub fn build_test_instances(count_x: usize, count_y: usize, count_z: usize, spacing: f32) -> Vec<AtomInstance> {
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
                    color: element_color(elem),
                });
            }
        }
    }
    instances
}
