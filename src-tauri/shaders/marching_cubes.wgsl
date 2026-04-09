// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

struct MCParams {
    grid_dims: vec4<u32>, // x,y,z = dims, w = unused
    lattice_a: vec4<f32>, // col 0
    lattice_b: vec4<f32>, // col 1
    lattice_c: vec4<f32>, // col 2
    origin: vec4<f32>,    // grid origin offset (Å)
    threshold: f32,
    sign_mode: u32,       // 0=positive, 1=negative, 2=both
    _pad0: f32,
    _pad1: f32,
}

@group(0) @binding(0) var<uniform> params: MCParams;
@group(0) @binding(1) var<storage, read> scalar_field: array<f32>;
@group(0) @binding(2) var<storage, read> edge_table: array<i32>;
@group(0) @binding(3) var<storage, read> tri_table: array<i32>;

struct IsoVertex {
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    norm_x: f32,
    norm_y: f32,
    norm_z: f32,
    sign_flag: f32,
    _pad: f32,
}

@group(0) @binding(4) var<storage, read_write> vertices: array<IsoVertex>;
@group(0) @binding(5) var<storage, read_write> counter: atomic<u32>;

const corner_offsets = array<vec3<u32>, 8>(
    vec3<u32>(0u,0u,0u), vec3<u32>(1u,0u,0u), vec3<u32>(1u,1u,0u), vec3<u32>(0u,1u,0u),
    vec3<u32>(0u,0u,1u), vec3<u32>(1u,0u,1u), vec3<u32>(1u,1u,1u), vec3<u32>(0u,1u,1u)
);

const edge_corners = array<vec3<u32>, 12>(
    vec3<u32>(0u, 1u, 0u), vec3<u32>(1u, 2u, 1u), vec3<u32>(3u, 2u, 2u), vec3<u32>(0u, 3u, 3u),
    vec3<u32>(4u, 5u, 4u), vec3<u32>(5u, 6u, 5u), vec3<u32>(7u, 6u, 6u), vec3<u32>(4u, 7u, 7u),
    vec3<u32>(0u, 4u, 8u), vec3<u32>(1u, 5u, 9u), vec3<u32>(2u, 6u, 10u), vec3<u32>(3u, 7u, 11u)
);

fn flat_idx(ix: u32, iy: u32, iz: u32) -> u32 {
    let nx = params.grid_dims.x;
    let ny = params.grid_dims.y;
    return ix + iy * nx + iz * nx * ny;
}

fn sample_field(ix: u32, iy: u32, iz: u32) -> f32 {
    return scalar_field[flat_idx(ix, iy, iz)];
}

fn interp_t(f0: f32, f1: f32, threshold: f32) -> f32 {
    if abs(f1 - f0) < 1e-7 {
        return 0.5;
    } else {
        return (threshold - f0) / (f1 - f0);
    }
}

fn frac_to_cart(u: f32, v: f32, w: f32) -> vec3<f32> {
    return params.origin.xyz + u * params.lattice_a.xyz + v * params.lattice_b.xyz + w * params.lattice_c.xyz;
}

fn grad(x: u32, y: u32, z: u32) -> vec3<f32> {
    let nx = params.grid_dims.x;
    let ny = params.grid_dims.y;
    let nz = params.grid_dims.z;

    let xm = select(x - 1u, 0u, x == 0u);
    let xp = select(x + 1u, nx - 1u, x + 1u >= nx);
    let ym = select(y - 1u, 0u, y == 0u);
    let yp = select(y + 1u, ny - 1u, y + 1u >= ny);
    let zm = select(z - 1u, 0u, z == 0u);
    let zp = select(z + 1u, nz - 1u, z + 1u >= nz);

    let dx = sample_field(xp, y, z) - sample_field(xm, y, z);
    let dy = sample_field(x, yp, z) - sample_field(x, ym, z);
    let dz = sample_field(x, y, zp) - sample_field(x, y, zm);

    return vec3<f32>(dx, dy, dz);
}

fn gradient_at(ix: u32, iy: u32, iz: u32, t: f32, edge_axis: u32) -> vec3<f32> {
    var g0: vec3<f32>;
    var g1: vec3<f32>;

    switch edge_axis {
        case 0u: { g0 = grad(ix, iy, iz);     g1 = grad(ix+1u, iy, iz); }
        case 1u: { g0 = grad(ix+1u, iy, iz);  g1 = grad(ix+1u, iy+1u, iz); }
        case 2u: { g0 = grad(ix, iy+1u, iz);  g1 = grad(ix+1u, iy+1u, iz); }
        case 3u: { g0 = grad(ix, iy, iz);     g1 = grad(ix, iy+1u, iz); }
        case 4u: { g0 = grad(ix, iy, iz+1u);  g1 = grad(ix+1u, iy, iz+1u); }
        case 5u: { g0 = grad(ix+1u, iy, iz+1u); g1 = grad(ix+1u, iy+1u, iz+1u); }
        case 6u: { g0 = grad(ix, iy+1u, iz+1u); g1 = grad(ix+1u, iy+1u, iz+1u); }
        case 7u: { g0 = grad(ix, iy, iz+1u);  g1 = grad(ix, iy+1u, iz+1u); }
        case 8u: { g0 = grad(ix, iy, iz);     g1 = grad(ix, iy, iz+1u); }
        case 9u: { g0 = grad(ix+1u, iy, iz);  g1 = grad(ix+1u, iy, iz+1u); }
        case 10u:{ g0 = grad(ix+1u, iy+1u, iz); g1 = grad(ix+1u, iy+1u, iz+1u); }
        case 11u:{ g0 = grad(ix, iy+1u, iz);  g1 = grad(ix, iy+1u, iz+1u); }
        default: { g0 = vec3<f32>(0.0); g1 = vec3<f32>(0.0); }
    }

    let g = mix(g0, g1, t);
    let len = length(g);
    if len < 1e-7 {
        return vec3<f32>(0.0, 0.0, 1.0);
    } else {
        return -g / len;
    }
}

@compute @workgroup_size(4, 4, 4)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let nx = params.grid_dims.x;
    let ny = params.grid_dims.y;
    let nz = params.grid_dims.z;

    let ix = global_id.x;
    let iy = global_id.y;
    let iz = global_id.z;

    if ix >= nx - 1u || iy >= ny - 1u || iz >= nz - 1u {
        return;
    }

    // Process voxel
    var cube_case: u32 = 0u;
    for (var i = 0u; i < 8u; i += 1u) {
        let offset = corner_offsets[i];
        let val = sample_field(ix + offset.x, iy + offset.y, iz + offset.z);
        var inside = false;
        if params.sign_mode == 0u {
            inside = val >= params.threshold;
        } else if params.sign_mode == 1u {
            inside = val <= -params.threshold;
        } else {
            inside = abs(val) >= params.threshold;
        }
        if inside {
            cube_case |= (1u << i);
        }
    }

    let edge_mask = u32(edge_table[cube_case]);
    if edge_mask == 0u {
        return;
    }

    var edge_pos: array<vec3<f32>, 12>;
    var edge_norm: array<vec3<f32>, 12>;
    var edge_sign: array<f32, 12>;

    for (var eidx = 0u; eidx < 12u; eidx += 1u) {
        if ((edge_mask & (1u << eidx)) != 0u) {
            let edge = edge_corners[eidx];
            let ca = edge.x;
            let cb = edge.y;
            let axis = edge.z;

            let oa = corner_offsets[ca];
            let ob = corner_offsets[cb];

            let fa = sample_field(ix + oa.x, iy + oa.y, iz + oa.z);
            let fb = sample_field(ix + ob.x, iy + ob.y, iz + ob.z);

            var eff_threshold = params.threshold;
            if params.sign_mode == 1u {
                eff_threshold = -params.threshold;
            } else if params.sign_mode == 2u {
                let avg = (fa + fb) * 0.5;
                eff_threshold = select(params.threshold, -params.threshold, avg < 0.0);
            }
            let t = interp_t(fa, fb, eff_threshold);

            let ua = f32(ix + oa.x) / f32(nx - 1u);
            let va = f32(iy + oa.y) / f32(ny - 1u);
            let wa = f32(iz + oa.z) / f32(nz - 1u);

            let ub = f32(ix + ob.x) / f32(nx - 1u);
            let vb = f32(iy + ob.y) / f32(ny - 1u);
            let wb = f32(iz + ob.z) / f32(nz - 1u);

            let pa = frac_to_cart(ua, va, wa);
            let pb = frac_to_cart(ub, vb, wb);
            let pos = mix(pa, pb, t);

            let norm = gradient_at(ix, iy, iz, t, axis);

            edge_pos[eidx] = pos;
            edge_norm[eidx] = norm;
            edge_sign[eidx] = select(1.0, -1.0, eff_threshold < 0.0);
        }
    }

    var ti = 0u;
    while ti < 15u {
        let e0i = tri_table[cube_case * 16u + ti];
        if e0i < 0 {
            break;
        }
        let e1i = tri_table[cube_case * 16u + ti + 1u];
        let e2i = tri_table[cube_case * 16u + ti + 2u];
        
        ti += 3u;

        let start_idx = atomicAdd(&counter, 3u);
        let max_vertices = arrayLength(&vertices);
        if start_idx + 2u >= max_vertices {
            return; // Out of Bounds safety net! Discard remaining writes.
        }
        
        let e0 = u32(e0i);
        let e1 = u32(e1i);
        let e2 = u32(e2i);

        let p0 = edge_pos[e0];
        let n0 = edge_norm[e0];
        let s0 = edge_sign[e0];
        vertices[start_idx] = IsoVertex(p0.x, p0.y, p0.z, n0.x, n0.y, n0.z, s0, 0.0);

        let p1 = edge_pos[e1];
        let n1 = edge_norm[e1];
        let s1 = edge_sign[e1];
        vertices[start_idx + 1u] = IsoVertex(p1.x, p1.y, p1.z, n1.x, n1.y, n1.z, s1, 0.0);

        let p2 = edge_pos[e2];
        let n2 = edge_norm[e2];
        let s2 = edge_sign[e2];
        vertices[start_idx + 2u] = IsoVertex(p2.x, p2.y, p2.z, n2.x, n2.y, n2.z, s2, 0.0);
    }
}
