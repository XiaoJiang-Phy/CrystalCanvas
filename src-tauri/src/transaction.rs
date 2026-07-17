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

#[derive(Clone, Copy)]
pub(crate) struct PendingVersion {
    base: u32,
    next: u32,
}

pub(crate) fn next_version(cs: &CrystalState) -> IpcResult<PendingVersion> {
    let next = cs
        .version
        .checked_add(1)
        .ok_or_else(|| IpcError::from("crystal state version exhausted"))?;
    Ok(PendingVersion {
        base: cs.version,
        next,
    })
}

pub(crate) fn commit_version(
    cs: &mut CrystalState,
    pending: PendingVersion,
) -> IpcResult<u32> {
    if cs.version != pending.base {
        return Err(IpcError::busy("crystal state changed before version commit"));
    }
    cs.version = pending.next;
    Ok(pending.next)
}

pub(crate) fn stamp_version(state: &mut CrystalState, pending: PendingVersion) -> u32 {
    state.version = pending.next;
    pending.next
}

pub fn stamp_next_version(source: &CrystalState, target: &mut CrystalState) -> IpcResult<u32> {
    Ok(stamp_version(target, next_version(source)?))
}

/// A read-only transaction for querying the crystal state without mutation.
pub fn with_state_read<F, R>(crystal_state: &State<'_, Mutex<CrystalState>>, f: F) -> IpcResult<R>
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
pub fn with_state_update<P, F>(
    app: &AppHandle,
    crystal_state: &State<'_, Mutex<CrystalState>>,
    settings_state: &State<'_, Mutex<AppSettings>>,
    renderer_state: &State<'_, Mutex<Renderer>>,
    undo_state: &State<'_, Mutex<UndoStack>>,
    preflight: P,
    f: F,
) -> IpcResult<()>
where
    P: FnOnce(&CrystalState) -> IpcResult<bool>,
    F: FnOnce(&mut CrystalState) -> IpcResult<()>,
{
    _with_state_update_impl(
        app,
        crystal_state,
        settings_state,
        renderer_state,
        undo_state,
        false,
        preflight,
        f,
    )
}

pub fn with_structural_state_update<P, F>(
    app: &AppHandle,
    crystal_state: &State<'_, Mutex<CrystalState>>,
    settings_state: &State<'_, Mutex<AppSettings>>,
    renderer_state: &State<'_, Mutex<Renderer>>,
    undo_state: &State<'_, Mutex<UndoStack>>,
    preflight: P,
    f: F,
) -> IpcResult<()>
where
    P: FnOnce(&CrystalState) -> IpcResult<bool>,
    F: FnOnce(&mut CrystalState) -> IpcResult<()>,
{
    _with_state_update_impl(
        app,
        crystal_state,
        settings_state,
        renderer_state,
        undo_state,
        true,
        preflight,
        f,
    )
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
    prepared
        .validate_cartesian_positions()
        .map_err(IpcError::invalid_argument)?;
    let version = stamp_version(&mut prepared, next_version(&cs)?);
    prepared.invalidate_structure_bound_data();

    let mut u_stack = undo_state
        .lock()
        .map_err(|_| IpcError::lock("undo stack lock poisoned"))?;
    let settings = settings_state
        .lock()
        .map_err(|_| IpcError::lock("settings lock poisoned"))?;
    let atom_scene = crate::renderer::instance::prepare_atom_scene(
        crate::wannier::build_atoms_with_ghosts(&prepared, &settings)?,
    )?;
    let line_scene = crate::renderer::instance::build_line_scene(&prepared, &settings)?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|_| IpcError::lock("renderer lock poisoned"))?;
    let previous_state = StructuralSnapshot::from_crystal_state(&cs);

    renderer.clear_structure_bound_overlays();
    renderer.commit_atoms(atom_scene);
    renderer.update_lines(&line_scene);
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

    app.emit(
        "state_changed",
        StateChangedPayload {
            version,
        },
    )
    .ok();
    app.emit(
        "undo_stack_changed",
        UndoStackPayload { can_undo, can_redo },
    )
    .ok();
    Ok(())
}

fn _with_state_update_impl<P, F>(
    app: &AppHandle,
    crystal_state: &State<'_, Mutex<CrystalState>>,
    settings_state: &State<'_, Mutex<AppSettings>>,
    renderer_state: &State<'_, Mutex<Renderer>>,
    undo_state: &State<'_, Mutex<UndoStack>>,
    invalidate_structure_bound_data: bool,
    preflight: P,
    f: F,
) -> IpcResult<()>
where
    P: FnOnce(&CrystalState) -> IpcResult<bool>,
    F: FnOnce(&mut CrystalState) -> IpcResult<()>,
{
    let mut cs = crystal_state
        .lock()
        .map_err(|_| IpcError::lock("crystal state lock poisoned"))?;
    if !preflight(&cs)? {
        return Ok(());
    }
    let pending_version = next_version(&cs)?;
    let pre_mutation_snapshot = StructuralSnapshot::from_crystal_state(&cs);
    let mut u_stack = undo_state
        .lock()
        .map_err(|_| IpcError::lock("undo stack lock poisoned"))?;
    let settings = settings_state
        .lock()
        .map_err(|_| IpcError::lock("settings lock poisoned"))?;

    if let Err(error) = f(&mut cs) {
        pre_mutation_snapshot.restore_for_rollback(&mut cs);
        return Err(error);
    }
    if let Err(error) = cs.validate_cartesian_positions() {
        pre_mutation_snapshot.restore_for_rollback(&mut cs);
        return Err(IpcError::invalid_argument(error));
    }

    let render_overlay = (!invalidate_structure_bound_data)
        .then(|| cs.wannier_overlay.as_ref())
        .flatten();
    let atom_scene = match crate::wannier::build_atoms_with_ghosts_with_overlay(
        &cs,
        &settings,
        render_overlay,
    )
    .and_then(crate::renderer::instance::prepare_atom_scene)
    {
        Ok(atom_scene) => atom_scene,
        Err(error) => {
            pre_mutation_snapshot.restore_for_rollback(&mut cs);
            return Err(error);
        }
    };
    let line_scene = match crate::renderer::instance::build_line_scene(&cs, &settings) {
        Ok(line_scene) => line_scene,
        Err(error) => {
            pre_mutation_snapshot.restore_for_rollback(&mut cs);
            return Err(error);
        }
    };
    let hopping_instances = render_overlay.map(|overlay| {
        crate::renderer::instance::build_hopping_instances(
            &overlay.visible_hoppings,
            overlay.hr_data.t_max,
        )
    }).transpose();
    let hopping_instances = match hopping_instances {
        Ok(instances) => instances,
        Err(error) => {
            pre_mutation_snapshot.restore_for_rollback(&mut cs);
            return Err(error);
        }
    };
    let mut renderer = match renderer_state.lock() {
        Ok(renderer) => renderer,
        Err(_) => {
            pre_mutation_snapshot.restore_for_rollback(&mut cs);
            return Err(IpcError::lock("renderer lock poisoned"));
        }
    };

    if invalidate_structure_bound_data {
        cs.invalidate_structure_bound_data();
        renderer.clear_structure_bound_overlays();
    }

    let version = commit_version(&mut cs, pending_version)?;
    u_stack.push(pre_mutation_snapshot);
    let can_undo = u_stack.can_undo();
    let can_redo = u_stack.can_redo();

    renderer.commit_atoms(atom_scene);
    renderer.update_lines(&line_scene);
    
    if let Some(hopping_instances) = &hopping_instances {
        renderer.update_hoppings(hopping_instances);
    }
    
    drop(renderer);
    drop(settings);
    drop(u_stack);
    drop(cs);

    app.emit("state_changed", StateChangedPayload { version })
        .ok();
    app.emit(
        "undo_stack_changed",
        UndoStackPayload { can_undo, can_redo },
    )
    .ok();

    Ok(())
}
