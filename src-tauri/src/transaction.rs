//! State transaction module encapsulating lock protocols and undo snapshotting
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::crystal_state::CrystalState;
use crate::renderer::renderer::Renderer;
use crate::settings::AppSettings;
use crate::undo::{LightweightState, UndoStack};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

/// Information emitted when the undo stack changes
#[derive(Clone, serde::Serialize)]
pub struct UndoStackPayload {
    pub can_undo: bool,
    pub can_redo: bool,
}

/// A read-only transaction for querying the crystal state without mutation.
pub fn with_state_read<F, R>(
    crystal_state: &State<'_, Mutex<CrystalState>>,
    f: F,
) -> Result<R, String>
where
    F: FnOnce(&CrystalState) -> Result<R, String>,
{
    let cs = crystal_state.lock().map_err(|e| format!("State lock failed: {}", e))?;
    f(&cs)
}

/// A non-blocking read-only transaction for high-frequency queries (e.g. preview scaling), uses try_lock.
pub fn with_state_read_try<F, R>(
    crystal_state: &State<'_, Mutex<CrystalState>>,
    f: F,
) -> Result<R, String>
where
    F: FnOnce(&CrystalState) -> Result<R, String>,
{
    let cs = crystal_state.try_lock().map_err(|_| "State currently in use, try again.")?;
    f(&cs)
}

/// A state transaction that mutates the structure, records history, and updates the renderer.
/// Lock ordering: CrystalState -> AppSettings -> Renderer
pub fn with_state_update<F>(
    app: &AppHandle,
    crystal_state: &State<'_, Mutex<CrystalState>>,
    settings_state: &State<'_, Mutex<AppSettings>>,
    renderer_state: &State<'_, Mutex<Renderer>>,
    undo_state: &State<'_, Mutex<UndoStack>>,
    f: F,
) -> Result<(), String>
where
    F: FnOnce(&mut CrystalState) -> Result<(), String>,
{
    _with_state_update_impl(app, crystal_state, settings_state, renderer_state, undo_state, false, f)
}

/// Same as `with_state_update`, but also auto-adjusts the camera distance to fit the new bounding box.
pub fn with_state_update_and_refit<F>(
    app: &AppHandle,
    crystal_state: &State<'_, Mutex<CrystalState>>,
    settings_state: &State<'_, Mutex<AppSettings>>,
    renderer_state: &State<'_, Mutex<Renderer>>,
    undo_state: &State<'_, Mutex<UndoStack>>,
    f: F,
) -> Result<(), String>
where
    F: FnOnce(&mut CrystalState) -> Result<(), String>,
{
    _with_state_update_impl(app, crystal_state, settings_state, renderer_state, undo_state, true, f)
}

fn _with_state_update_impl<F>(
    app: &AppHandle,
    crystal_state: &State<'_, Mutex<CrystalState>>,
    settings_state: &State<'_, Mutex<AppSettings>>,
    renderer_state: &State<'_, Mutex<Renderer>>,
    undo_state: &State<'_, Mutex<UndoStack>>,
    refit_camera: bool,
    f: F,
) -> Result<(), String>
where
    F: FnOnce(&mut CrystalState) -> Result<(), String>,
{
    let mut cs = crystal_state.lock().map_err(|e| format!("State lock failed: {}", e))?;
    
    // Save snapshot before mutation
    let pre_mutation_snapshot = LightweightState::from_crystal_state(&cs);
    
    // Mutate state
    f(&mut cs)?;
    cs.version += 1;
    
    let mut u_stack = undo_state.lock().map_err(|e| format!("Undo lock failed: {}", e))?;
    u_stack.push(pre_mutation_snapshot);
    let can_undo = u_stack.can_undo();
    let can_redo = u_stack.can_redo();
    drop(u_stack);
    
    // Retrieve settings
    let settings = settings_state.lock().map_err(|e| format!("Settings lock failed: {}", e))?;

    // Build atoms + lines + bonds
    let instances = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
    let bond_instances = crate::renderer::instance::build_bond_instances(&cs, &settings, &cs.selected_atoms);

    // Update renderer
    let mut renderer = renderer_state.lock().map_err(|e| format!("Renderer lock failed: {}", e))?;
    renderer.update_atoms(&instances);
    renderer.update_lines(&cs, &settings);
    renderer.update_bonds(&bond_instances);
    
    // Update hoppings if they exist
    if let Some(overlay) = &cs.wannier_overlay {
        let hopping_instances = crate::renderer::instance::build_hopping_instances(&overlay.visible_hoppings, overlay.hr_data.t_max);
        renderer.update_hoppings(&hopping_instances);
    }
    
    if refit_camera {
        let extent = cs.cell_a.max(cs.cell_b).max(cs.cell_c) as f32;
        let center = cs.unit_cell_center();
        let center_vec = glam::Vec3::from_array(center);
        renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
        renderer.camera.target = center_vec;
        if !renderer.camera.is_perspective {
            renderer.camera.set_orthographic(extent * 1.5);
        }
        renderer.update_camera();
    }
    
    // We must drop locks before emitting events to avoid potential deadlocks with frontend event handlers
    drop(renderer);
    drop(settings);
    drop(cs);

    app.emit("state_changed", ()).ok();
    app.emit("undo_stack_changed", UndoStackPayload { can_undo, can_redo }).ok();

    Ok(())
}
