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

fn build_menu(app: &mut tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
    let quit = PredefinedMenuItem::quit(app, None::<&str>)?;
    let app_menu = Submenu::with_items(app, "CrystalCanvas", true, &[&quit])?;
    let import = MenuItem::with_id(app, "menu_import_cif", "Import CIF...", true, None::<&str>)?;
    let export_poscar = MenuItem::with_id(app, "menu_export_poscar", "Export Poscar...", true, None::<&str>)?;
    let export_qe = MenuItem::with_id(app, "menu_export_qe", "Export QE...", true, None::<&str>)?;
    let export_lammps = MenuItem::with_id(app, "menu_export_lammps", "Export LAMMPS...", true, None::<&str>)?;
    let file_menu = Submenu::with_items(app, "File", true, &[&import, &export_poscar, &export_qe, &export_lammps])?;
    let menu = Menu::with_items(app, &[&app_menu, &file_menu])?;
    app.set_menu(menu)?;
    Ok(())
}

use crystal_canvas::*;

// Prevent macOS linker from stripping CXX exception handling symbols
extern crate cxx;

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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

            // Startup with empty crystal canvas (No test instances)
            renderer.update_atoms(&[]);

            // Store it in Tauri managed state so commands and the event loop can access it
            app.manage(std::sync::Mutex::new(renderer));
            app.manage(std::sync::Mutex::new(crystal_state::CrystalState::default()));
            app.manage(commands::LlmState(std::sync::Mutex::new(None)));

            // --- Menu Construction ---
            let _ = build_menu(app);

            app.on_menu_event(move |app_handle, event| {
                use tauri::Emitter;
                let id = event.id().0.as_str();
                let _ = app_handle.emit(id, ());
            });

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
            commands::preview_supercell,
            commands::export_file,
            commands::llm_configure,
            commands::llm_chat,
            commands::llm_execute_command,
            commands::get_crystal_state
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::MainEventsCleared = event {
                let Some(renderer_mutex) =
                    app_handle.try_state::<std::sync::Mutex<renderer::renderer::Renderer>>()
                else {
                    return;
                };
                let Ok(mut renderer) = renderer_mutex.try_lock() else {
                    return;
                };
                if let Err(e) = renderer.render() {
                    log::warn!("Render error: {:?}", e);
                }
            }
        });
}
