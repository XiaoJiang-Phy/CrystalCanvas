// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React, { useEffect, useRef, useState } from 'react';
import { safeInvoke, safeListen } from './utils/tauri-mock';
import { CrystalState } from './types/crystal';
import { Shell } from './components/layout/Shell';
import { TopNavBar } from './components/layout/TopNavBar';
import { LeftSidebar } from './components/layout/LeftSidebar';
import { RightSidebar } from './components/layout/RightSidebar';
import { BottomStatusBar } from './components/layout/BottomStatusBar';
import { LlmAssistant } from './components/layout/LlmAssistant';

function App() {
    const viewportRef = useRef<HTMLDivElement>(null);
    const [isDragging, setIsDragging] = useState(false);
    const [showAssistant, setShowAssistant] = useState(true);

    const [contextMenu, setContextMenu] = useState<{ x: number, y: number } | null>(null);
    const [crystalState, setCrystalState] = useState<CrystalState | null>(null);

    const fetchCrystalState = async () => {
        try {
            const state = await safeInvoke<CrystalState>('get_crystal_state');
            if (state) setCrystalState(state);
        } catch (e) {
            console.error(e);
        }
    };

    useEffect(() => {
        fetchCrystalState();
        const interval = setInterval(fetchCrystalState, 1000);
        return () => clearInterval(interval);
    }, []);

    // Menu and File drop event listener
    useEffect(() => {
        let unlistenDrop = () => { };
        let unlistenHover = () => { };
        let unlistenCancel = () => { };

        safeListen<{ paths: string[] }>('tauri://file-drop', (event) => {
            setIsDragging(false);
            const path = event.payload.paths[0];
            if (path && path.endsWith('.cif')) {
                safeInvoke('load_cif_file', { path })
                    .then(fetchCrystalState)
                    .catch(console.error);
            }
        }).then(f => unlistenDrop = f).catch(console.warn);

        safeListen('tauri://file-drop-hover', () => setIsDragging(true)).then(f => unlistenHover = f).catch(console.warn);
        safeListen('tauri://file-drop-cancelled', () => setIsDragging(false)).then(f => unlistenCancel = f).catch(console.warn);

        return () => {
            unlistenDrop();
            unlistenHover();
            unlistenCancel();
        };
    }, []);

    // ResizeObserver to update viewport bounds to wgpu backend
    useEffect(() => {
        if (!viewportRef.current) return;

        const observer = new ResizeObserver((entries) => {
            for (const entry of entries) {
                const { width, height } = entry.contentRect;
                safeInvoke('update_viewport_size', {
                    width: Math.max(1, Math.round(width)),
                    height: Math.max(1, Math.round(height))
                }).catch(() => { });
            }
        });

        observer.observe(viewportRef.current);

        return () => {
            observer.disconnect();
        };
    }, []);

    const handleContextMenu = (e: React.MouseEvent) => {
        e.preventDefault();
        setContextMenu({ x: e.clientX, y: e.clientY });
    };

    return (
        <div onContextMenu={handleContextMenu} onClick={() => setContextMenu(null)} className="w-full h-full">
            <Shell viewportRef={viewportRef}>

                <TopNavBar
                    showAssistant={showAssistant}
                    onToggleAssistant={() => setShowAssistant(prev => !prev)}
                />

                {/* Middle Section: Sidebars + Spacer */}
                <div className="flex-1 flex justify-between overflow-hidden relative">
                    <LeftSidebar crystalState={crystalState} />
                    <RightSidebar crystalState={crystalState} />

                    {/* Drag Overlay */}
                    {isDragging && (
                        <div className="absolute inset-0 z-50 bg-emerald-500/10 backdrop-blur-sm border-2 border-dashed border-emerald-500 rounded-xl m-4 flex items-center justify-center pointer-events-none">
                            <div className="bg-white dark:bg-slate-800 px-6 py-4 rounded-xl shadow-xl font-medium text-emerald-600 dark:text-emerald-400">
                                Drop .cif file here to load
                            </div>
                        </div>
                    )}
                </div>

                {/* Overlays */}
                <LlmAssistant isOpen={showAssistant} onClose={() => setShowAssistant(false)} />
                <BottomStatusBar />

                {/* Right-Click Context Menu */}
                {contextMenu && (
                    <div
                        className="absolute z-[100] bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 shadow-xl rounded-lg overflow-hidden min-w-[180px] pointer-events-auto flex flex-col p-1 backdrop-blur-md"
                        style={{ top: contextMenu.y, left: contextMenu.x }}
                    >
                        <button className="text-left px-3 py-2 text-sm hover:bg-emerald-50 dark:hover:bg-slate-700 rounded-md transition-colors text-slate-700 dark:text-slate-300 w-full flex items-center gap-2">
                            <span className="w-3 h-3 bg-emerald-500 rounded-full shadow-sm" /> Add Atom
                        </button>
                        <div className="h-px bg-slate-200 dark:bg-slate-700 my-1" />
                        <button disabled className="text-left px-3 py-2 text-sm transition-colors text-slate-400 dark:text-slate-500 cursor-not-allowed w-full rounded-md">
                            Properties...
                        </button>
                    </div>
                )}
            </Shell>
        </div>
    );
}

export default App;
