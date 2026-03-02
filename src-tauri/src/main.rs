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
    let separator = PredefinedMenuItem::separator(app)?;
    let export_poscar =
        MenuItem::with_id(app, "menu_export_poscar", "Export POSCAR...", true, None::<&str>)?;
    let export_qe =
        MenuItem::with_id(app, "menu_export_qe", "Export QE...", true, None::<&str>)?;
    let export_lammps =
        MenuItem::with_id(app, "menu_export_lammps", "Export LAMMPS...", true, None::<&str>)?;
    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[&import, &separator, &export_poscar, &export_qe, &export_lammps],
    )?;
    let menu = Menu::with_items(app, &[&app_menu, &file_menu])?;
    app.set_menu(menu)?;
    Ok(())
}

/// Handle native menu events by directly invoking dialogs and I/O on the Rust side.
/// This avoids the fragile Rust→emit→frontend→dialog→invoke→Rust round-trip.
fn handle_menu_event(app_handle: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    use tauri_plugin_dialog::DialogExt;

    let id = event.id().0.as_str();
    log::info!("Menu event received: {}", id);

    match id {
        "menu_import_cif" => {
            let handle = app_handle.clone();
            app_handle
                .dialog()
                .file()
                .add_filter("Crystal Files", &["cif", "pdb"])
                .set_title("Import CIF File")
                .pick_file(move |file_path| {
                    let Some(path) = file_path else { return };
                    let path_str = path.to_string();
                    log::info!("User selected file for import: {}", path_str);

                    // Load the CIF file using io::import
                    match crate::io::import::load_file(&path_str) {
                        Ok(state) => {
                            // Update crystal state
                            if let Some(cs_mutex) = handle
                                .try_state::<std::sync::Mutex<crate::crystal_state::CrystalState>>()
                                && let Ok(mut cs) = cs_mutex.try_lock()
                            {
                                *cs = state.clone();
                            }
                            // Build instance data and update renderer
                            let instances = crate::renderer::instance::build_instance_data(
                                &state.cart_positions,
                                &state.atomic_numbers,
                                &state.elements,
                            );
                            if let Some(renderer_mutex) = handle.try_state::<
                                std::sync::Mutex<crate::renderer::renderer::Renderer>,
                            >()
                                && let Ok(mut renderer) = renderer_mutex.try_lock()
                            {
                                renderer.update_atoms(&instances);
                                // Auto-adjust camera
                                let extent =
                                    state.cell_a.max(state.cell_b).max(state.cell_c) as f32;
                                renderer.camera.eye =
                                    glam::Vec3::new(0.0, 0.0, extent * 2.0);
                                renderer.camera.target = glam::Vec3::ZERO;
                                if !renderer.camera.is_perspective {
                                    renderer.camera.set_orthographic(extent * 1.5);
                                }
                            }
                            log::info!("CIF file loaded successfully: {}", path_str);
                        }
                        Err(e) => {
                            log::error!("Failed to load CIF file: {}", e);
                        }
                    }
                });
        }
        "menu_export_poscar" => {
            handle_export(app_handle, "POSCAR", "poscar");
        }
        "menu_export_qe" => {
            handle_export(app_handle, "QE", "in");
        }
        "menu_export_lammps" => {
            handle_export(app_handle, "LAMMPS", "lmp");
        }
        _ => {}
    }
}

/// Helper: show a save dialog and export crystal state to the chosen format.
fn handle_export(app_handle: &tauri::AppHandle, format: &'static str, extension: &'static str) {
    use tauri_plugin_dialog::DialogExt;

    let handle = app_handle.clone();
    app_handle
        .dialog()
        .file()
        .add_filter(format, &[extension, "txt"])
        .set_title(format!("Export as {}", format))
        .save_file(move |file_path| {
            let Some(path) = file_path else { return };
            let path_str = path.to_string();
            log::info!("User selected path for export: {} (format={})", path_str, format);

            if let Some(cs_mutex) =
                handle.try_state::<std::sync::Mutex<crate::crystal_state::CrystalState>>()
                && let Ok(cs) = cs_mutex.try_lock()
            {
                let result = match format {
                    "POSCAR" => {
                        crate::io::export::export_poscar(&cs, &path_str).map_err(|e| e.to_string())
                    }
                    "QE" => {
                        crate::io::export::export_qe_input(&cs, &path_str).map_err(|e| e.to_string())
                    }
                    "LAMMPS" => {
                        crate::io::export::export_lammps_data(&cs, &path_str)
                            .map_err(|e| e.to_string())
                    }
                    _ => Err(format!("Unsupported format: {}", format)),
                };
                match result {
                    Ok(()) => log::info!("Export {} to {} succeeded", format, path_str),
                    Err(e) => log::error!("Export failed: {}", e),
                }
            }
        });
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

            // --- Menu Event Handler (operates entirely in Rust) ---
            app.on_menu_event(handle_menu_event);

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
            commands::get_crystal_state,
            commands::check_api_key_status
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
