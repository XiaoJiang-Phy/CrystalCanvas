//! State transaction module encapsulating lock protocols and undo snapshotting
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::crystal_state::CrystalState;
use crate::ipc::{IpcError, IpcResult};
use crate::renderer::renderer::Renderer;
use crate::settings::AppSettings;
use crate::undo::{StructuralSnapshot, UndoStack};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

/// Information emitted when the undo stack changes
#[derive(Clone, serde::Serialize)]
pub struct UndoStackPayload {
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, serde::Serialize)]
pub struct StateChangedPayload {
    pub version: u32,
}

/// A read-only transaction for querying the crystal state without mutation.
pub fn with_state_read<F, R>(
    crystal_state: &State<'_, Mutex<CrystalState>>,
    f: F,
) -> IpcResult<R>
where
    F: FnOnce(&CrystalState) -> IpcResult<R>,
{
    let cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    f(&cs)
}

/// A non-blocking read-only transaction for high-frequency queries (e.g. preview scaling), uses try_lock.
pub fn with_state_read_try<F, R>(
    crystal_state: &State<'_, Mutex<CrystalState>>,
    f: F,
) -> IpcResult<R>
where
    F: FnOnce(&CrystalState) -> IpcResult<R>,
{
    let cs = crystal_state
        .try_lock()
        .map_err(|error| IpcError::from_try_lock(error, "crystal state"))?;
    f(&cs)
}

/// A state transaction that mutates the structure, records history, and updates the renderer.
/// Lock ordering: CrystalState -> UndoStack -> AppSettings -> Renderer
pub fn with_state_update<F>(
    app: &AppHandle,
    crystal_state: &State<'_, Mutex<CrystalState>>,
    settings_state: &State<'_, Mutex<AppSettings>>,
    renderer_state: &State<'_, Mutex<Renderer>>,
    undo_state: &State<'_, Mutex<UndoStack>>,
    f: F,
) -> IpcResult<()>
where
    F: FnOnce(&mut CrystalState) -> IpcResult<()>,
{
    _with_state_update_impl(app, crystal_state, settings_state, renderer_state, undo_state, false, f)
}

pub fn with_structural_state_update<F>(
    app: &AppHandle,
    crystal_state: &State<'_, Mutex<CrystalState>>,
    settings_state: &State<'_, Mutex<AppSettings>>,
    renderer_state: &State<'_, Mutex<Renderer>>,
    undo_state: &State<'_, Mutex<UndoStack>>,
    f: F,
) -> IpcResult<()>
where
    F: FnOnce(&mut CrystalState) -> IpcResult<()>,
{
    _with_state_update_impl(app, crystal_state, settings_state, renderer_state, undo_state, true, f)
}

pub fn with_prepared_state_update<F>(
    app: &AppHandle,
    crystal_state: &State<'_, Mutex<CrystalState>>,
    settings_state: &State<'_, Mutex<AppSettings>>,
    renderer_state: &State<'_, Mutex<Renderer>>,
    undo_state: &State<'_, Mutex<UndoStack>>,
    prepare: F,
) -> IpcResult<()>
where
    F: FnOnce(&CrystalState) -> IpcResult<CrystalState>,
{
    _with_prepared_state_update_impl(
        app,
        crystal_state,
        settings_state,
        renderer_state,
        undo_state,
        false,
        prepare,
    )
}

pub fn with_prepared_state_update_and_refit<F>(
    app: &AppHandle,
    crystal_state: &State<'_, Mutex<CrystalState>>,
    settings_state: &State<'_, Mutex<AppSettings>>,
    renderer_state: &State<'_, Mutex<Renderer>>,
    undo_state: &State<'_, Mutex<UndoStack>>,
    prepare: F,
) -> IpcResult<()>
where
    F: FnOnce(&CrystalState) -> IpcResult<CrystalState>,
{
    _with_prepared_state_update_impl(
        app,
        crystal_state,
        settings_state,
        renderer_state,
        undo_state,
        true,
        prepare,
    )
}

fn _with_prepared_state_update_impl<F>(
    app: &AppHandle,
    crystal_state: &State<'_, Mutex<CrystalState>>,
    settings_state: &State<'_, Mutex<AppSettings>>,
    renderer_state: &State<'_, Mutex<Renderer>>,
    undo_state: &State<'_, Mutex<UndoStack>>,
    refit_camera: bool,
    prepare: F,
) -> IpcResult<()>
where
    F: FnOnce(&CrystalState) -> IpcResult<CrystalState>,
{
    let mut cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let mut prepared = prepare(&cs)?;
    let next_version = cs
        .version
        .checked_add(1)
        .ok_or_else(|| IpcError::from("crystal state version exhausted"))?;
    prepared.version = next_version;
    prepared.invalidate_structure_bound_data();

    let mut u_stack = undo_state
        .lock()
        .map_err(|_| IpcError::lock("undo stack lock poisoned"))?;
    let settings = settings_state
        .lock()
        .map_err(|_| IpcError::lock("settings lock poisoned"))?;
    let instances = crate::wannier::build_atoms_with_ghosts(&prepared, &settings);
    let bond_instances = crate::renderer::instance::build_bond_instances(
        &prepared,
        &settings,
        &prepared.selected_atoms,
    );
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    let previous_state = StructuralSnapshot::from_crystal_state(&cs);

    renderer.clear_structure_bound_overlays();
    renderer.update_atoms(&instances);
    renderer.update_lines(&prepared, &settings);
    renderer.update_bonds(&bond_instances);
    if refit_camera {
        let extent = prepared.cell_a.max(prepared.cell_b).max(prepared.cell_c) as f32;
        let center_vec = glam::Vec3::from_array(prepared.unit_cell_center());
        renderer.camera.eye = center_vec + glam::Vec3::new(0.0, 0.0, extent * 2.0);
        renderer.camera.target = center_vec;
        if !renderer.camera.is_perspective {
            renderer.camera.set_orthographic(extent * 1.5);
        }
        renderer.update_camera();
    }

    *cs = prepared;
    u_stack.push(previous_state);
    let can_undo = u_stack.can_undo();
    let can_redo = u_stack.can_redo();

    drop(renderer);
    drop(settings);
    drop(u_stack);
    drop(cs);

    app.emit("state_changed", StateChangedPayload { version: next_version }).ok();
    app.emit("undo_stack_changed", UndoStackPayload { can_undo, can_redo }).ok();
    Ok(())
}

fn _with_state_update_impl<F>(
    app: &AppHandle,
    crystal_state: &State<'_, Mutex<CrystalState>>,
    settings_state: &State<'_, Mutex<AppSettings>>,
    renderer_state: &State<'_, Mutex<Renderer>>,
    undo_state: &State<'_, Mutex<UndoStack>>,
    invalidate_structure_bound_data: bool,
    f: F,
) -> IpcResult<()>
where
    F: FnOnce(&mut CrystalState) -> IpcResult<()>,
{
    let mut cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    let mut u_stack = undo_state
        .lock()
        .map_err(|_| IpcError::lock("undo stack lock poisoned"))?;
    let settings = settings_state
        .lock()
        .map_err(|_| IpcError::lock("settings lock poisoned"))?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    let next_version = cs
        .version
        .checked_add(1)
        .ok_or_else(|| IpcError::from("crystal state version exhausted"))?;
    let pre_mutation_snapshot = StructuralSnapshot::from_crystal_state(&cs);

    if let Err(error) = f(&mut cs) {
        pre_mutation_snapshot.restore_for_rollback(&mut cs);
        return Err(error);
    }

    if invalidate_structure_bound_data {
        cs.invalidate_structure_bound_data();
        renderer.clear_structure_bound_overlays();
    }

    cs.version = next_version;
    let version = next_version;
    u_stack.push(pre_mutation_snapshot);
    let can_undo = u_stack.can_undo();
    let can_redo = u_stack.can_redo();

    let instances = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
    let bond_instances = crate::renderer::instance::build_bond_instances(&cs, &settings, &cs.selected_atoms);

    renderer.update_atoms(&instances);
    renderer.update_lines(&cs, &settings);
    renderer.update_bonds(&bond_instances);
    
    if let Some(overlay) = &cs.wannier_overlay {
        let hopping_instances = crate::renderer::instance::build_hopping_instances(&overlay.visible_hoppings, overlay.hr_data.t_max);
        renderer.update_hoppings(&hopping_instances);
    }
    
    drop(renderer);
    drop(settings);
    drop(u_stack);
    drop(cs);

    app.emit("state_changed", StateChangedPayload { version }).ok();
    app.emit("undo_stack_changed", UndoStackPayload { can_undo, can_redo }).ok();

    Ok(())
}
