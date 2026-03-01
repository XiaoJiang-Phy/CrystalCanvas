//! GPU adapter info and device limits — logged at startup per TDD §1.5
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use wgpu;

/// Runtime GPU configuration captured from the adapter at initialization.
/// Used to log device capabilities and enforce feature constraints.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Name of the GPU device (e.g. "Intel Iris Plus Graphics 640")
    pub device_name: String,
    /// Graphics backend name (e.g. "Metal", "Vulkan", "Dx12")
    pub backend_name: String,
    /// Device type (e.g. IntegratedGpu, DiscreteGpu)
    pub device_type: String,
    /// Maximum buffer size in bytes
    pub max_buffer_size: u64,
    /// Maximum 2D texture dimension (affects picking render target)
    pub max_texture_dimension_2d: u32,
    /// Maximum number of bind groups
    pub max_bind_groups: u32,
}

impl RenderConfig {
    /// Capture GPU configuration from a wgpu Adapter.
    /// Logs all relevant device info for diagnostics.
    pub fn from_adapter(adapter: &wgpu::Adapter) -> Self {
        let info = adapter.get_info();
        let limits = adapter.limits();

        let config = Self {
            device_name: info.name.clone(),
            backend_name: format!("{:?}", info.backend),
            device_type: format!("{:?}", info.device_type),
            max_buffer_size: limits.max_buffer_size,
            max_texture_dimension_2d: limits.max_texture_dimension_2d,
            max_bind_groups: limits.max_bind_groups,
        };

        // Log device baseline per TDD §1.5
        log::info!("=== GPU Device Baseline ===");
        log::info!("  Device:     {}", config.device_name);
        log::info!("  Backend:    {}", config.backend_name);
        log::info!("  Type:       {}", config.device_type);
        log::info!(
            "  Max buffer: {} MB",
            config.max_buffer_size / (1024 * 1024)
        );
        log::info!("  Max tex 2D: {}", config.max_texture_dimension_2d);
        log::info!("  Bind groups:{}", config.max_bind_groups);
        log::info!("===========================");

        config
    }
}
