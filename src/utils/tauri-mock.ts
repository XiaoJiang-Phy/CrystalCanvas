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
 * Returns undefined when not running in Tauri.
 */
export function safeInvoke<Command extends TypedIpcCommand>(
    cmd: Command,
    ...args: IpcArgs<Command> extends undefined ? [] : [args: IpcArgs<Command>]
): Promise<IpcResult<Command> | (Command extends ReadIpcCommand ? undefined : never)>;
export async function safeInvoke(
    cmd: TypedIpcCommand,
    args?: Record<string, unknown>
): Promise<unknown | undefined> {
    if (!isTauri()) {
        if (IPC_COMMAND_CLASSIFICATION[cmd] === 'read') return undefined;
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
