/**
 * Tauri API shim for browser-only environments.
 * When running in a browser (npm run dev without Tauri), the @tauri-apps/api
 * modules throw because __TAURI_INTERNALS__ is not defined.
 * This module provides safe wrappers that no-op gracefully.
 */
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

const isTauri = (): boolean => {
    return typeof (window as any).__TAURI_INTERNALS__ !== 'undefined';
};

/**
 * Safe wrapper around Tauri's invoke.
 * Returns undefined when not running in Tauri.
 */
export async function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T | undefined> {
    if (!isTauri()) {
        // Silently no-op in browser mode
        return undefined;
    }
    try {
        const { invoke } = await import('@tauri-apps/api/core');
        return await invoke<T>(cmd, args);
    } catch (e) {
        console.warn(`[tauri-mock] invoke('${cmd}') failed:`, e);
        return undefined;
    }
}

/**
 * Safe wrapper around Tauri's listen.
 * Returns a no-op unlisten function when not running in Tauri.
 */
export async function safeListen<T>(
    event: string,
    handler: (event: { payload: T }) => void
): Promise<() => void> {
    if (!isTauri()) {
        return () => {};
    }
    try {
        const { listen } = await import('@tauri-apps/api/event');
        return await listen<T>(event, handler);
    } catch (e) {
        console.warn(`[tauri-mock] listen('${event}') failed:`, e);
        return () => {};
    }
}
