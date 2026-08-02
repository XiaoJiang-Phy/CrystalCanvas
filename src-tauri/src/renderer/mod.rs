//! Rendering engine module — wgpu-based Impostor Sphere renderer with Metal/Vulkan backends
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod bz_renderer;
pub mod camera;
pub mod field_scene;
pub mod field_slice;
pub mod gpu_context;
pub mod instance;
pub mod pipeline;
pub mod publication_look;
pub mod ray_picking;
pub mod render_config;
#[allow(clippy::module_inception)]
pub mod renderer;

pub mod isosurface;
pub mod mc_lut;
pub mod volume_raycast;
