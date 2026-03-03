// Bond Cylinder shader — purely geometric, built in VS from instanced line segments
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal_view: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) start: vec3<f32>,
    @location(1) radius_len: f32,
    @location(2) end: vec3<f32>,
    @location(3) color: vec4<f32>,
    @builtin(vertex_index) vi: u32,
) -> VertexOutput {
    // A cylinder with 12 segments (12 faces -> 24 triangles -> 72 vertices).
    let segs = 12u;
    let face = vi / 6u;
    let v_idx = vi % 6u;

    var a = f32(face);
    var b = f32(face + 1u);
    if (face == segs - 1u) { b = 0.0; }

    var angle0 = a * 6.2831853 / f32(segs);
    var angle1 = b * 6.2831853 / f32(segs);

    let axis = end - start;
    let up = normalize(axis);
    
    // Create an orthonormal basis (right, fw, up)
    var right = vec3<f32>(1.0, 0.0, 0.0);
    if (abs(up.x) > 0.99) {
        right = vec3<f32>(0.0, 1.0, 0.0);
    }
    let fw = normalize(cross(right, up));
    right = cross(up, fw);
    
    // Quad coordinates mapping
    var t_angle = angle0;
    var is_end = 0.0;

    if (v_idx == 0u) { t_angle = angle0; is_end = 0.0; }
    else if (v_idx == 1u) { t_angle = angle1; is_end = 0.0; }
    else if (v_idx == 2u) { t_angle = angle0; is_end = 1.0; }
    else if (v_idx == 3u) { t_angle = angle0; is_end = 1.0; }
    else if (v_idx == 4u) { t_angle = angle1; is_end = 0.0; }
    else if (v_idx == 5u) { t_angle = angle1; is_end = 1.0; }

    let local_p = right * cos(t_angle) * radius_len + fw * sin(t_angle) * radius_len;
    let pos_world = mix(start, end, is_end) + local_p;

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(pos_world, 1.0);
    
    let normal_world = normalize(local_p);
    // Transform normal into view space and zero the translation component
    out.normal_view = (camera.view * vec4<f32>(normal_world, 0.0)).xyz;
    out.color = color;
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.normal_view);
    let light_dir = normalize(vec3<f32>(0.3, 0.6, 0.8));
    
    let ambient = 0.15;
    let diff = max(dot(normal, light_dir), 0.0);
    let diffuse = 0.7 * diff;
    
    let view_dir = vec3<f32>(0.0, 0.0, 1.0);
    let half_dir = normalize(light_dir + view_dir);
    let spec = pow(max(dot(normal, half_dir), 0.0), 16.0);
    let specular = 0.3 * spec;
    
    let brightness = ambient + diffuse + specular;
    
    return vec4<f32>(in.color.rgb * brightness, in.color.a);
}
