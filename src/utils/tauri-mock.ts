import { IpcException, normalize_ipc_error, validate_ipc_event, validate_ipc_result } from '../ipc/contracts';
import type { IpcArgs, IpcEventContract, IpcResult, TypedIpcCommand, TypedIpcEvent } from '../ipc/contracts';
import { IPC_COMMAND_CLASSIFICATION } from '../ipc/commands.generated';
import type { ReadIpcCommand } from '../ipc/commands.generated';
import type { OpenDialogOptions, SaveDialogOptions } from '@tauri-apps/plugin-dialog';

declare global {
    interface Window {
        __TAURI_INTERNALS__?: unknown;
    }
}

// [Overview: Safe Tauri API wrappers for seamless browser fallback development.]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
/**
 * Tauri API shim for browser-only environments.
 * When running in a browser (npm run dev without Tauri), the @tauri-apps/api
 * modules throw because __TAURI_INTERNALS__ is not defined.
 * This module provides safe wrappers that no-op gracefully.
 */

const isTauri = (): boolean => {
    return typeof window.__TAURI_INTERNALS__ !== 'undefined';
};

/**
 * Safe wrapper around Tauri's invoke.
 * Browser reads receive neutral validated fixtures; mutations reject explicitly.
 */
export function safeInvoke<Command extends TypedIpcCommand>(
    cmd: Command,
    ...args: IpcArgs<Command> extends undefined ? [] : [args: IpcArgs<Command>]
): Promise<IpcResult<Command>>;
export async function safeInvoke(
    cmd: TypedIpcCommand,
    args?: Record<string, unknown>
): Promise<unknown> {
    if (!isTauri()) {
        if (browser_policy_for(cmd) === 'fixture') {
            return browser_read_fixture(cmd as ReadIpcCommand);
        }
        throw new IpcException({
            code: 'not_in_tauri',
            message: `IPC command '${cmd}' requires the Tauri runtime`,
            recoverable: false,
        });
    }
    try {
        const { invoke } = await import('@tauri-apps/api/core');
        const result = await invoke(cmd, args);
        return validate_ipc_result(cmd, result);
    } catch (e) {
        console.warn(`[tauri-mock] invoke('${cmd}') failed:`, e);
        throw normalize_ipc_error(e);
    }
}

type BrowserReadFixtureFactory = {
    [Command in ReadIpcCommand]: () => IpcResult<Command>;
};

const browser_read_fixtures: BrowserReadFixtureFactory = {
    check_api_key_status: () => false,
    generate_kpath_text: () => ({ qe: '', vasp: '' }),
    get_bond_analysis: () => ({
        bonds: [],
        coordination: [],
        bond_length_stats: [],
        distortion_indices: [],
        threshold_factor: 1,
    }),
    get_bz_label_positions: () => [],
    get_crystal_state: empty_crystal_state,
    get_kpath_info: () => ({ points: [], segments: [] }),
    get_measurement_labels_screen: () => [],
    get_measurements: () => [],
    get_settings: () => ({
        atom_scale: 1,
        bond_tolerance: 0,
        bond_radius: 0,
        bond_color: [0, 0, 0, 1],
        custom_atom_colors: {},
    }),
    get_volumetric_info: () => null,
    pick_atom: () => null,
    preview_slab: empty_crystal_state,
    preview_supercell: empty_crystal_state,
};

function browser_policy_for(command: TypedIpcCommand): 'fixture' | 'reject' {
    return IPC_COMMAND_CLASSIFICATION[command] === 'read' ? 'fixture' : 'reject';
}

function browser_read_fixture<Command extends ReadIpcCommand>(command: Command): IpcResult<Command> {
    const factory = browser_read_fixtures[command] as () => IpcResult<Command>;
    return validate_ipc_result(command, factory());
}

function empty_crystal_state(): IpcResult<'get_crystal_state'> {
    return {
        name: '',
        cell_a: 0,
        cell_b: 0,
        cell_c: 0,
        cell_alpha: 0,
        cell_beta: 0,
        cell_gamma: 0,
        spacegroup_hm: '',
        spacegroup_number: 0,
        labels: [],
        elements: [],
        atomic_numbers: [],
        fract_x: [],
        fract_y: [],
        fract_z: [],
        occupancies: [],
        cart_positions: [],
        version: 0,
        intrinsic_sites: 0,
        is_2d: false,
        vacuum_axis: null,
        measurements: [],
    };
}

/**
 * Safe wrapper around Tauri's listen.
 * Returns a no-op unlisten function when not running in Tauri.
 */
export async function safeListen<Event extends TypedIpcEvent>(
    event: Event,
    handler: (event: { payload: IpcEventContract[Event] }) => void
): Promise<() => void> {
    if (!isTauri()) {
        return () => {};
    }
    try {
        const { listen } = await import('@tauri-apps/api/event');
        return await listen<unknown>(event, (message) => {
            try {
                handler({ payload: validate_ipc_event(event, message.payload) });
            } catch (error) {
                console.error(`[tauri-mock] invalid payload for '${event}':`, error);
            }
        });
    } catch (e) {
        console.warn(`[tauri-mock] listen('${event}') failed:`, e);
        return () => {};
    }
}

/**
 * Safe wrapper around Tauri dialog plugin's open.
 */
export async function safeDialogOpen(options?: OpenDialogOptions): Promise<string | string[] | null | undefined> {
    if (!isTauri()) return undefined;
    try {
        const { open } = await import('@tauri-apps/plugin-dialog');
        return await open(options);
    } catch (e) {
        console.warn(`[tauri-mock] dialog.open failed:`, e);
        return undefined;
    }
}

/**
 * Safe wrapper around Tauri dialog plugin's save.
 */
export async function safeDialogSave(options?: SaveDialogOptions): Promise<string | null | undefined> {
    if (!isTauri()) return undefined;
    try {
        const { save } = await import('@tauri-apps/plugin-dialog');
        return await save(options);
    } catch (e) {
        console.warn(`[tauri-mock] dialog.save failed:`, e);
        return undefined;
    }
}
