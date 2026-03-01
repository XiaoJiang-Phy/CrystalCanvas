//! Standalone render demo — 500 Impostor Sphere atoms in a winit window with FPS counter
//!
//! Run: `cargo run --bin render_demo`
//! Controls: Close window to exit. Window is resizable.

use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::Vec3;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use crystal_canvas::renderer::instance::build_test_instances;
use crystal_canvas::renderer::renderer::Renderer;

/// Application state for the render demo.
struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    frame_count: u32,
    fps_timer: Instant,
    last_fps: f32,

    // Camera interaction state
    is_left_clicked: bool,
    last_cursor_pos: Option<(f64, f64)>,
    camera_yaw: f32,
    camera_pitch: f32,
    camera_distance: f32,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            renderer: None,
            frame_count: 0,
            fps_timer: Instant::now(),
            last_fps: 0.0,
            is_left_clicked: false,
            last_cursor_pos: None,
            camera_yaw: 0.0,
            camera_pitch: 0.3217, // ~10/30 atan
            camera_distance: 31.62,
        }
    }

    fn update_camera_eye(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            let limit = std::f32::consts::PI / 2.0 - 0.01;
            self.camera_pitch = self.camera_pitch.clamp(-limit, limit);

            let y = self.camera_distance * self.camera_pitch.sin();
            let xz = self.camera_distance * self.camera_pitch.cos();
            let x = xz * self.camera_yaw.sin();
            let z = xz * self.camera_yaw.cos();

            renderer.camera.eye = Vec3::new(x, y, z);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return; // Already initialized
        }

        let attrs = WindowAttributes::default()
            .with_title("CrystalCanvas Render Demo — loading...")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );

        let size = window.inner_size();
        let mut renderer = Renderer::new(window.clone(), size.width, size.height);

        // Build a ~500 atom test grid (8 x 8 x 8 = 512 atoms)
        let instances = build_test_instances(8, 8, 8, 3.0);
        log::info!("Demo: {} test atoms loaded", instances.len());
        renderer.update_atoms(&instances);

        self.renderer = Some(renderer);
        self.window = Some(window.clone());
        self.fps_timer = Instant::now();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                log::info!("Window close requested — exiting");
                event_loop.exit();
            }

            WindowEvent::Resized(new_size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(new_size);
                }
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.is_left_clicked = state == ElementState::Pressed;
                if !self.is_left_clicked {
                    self.last_cursor_pos = None;
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                if self.is_left_clicked {
                    if let Some((last_x, last_y)) = self.last_cursor_pos {
                        let dx = position.x - last_x;
                        let dy = position.y - last_y;
                        self.camera_yaw -= (dx as f32) * 0.01;
                        self.camera_pitch += (dy as f32) * 0.01;
                        self.update_camera_eye();
                    }
                    self.last_cursor_pos = Some((position.x, position.y));
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let y_delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 2.0,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.1,
                };
                self.camera_distance -= y_delta;
                if self.camera_distance < 1.0 {
                    self.camera_distance = 1.0;
                }
                self.update_camera_eye();
            }

            WindowEvent::RedrawRequested => {
                if let Some(renderer) = &mut self.renderer {
                    match renderer.render() {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::Lost) => {
                            let size = renderer.gpu.size;
                            renderer.resize(size);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            log::error!("GPU out of memory — exiting");
                            event_loop.exit();
                        }
                        Err(e) => {
                            log::warn!("Surface error: {:?}", e);
                        }
                    }

                    // FPS counter
                    self.frame_count += 1;
                    let elapsed = self.fps_timer.elapsed();
                    if elapsed >= Duration::from_secs(1) {
                        self.last_fps = self.frame_count as f32 / elapsed.as_secs_f32();
                        self.frame_count = 0;
                        self.fps_timer = Instant::now();

                        // Update window title with FPS
                        if let Some(renderer) = &self.renderer {
                            let gpu = &renderer.gpu;
                            let title = format!(
                                "CrystalCanvas Render Demo — {:.0} FPS | {}x{} | {}",
                                self.last_fps,
                                gpu.config.width,
                                gpu.config.height,
                                gpu.render_config.device_name,
                            );
                            if let Some(window) = &self.window {
                                window.set_title(&title);
                            }
                        }
                    }

                    // Request next frame
                    if let Some(renderer) = &self.renderer {
                        renderer.gpu.surface.get_current_texture().ok(); // keep alive
                    }
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Continuously request redraws for animation
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();
    log::info!("CrystalCanvas Render Demo starting...");

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::new();

    event_loop.run_app(&mut app).expect("Event loop error");
}
