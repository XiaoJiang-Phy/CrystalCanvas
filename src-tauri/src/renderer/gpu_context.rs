//! wgpu device/queue/surface initialization — Metal backend on macOS, Vulkan on Linux

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::sync::Arc;
use wgpu;

use super::render_config::RenderConfig;

/// Holds all wgpu GPU resources needed for rendering.
/// Created once at startup, lives for the application lifetime.
pub struct GpuContext {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    #[allow(dead_code)]
    pub render_config: RenderConfig,
    pub size: winit::dpi::PhysicalSize<u32>,
}

impl GpuContext {
    /// Initialize wgpu with the best available backend (Metal on macOS, Vulkan on Linux).
    /// Logs adapter info and device limits at startup.
    ///
    /// # Arguments
    /// * `window` - Arc-wrapped window implementing `HasWindowHandle` + `HasDisplayHandle`.
    /// * `width` - Initial surface width.
    /// * `height` - Initial surface height.
    pub fn new<W>(window: Arc<W>, width: u32, height: u32) -> Self
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        let size = winit::dpi::PhysicalSize::new(width, height);

        // Create wgpu instance with primary backends (Metal / Vulkan / DX12)
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        // Create surface from the window handle
        let surface = instance
            .create_surface(window)
            .expect("Failed to create wgpu surface");

        // Request the best available adapter (prefer high-performance GPU)
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find a suitable GPU adapter");

        // Log device capabilities
        let render_config = RenderConfig::from_adapter(&adapter);

        // Request device with default limits (sufficient for ≤1K atoms)
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("CrystalCanvas GPU Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .expect("Failed to create GPU device");

        // Configure the surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let alpha_mode = if surface_caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PostMultiplied) {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else if surface_caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            surface_caps.alpha_modes[0]
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        log::info!(
            "Surface configured: {}x{}, format {:?}",
            config.width,
            config.height,
            surface_format
        );

        Self {
            surface,
            device,
            queue,
            config,
            render_config,
            size,
        }
    }

    /// Reconfigure the surface after a window resize.
    /// Width and height are clamped to at least 1 to avoid wgpu validation errors.
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            log::debug!("Surface resized to {}x{}", new_size.width, new_size.height);
        }
    }

    /// Get the current surface texture format.
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Get the current aspect ratio (width / height).
    #[allow(dead_code)]
    pub fn aspect_ratio(&self) -> f32 {
        self.config.width as f32 / self.config.height as f32
    }
}
