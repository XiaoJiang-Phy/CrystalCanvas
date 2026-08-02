// [Overview: FIGURE-2 field-slice sampling shader with column-major grid-point interpolation.]
// Implementation: declaration-only slice sampling primitives shared by interactive and publication passes.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
};

// CPU realization rejects a non-invertible registered field mapping before a
// slice vertex buffer is created; this shader receives only validated Å-space vertices.

@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct SliceVertexIn {
    @location(0) position: vec3<f32>,
    @location(1) color_linear_rgba: vec4<f32>,
};

struct SliceVertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color_linear_rgba: vec4<f32>,
};

@vertex
fn field_slice_vertex(input: SliceVertexIn) -> SliceVertexOut {
    var out: SliceVertexOut;
    out.position = camera.view_proj * vec4<f32>(input.position, 1.0);
    out.color_linear_rgba = input.color_linear_rgba;
    return out;
}

@fragment
fn field_slice_fragment(input: SliceVertexOut) -> @location(0) vec4<f32> {
    return input.color_linear_rgba;
}
