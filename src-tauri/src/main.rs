//! CrystalCanvas Tauri application entry point

// Prevents additional console window on Windows in release
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::Arc;
use tauri::{Emitter, Manager};

fn build_menu(app: &mut tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};

    // ── CrystalCanvas (App) Menu ─────────────────────────────────────────
    let about = PredefinedMenuItem::about(app, None::<&str>, None)?;
    let sep_app1 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "menu_settings", "Settings...", false, None::<&str>)?;
    let sep_app2 = PredefinedMenuItem::separator(app)?;
    let quit = PredefinedMenuItem::quit(app, None::<&str>)?;
    let app_menu = Submenu::with_items(
        app, "CrystalCanvas", true,
        &[&about, &sep_app1, &settings, &sep_app2, &quit],
    )?;

    // ── File Menu ────────────────────────────────────────────────────────
    let new_structure = MenuItem::with_id(app, "menu_new_structure", "New Structure", true, None::<&str>)?;
    let sep_f1 = PredefinedMenuItem::separator(app)?;
    let open_file = MenuItem::with_id(app, "menu_open_file", "Open...", true, None::<&str>)?;
    let sep_f2 = PredefinedMenuItem::separator(app)?;
    // Export submenu
    let exp_poscar = MenuItem::with_id(app, "menu_export_poscar", "POSCAR...", true, None::<&str>)?;
    let exp_qe = MenuItem::with_id(app, "menu_export_qe", "Quantum ESPRESSO...", true, None::<&str>)?;
    let exp_lammps = MenuItem::with_id(app, "menu_export_lammps", "LAMMPS Data...", true, None::<&str>)?;
    let exp_sep = PredefinedMenuItem::separator(app)?;
    let exp_xyz = MenuItem::with_id(app, "menu_export_xyz", "XYZ...", false, None::<&str>)?; // placeholder
    let exp_cif = MenuItem::with_id(app, "menu_export_cif", "CIF...", false, None::<&str>)?; // placeholder
    let export_sub = Submenu::with_items(
        app, "Export As", true,
        &[&exp_poscar, &exp_qe, &exp_lammps, &exp_sep, &exp_xyz, &exp_cif],
    )?;
    let sep_f3 = PredefinedMenuItem::separator(app)?;
    let close_win = PredefinedMenuItem::close_window(app, None::<&str>)?;
    let file_menu = Submenu::with_items(
        app, "File", true,
        &[&new_structure, &sep_f1, &open_file, &sep_f2, &export_sub, &sep_f3, &close_win],
    )?;

    // ── Edit Menu (macOS requires this for Cmd+C/V to work in WebView) ──
    let undo = PredefinedMenuItem::undo(app, None::<&str>)?;
    let redo = PredefinedMenuItem::redo(app, None::<&str>)?;
    let sep_e1 = PredefinedMenuItem::separator(app)?;
    let select_all = PredefinedMenuItem::select_all(app, None::<&str>)?;
    let deselect_all = MenuItem::with_id(app, "menu_deselect_all", "Deselect All", false, None::<&str>)?;
    let sep_e2 = PredefinedMenuItem::separator(app)?;
    let delete_sel = MenuItem::with_id(app, "menu_delete_selected", "Delete Selected", true, None::<&str>)?;
    let sep_e3 = PredefinedMenuItem::separator(app)?;
    let cut = PredefinedMenuItem::cut(app, None::<&str>)?;
    let copy = PredefinedMenuItem::copy(app, None::<&str>)?;
    let paste = PredefinedMenuItem::paste(app, None::<&str>)?;
    let edit_menu = Submenu::with_items(
        app, "Edit", true,
        &[&undo, &redo, &sep_e1, &select_all, &deselect_all, &sep_e2,
          &delete_sel, &sep_e3, &cut, &copy, &paste],
    )?;

    // ── View Menu ────────────────────────────────────────────────────────
    let v_persp = MenuItem::with_id(app, "menu_view_perspective", "Perspective", true, Some("CommandOrControl+1"))?;
    let v_ortho = MenuItem::with_id(app, "menu_view_orthographic", "Orthographic", true, Some("CommandOrControl+2"))?;
    let sep_v1 = PredefinedMenuItem::separator(app)?;
    let v_a  = MenuItem::with_id(app, "menu_view_a",  "View Along a",  true, None::<&str>)?;
    let v_b  = MenuItem::with_id(app, "menu_view_b",  "View Along b",  true, None::<&str>)?;
    let v_c  = MenuItem::with_id(app, "menu_view_c",  "View Along c",  true, None::<&str>)?;
    let v_as = MenuItem::with_id(app, "menu_view_a_star", "View Along a*", true, None::<&str>)?;
    let v_bs = MenuItem::with_id(app, "menu_view_b_star", "View Along b*", true, None::<&str>)?;
    let v_cs = MenuItem::with_id(app, "menu_view_c_star", "View Along c*", true, None::<&str>)?;
    let sep_v2 = PredefinedMenuItem::separator(app)?;
    let v_reset = MenuItem::with_id(app, "menu_reset_view", "Reset View", true, None::<&str>)?;
    let sep_v3 = PredefinedMenuItem::separator(app)?;
    let v_labels = MenuItem::with_id(app, "menu_toggle_labels", "Show Labels", true, None::<&str>)?;
    let v_cell   = MenuItem::with_id(app, "menu_toggle_cell",   "Show Unit Cell", true, None::<&str>)?;
    let v_bonds  = MenuItem::with_id(app, "menu_toggle_bonds",  "Show Bonds", false, None::<&str>)?; // placeholder
    let sep_v4 = PredefinedMenuItem::separator(app)?;
    let v_dark = MenuItem::with_id(app, "menu_toggle_dark", "Toggle Dark Mode", true, None::<&str>)?;
    let view_menu = Submenu::with_items(
        app, "View", true,
        &[&v_persp, &v_ortho, &sep_v1,
          &v_a, &v_b, &v_c, &v_as, &v_bs, &v_cs, &sep_v2,
          &v_reset, &sep_v3,
          &v_labels, &v_cell, &v_bonds, &sep_v4, &v_dark],
    )?;

    // ── Structure Menu ───────────────────────────────────────────────────
    let s_super = MenuItem::with_id(app, "menu_build_supercell", "Build Supercell...", true, None::<&str>)?;
    let s_slab  = MenuItem::with_id(app, "menu_cleave_slab",     "Cleave Slab...",    true, None::<&str>)?;
    let sep_s1 = PredefinedMenuItem::separator(app)?;
    let s_add   = MenuItem::with_id(app, "menu_add_atom",     "Add Atom...",        true, None::<&str>)?;
    let s_repl  = MenuItem::with_id(app, "menu_replace_elem", "Replace Element...", true, None::<&str>)?;
    let sep_s2 = PredefinedMenuItem::separator(app)?;
    let s_sg    = MenuItem::with_id(app, "menu_spacegroup",   "Space Group Analysis", true, None::<&str>)?;
    let s_val   = MenuItem::with_id(app, "menu_validate",     "Validate Structure",   false, None::<&str>)?; // placeholder
    let structure_menu = Submenu::with_items(
        app, "Structure", true,
        &[&s_super, &s_slab, &sep_s1, &s_add, &s_repl, &sep_s2, &s_sg, &s_val],
    )?;

    // ── Window Menu ──────────────────────────────────────────────────────
    let w_llm = MenuItem::with_id(app, "menu_toggle_llm", "Toggle LLM Assistant", true, None::<&str>)?;
    let sep_w1 = PredefinedMenuItem::separator(app)?;
    let w_min = PredefinedMenuItem::minimize(app, None::<&str>)?;
    let w_zoom = PredefinedMenuItem::maximize(app, None::<&str>)?;
    let w_full = PredefinedMenuItem::fullscreen(app, None::<&str>)?;
    let window_menu = Submenu::with_items(
        app, "Window", true,
        &[&w_llm, &sep_w1, &w_min, &w_zoom, &w_full],
    )?;

    // ── Help Menu ────────────────────────────────────────────────────────
    let h_docs   = MenuItem::with_id(app, "menu_docs",   "Documentation",  true, None::<&str>)?;
    let h_issues = MenuItem::with_id(app, "menu_issues", "Report Issue",   true, None::<&str>)?;
    let sep_h1 = PredefinedMenuItem::separator(app)?;
    let h_about = MenuItem::with_id(app, "menu_about_cc", "About CrystalCanvas", true, None::<&str>)?;
    let help_menu = Submenu::with_items(
        app, "Help", true,
        &[&h_docs, &h_issues, &sep_h1, &h_about],
    )?;

    // ── Assemble Full Menu Bar ───────────────────────────────────────────
    let menu = Menu::with_items(
        app,
        &[&app_menu, &file_menu, &edit_menu, &view_menu, &structure_menu, &window_menu, &help_menu],
    )?;
    app.set_menu(menu)?;
    Ok(())
}

/// Handle native menu events by directly invoking dialogs and I/O on the Rust side.
fn handle_menu_event(app_handle: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    use tauri_plugin_dialog::DialogExt;

    let id = event.id().0.as_str();
    log::info!("Menu event: {}", id);

    match id {
        // ── File ─────────────────────────────────────────────────────
        "menu_new_structure" => {
            // Reset crystal state to empty
            if let Some(cs_mutex) =
                app_handle.try_state::<std::sync::Mutex<crate::crystal_state::CrystalState>>()
                && let Ok(mut cs) = cs_mutex.try_lock()
            {
                *cs = crate::crystal_state::CrystalState::default();
            }
            // Clear renderer
            if let Some(r) = app_handle.try_state::<std::sync::Mutex<crate::renderer::renderer::Renderer>>()
                && let Ok(mut renderer) = r.try_lock()
            {
                renderer.update_atoms(&[]);
            }
            log::info!("New empty structure created.");
        }
        "menu_open_file" => {
            let handle = app_handle.clone();
            app_handle
                .dialog()
                .file()
                .add_filter("Crystal Files", &["cif", "pdb", "xyz"])
                .set_title("Open Structure File")
                .pick_file(move |file_path| {
                    let Some(path) = file_path else { return };
                    let path_str = path.to_string();
                    log::info!("Opening file: {}", path_str);
                    match crate::io::import::load_file(&path_str) {
                        Ok(state) => {
                            if let Some(cs_mutex) = handle
                                .try_state::<std::sync::Mutex<crate::crystal_state::CrystalState>>()
                                && let Ok(mut cs) = cs_mutex.try_lock()
                            {
                                *cs = state.clone();
                            }
                            let instances = crate::renderer::instance::build_instance_data(
                                &state.cart_positions, &state.atomic_numbers, &state.elements,
                            );
                            if let Some(r) = handle.try_state::<std::sync::Mutex<crate::renderer::renderer::Renderer>>()
                                && let Ok(mut renderer) = r.try_lock()
                            {
                                renderer.update_atoms(&instances);
                                renderer.update_lines(&state);
                                let extent = state.cell_a.max(state.cell_b).max(state.cell_c) as f32;
                                renderer.camera.eye = glam::Vec3::new(0.0, 0.0, extent * 2.0);
                                renderer.camera.target = glam::Vec3::ZERO;
                                if !renderer.camera.is_perspective {
                                    renderer.camera.set_orthographic(extent * 1.5);
                                }
                            }
                            log::info!("File loaded: {}", path_str);
                        }
                        Err(e) => log::error!("Failed to load file: {}", e),
                    }
                });
        }
        "menu_export_poscar" => handle_export(app_handle, "POSCAR", "poscar"),
        "menu_export_qe"     => handle_export(app_handle, "QE", "in"),
        "menu_export_lammps" => handle_export(app_handle, "LAMMPS", "lmp"),

        // ── Edit ─────────────────────────────────────────────────────
        "menu_delete_selected" => {
            log::info!("Delete Selected triggered (requires frontend selection).");
            let _ = app_handle.emit("menu-action", "delete_selected");
        }
        "menu_deselect_all" => {
            let _ = app_handle.emit("menu-action", "deselect_all");
        }

        // ── View ─────────────────────────────────────────────────────
        "menu_view_perspective" => {
            if let Some(r) = app_handle.try_state::<std::sync::Mutex<crate::renderer::renderer::Renderer>>()
                && let Ok(mut renderer) = r.try_lock()
            {
                renderer.camera.set_perspective();
            }
        }
        "menu_view_orthographic" => {
            if let Some(r) = app_handle.try_state::<std::sync::Mutex<crate::renderer::renderer::Renderer>>()
                && let Ok(mut renderer) = r.try_lock()
            {
                renderer.camera.set_orthographic(30.0);
            }
        }
        "menu_view_a" | "menu_view_b" | "menu_view_c" |
        "menu_view_a_star" | "menu_view_b_star" | "menu_view_c_star" => {
            let axis = id.replace("menu_view_", "");
            log::info!("View along axis: {}", axis);
            let _ = app_handle.emit("menu-action", &format!("view_axis_{}", axis));
        }
        "menu_reset_view" => {
            if let Some(r) = app_handle.try_state::<std::sync::Mutex<crate::renderer::renderer::Renderer>>()
                && let Ok(mut renderer) = r.try_lock()
            {
                // Reset camera to default position
                renderer.camera.eye = glam::Vec3::new(0.0, 0.0, 20.0);
                renderer.camera.target = glam::Vec3::ZERO;
                renderer.camera.up = glam::Vec3::Y;
            }
        }
        "menu_toggle_labels" | "menu_toggle_cell" | "menu_toggle_bonds" => {
            let flag = id.replace("menu_toggle_", "");
            log::info!("Toggle render flag: {}", flag);
            let _ = app_handle.emit("menu-action", &format!("toggle_{}", flag));
        }
        "menu_toggle_dark" => {
            let _ = app_handle.emit("menu-action", "toggle_dark_mode");
        }

        // ── Structure ────────────────────────────────────────────────
        "menu_build_supercell" => {
            let _ = app_handle.emit("menu-action", "open_supercell_dialog");
        }
        "menu_cleave_slab" => {
            let _ = app_handle.emit("menu-action", "open_slab_dialog");
        }
        "menu_add_atom" => {
            let _ = app_handle.emit("menu-action", "open_add_atom_dialog");
        }
        "menu_replace_elem" => {
            let _ = app_handle.emit("menu-action", "open_replace_element_dialog");
        }
        "menu_spacegroup" => {
            // Read spacegroup from crystal state and emit to frontend
            let sg = if let Some(cs_mutex) =
                app_handle.try_state::<std::sync::Mutex<crate::crystal_state::CrystalState>>()
                && let Ok(cs) = cs_mutex.try_lock()
            {
                cs.spacegroup_hm.clone()
            } else {
                "N/A".to_string()
            };
            log::info!("Space Group: {}", sg);
            let _ = app_handle.emit("menu-action", &format!("show_spacegroup:{}", sg));
        }

        // ── Window ───────────────────────────────────────────────────
        "menu_toggle_llm" => {
            let _ = app_handle.emit("menu-action", "toggle_llm_assistant");
        }

        // ── Help ─────────────────────────────────────────────────────
        "menu_docs" => {
            let _ = open::that("https://github.com/XiaoJiang-Phy/CrystalCanvas/wiki");
        }
        "menu_issues" => {
            let _ = open::that("https://github.com/XiaoJiang-Phy/CrystalCanvas/issues");
        }
        "menu_about_cc" => {
            log::info!("About CrystalCanvas v0.1.0");
            let _ = app_handle.emit("menu-action", "show_about");
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
            log::info!("Exporting {} to {}", format, path_str);

            if let Some(cs_mutex) =
                handle.try_state::<std::sync::Mutex<crate::crystal_state::CrystalState>>()
                && let Ok(cs) = cs_mutex.try_lock()
            {
                let result = match format {
                    "POSCAR" => crate::io::export::export_poscar(&cs, &path_str).map_err(|e| e.to_string()),
                    "QE"     => crate::io::export::export_qe_input(&cs, &path_str).map_err(|e| e.to_string()),
                    "LAMMPS" => crate::io::export::export_lammps_data(&cs, &path_str).map_err(|e| e.to_string()),
                    _ => Err(format!("Unsupported format: {}", format)),
                };
                match result {
                    Ok(()) => log::info!("Export {} succeeded", format),
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
        .plugin(tauri_plugin_clipboard_manager::init())
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
            commands::check_api_key_status,
            commands::rotate_camera,
            commands::zoom_camera,
            commands::pan_camera,
            commands::reset_camera,
            commands::pick_atom,
            commands::set_render_flags,
            commands::apply_supercell,
            commands::apply_slab,
            commands::set_camera_view_axis
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
