import type { GeneratedIpcArgs, IpcCommandName } from '../ipc/commands.generated';
import { normalize_ipc_error, validate_ipc_event, validate_ipc_result } from '../ipc/contracts';
import type { IpcArgs, IpcEventContract, IpcResult, TypedIpcCommand, TypedIpcEvent } from '../ipc/contracts';
import type { OpenDialogOptions, SaveDialogOptions } from '@tauri-apps/plugin-dialog';

type SafeIpcArgs<Command extends IpcCommandName> = Command extends TypedIpcCommand
    ? IpcArgs<Command>
    : GeneratedIpcArgs<Command>;

type SafeIpcResult<Command extends IpcCommandName> = Command extends TypedIpcCommand
    ? IpcResult<Command>
    : unknown;

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
 * Returns undefined when not running in Tauri.
 */
export function safeInvoke<Command extends IpcCommandName>(
    cmd: Command,
    ...args: SafeIpcArgs<Command> extends undefined ? [] : [args: SafeIpcArgs<Command>]
): Promise<SafeIpcResult<Command> | undefined>;
export async function safeInvoke(
    cmd: IpcCommandName,
    args?: Record<string, unknown>
): Promise<unknown | undefined> {
    if (!isTauri()) {
        // Silently no-op in browser mode
        return undefined;
    }
    try {
        const { invoke } = await import('@tauri-apps/api/core');
        const result = await invoke(cmd, args);
        if (cmd === 'load_wannier_hr' || cmd === 'load_volumetric_file'
            || cmd === 'check_api_key_status' || cmd === 'compute_brillouin_zone'
            || cmd === 'generate_kpath_text' || cmd === 'get_bond_analysis'
            || cmd === 'get_bz_label_positions' || cmd === 'get_crystal_state'
            || cmd === 'get_measurement_labels_screen' || cmd === 'get_settings'
            || cmd === 'llm_chat' || cmd === 'load_axsf_phonon'
            || cmd === 'load_phonon_interactive' || cmd === 'pick_atom'
            || cmd === 'load_cif_file' || cmd === 'export_file'
            || cmd === 'export_image' || cmd === 'write_text_file'
            || cmd === 'set_wannier_t_min' || cmd === 'set_wannier_r_shell'
            || cmd === 'set_wannier_orbital' || cmd === 'toggle_wannier_onsite'
            || cmd === 'toggle_hopping_display' || cmd === 'clear_wannier') {
            return validate_ipc_result(cmd, result);
        }
        return result;
    } catch (e) {
        console.warn(`[tauri-mock] invoke('${cmd}') failed:`, e);
        throw normalize_ipc_error(e);
    }
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
