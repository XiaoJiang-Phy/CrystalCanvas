// [功能概述：主 React 应用组件，包含侧边栏、工具栏和提供透明背景给 wgpu 的占位层]
import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

function App() {
    const viewportRef = useRef<HTMLDivElement>(null);
    const [atomCount, setAtomCount] = useState(0);
    const [isPerspective, setIsPerspective] = useState(true);
    const [isDragging, setIsDragging] = useState(false);

    // File drop event listener
    useEffect(() => {
        const unlistenDrop = listen<{ paths: string[] }>('tauri://file-drop', (event) => {
            setIsDragging(false);
            const path = event.payload.paths[0]; // Take the first dropped file
            if (path && path.endsWith('.cif')) {
                invoke('load_cif_file', { path })
                    .then(() => {
                        // Dummy atom count update for now, to be driven by Rust state later
                        setAtomCount((prev) => prev == 0 ? 512 : prev + 100);
                    })
                    .catch(console.error);
            }
        });

        const unlistenHover = listen('tauri://file-drop-hover', () => setIsDragging(true));
        const unlistenCancel = listen('tauri://file-drop-cancelled', () => setIsDragging(false));

        return () => {
            unlistenDrop.then((f) => f());
            unlistenHover.then((f) => f());
            unlistenCancel.then((f) => f());
        };
    }, []);

    // Setup viewport resize observer to sync dimensions with Rust wgpu
    useEffect(() => {
        if (!viewportRef.current) return;

        const observer = new ResizeObserver((entries) => {
            for (let entry of entries) {
                const { width, height } = entry.contentRect;
                invoke('update_viewport_size', { width: Math.floor(width), height: Math.floor(height) }).catch(console.error);
            }
        });

        observer.observe(viewportRef.current);
        return () => observer.disconnect();
    }, []);

    const handleProjectionChange = (perspective: boolean) => {
        setIsPerspective(perspective);
        invoke('set_camera_projection', { isPerspective: perspective }).catch(console.error);
    };

    return (
        <div className={`w-screen h-screen bg-transparent text-white overflow-hidden flex relative ${isDragging ? 'pointer-events-auto' : 'pointer-events-none'
            }`}>
            {/* Drag Overlay */}
            {isDragging && (
                <div className="absolute inset-0 bg-blue-500/20 backdrop-blur-sm z-50 flex items-center justify-center border-4 border-dashed border-blue-400 m-4 rounded-xl pointer-events-none">
                    <h2 className="text-3xl font-bold text-white drop-shadow-md">Drop .cif file here</h2>
                </div>
            )}

            {/* Sidebar UI */}
            <div className="w-64 h-full bg-slate-900/80 p-4 border-r border-slate-700 pointer-events-auto backdrop-blur-sm flex flex-col shrink-0 z-10">
                <h1 className="text-xl font-bold mb-6 bg-clip-text text-transparent bg-gradient-to-r from-blue-400 to-emerald-400">
                    CrystalCanvas
                </h1>

                <div className="flex-1">
                    <h2 className="text-sm font-semibold text-slate-400 mb-2 uppercase tracking-wide">Structure Info</h2>
                    <div className="bg-slate-800/50 rounded-lg p-3 border border-slate-700/50 mb-4">
                        <div className="flex justify-between items-center mb-1">
                            <span className="text-slate-400 text-sm">Atoms:</span>
                            <span className="font-mono text-emerald-400">{atomCount}</span>
                        </div>
                        <div className="flex justify-between items-center">
                            <span className="text-slate-400 text-sm">Space Group:</span>
                            <span className="font-mono text-blue-400">--</span>
                        </div>
                    </div>

                    <div className="mt-8">
                        <p className="text-xs text-slate-500 italic mb-4">
                            Drop a .cif file anywhere to load.
                        </p>
                        <button
                            className="w-full bg-emerald-600 hover:bg-emerald-500 text-white text-sm font-medium py-2 px-4 rounded transition-colors"
                            onClick={() => {
                                invoke('load_cif_file', { path: '/dummy/path/to/test.cif' })
                                    .then(() => setAtomCount(512)) // Dummy update
                                    .catch(console.error);
                            }}
                        >
                            Test Load CIF (IPC)
                        </button>
                    </div>
                </div>
            </div>

            {/* Right Content Area */}
            <div className="flex-1 flex flex-col relative pointer-events-none">
                {/* Toolbar */}
                <div className="absolute top-4 right-4 z-10 pointer-events-auto flex gap-2">
                    <div className="bg-slate-900/80 backdrop-blur-sm rounded-lg border border-slate-700 p-1 flex">
                        <button
                            className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors ${isPerspective ? 'bg-blue-500/20 text-blue-400' : 'text-slate-400 hover:text-white'
                                }`}
                            onClick={() => handleProjectionChange(true)}
                        >
                            Perspective
                        </button>
                        <button
                            className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors ${!isPerspective ? 'bg-blue-500/20 text-blue-400' : 'text-slate-400 hover:text-white'
                                }`}
                            onClick={() => handleProjectionChange(false)}
                        >
                            Orthographic
                        </button>
                    </div>
                </div>

                {/* 3D Viewport Placeholder (transparent) */}
                <div
                    ref={viewportRef}
                    id="wgpu-viewport"
                    className="flex-1 pointer-events-auto"
                >
                    {/* We will attach a ResizeObserver to this */}
                </div>
            </div>
        </div>
    );
}

export default App;
