// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> camera: CameraUniforms;

struct IsosurfaceUniforms {
    color: vec4<f32>,
    color_negative: vec4<f32>,
}
@group(1) @binding(0) var<uniform> iso_params: IsosurfaceUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) sign_flag: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_normal: vec3<f32>,
    @location(1) sign_flag: f32,
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
    out.view_normal = (camera.view * vec4<f32>(model.normal, 0.0)).xyz;
    out.sign_flag = model.sign_flag;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.view_normal);
    let is_front = normal.z > 0.0;
    let face_normal = select(-normal, normal, is_front);

    let light_dir = normalize(vec3<f32>(0.3, 0.6, 0.8));
    let view_dir = vec3<f32>(0.0, 0.0, 1.0);
    let half_dir = normalize(light_dir + view_dir);

    let diff = max(dot(face_normal, light_dir), 0.0);
    let spec = pow(max(dot(face_normal, half_dir), 0.0), 32.0);
    let NdotV = max(dot(face_normal, view_dir), 0.0);
    let fresnel = pow(1.0 - NdotV, 3.0);

    // ── VESTA-style two-sided shading ────────────────────────────────────
    var ambient: f32;
    var diff_w: f32;
    var spec_w: f32;
    var rim_w: f32;

    if is_front {
        ambient = 0.18;
        diff_w = 0.72;
        spec_w = 0.35;
        rim_w = 0.25 * fresnel;
    } else {
        ambient = 0.22;
        diff_w = 0.58;
        spec_w = 0.15;
        rim_w = 0.0;
    }

    let brightness = ambient + diff_w * diff + spec_w * spec + rim_w;

    // Dual-color: positive lobe uses color, negative lobe uses color_negative
    let base_color = select(iso_params.color, iso_params.color_negative, in.sign_flag < 0.0);
    let alpha = base_color.a;

    return vec4<f32>(base_color.rgb * brightness, alpha);
}
