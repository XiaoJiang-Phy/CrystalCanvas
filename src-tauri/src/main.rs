//! CrystalCanvas Tauri application entry point

//! CrystalCanvas Tauri application entry point

// Prevents additional console window on Windows in release
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::Arc;
use tauri::Manager;

mod crystal_state;
mod ffi;
mod renderer;
mod commands;

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .setup(|app| {
            // Get the main window. In Tauri 2.0, windows and webviews are separated.
            // We need to create a window, grab its handle, then optionally attach a webview.
            
            // For now, just ensure the app builds and runs.
            let window = app.get_webview_window("main").unwrap();
            
            // Note: Since Tauri 2.0 uses tao/wry under the hood, 
            // WebviewWindow implements HasWindowHandle and HasDisplayHandle.
            let arc_window = Arc::new(window);
            
            // 1. Initialize GPU Context & Renderer
            // The dimensions will be updated later by React via IPC, 
            // but we need an initial size (e.g., 1280x800).
            let mut renderer = renderer::renderer::Renderer::new(arc_window.clone(), 1280, 800);
            
            // Build a test grid (8 x 8 x 8 = 512 atoms) so we have something to look at immediately
            let instances = renderer::instance::build_test_instances(8, 8, 8, 3.0);
            renderer.update_atoms(&instances);

            // Store it in Tauri managed state so commands and the event loop can access it
            app.manage(std::sync::Mutex::new(renderer));
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::update_viewport_size,
            commands::set_camera_projection,
            commands::load_cif_file
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| match event {
            tauri::RunEvent::MainEventsCleared => {
                // Render the next frame
                if let Some(renderer_mutex) = app_handle.try_state::<std::sync::Mutex<renderer::renderer::Renderer>>() {
                    if let Ok(mut renderer) = renderer_mutex.try_lock() {
                        if let Err(e) = renderer.render() {
                            log::warn!("Render error: {:?}", e);
                        }
                    }
                }
            }
            _ => {}
        });
}
