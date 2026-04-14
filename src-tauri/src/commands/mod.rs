//! Domain-specific commands module
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod analysis;
pub mod editing;
pub mod file_io;
pub mod geometry;
pub mod reciprocal;
pub mod viewport;
pub mod volumetric;
pub mod wannier;

pub use analysis::*;
pub use editing::*;
pub use file_io::*;
pub use geometry::*;
pub use reciprocal::*;
pub use viewport::*;
pub use volumetric::*;
pub use wannier::*;

/// Managed state to store the "base" primitive/standard unit cell before supercell/slab expansions.
pub struct BaseCrystalState(pub std::sync::Mutex<Option<crate::crystal_state::CrystalState>>);

pub struct LlmState(pub std::sync::Mutex<Option<crate::llm::provider::ProviderConfig>>);

#[derive(serde::Serialize)]
pub struct VolumetricInfo {
    pub grid_dims: [usize; 3],
    pub data_min: f32,
    pub data_max: f32,
    pub format: String,
}
