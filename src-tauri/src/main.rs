//! CrystalCanvas Tauri application entry point
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// Prevents additional console window on Windows in release
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::Arc;
use tauri::{Emitter, Manager};

fn build_menu(app: &mut tauri::App) -> tauri::Result<()> {
    use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

    // ── CrystalCanvas (App) Menu ─────────────────────────────────────────
    let about = PredefinedMenuItem::about(app, None::<&str>, None)?;
    let sep_app1 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "menu_settings", "Settings...", true, None::<&str>)?;
    let sep_app2 = PredefinedMenuItem::separator(app)?;
    let quit = PredefinedMenuItem::quit(app, None::<&str>)?;
    let app_menu = Submenu::with_items(
        app,
        "CrystalCanvas",
        true,
        &[&about, &sep_app1, &settings, &sep_app2, &quit],
    )?;

    // ── File Menu ────────────────────────────────────────────────────────
    let new_structure = MenuItem::with_id(
        app,
        "menu_new_structure",
        "New Structure",
        true,
        None::<&str>,
    )?;
    let sep_f1 = PredefinedMenuItem::separator(app)?;
    let open_file = MenuItem::with_id(app, "menu_open_file", "Open...", true, None::<&str>)?;
    let sep_f2 = PredefinedMenuItem::separator(app)?;
    // Export submenu
    let exp_poscar = MenuItem::with_id(app, "menu_export_poscar", "POSCAR...", true, None::<&str>)?;
    let exp_qe = MenuItem::with_id(
        app,
        "menu_export_qe",
        "Quantum ESPRESSO...",
        true,
        None::<&str>,
    )?;
    let exp_lammps = MenuItem::with_id(
        app,
        "menu_export_lammps",
        "LAMMPS Data...",
        true,
        None::<&str>,
    )?;
    let exp_sep = PredefinedMenuItem::separator(app)?;
    let exp_xyz = MenuItem::with_id(app, "menu_export_xyz", "XYZ...", false, None::<&str>)?; // placeholder
    let exp_cif = MenuItem::with_id(app, "menu_export_cif", "CIF...", false, None::<&str>)?; // placeholder
    let export_sub = Submenu::with_items(
        app,
        "Export As",
        true,
        &[
            &exp_poscar,
            &exp_qe,
            &exp_lammps,
            &exp_sep,
            &exp_xyz,
            &exp_cif,
        ],
    )?;
    let sep_f3 = PredefinedMenuItem::separator(app)?;
    let exp_image = MenuItem::with_id(
        app,
        "menu_export_image",
        "Export Image...",
        true,
        None::<&str>,
    )?;
    let sep_f4 = PredefinedMenuItem::separator(app)?;
    let close_win = PredefinedMenuItem::close_window(app, None::<&str>)?;
    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &new_structure,
            &sep_f1,
            &open_file,
            &sep_f2,
            &export_sub,
            &sep_f3,
            &exp_image,
            &sep_f4,
            &close_win,
        ],
    )?;

    // ── Edit Menu (macOS requires this for Cmd+C/V to work in WebView) ──
    let undo = MenuItem::with_id(
        app,
        "menu_undo",
        "Undo",
        true,
        Some("CommandOrControl+Z"),
    )?;
    let redo = MenuItem::with_id(
        app,
        "menu_redo",
        "Redo",
        true,
        Some("CommandOrControl+Shift+Z"),
    )?;
    let sep_e1 = PredefinedMenuItem::separator(app)?;
    let select_all = PredefinedMenuItem::select_all(app, None::<&str>)?;
    let deselect_all = MenuItem::with_id(
        app,
        "menu_deselect_all",
        "Deselect All",
        false,
        None::<&str>,
    )?;
    let sep_e2 = PredefinedMenuItem::separator(app)?;
    let delete_sel = MenuItem::with_id(
        app,
        "menu_delete_selected",
        "Delete Selected",
        true,
        None::<&str>,
    )?;
    let sep_e3 = PredefinedMenuItem::separator(app)?;
    let cut = PredefinedMenuItem::cut(app, None::<&str>)?;
    let copy = PredefinedMenuItem::copy(app, None::<&str>)?;
    let paste = PredefinedMenuItem::paste(app, None::<&str>)?;
    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &undo,
            &redo,
            &sep_e1,
            &select_all,
            &deselect_all,
            &sep_e2,
            &delete_sel,
            &sep_e3,
            &cut,
            &copy,
            &paste,
        ],
    )?;

    // ── View Menu ────────────────────────────────────────────────────────
    let v_persp = MenuItem::with_id(
        app,
        "menu_view_perspective",
        "Perspective",
        true,
        Some("CommandOrControl+1"),
    )?;
    let v_ortho = MenuItem::with_id(
        app,
        "menu_view_orthographic",
        "Orthographic",
        true,
        Some("CommandOrControl+2"),
    )?;
    let sep_v1 = PredefinedMenuItem::separator(app)?;
    let v_a = MenuItem::with_id(app, "menu_view_a", "View Along a", true, None::<&str>)?;
    let v_b = MenuItem::with_id(app, "menu_view_b", "View Along b", true, None::<&str>)?;
    let v_c = MenuItem::with_id(app, "menu_view_c", "View Along c", true, None::<&str>)?;
    let v_as = MenuItem::with_id(app, "menu_view_a_star", "View Along a*", true, None::<&str>)?;
    let v_bs = MenuItem::with_id(app, "menu_view_b_star", "View Along b*", true, None::<&str>)?;
    let v_cs = MenuItem::with_id(app, "menu_view_c_star", "View Along c*", true, None::<&str>)?;
    let sep_v2 = PredefinedMenuItem::separator(app)?;
    let v_reset = MenuItem::with_id(app, "menu_reset_view", "Reset View", true, None::<&str>)?;
    let sep_v3 = PredefinedMenuItem::separator(app)?;
    let v_labels = CheckMenuItem::with_id(
        app,
        "menu_toggle_labels",
        "Show Labels",
        true,
        false,
        None::<&str>,
    )?;
    let v_cell = CheckMenuItem::with_id(
        app,
        "menu_toggle_cell",
        "Show Unit Cell",
        true,
        true,
        None::<&str>,
    )?;
    let v_bonds = CheckMenuItem::with_id(
        app,
        "menu_toggle_bonds",
        "Show Bonds",
        true,
        true,
        None::<&str>,
    )?;
    let sep_v4 = PredefinedMenuItem::separator(app)?;
    let v_dark = MenuItem::with_id(
        app,
        "menu_toggle_dark",
        "Toggle Dark Mode",
        true,
        None::<&str>,
    )?;
    let view_menu = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &v_persp, &v_ortho, &sep_v1, &v_a, &v_b, &v_c, &v_as, &v_bs, &v_cs, &sep_v2, &v_reset,
            &sep_v3, &v_labels, &v_cell, &v_bonds, &sep_v4, &v_dark,
        ],
    )?;

    // ── Structure Menu ───────────────────────────────────────────────────
    let s_super = MenuItem::with_id(
        app,
        "menu_build_supercell",
        "Build Supercell...",
        true,
        None::<&str>,
    )?;
    let s_slab = MenuItem::with_id(
        app,
        "menu_cleave_slab",
        "Cleave Slab...",
        true,
        None::<&str>,
    )?;
    let sep_s1 = PredefinedMenuItem::separator(app)?;
    let s_add = MenuItem::with_id(app, "menu_add_atom", "Add Atom...", true, None::<&str>)?;
    let s_repl = MenuItem::with_id(
        app,
        "menu_replace_elem",
        "Replace Element...",
        true,
        None::<&str>,
    )?;
    let sep_s2 = PredefinedMenuItem::separator(app)?;
    let s_sg = MenuItem::with_id(
        app,
        "menu_spacegroup",
        "Space Group Analysis",
        true,
        None::<&str>,
    )?;
    let s_val = MenuItem::with_id(
        app,
        "menu_validate",
        "Validate Structure",
        false,
        None::<&str>,
    )?; // placeholder
    let structure_menu = Submenu::with_items(
        app,
        "Structure",
        true,
        &[
            &s_super, &s_slab, &sep_s1, &s_add, &s_repl, &sep_s2, &s_sg, &s_val,
        ],
    )?;

    // ── Window Menu ──────────────────────────────────────────────────────
    let w_llm = MenuItem::with_id(
        app,
        "menu_toggle_llm",
        "Toggle LLM Assistant",
        true,
        None::<&str>,
    )?;
    let sep_w1 = PredefinedMenuItem::separator(app)?;
    let w_min = PredefinedMenuItem::minimize(app, None::<&str>)?;
    let w_zoom = PredefinedMenuItem::maximize(app, None::<&str>)?;
    let w_full = PredefinedMenuItem::fullscreen(app, None::<&str>)?;
    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[&w_llm, &sep_w1, &w_min, &w_zoom, &w_full],
    )?;

    // ── Help Menu ────────────────────────────────────────────────────────
    let h_docs = MenuItem::with_id(app, "menu_docs", "Documentation", true, None::<&str>)?;
    let h_issues = MenuItem::with_id(app, "menu_issues", "Report Issue", true, None::<&str>)?;
    let sep_h1 = PredefinedMenuItem::separator(app)?;
    let h_about = MenuItem::with_id(
        app,
        "menu_about_cc",
        "About CrystalCanvas",
        true,
        None::<&str>,
    )?;
    let help_menu =
        Submenu::with_items(app, "Help", true, &[&h_docs, &h_issues, &sep_h1, &h_about])?;

    // ── Assemble Full Menu Bar ───────────────────────────────────────────
    let menu = Menu::with_items(
        app,
        &[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &structure_menu,
            &window_menu,
            &help_menu,
        ],
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
        "menu_settings" => {
            let _ = app_handle.emit("menu-action", "view_settings");
        }
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
            if let Some(r) =
                app_handle.try_state::<std::sync::Mutex<crate::renderer::renderer::Renderer>>()
                && let Ok(mut renderer) = r.try_lock()
            {
                renderer.clear_atoms();
            }
            log::info!("New empty structure created.");
        }
        "menu_open_file" => {
            let handle = app_handle.clone();
            app_handle
                .dialog()
                .file()
                .add_filter(
                    "All Supported",
                    &[
                        "cif", "xyz", "pdb", "poscar", "contcar", "vasp", "in", "pwi", "chgcar",
                        "locpot", "cube", "xsf", "dat",
                    ],
                )
                .add_filter("Volumetric", &["chgcar", "locpot", "cube", "xsf"])
                .add_filter("Wannier Hopping", &["dat"])
                .set_title("Open Structure File")
                .pick_file(move |file_path| {
                    let Some(path) = file_path else { return };
                    let path_str = path.to_string();
                    log::info!("Opening file: {}", path_str);
                    match crate::io::import::load_file(&path_str) {
                        Ok(mut state) => {
                            if let Err(error) = state.validate_structural_invariants() {
                                log::error!("Invalid structure in {}: {}", path_str, error);
                                return;
                            }
                            let vol_data = state.volumetric_data.take();
                            let base_snapshot = state.clone();
                            state.volumetric_data = vol_data;
                            let Some(base_st) = handle.try_state::<commands::BaseCrystalState>()
                            else {
                                log::error!("Base crystal state is unavailable");
                                return;
                            };
                            let mut base = match base_st.0.lock() {
                                Ok(base) => base,
                                Err(error) => {
                                    log::error!("Failed to lock base crystal state: {}", error);
                                    return;
                                }
                            };
                            let cs_mutex = handle
                                .state::<std::sync::Mutex<crate::crystal_state::CrystalState>>();
                            let mut cs = match cs_mutex.lock() {
                                Ok(cs) => cs,
                                Err(error) => {
                                    log::error!("Failed to lock crystal state: {}", error);
                                    return;
                                }
                            };
                            let undo_state =
                                handle.state::<std::sync::Mutex<crate::undo::UndoStack>>();
                            let mut u_stack = match undo_state.lock() {
                                Ok(stack) => stack,
                                Err(error) => {
                                    log::error!("Failed to lock undo stack: {}", error);
                                    return;
                                }
                            };
                            let settings_st =
                                handle.state::<std::sync::Mutex<crate::settings::AppSettings>>();
                            let settings = match settings_st.lock() {
                                Ok(settings) => settings,
                                Err(error) => {
                                    log::error!("Failed to lock settings: {}", error);
                                    return;
                                }
                            };
                            let atom_scene =
                                match crate::wannier::build_atoms_with_ghosts(&state, &settings) {
                                    Ok(instances) => match crate::renderer::instance::prepare_atom_scene(instances) {
                                        Ok(scene) => scene,
                                        Err(error) => {
                                            log::error!(
                                                "Failed to prepare renderer scene: {}",
                                                error.message
                                            );
                                            return;
                                        }
                                    },
                                    Err(error) => {
                                        log::error!(
                                            "Failed to build renderer scene: {}",
                                            error.message
                            );
                                        return;
                                    }
                                };
                            let line_scene = match crate::renderer::instance::build_line_scene(
                                &state,
                                &settings,
                            ) {
                                Ok(scene) => scene,
                                Err(error) => {
                                    log::error!(
                                        "Failed to prepare render lines: {}",
                                        error.message
                                    );
                                    return;
                                }
                            };
                            let renderer_state = handle
                                .state::<std::sync::Mutex<crate::renderer::renderer::Renderer>>();
                            let mut renderer = match renderer_state.lock() {
                                Ok(renderer) => renderer,
                                Err(error) => {
                                    log::error!("Failed to lock renderer: {}", error);
                                    return;
                                }
                            };
                            let prepared_volumetric = match state
                                .volumetric_data
                                .as_ref()
                                .map(|vol| renderer.prepare_volumetric(vol))
                                .transpose()
                            {
                                Ok(prepared) => prepared,
                                Err(()) => {
                                    log::error!(
                                        "GPU out of memory while preparing volumetric grid"
                                    );
                                    return;
                                }
                            };
                            let version = match crate::transaction::stamp_next_version(
                                &cs,
                                &mut state,
                            ) {
                                Ok(version) => version,
                                Err(error) => {
                                    log::error!("{}", error.message);
                                    return;
                                }
                            };
                            let previous_state =
                                crate::undo::StructuralSnapshot::from_crystal_state(&cs);
                            renderer.clear_structure_bound_overlays();
                            renderer.commit_atoms(atom_scene);
                            renderer.update_lines(&line_scene);
                            if let Some(prepared) = prepared_volumetric {
                                renderer.commit_volumetric(prepared);
                            }
                            let extent = state.cell_a.max(state.cell_b).max(state.cell_c) as f32;
                            let center_vec = glam::Vec3::from_array(state.unit_cell_center());
                            renderer.camera.eye =
                                center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
                            renderer.camera.target = center_vec;
                            if !renderer.camera.is_perspective {
                                renderer.camera.set_orthographic(extent * 1.5);
                            }
                            renderer.update_camera();
                            *base = Some(base_snapshot);
                            *cs = state;
                            u_stack.push(previous_state);
                            let can_undo = u_stack.can_undo();
                            let can_redo = u_stack.can_redo();
                            drop(renderer);
                            drop(settings);
                            drop(u_stack);
                            drop(cs);
                            drop(base);
                            let _ = handle.emit(
                                "state_changed",
                                crate::transaction::StateChangedPayload {
                                    version,
                                },
                            );
                            let _ = handle.emit(
                                "undo_stack_changed",
                                crate::transaction::UndoStackPayload { can_undo, can_redo },
                            );
                            log::info!("File loaded and base state saved: {}", path_str);
                        }
                        Err(e) => log::error!("Failed to load file: {}", e),
                    }
                });
        }
        "menu_export_poscar" => handle_export(app_handle, "POSCAR", "poscar"),
        "menu_export_qe" => handle_export(app_handle, "QE", "in"),
        "menu_export_lammps" => handle_export(app_handle, "LAMMPS", "lmp"),
        "menu_export_image" => {
            let _ = app_handle.emit("menu-action", "export_image");
        }

        // ── Edit ─────────────────────────────────────────────────────
        "menu_undo" => {
            let _ = app_handle.emit("menu-action", "undo");
        }
        "menu_redo" => {
            let _ = app_handle.emit("menu-action", "redo");
        }
        "menu_delete_selected" => {
            log::info!("Delete Selected triggered (requires frontend selection).");
            let _ = app_handle.emit("menu-action", "delete_selected");
        }
        "menu_deselect_all" => {
            let _ = app_handle.emit("menu-action", "deselect_all");
        }

        // ── View ─────────────────────────────────────────────────────
        "menu_view_perspective" => {
            if let Some(r_mutex) =
                app_handle.try_state::<std::sync::Mutex<crate::renderer::renderer::Renderer>>()
                && let Ok(mut renderer) = r_mutex.lock()
            {
                renderer.camera.set_perspective();
                #[derive(Clone, serde::Serialize)]
                struct Payload {
                    is_perspective: bool,
                }
                let _ = app_handle.emit(
                    "view_projection_changed",
                    Payload {
                        is_perspective: true,
                    },
                );
            }
        }
        "menu_view_orthographic" => {
            if let Some(r_mutex) =
                app_handle.try_state::<std::sync::Mutex<crate::renderer::renderer::Renderer>>()
                && let Ok(mut renderer) = r_mutex.lock()
            {
                let mut scale = 15.0;
                if let Some(cs_mutex) =
                    app_handle.try_state::<std::sync::Mutex<crate::crystal_state::CrystalState>>()
                    && let Ok(cs) = cs_mutex.lock()
                {
                    let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
                    scale = if extent > 0.0 { extent * 1.5 } else { 15.0 };
                }
                renderer.camera.set_orthographic(scale);
                #[derive(Clone, serde::Serialize)]
                struct Payload {
                    is_perspective: bool,
                }
                let _ = app_handle.emit(
                    "view_projection_changed",
                    Payload {
                        is_perspective: false,
                    },
                );
            }
        }
        "menu_view_a" | "menu_view_b" | "menu_view_c" | "menu_view_a_star" | "menu_view_b_star"
        | "menu_view_c_star" => {
            let axis = id.replace("menu_view_", "");
            log::info!("View along axis: {}", axis);
            let _ = app_handle.emit("menu-action", &format!("view_axis_{}", axis));
        }
        "menu_reset_view" => {
            if let Some(r_mutex) =
                app_handle.try_state::<std::sync::Mutex<crate::renderer::renderer::Renderer>>()
                && let Ok(mut renderer) = r_mutex.lock()
            {
                let mut dist = 30.0;
                let mut center_vec = glam::Vec3::ZERO;
                if let Some(cs_mutex) =
                    app_handle.try_state::<std::sync::Mutex<crate::crystal_state::CrystalState>>()
                    && let Ok(cs) = cs_mutex.lock()
                {
                    let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
                    dist = extent * 2.5;
                    center_vec = glam::Vec3::from_array(cs.unit_cell_center());
                }
                renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, dist);
                renderer.camera.target = center_vec;
                renderer.camera.up = glam::Vec3::Y;
                if !renderer.camera.is_perspective {
                    renderer.camera.set_orthographic(dist * 0.6);
                }
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
                    "POSCAR" => {
                        crate::io::export::export_poscar(&cs, &path_str).map_err(|e| e.to_string())
                    }
                    "QE" => crate::io::export::export_qe_input(&cs, &path_str)
                        .map_err(|e| e.to_string()),
                    "LAMMPS" => crate::io::export::export_lammps_data(&cs, &path_str)
                        .map_err(|e| e.to_string()),
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
            renderer.clear_atoms();

            // Store it in Tauri managed state so commands and the event loop can access it
            app.manage(std::sync::Mutex::new(renderer));
            app.manage(std::sync::Mutex::new(crystal_state::CrystalState::default()));
            let loaded_settings = crate::settings::AppSettings::load(app.handle());
            app.manage(std::sync::Mutex::new(loaded_settings));
            app.manage(commands::LlmState(std::sync::Mutex::new(None)));
            app.manage(commands::BaseCrystalState(std::sync::Mutex::new(None)));
            app.manage(std::sync::Mutex::new(crate::undo::UndoStack::new(20)));

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
            commands::translate_atoms_screen,
            commands::delete_atoms,
            commands::update_selection,
            commands::substitute_atoms,
            commands::preview_slab,
            commands::preview_supercell,
            commands::export_file,
            commands::restore_unitcell,
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
            commands::apply_niggli_reduce,
            commands::apply_cell_standardize,
            commands::set_camera_view_axis,
            commands::get_settings,
            commands::update_settings,
            commands::get_bond_analysis,
            commands::load_phonon,
            commands::load_phonon_interactive,
            commands::load_axsf_phonon,
            commands::set_phonon_mode,
            commands::set_phonon_phase,
            commands::update_lattice_params,
            commands::export_image,
            commands::shift_termination,
            commands::add_measurement,
            commands::clear_measurements,
            commands::get_measurements,
            commands::get_measurement_labels_screen,
            commands::load_volumetric_file,
            commands::set_isovalue,
            commands::set_isosurface_color,
            commands::set_isosurface_opacity,
            commands::set_isosurface_sign_mode,
            commands::set_volume_render_mode,
            commands::set_volume_opacity_range,
            commands::set_volume_density_cutoff,
            commands::set_volume_colormap,
            commands::get_volumetric_info,
            commands::compute_brillouin_zone,
            commands::toggle_bz_display,
            commands::get_kpath_info,
            commands::set_bz_scale,
            commands::get_bz_label_positions,
            commands::generate_kpath_text,
            commands::write_text_file,
            commands::load_wannier_hr,
            commands::set_wannier_t_min,
            commands::set_wannier_r_shell,
            commands::set_wannier_orbital,
            commands::toggle_wannier_onsite,
            commands::toggle_hopping_display,
            commands::clear_wannier,
            commands::undo,
            commands::redo
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
                match renderer.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        // Reconfigure the surface to recover from transient GPU state errors
                        log::warn!("Surface lost/outdated — reconfiguring");
                        let size = renderer.gpu.size;
                        renderer
                            .gpu
                            .surface
                            .configure(&renderer.gpu.device, &renderer.gpu.config);
                        renderer.resize(size);
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        log::error!("GPU out of memory — cannot recover");
                    }
                    Err(e) => {
                        log::warn!("Render error: {:?}", e);
                    }
                }
            }
        });
}
