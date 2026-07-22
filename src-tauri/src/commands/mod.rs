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

#[derive(Clone, Default)]
pub struct PhononFrameWake {
    generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl PhononFrameWake {
    const INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);

    pub fn start(&self, app: tauri::AppHandle) -> crate::ipc::IpcResult<()> {
        let generation = self
            .generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
            .wrapping_add(1);
        let wake_generation = std::sync::Arc::clone(&self.generation);
        std::thread::Builder::new()
            .name("phonon-frame-wake".into())
            .spawn(move || {
                while wake_generation.load(std::sync::atomic::Ordering::Acquire) == generation {
                    std::thread::sleep(Self::INTERVAL);
                    if wake_generation.load(std::sync::atomic::Ordering::Acquire) != generation {
                        break;
                    }
                    if app.run_on_main_thread(|| {}).is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| {
                crate::ipc::IpcError::render(format!(
                    "unable to start phonon frame wake: {error}"
                ))
            })?;
        Ok(())
    }

    pub fn stop(&self) {
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }
}

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
