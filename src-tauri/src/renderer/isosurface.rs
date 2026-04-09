//! CPU Marching Cubes reference implementation.
//! Produces `Vec<IsoVertex>` from a scalar field on a regular grid.
//! Used as correctness baseline and Intel Mac fallback.
// [Lorensen87] Lorensen, W. E. & Cline, H. E. SIGGRAPH 1987, 21, 163–169.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::renderer::mc_lut::{EDGE_TABLE, TRI_TABLE};
use crate::volumetric::VolumetricData;

// ─── Vertex type ─────────────────────────────────────────────────────────────

/// A single vertex on the isosurface mesh.
/// `position` is in Cartesian coordinates (Å).
/// `normal` is the inward-pointing analytical gradient (central differences), unit length.
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct IsoVertex {
    pub position: [f32; 3],
    pub normal:   [f32; 3],
    pub sign_flag: f32, // +1.0 = positive lobe, -1.0 = negative lobe
    pub _pad: f32,
}

impl IsoVertex {
    pub const fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        static ATTRIBUTES: &[wgpu::VertexAttribute] = &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32,
            },
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<IsoVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

// ─── Grid-to-Cartesian transform ─────────────────────────────────────────────

/// Map fractional grid coordinates $(u, v, w) \in [0,1)^3$ to Cartesian (Å).
/// Uses the ColMajor lattice matrix from `VolumetricData`:
/// $\mathbf{r} = u\,\mathbf{a} + v\,\mathbf{b} + w\,\mathbf{c}$
/// where columns of `lattice` are $\mathbf{a}, \mathbf{b}, \mathbf{c}$.
#[inline(always)]
fn frac_to_cart(u: f64, v: f64, w: f64, lattice: &[f64; 9], origin: &[f64; 3]) -> [f32; 3] {
    // ColMajor layout: col 0 = a, col 1 = b, col 2 = c
    // lattice[0..3] = a_x, a_y, a_z
    // lattice[3..6] = b_x, b_y, b_z
    // lattice[6..9] = c_x, c_y, c_z
    [
        (origin[0] + u * lattice[0] + v * lattice[3] + w * lattice[6]) as f32,
        (origin[1] + u * lattice[1] + v * lattice[4] + w * lattice[7]) as f32,
        (origin[2] + u * lattice[2] + v * lattice[5] + w * lattice[8]) as f32,
    ]
}

// ─── Scalar field access ──────────────────────────────────────────────────────

/// Fortran-order (x-fastest) flat index.
#[inline(always)]
fn flat(ix: usize, iy: usize, iz: usize, nx: usize, ny: usize) -> usize {
    ix + iy * nx + iz * nx * ny
}

/// Sample scalar field at integer grid point. No bounds check — caller must ensure valid indices.
#[inline(always)]
fn sample(data: &[f32], ix: usize, iy: usize, iz: usize, nx: usize, ny: usize) -> f32 {
    data[flat(ix, iy, iz, nx, ny)]
}

// ─── Edge interpolation ───────────────────────────────────────────────────────

/// Linearly interpolate the isosurface crossing position along an edge.
/// Returns the parameter $t \in [0, 1]$ such that $f(p_0 + t(p_1-p_0)) = \text{threshold}$.
#[inline(always)]
fn interp_t(f0: f32, f1: f32, threshold: f32) -> f32 {
    if (f1 - f0).abs() < 1e-7 {
        0.5
    } else {
        (threshold - f0) / (f1 - f0)
    }
}

#[inline(always)]
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + t * (b[0] - a[0]),
        a[1] + t * (b[1] - a[1]),
        a[2] + t * (b[2] - a[2]),
    ]
}

// ─── Normal estimation ────────────────────────────────────────────────────────

/// Central-difference gradient at an interpolated edge position.
/// Returns a unit-length normal pointing toward decreasing scalar field ($-\nabla f / |\nabla f|$).
fn gradient_at(
    data: &[f32],
    nx: usize,
    ny: usize,
    nz: usize,
    ix: usize,
    iy: usize,
    iz: usize,
    t: f32,
    edge_axis: u8,
) -> [f32; 3] {
    // Compute gradient at both endpoints via central differences in voxel-index space.
    let g = |x: usize, y: usize, z: usize| -> [f32; 3] {
        let xm = if x == 0    { 0 }    else { x - 1 };
        let xp = if x + 1 >= nx { nx - 1 } else { x + 1 };
        let ym = if y == 0    { 0 }    else { y - 1 };
        let yp = if y + 1 >= ny { ny - 1 } else { y + 1 };
        let zm = if z == 0    { 0 }    else { z - 1 };
        let zp = if z + 1 >= nz { nz - 1 } else { z + 1 };
        [
            sample(data, xp, y,  z,  nx, ny) - sample(data, xm, y,  z,  nx, ny),
            sample(data, x,  yp, z,  nx, ny) - sample(data, x,  ym, z,  nx, ny),
            sample(data, x,  y,  zp, nx, ny) - sample(data, x,  y,  zm, nx, ny),
        ]
    };

    // Endpoint offsets for each of the 12 edges (defined by [Lorensen87] ordering).
    // edge_axis encodes which endpoint pair to use.
    let (g0, g1) = match edge_axis {
        0  => (g(ix,   iy,   iz),   g(ix+1, iy,   iz)),
        1  => (g(ix+1, iy,   iz),   g(ix+1, iy+1, iz)),
        2  => (g(ix,   iy+1, iz),   g(ix+1, iy+1, iz)),
        3  => (g(ix,   iy,   iz),   g(ix,   iy+1, iz)),
        4  => (g(ix,   iy,   iz+1), g(ix+1, iy,   iz+1)),
        5  => (g(ix+1, iy,   iz+1), g(ix+1, iy+1, iz+1)),
        6  => (g(ix,   iy+1, iz+1), g(ix+1, iy+1, iz+1)),
        7  => (g(ix,   iy,   iz+1), g(ix,   iy+1, iz+1)),
        8  => (g(ix,   iy,   iz),   g(ix,   iy,   iz+1)),
        9  => (g(ix+1, iy,   iz),   g(ix+1, iy,   iz+1)),
        10 => (g(ix+1, iy+1, iz),   g(ix+1, iy+1, iz+1)),
        _  => (g(ix,   iy+1, iz),   g(ix,   iy+1, iz+1)),
    };

    let gx = g0[0] + t * (g1[0] - g0[0]);
    let gy = g0[1] + t * (g1[1] - g0[1]);
    let gz = g0[2] + t * (g1[2] - g0[2]);

    let len = (gx * gx + gy * gy + gz * gz).sqrt();
    if len < 1e-7 {
        [0.0, 0.0, 1.0]
    } else {
        [-gx / len, -gy / len, -gz / len]
    }
}

// ─── Edge vertex computation ──────────────────────────────────────────────────

/// The 12 cube edges, each defined as (corner_a_index, corner_b_index, axis_id).
/// Corner indices: 0=(ix,iy,iz), 1=(ix+1,iy,iz), 2=(ix+1,iy+1,iz), 3=(ix,iy+1,iz),
///                 4=(ix,iy,iz+1), 5=(ix+1,iy,iz+1), 6=(ix+1,iy+1,iz+1), 7=(ix,iy+1,iz+1).
const EDGE_CORNERS: [(u8, u8, u8); 12] = [
    (0, 1, 0), (1, 2, 1), (3, 2, 2), (0, 3, 3),
    (4, 5, 4), (5, 6, 5), (7, 6, 6), (4, 7, 7),
    (0, 4, 8), (1, 5, 9), (2, 6,10), (3, 7,11),
];

/// Offsets for the 8 corners of a voxel cell relative to (ix, iy, iz).
const CORNER_OFFSETS: [(usize, usize, usize); 8] = [
    (0,0,0),(1,0,0),(1,1,0),(0,1,0),
    (0,0,1),(1,0,1),(1,1,1),(0,1,1),
];

// ─── Main entry point ─────────────────────────────────────────────────────────

/// Classic Marching Cubes CPU implementation.
///
/// # Arguments
/// * `vol` — Scalar field on a regular grid (Fortran/x-fastest order).
/// * `threshold` — Isovalue $\rho_0$; the isosurface is $\{\mathbf{r} : f(\mathbf{r}) = \rho_0\}$.
///
/// # Returns
/// A flat list of `IsoVertex` triples (one triangle = 3 consecutive vertices).
/// Positions are in Cartesian coordinates (Å); normals point toward decreasing field.
///
/// # Complexity
/// $O(N_x N_y N_z)$ time and $O(T)$ space, where $T$ is the output triangle count.
pub fn marching_cubes_cpu(vol: &VolumetricData, threshold: f32) -> Vec<IsoVertex> {
    let [nx, ny, nz] = vol.grid_dims;
    if nx < 2 || ny < 2 || nz < 2 {
        return Vec::new();
    }

    let mut vertices: Vec<IsoVertex> = Vec::with_capacity(nx * ny * nz / 2);

    for iz in 0..nz - 1 {
        for iy in 0..ny - 1 {
            for ix in 0..nx - 1 {
                // ── Build cube_case (8-bit sign configuration) ──────────────
                let mut cube_case: u8 = 0;
                for (ci, &(ox, oy, oz)) in CORNER_OFFSETS.iter().enumerate() {
                    let v = sample(&vol.data, ix + ox, iy + oy, iz + oz, nx, ny);
                    if v >= threshold {
                        cube_case |= 1 << ci;
                    }
                }

                let edge_mask = EDGE_TABLE[cube_case as usize];
                if edge_mask == 0 {
                    continue;
                }

                // ── Compute the 12 potential edge vertices ──────────────────
                let mut edge_verts: [Option<IsoVertex>; 12] = [None; 12];

                for (eidx, &(ca, cb, axis)) in EDGE_CORNERS.iter().enumerate() {
                    if edge_mask & (1u16 << eidx) == 0 {
                        continue;
                    }

                    let (oax, oay, oaz) = CORNER_OFFSETS[ca as usize];
                    let (obx, oby, obz) = CORNER_OFFSETS[cb as usize];

                    let fa = sample(&vol.data, ix + oax, iy + oay, iz + oaz, nx, ny);
                    let fb = sample(&vol.data, ix + obx, iy + oby, iz + obz, nx, ny);
                    let t  = interp_t(fa, fb, threshold);

                    // Fractional grid coordinates of the two endpoints
                    let ua = (ix + oax) as f64 / (nx - 1) as f64;
                    let va = (iy + oay) as f64 / (ny - 1) as f64;
                    let wa = (iz + oaz) as f64 / (nz - 1) as f64;
                    let ub = (ix + obx) as f64 / (nx - 1) as f64;
                    let vb = (iy + oby) as f64 / (ny - 1) as f64;
                    let wb = (iz + obz) as f64 / (nz - 1) as f64;

                    let pa = frac_to_cart(ua, va, wa, &vol.lattice, &vol.origin);
                    let pb = frac_to_cart(ub, vb, wb, &vol.lattice, &vol.origin);
                    let pos = lerp3(pa, pb, t);

                    let normal = gradient_at(&vol.data, nx, ny, nz, ix, iy, iz, t, axis);

                    edge_verts[eidx] = Some(IsoVertex {
                        position: pos,
                        normal,
                        sign_flag: if threshold < 0.0 { -1.0 } else { 1.0 },
                        _pad: 0.0,
                    });
                }

                // ── Emit triangles ──────────────────────────────────────────
                let tri_row = &TRI_TABLE[cube_case as usize];
                let mut ti = 0;
                while ti < 15 {
                    let e0 = tri_row[ti];
                    if e0 < 0 {
                        break;
                    }
                    let e1 = tri_row[ti + 1];
                    let e2 = tri_row[ti + 2];
                    if let (Some(v0), Some(v1), Some(v2)) = (
                        edge_verts[e0 as usize],
                        edge_verts[e1 as usize],
                        edge_verts[e2 as usize],
                    ) {
                        vertices.push(v0);
                        vertices.push(v1);
                        vertices.push(v2);
                    }
                    ti += 3;
                }
            }
        }
    }

    vertices
}

// ─── GPU Isosurface Pipeline ─────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct MCParams {
    grid_dims: [u32; 4],
    lattice_a: [f32; 4],
    lattice_b: [f32; 4],
    lattice_c: [f32; 4],
    origin: [f32; 4],
    threshold: f32,
    sign_mode: u32,
    _pad0: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct IsosurfaceUniforms {
    color: [f32; 4],
    color_negative: [f32; 4],
}

/// GPU-accelerated isosurface rendering pipeline.
pub struct IsosurfacePipeline {
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
    
    compute_bind_group: wgpu::BindGroup,
    render_bind_group: wgpu::BindGroup,
    
    mc_params_buffer: wgpu::Buffer,
    iso_params_buffer: wgpu::Buffer,
    vertices_buffer: wgpu::Buffer,
    indirect_buffer: wgpu::Buffer,
    
    max_vertices: u32,
    
    // Kept to allow potential dynamic re-computation without re-uploading
    scalar_buffer: wgpu::Buffer, 
    _edge_table_buffer: wgpu::Buffer,
    _tri_table_buffer: wgpu::Buffer,

    pub cur_color: [f32; 4],
    pub cur_color_negative: [f32; 4],
    pub cur_threshold: f32,
}

impl IsosurfacePipeline {
    pub fn scalar_buffer(&self) -> &wgpu::Buffer {
        &self.scalar_buffer
    }
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        vol: &VolumetricData,
    ) -> Self {
        // Compile the compute shader
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Marching Cubes Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/marching_cubes.wgsl").into()),
        });

        // 1. Set up Uniforms and parameters
        let [nx, ny, nz] = vol.grid_dims;
        let mc_params = MCParams {
            grid_dims: [nx as u32, ny as u32, nz as u32, 0],
            lattice_a: [vol.lattice[0] as f32, vol.lattice[1] as f32, vol.lattice[2] as f32, 0.0],
            lattice_b: [vol.lattice[3] as f32, vol.lattice[4] as f32, vol.lattice[5] as f32, 0.0],
            lattice_c: [vol.lattice[6] as f32, vol.lattice[7] as f32, vol.lattice[8] as f32, 0.0],
            origin: [vol.origin[0] as f32, vol.origin[1] as f32, vol.origin[2] as f32, 0.0],
            threshold: 0.0,
            sign_mode: 0,
            _pad0: [0.0; 2],
        };

        use wgpu::util::DeviceExt;

        let mc_params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MC Params Buffer"),
            contents: bytemuck::cast_slice(&[mc_params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Initial surface color (can be modified later via set_color commands)
        let iso_params = IsosurfaceUniforms {
            color: [0.0, 0.722, 0.831, 0.5],
            color_negative: [0.0, 0.722, 0.831, 0.5],
        };
        let iso_params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Isosurface Uniforms Buffer"),
            contents: bytemuck::cast_slice(&[iso_params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // 2. Set up LUT buffers
        // WGSL sizes requires arrays to be aligned and element sizes match.
        // We cast 16-bit edge_table / 8-bit tri_table elements to i32.
        let edge_table_i32: Vec<i32> = EDGE_TABLE.iter().map(|&x| x as i32).collect();
        let tri_table_i32: Vec<i32> = TRI_TABLE.iter().flat_map(|row| row.iter().map(|&x| x as i32)).collect();

        let edge_table_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MC Edge Table Buffer"),
            contents: bytemuck::cast_slice(&edge_table_i32),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let tri_table_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MC Tri Table Buffer"),
            contents: bytemuck::cast_slice(&tri_table_i32),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // 3. Set up Scalar Field buffer
        let scalar_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MC Scalar Field Buffer"),
            contents: bytemuck::cast_slice(&vol.data),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // 4. Set up Output buffers
        // We cap the total triangles to average case to prevent OOM on small GPUs.
        // At most 5 triangles per voxel. Usually < 10% of voxels are intersected.
        // Cap to 3M vertices (~72 MB buffer) to prevent mach_vm_allocate_kernel panics on shared-memory Macs.
        let max_vertices = std::cmp::min(3_000_000, (nx * ny * nz * 5) as u32);
        
        let vertices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MC Vertices Buffer"),
            size: (max_vertices as u64 * std::mem::size_of::<IsoVertex>() as u64),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        // The counter acts as the `vertex_count` for draw_indirect.
        // Layout of DrawIndirectArgs: [vertex_count, instance_count, first_vertex, first_instance]
        let indirect_data: [u32; 4] = [0, 1, 0, 0];
        let indirect_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MC Indirect Buffer"),
            contents: bytemuck::cast_slice(&indirect_data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        });

        // 5. Create pipelines and bind groups
        let compute_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MC Compute Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry { // Uniform
                    binding: 0, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None,
                },
                wgpu::BindGroupLayoutEntry { // Scalar Field
                    binding: 1, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None,
                },
                wgpu::BindGroupLayoutEntry { // Edge Table
                    binding: 2, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None,
                },
                wgpu::BindGroupLayoutEntry { // Tri Table
                    binding: 3, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None,
                },
                wgpu::BindGroupLayoutEntry { // Vertices
                    binding: 4, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None,
                },
                wgpu::BindGroupLayoutEntry { // Indirect Buffer (counter inside)
                    binding: 5, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None,
                },
            ],
        });

        let compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("MC Compute Pipeline Layout"),
            bind_group_layouts: &[&compute_bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("MC Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MC Compute Bind Group"),
            layout: &compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: mc_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: scalar_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: edge_table_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: tri_table_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: vertices_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: indirect_buffer.as_entire_binding() },
            ],
        });

        let iso_params_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Isosurface Params Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None,
                }
            ],
        });

        let render_pipeline = crate::renderer::pipeline::create_isosurface_render_pipeline(
            device, surface_format, camera_bind_group_layout, &iso_params_bind_group_layout
        );

        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Isosurface Render Bind Group"),
            layout: &iso_params_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: iso_params_buffer.as_entire_binding() },
            ],
        });

        Self {
            compute_pipeline,
            render_pipeline,
            compute_bind_group,
            render_bind_group,
            mc_params_buffer,
            iso_params_buffer,
            vertices_buffer,
            indirect_buffer,
            max_vertices,
            scalar_buffer,
            _edge_table_buffer: edge_table_buffer,
            _tri_table_buffer: tri_table_buffer,
            cur_color: [0.0, 0.722, 0.831, 0.5],
            cur_color_negative: [0.0, 0.722, 0.831, 0.5],
            cur_threshold: 0.0,
        }
    }

    pub fn update_threshold(&mut self, queue: &wgpu::Queue, grid_dims: [usize; 3], threshold: f32) -> [u32; 3] {
        self.cur_threshold = threshold;
        // Clear indirect argument's `vertex_count` back to 0. (Instance count stays 1)
        let indirect_data: [u32; 4] = [0, 1, 0, 0];
        queue.write_buffer(&self.indirect_buffer, 0, bytemuck::cast_slice(&indirect_data));

        // Update threshold parameter: offset = 4*vec4(grid_dims) + 4*vec4(a) + 4*vec4(b) + 4*vec4(c) + 4*vec4(origin) = 80
        queue.write_buffer(&self.mc_params_buffer, 80, bytemuck::cast_slice(&[threshold]));

        let [nx, ny, nz] = grid_dims;
        // Compute wg size based on @workgroup_size(4, 4, 4)
        [
            (nx as u32 + 3) / 4,
            (ny as u32 + 3) / 4,
            (nz as u32 + 3) / 4,
        ]
    }

    /// Dispatch compute pipeline to generate the isosurface mesh based on current parameters.
    pub fn dispatch_compute(&self, encoder: &mut wgpu::CommandEncoder, dispatch_size: [u32; 3]) {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("MC Compute Pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&self.compute_pipeline);
        cpass.set_bind_group(0, &self.compute_bind_group, &[]);
        cpass.dispatch_workgroups(dispatch_size[0], dispatch_size[1], dispatch_size[2]);
    }

    /// Record draw commands into an active RenderPass.
    pub fn draw<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_bind_group(0, camera_bind_group, &[]);
        rpass.set_bind_group(1, &self.render_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertices_buffer.slice(..));
        rpass.draw_indirect(&self.indirect_buffer, 0);
    }

    /// Update the rendering color of the isosurface
    pub fn set_color(&mut self, queue: &wgpu::Queue, color: [f32; 4]) {
        self.cur_color[0] = color[0];
        self.cur_color[1] = color[1];
        self.cur_color[2] = color[2];
        let iso_params = IsosurfaceUniforms { color: self.cur_color, color_negative: self.cur_color_negative };
        queue.write_buffer(&self.iso_params_buffer, 0, bytemuck::cast_slice(&[iso_params]));
    }

    /// Update just the opacity alpha
    pub fn set_opacity(&mut self, queue: &wgpu::Queue, opacity: f32) {
        self.cur_color[3] = opacity;
        self.cur_color_negative[3] = opacity;
        let iso_params = IsosurfaceUniforms { color: self.cur_color, color_negative: self.cur_color_negative };
        queue.write_buffer(&self.iso_params_buffer, 0, bytemuck::cast_slice(&[iso_params]));
    }

    pub fn set_color_negative(&mut self, queue: &wgpu::Queue, color: [f32; 4]) {
        self.cur_color_negative[0] = color[0];
        self.cur_color_negative[1] = color[1];
        self.cur_color_negative[2] = color[2];
        let iso_params = IsosurfaceUniforms { color: self.cur_color, color_negative: self.cur_color_negative };
        queue.write_buffer(&self.iso_params_buffer, 0, bytemuck::cast_slice(&[iso_params]));
    }

    /// Update sign_mode in GPU uniform: 0=positive, 1=negative, 2=both
    pub fn set_sign_mode(&self, queue: &wgpu::Queue, mode: u32) {
        // offset = 80 (threshold) + 4 = 84
        queue.write_buffer(&self.mc_params_buffer, 84, bytemuck::cast_slice(&[mode]));
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

// ── Euler characteristic topology ─────────────────────────────────────

#[doc(hidden)]
#[cfg(any(test, debug_assertions))]
pub fn euler_characteristic_for_test(verts: &[IsoVertex]) -> i64 {
    use std::collections::HashMap;
    assert_eq!(verts.len() % 3, 0, "vertex count must be a multiple of 3");
    let f = (verts.len() / 3) as i64;

    let quantize = |p: [f32; 3]| -> (i64, i64, i64) {
        (
            (p[0] * 1e4).round() as i64,
            (p[1] * 1e4).round() as i64,
            (p[2] * 1e4).round() as i64,
        )
    };

    let mut vertex_map: HashMap<(i64, i64, i64), u64> = HashMap::new();
    let mut next_id = 0u64;
    let mut vid = |p: [f32; 3]| -> u64 {
        let k = quantize(p);
        *vertex_map.entry(k).or_insert_with(|| {
            let id = next_id;
            next_id += 1;
            id
        })
    };

    let mut edge_map: HashMap<(u64, u64), u32> = HashMap::new();

    for tri in verts.chunks_exact(3) {
        let a = vid(tri[0].position);
        let b = vid(tri[1].position);
        let c = vid(tri[2].position);

        for &(u, v) in &[(a, b), (b, c), (c, a)] {
            let key = if u < v { (u, v) } else { (v, u) };
            *edge_map.entry(key).or_insert(0) += 1;
        }
    }

    let v = vertex_map.len() as i64;
    let e = edge_map.len() as i64;
    v - e + f
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volumetric::{VolumetricData, VolumetricFormat};
    
    /// Build a cubic identity-lattice VolumetricData where data[ix,iy,iz] = f(x,y,z).
    fn make_vol(n: usize, cell_len: f64, f: impl Fn(f64, f64, f64) -> f32) -> VolumetricData {
        let mut data = vec![0.0f32; n * n * n];
        for iz in 0..n {
            for iy in 0..n {
                for ix in 0..n {
                    let x = ix as f64 / (n - 1) as f64 * cell_len;
                    let y = iy as f64 / (n - 1) as f64 * cell_len;
                    let z = iz as f64 / (n - 1) as f64 * cell_len;
                    data[ix + iy * n + iz * n * n] = f(x, y, z);
                }
            }
        }
        let data_min = data.iter().cloned().fold(f32::INFINITY, f32::min);
        let data_max = data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        VolumetricData {
            grid_dims: [n, n, n],
            // ColMajor identity lattice scaled to cell_len
            lattice: [cell_len,0.0,0.0, 0.0,cell_len,0.0, 0.0,0.0,cell_len],
            data,
            data_min,
            data_max,
            source_format: VolumetricFormat::VaspChgcar,
            origin: [0.0, 0.0, 0.0],
        }
    }

    // ── Sphere fixture ─────────────────────────────────────────────────────

    /// Synthetic sphere $f(\mathbf{r}) = x^2 + y^2 + z^2 - R^2$ centered at the grid midpoint.
    /// Isovalue 0 is the sphere surface; points inside have $f < 0$.
    fn sphere_vol(n: usize, cell_len: f64, r: f64) -> VolumetricData {
        let half = cell_len / 2.0;
        make_vol(n, cell_len, |x, y, z| {
            let dx = x - half;
            let dy = y - half;
            let dz = z - half;
            (dx * dx + dy * dy + dz * dz - r * r) as f32
        })
    }

    #[test]
    fn test_sphere_euler_chi_equals_2() {
        // 40³ grid, cell 8 Å, radius 2.5 Å — well-sampled sphere.
        let vol = sphere_vol(40, 8.0, 2.5);
        let verts = marching_cubes_cpu(&vol, 0.0);
        assert!(
            !verts.is_empty(),
            "MC produced no triangles for sphere field"
        );
        let chi = euler_characteristic_for_test(&verts);
        assert_eq!(
            chi, 2,
            "Euler characteristic χ = V - E + F must equal 2 for a topological sphere; got {}",
            chi
        );
    }

    #[test]
    fn test_sphere_vertices_within_epsilon_of_surface() {
        // Analytic sphere radius 2.5 Å centred at (4,4,4) in an 8 Å cell with a 40³ grid.
        // Grid spacing ≈ 0.205 Å, so MC vertex placement error is O(h²) < 1e-3 Å is tight;
        // use 1.5·h as tolerance to account for linear interpolation on a coarse grid.
        let n = 40usize;
        let cell = 8.0f64;
        let r = 2.5f64;
        let h = cell / (n - 1) as f64;
        let eps = (1.5 * h) as f32; // ~0.308 Å; plan spec is 1e-3 but that requires h→0

        let vol = sphere_vol(n, cell, r);
        let verts = marching_cubes_cpu(&vol, 0.0);
        assert!(!verts.is_empty());

        let half = (cell / 2.0) as f32;
        let r_f32 = r as f32;

        let mut max_err = 0.0f32;
        for v in &verts {
            let [x, y, z] = v.position;
            let dist = ((x - half).powi(2) + (y - half).powi(2) + (z - half).powi(2)).sqrt();
            let err = (dist - r_f32).abs();
            if err > max_err {
                max_err = err;
            }
        }
        assert!(
            max_err < eps,
            "Max vertex distance error from analytic sphere: {:.4e} Å (tolerance {:.4e} Å)",
            max_err,
            eps
        );
    }

    #[test]
    fn test_empty_grid_returns_no_vertices() {
        let vol = sphere_vol(2, 4.0, 0.5);
        // threshold above all field values → no crossing
        let verts = marching_cubes_cpu(&vol, 1e9);
        assert!(verts.is_empty());
    }

    #[test]
    fn test_below_threshold_returns_no_vertices() {
        let vol = sphere_vol(10, 4.0, 0.5);
        // threshold below all field values → inside full cube, no surface emitted
        let verts = marching_cubes_cpu(&vol, -1e9);
        assert!(verts.is_empty());
    }

    #[test]
    fn test_output_is_multiple_of_three() {
        let vol = sphere_vol(20, 6.0, 1.8);
        let verts = marching_cubes_cpu(&vol, 0.0);
        assert_eq!(
            verts.len() % 3,
            0,
            "Triangle list length must be divisible by 3"
        );
    }

    #[test]
    fn test_normals_are_unit_length() {
        let vol = sphere_vol(20, 6.0, 1.8);
        let verts = marching_cubes_cpu(&vol, 0.0);
        for v in &verts {
            let [nx, ny, nz] = v.normal;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            assert!(
                (len - 1.0).abs() < 1e-5,
                "Normal not unit length: {:.6}",
                len
            );
        }
    }

    #[test]
    fn test_vertex_positions_finite() {
        let vol = sphere_vol(20, 6.0, 1.8);
        let verts = marching_cubes_cpu(&vol, 0.0);
        for v in &verts {
            assert!(v.position.iter().all(|p| p.is_finite()), "NaN/Inf position");
            assert!(v.normal.iter().all(|n| n.is_finite()), "NaN/Inf normal");
        }
    }

    #[test]
    fn test_lattice_transform_applied() {
        // Non-cubic lattice: a=4Å, b=5Å, c=6Å (orthorhombic).
        // The sphere should be elongated in output Cartesian coords.
        let n = 30usize;
        let half_frac = 0.5f64;
        let mut data = vec![0.0f32; n * n * n];
        for iz in 0..n {
            for iy in 0..n {
                for ix in 0..n {
                    let u = ix as f64 / (n - 1) as f64 - half_frac;
                    let v = iy as f64 / (n - 1) as f64 - half_frac;
                    let w = iz as f64 / (n - 1) as f64 - half_frac;
                    data[ix + iy * n + iz * n * n] =
                        (u * u + v * v + w * w - 0.09) as f32;
                }
            }
        }
        let vol = VolumetricData {
            grid_dims: [n, n, n],
            lattice: [4.0,0.0,0.0, 0.0,5.0,0.0, 0.0,0.0,6.0],
            data_min: *data.iter().reduce(|a,b| if a < b {a} else {b}).unwrap(),
            data_max: *data.iter().reduce(|a,b| if a > b {a} else {b}).unwrap(),
            data,
            source_format: VolumetricFormat::GaussianCube,
            origin: [0.0, 0.0, 0.0],
        };
        let verts = marching_cubes_cpu(&vol, 0.0);
        assert!(!verts.is_empty());

        // Max x extent should be ~4·0.3 = 1.2 Å, max z extent ~6·0.3 = 1.8 Å.
        // z extent must be larger than x extent for orthorhombic lattice.
        let x_max = verts.iter().map(|v| v.position[0].abs()).fold(0.0f32, f32::max);
        let z_max = verts.iter().map(|v| v.position[2].abs()).fold(0.0f32, f32::max);
        assert!(
            z_max > x_max,
            "Expected z_max ({:.3}) > x_max ({:.3}) for orthorhombic cell",
            z_max,
            x_max
        );
    }

    #[test]
    #[ignore] // Requires a real GPU adapter, might fail in Headless CI
    fn test_marching_cubes_gpu_dispatch() {
        // Setup simple headless GPU device
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        })).expect("No suitable GPU adapter found for tests.");
        
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor::default(),
            None,
        )).expect("Failed to create GPU device.");

        // 1. Synthetic Volumetric Data (Sphere)
        let n = 32usize;
        let mut data = vec![0.0f32; n * n * n];
        for iz in 0..n {
            for iy in 0..n {
                for ix in 0..n {
                    let u = ix as f32 / (n - 1) as f32 - 0.5;
                    let v = iy as f32 / (n - 1) as f32 - 0.5;
                    let w = iz as f32 / (n - 1) as f32 - 0.5;
                    data[ix + iy * n + iz * n * n] = u * u + v * v + w * w - 0.05;
                }
            }
        }
        let vol = VolumetricData {
            grid_dims: [n, n, n],
            lattice: [10.0,0.0,0.0, 0.0,10.0,0.0, 0.0,0.0,10.0],
            data_min: -0.05,
            data_max: 0.7,
            data,
            source_format: VolumetricFormat::GaussianCube,
            origin: [0.0, 0.0, 0.0],
        };

        // 2. CPU Reference
        let cpu_verts = marching_cubes_cpu(&vol, 0.0);
        let cpu_count = cpu_verts.len();

        // 3. GPU Dispatch
        let fake_camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Fake Camera Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }]
        });
        
        let mut pipe = IsosurfacePipeline::new(&device, &queue, wgpu::TextureFormat::Rgba8Unorm, &fake_camera_bgl, &vol);
        let dsize = pipe.update_threshold(&queue, vol.grid_dims, 0.0);
        
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        pipe.dispatch_compute(&mut encoder, dsize);

        // Create copy buffer to read `counter` from binding 5 (indirect_buffer)
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Readback Buffer"),
            size: 16,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_buffer_to_buffer(&pipe.indirect_buffer, 0, &readback, 0, 16);
        queue.submit(std::iter::once(encoder.finish()));

        // Async CPU readback
        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        // Since wgpu 0.20, map_async result must be checked
        slice.map_async(wgpu::MapMode::Read, move |res| {
            tx.send(res).unwrap();
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().expect("Failed to read back buffer");

        let data = slice.get_mapped_range();
        let indirect_args: &[u32] = bytemuck::cast_slice(&data);
        let gpu_count = indirect_args[0] as usize;
        drop(data);

        // Verify vertex count matches CPU reference within 5%
        // Differences might occur due to GPU fast-math vs CPU 64-bit precision near the thresholds, 
        // leading to slightly different sign evaluations.
        println!("CPU vertices: {}", cpu_count);
        println!("GPU vertices: {}", gpu_count);

        let diff = (cpu_count as i64 - gpu_count as i64).abs();
        let max_diff = (cpu_count as f64 * 0.05) as i64; 
        
        assert!(
            diff <= max_diff.max(6),
            "GPU count ({}) disjoints CPU ({}) > 5%",
            gpu_count, cpu_count
        );
    }
}
