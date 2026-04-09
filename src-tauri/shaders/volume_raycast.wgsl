// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> camera: CameraUniforms;

struct VolumeRaycastUniforms {
    lattice_a: vec4<f32>,
    lattice_b: vec4<f32>,
    lattice_c: vec4<f32>,
    inv_lattice_a: vec4<f32>,
    inv_lattice_b: vec4<f32>,
    inv_lattice_c: vec4<f32>,
    eye_pos: vec4<f32>,
    origin: vec4<f32>,
    grid_dims: vec4<u32>,
    transfer_range: vec2<f32>,
    opacity_scale: f32,
    step_size: f32,
    max_steps: u32,
    colormap_mode: u32,
    is_orthographic: u32,
    use_signed_mapping: u32,
    camera_forward: vec4<f32>,
    volume_clip_threshold: f32,
    volume_density_cutoff: f32,
    _pad1_b: f32,
    _pad1_c: f32,
}

@group(1) @binding(0) var<uniform> params: VolumeRaycastUniforms;
@group(1) @binding(1) var<storage, read> scalar_field: array<f32>;
@group(1) @binding(2) var depth_tex: texture_depth_2d;

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) frac_pos: vec3<f32>,
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    let world_pos = params.origin.xyz +
                    model.position.x * params.lattice_a.xyz +
                    model.position.y * params.lattice_b.xyz +
                    model.position.z * params.lattice_c.xyz;
                    
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_pos = world_pos;
    out.frac_pos = model.position;
    
    return out;
}

fn intersect_unit_cube(ray_origin: vec3<f32>, ray_dir: vec3<f32>) -> vec2<f32> {
    let inv_dir = 1.0 / ray_dir;
    
    let t0 = (vec3<f32>(0.0) - ray_origin) * inv_dir;
    let t1 = (vec3<f32>(1.0) - ray_origin) * inv_dir;
    
    let t_min = min(t0, t1);
    let t_max = max(t0, t1);
    
    let t_enter = max(max(t_min.x, t_min.y), max(t_min.z, 0.0));
    let t_exit = min(min(t_max.x, t_max.y), t_max.z);
    
    return vec2<f32>(t_enter, t_exit);
}

fn flat_idx(ix: u32, iy: u32, iz: u32) -> u32 {
    let nx = params.grid_dims.x;
    let ny = params.grid_dims.y;
    return ix + iy * nx + iz * nx * ny;
}

fn sample_field_index(ix: u32, iy: u32, iz: u32) -> f32 {
    let nx = params.grid_dims.x;
    let ny = params.grid_dims.y;
    let nz = params.grid_dims.z;
    let cx = clamp(ix, 0u, nx - 1u);
    let cy = clamp(iy, 0u, ny - 1u);
    let cz = clamp(iz, 0u, nz - 1u);
    return scalar_field[flat_idx(cx, cy, cz)];
}

fn sample_field_frac(frac: vec3<f32>) -> f32 {
    let nx = f32(params.grid_dims.x - 1u);
    let ny = f32(params.grid_dims.y - 1u);
    let nz = f32(params.grid_dims.z - 1u);
    
    let p = frac * vec3<f32>(nx, ny, nz);
    let p0 = vec3<u32>(floor(p));
    let p1 = p0 + vec3<u32>(1u, 1u, 1u);
    let f = fract(p);
    
    let v000 = sample_field_index(p0.x, p0.y, p0.z);
    let v100 = sample_field_index(p1.x, p0.y, p0.z);
    let v010 = sample_field_index(p0.x, p1.y, p0.z);
    let v110 = sample_field_index(p1.x, p1.y, p0.z);
    let v001 = sample_field_index(p0.x, p0.y, p1.z);
    let v101 = sample_field_index(p1.x, p0.y, p1.z);
    let v011 = sample_field_index(p0.x, p1.y, p1.z);
    let v111 = sample_field_index(p1.x, p1.y, p1.z);
    
    let c00 = mix(v000, v100, f.x);
    let c10 = mix(v010, v110, f.x);
    let c01 = mix(v001, v101, f.x);
    let c11 = mix(v011, v111, f.x);
    let c0 = mix(c00, c10, f.y);
    let c1 = mix(c01, c11, f.y);
    
    return mix(c0, c1, f.z);
}

fn colormap_viridis(t: f32) -> vec3<f32> {
    let c0 = vec3<f32>(0.2777273, 0.00540734, 0.33409981);
    let c1 = vec3<f32>(0.10509304, 0.59800696, 0.55836266);
    let c2 = vec3<f32>(0.99320573, 0.90615594, 0.143936);
    let s = smoothstep(0.0, 0.5, t);
    let c0_1 = mix(c0, c1, s);
    let s2 = smoothstep(0.5, 1.0, t);
    return mix(c0_1, c2, s2);
}

fn colormap_grayscale(t: f32) -> vec3<f32> {
    let v = 0.2 + 0.8 * t;
    return vec3<f32>(v, v, v);
}

fn colormap_inferno(t: f32) -> vec3<f32> {
    let c0 = vec3<f32>(0.0002, 0.0016, 0.0139);
    let c1 = vec3<f32>(0.8651, 0.3165, 0.2261);
    let c2 = vec3<f32>(0.9882, 0.9984, 0.6449);
    let s = smoothstep(0.0, 0.5, t);
    let m01 = mix(c0, c1, s);
    let s2 = smoothstep(0.5, 1.0, t);
    return mix(m01, c2, s2);
}

fn colormap_plasma(t: f32) -> vec3<f32> {
    let c0 = vec3<f32>(0.0504, 0.0298, 0.5280);
    let c1 = vec3<f32>(0.7981, 0.2239, 0.4471);
    let c2 = vec3<f32>(0.9400, 0.9752, 0.1313);
    let s = smoothstep(0.0, 0.5, t);
    let m01 = mix(c0, c1, s);
    let s2 = smoothstep(0.5, 1.0, t);
    return mix(m01, c2, s2);
}

// Diverging: blue → white → red, ideal for charge difference $\Delta\rho$
fn colormap_coolwarm(t: f32) -> vec3<f32> {
    let blue = vec3<f32>(0.2298, 0.2987, 0.7537);
    let white = vec3<f32>(0.9647, 0.9647, 0.9647);
    let red = vec3<f32>(0.7059, 0.0156, 0.1502);
    if t < 0.5 {
        return mix(blue, white, smoothstep(0.0, 0.5, t));
    } else {
        return mix(white, red, smoothstep(0.5, 1.0, t));
    }
}

fn colormap_hot(t: f32) -> vec3<f32> {
    let r = clamp(t * 2.5, 0.0, 1.0);
    let g = clamp((t - 0.4) * 2.5, 0.0, 1.0);
    let b = clamp((t - 0.7) * 3.33, 0.0, 1.0);
    return vec3<f32>(r, g, b);
}

fn colormap_magma(t: f32) -> vec3<f32> {
    let c0 = vec3<f32>(0.0015, 0.0005, 0.0139);
    let c1 = vec3<f32>(0.7107, 0.0221, 0.3264);
    let c2 = vec3<f32>(0.9873, 0.9913, 0.7494);
    let s = smoothstep(0.0, 0.5, t);
    let m01 = mix(c0, c1, s);
    let s2 = smoothstep(0.5, 1.0, t);
    return mix(m01, c2, s2);
}

fn colormap_cividis(t: f32) -> vec3<f32> {
    let c0 = vec3<f32>(0.0, 0.1262, 0.3015);
    let c1 = vec3<f32>(0.5529, 0.5529, 0.5059);
    let c2 = vec3<f32>(0.9955, 0.9110, 0.1459);
    let s = smoothstep(0.0, 0.5, t);
    let m01 = mix(c0, c1, s);
    let s2 = smoothstep(0.5, 1.0, t);
    return mix(m01, c2, s2);
}

fn colormap_turbo(t: f32) -> vec3<f32> {
    let c0 = vec3<f32>(0.1900, 0.0718, 0.2322);
    let c1 = vec3<f32>(0.1602, 0.7346, 0.9398);
    let c2 = vec3<f32>(0.9445, 0.8530, 0.1094);
    let c3 = vec3<f32>(0.4796, 0.0158, 0.0106);
    if t < 0.33 {
        return mix(c0, c1, smoothstep(0.0, 0.33, t));
    } else if t < 0.66 {
        return mix(c1, c2, smoothstep(0.33, 0.66, t));
    } else {
        return mix(c2, c3, smoothstep(0.66, 1.0, t));
    }
}

// Diverging: red → yellow → blue
fn colormap_rdylbu(t: f32) -> vec3<f32> {
    let red = vec3<f32>(0.6471, 0.0, 0.1490);
    let yellow = vec3<f32>(1.0, 1.0, 0.749);
    let blue = vec3<f32>(0.1922, 0.2118, 0.5843);
    if t < 0.5 {
        return mix(red, yellow, smoothstep(0.0, 0.5, t));
    } else {
        return mix(yellow, blue, smoothstep(0.5, 1.0, t));
    }
}

fn apply_transfer_function(value: f32) -> vec4<f32> {
    let abs_max = max(abs(params.transfer_range.x), abs(params.transfer_range.y));

    // ── Density cutoff: discard voxels below user-specified minimum |density| ──
    if params.volume_density_cutoff > 0.0 {
        let abs_val = abs(value);
        if abs_val < params.volume_density_cutoff {
            return vec4<f32>(0.0);
        }
    }

    // ── Soft-fade clipping: volume fades to transparent near isosurface boundary ──
    // When clip_threshold > 0 (Both mode), smoothly fade volume opacity
    // from full at 2× threshold to zero at 1× threshold.
    var clip_fade = 1.0;
    if params.volume_clip_threshold > 0.0 {
        let iso_t = params.volume_clip_threshold;
        let abs_val = abs(value);
        if abs_val < iso_t {
            return vec4<f32>(0.0); // below isosurface → fully transparent
        }
        // Soft transition zone: [iso_t, iso_t * 2.0]
        clip_fade = smoothstep(iso_t, iso_t * 2.0, abs_val);
    }

    // ── Signed mapping: [-max, +max] → [0, 1] (Wannier / orbital / Δρ) ──
    // Use $\sqrt{|v/v_{\max}|}$ to stretch small values away from the colormap
    // midpoint, ensuring positive/negative lobes are visually distinct even
    // for sequential colormaps (Viridis, Inferno, etc.).
    if params.use_signed_mapping == 1u {
        let normalized = clamp(value / max(abs_max, 1e-10), -1.0, 1.0);
        let magnitude = abs(normalized);
        if magnitude < 0.01 {
            return vec4<f32>(0.0); // near-zero → transparent
        }

        // sqrt perceptual stretch: positive → upper half [0.5, 1.0], negative → lower half [0.5, 0.0]
        let stretched = sqrt(magnitude);
        var t_signed: f32;
        if value > 0.0 {
            t_signed = 0.5 + 0.5 * stretched;
        } else {
            t_signed = 0.5 - 0.5 * stretched;
        }
        t_signed = clamp(t_signed, 0.0, 1.0);

        let t_smooth = smoothstep(0.01, 0.15, magnitude);

        var color: vec3<f32>;
        if params.colormap_mode == 1u {
            color = colormap_grayscale(t_signed);
        } else if params.colormap_mode == 2u {
            color = colormap_inferno(t_signed);
        } else if params.colormap_mode == 3u {
            color = colormap_plasma(t_signed);
        } else if params.colormap_mode == 4u {
            color = colormap_coolwarm(t_signed);
        } else if params.colormap_mode == 5u {
            color = colormap_hot(t_signed);
        } else if params.colormap_mode == 6u {
            color = colormap_magma(t_signed);
        } else if params.colormap_mode == 7u {
            color = colormap_cividis(t_signed);
        } else if params.colormap_mode == 8u {
            color = colormap_turbo(t_signed);
        } else if params.colormap_mode == 9u {
            color = colormap_rdylbu(t_signed);
        } else {
            color = colormap_viridis(t_signed);
        }
        let alpha = t_smooth * params.opacity_scale * clip_fade;
        return vec4<f32>(color, alpha);
    }

    // ── Unsigned mapping: abs(value)/max → [0, 1] (charge density) ───────
    let t = clamp(abs(value) / max(abs_max, 1e-10), 0.0, 1.0);

    if t < 0.05 {
        return vec4<f32>(0.0);
    }

    let t_smooth = smoothstep(0.05, 0.3, t);

    var color: vec3<f32>;
    if params.colormap_mode == 1u {
        color = colormap_grayscale(t);
    } else if params.colormap_mode == 2u {
        color = colormap_inferno(t);
    } else if params.colormap_mode == 3u {
        color = colormap_plasma(t);
    } else if params.colormap_mode == 4u {
        color = colormap_coolwarm(t);
    } else if params.colormap_mode == 5u {
        color = colormap_hot(t);
    } else if params.colormap_mode == 6u {
        color = colormap_magma(t);
    } else if params.colormap_mode == 7u {
        color = colormap_cividis(t);
    } else if params.colormap_mode == 8u {
        color = colormap_turbo(t);
    } else if params.colormap_mode == 9u {
        color = colormap_rdylbu(t);
    } else {
        color = colormap_viridis(t);
    }

    let alpha = t_smooth * params.opacity_scale * clip_fade;
    return vec4<f32>(color, alpha);
}

fn gradient_frac(frac: vec3<f32>) -> vec3<f32> {
    let nx = f32(params.grid_dims.x);
    let ny = f32(params.grid_dims.y);
    let nz = f32(params.grid_dims.z);
    
    let dx = vec3<f32>(1.0 / nx, 0.0, 0.0);
    let dy = vec3<f32>(0.0, 1.0 / ny, 0.0);
    let dz = vec3<f32>(0.0, 0.0, 1.0 / nz);
    
    let gx = sample_field_frac(frac + dx) - sample_field_frac(frac - dx);
    let gy = sample_field_frac(frac + dy) - sample_field_frac(frac - dy);
    let gz = sample_field_frac(frac + dz) - sample_field_frac(frac - dz);
    
    return vec3<f32>(gx, gy, gz);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.clip_position.xy);
    let opaque_depth = textureLoad(depth_tex, pixel, 0);
    
    var ray_dir_world: vec3<f32>;
    var ray_origin_shifted: vec3<f32>;
    
    if params.is_orthographic == 1u {
        // Orthographic: parallel rays, each pixel's world_pos is the ray origin
        ray_dir_world = normalize(params.camera_forward.xyz);
        ray_origin_shifted = in.world_pos - params.origin.xyz;
    } else {
        // Perspective: diverging rays from eye position
        ray_dir_world = normalize(in.world_pos - params.eye_pos.xyz);
        ray_origin_shifted = params.eye_pos.xyz - params.origin.xyz;
    }
    
    var ray_origin_frac: vec3<f32>;
    ray_origin_frac.x = dot(params.inv_lattice_a.xyz, ray_origin_shifted);
    ray_origin_frac.y = dot(params.inv_lattice_b.xyz, ray_origin_shifted);
    ray_origin_frac.z = dot(params.inv_lattice_c.xyz, ray_origin_shifted);
    
    var ray_dir_frac: vec3<f32>;
    ray_dir_frac.x = dot(params.inv_lattice_a.xyz, ray_dir_world);
    ray_dir_frac.y = dot(params.inv_lattice_b.xyz, ray_dir_world);
    ray_dir_frac.z = dot(params.inv_lattice_c.xyz, ray_dir_world);
    
    let t_bounds = intersect_unit_cube(ray_origin_frac, ray_dir_frac);
    let t_min = max(t_bounds.x, 0.0);
    let t_max = t_bounds.y;
    
    if t_min >= t_max {
        discard;
    }
    
    // ── Precompute depth early-out via linear clip-space evolution ────────
    let step_world = params.step_size;
    let step_frac = ray_dir_frac * step_world;
    
    // World-space step vector (constant per-frame)
    let world_step_vec = step_frac.x * params.lattice_a.xyz +
                         step_frac.y * params.lattice_b.xyz +
                         step_frac.z * params.lattice_c.xyz;
    
    var pos_frac = ray_origin_frac + ray_dir_frac * t_min;
    
    // Initial world position at t_min
    var world_pos = params.origin.xyz +
        pos_frac.x * params.lattice_a.xyz +
        pos_frac.y * params.lattice_b.xyz +
        pos_frac.z * params.lattice_c.xyz;
    
    // Clip-space evolution: precompute clip_pos and clip_delta
    // $\mathbf{c}(i) = \mathbf{c}_0 + i \cdot \Delta\mathbf{c}$ (linear in homogeneous coords)
    var clip_pos = camera.view_proj * vec4<f32>(world_pos, 1.0);
    let clip_delta = camera.view_proj * vec4<f32>(world_step_vec, 0.0);
    
    var accumulated_color = vec4<f32>(0.0);
    var t_current = t_min;
    
    for (var i = 0u; i < params.max_steps; i = i + 1u) {
        if t_current > t_max {
            break;
        }
        
        // Depth early-out: stop ray behind opaque geometry
        let step_depth = clip_pos.z / clip_pos.w;
        if step_depth >= opaque_depth {
            break;
        }
        
        let val = sample_field_frac(pos_frac);
        var sample_color = apply_transfer_function(val);
        
        if sample_color.a > 0.001 {
            let grad_frac = gradient_frac(pos_frac);
            var normal = vec3<f32>(
                params.inv_lattice_a.x * grad_frac.x + params.inv_lattice_b.x * grad_frac.y + params.inv_lattice_c.x * grad_frac.z,
                params.inv_lattice_a.y * grad_frac.x + params.inv_lattice_b.y * grad_frac.y + params.inv_lattice_c.y * grad_frac.z,
                params.inv_lattice_a.z * grad_frac.x + params.inv_lattice_b.z * grad_frac.y + params.inv_lattice_c.z * grad_frac.z
            );
            
            let len = length(normal);
            if len > 1e-6 {
                normal = normal / len;
                if dot(normal, ray_dir_world) > 0.0 {
                    normal = -normal;
                }
                
                let light_dir = normalize(vec3<f32>(0.3, 0.6, 0.8));
                let view_dir = -ray_dir_world;
                let half_vec = normalize(light_dir + view_dir);
                
                let diffuse = max(dot(normal, light_dir), 0.0);
                let specular = pow(max(dot(normal, half_vec), 0.0), 32.0);
                let ambient = 0.2;
                
                let light_intensity = ambient + 0.6 * diffuse + 0.2 * specular;
                sample_color = vec4<f32>(sample_color.rgb * light_intensity, sample_color.a);
            }
        
            // Front-to-back compositing (pre-multiplied alpha output)
            let a = clamp(sample_color.a * step_world, 0.0, 1.0);
            accumulated_color = accumulated_color + vec4<f32>(sample_color.rgb * a, a) * (1.0 - accumulated_color.a);
            
            if accumulated_color.a >= 0.95 {
                accumulated_color.a = 1.0;
                break;
            }
        }
        
        pos_frac = pos_frac + step_frac;
        clip_pos = clip_pos + clip_delta;
        t_current = t_current + step_world;
    }
    
    if accumulated_color.a < 0.01 {
        discard;
    }
    
    return accumulated_color;
}
