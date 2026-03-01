//! CrystalCanvas Tauri application entry point

// Prevents additional console window on Windows in release
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::Arc;
use tauri::Manager;

use crystal_canvas::*;

// Prevent macOS linker from stripping CXX exception handling symbols
extern crate cxx;

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
            app.manage(std::sync::Mutex::new(crystal_state::CrystalState::default()));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::update_viewport_size,
            commands::set_camera_projection,
            commands::load_cif_file,
            commands::add_atom,
            commands::delete_atoms,
            commands::substitute_atoms,
            commands::preview_slab,
            commands::preview_supercell
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::MainEventsCleared = event {
                if let Some(renderer_mutex) =
                    app_handle.try_state::<std::sync::Mutex<renderer::renderer::Renderer>>()
                {
                    if let Ok(mut renderer) = renderer_mutex.try_lock() {
                        if let Err(e) = renderer.render() {
                            log::warn!("Render error: {:?}", e);
                        }
                    }
                }
            }
        });
}
