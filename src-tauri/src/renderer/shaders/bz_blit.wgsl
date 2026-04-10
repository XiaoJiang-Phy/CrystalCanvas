// [Overview: Picture-in-Picture Blit Shader for Brillouin Zone]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Generate a fullscreen quad for the current viewport using 6 vertices
    var pos = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0)
    );
    let p = pos[in_vertex_index % 6u];
    
    // map xy from [-1, 1] to uv [0, 1]
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    out.clip_position = vec4<f32>(p, 0.0, 1.0);
    return out;
}

@group(0) @binding(0) var bz_texture: texture_2d<f32>;
@group(0) @binding(1) var bz_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(bz_texture, bz_sampler, in.uv);
    
    // Border
    let edge_dist = min(min(in.uv.x, 1.0 - in.uv.x), min(in.uv.y, 1.0 - in.uv.y));
    if edge_dist < 0.015 {
        return vec4<f32>(0.6, 0.6, 0.6, 1.0); // Border color
    }
    
    // Semi-transparent background pad if nothing is rendered
    if color.a < 0.1 {
        return vec4<f32>(0.15, 0.15, 0.15, 0.85); // Nice dark translucent pad
    }
    
    return color;
}
