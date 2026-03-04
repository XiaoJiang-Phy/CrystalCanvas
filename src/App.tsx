// [Overview: Root React application component managing global state and layout interactions.]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React, { useEffect, useRef, useState } from 'react';
import { safeInvoke, safeListen } from './utils/tauri-mock';
import { CrystalState, PhononModeSummary } from './types/crystal';
import { Shell } from './components/layout/Shell';
import { TopNavBar } from './components/layout/TopNavBar';
import { LeftSidebar } from './components/layout/LeftSidebar';
import { RightSidebar } from './components/layout/RightSidebar';
import { BottomStatusBar } from './components/layout/BottomStatusBar';
import { LlmAssistant } from './components/layout/LlmAssistant';
import { SettingsModal } from './components/layout/SettingsModal';

function App() {
    const viewportRef = useRef<HTMLDivElement>(null);
    const [isDragging, setIsDragging] = useState(false);
    const [showAssistant, setShowAssistant] = useState(true);

    // Global UI State
    const [isPerspective, setIsPerspective] = useState(false);
    const [showCell, setShowCell] = useState(true);
    const [showBonds, setShowBonds] = useState(true);
    const [showLabels, setShowLabels] = useState(false);
    const [isSettingsOpen, setIsSettingsOpen] = useState(false);
    const renderFlagsRef = useRef({ cell: true, bonds: true, labels: false });
    const [atomScale, setAtomScale] = useState(1.0);
    const [interactionMode, setInteractionMode] = useState<'select' | 'move' | 'rotate' | 'measure'>('rotate');

    // Canvas Interaction State
    const isDraggingCamera = useRef(false);
    const lastMousePos = useRef({ x: 0, y: 0 });
    const pointerDownPos = useRef({ x: 0, y: 0 });

    const [contextMenu, setContextMenu] = useState<{ x: number, y: number } | null>(null);
    const [crystalState, setCrystalState] = useState<CrystalState | null>(null);
    const [selectedAtoms, setSelectedAtoms] = useState<number[]>([]);
    const [bondCount, setBondCount] = useState<number | undefined>(undefined);
    const [activePhononMode, setActivePhononMode] = useState<PhononModeSummary | null>(null);

    const fetch_crystal_state = async () => {
        try {
            const state = await safeInvoke<CrystalState>('get_crystal_state');
            if (state) setCrystalState(state);
        } catch (e) {
            console.error(e);
        }
    };

    useEffect(() => {
        fetch_crystal_state();
        const interval = setInterval(fetch_crystal_state, 1000);
        return () => clearInterval(interval);
    }, []);

    // Menu and File drop event listener
    useEffect(() => {
        let unlistenDrop = () => { };
        let unlistenHover = () => { };
        let unlistenCancel = () => { };
        let unlistenMenu = () => { };
        let unlistenProjection = () => { };

        safeListen<{ paths: string[] }>('tauri://file-drop', (event) => {
            setIsDragging(false);
            const path = event.payload.paths[0];
            if (path && path.endsWith('.cif')) {
                safeInvoke('load_cif_file', { path })
                    .then(fetch_crystal_state)
                    .catch(console.error);
            }
        }).then(f => unlistenDrop = f).catch(console.warn);

        safeListen('tauri://file-drop-hover', () => setIsDragging(true)).then(f => unlistenHover = f).catch(console.warn);
        safeListen('tauri://file-drop-cancelled', () => setIsDragging(false)).then(f => unlistenCancel = f).catch(console.warn);

        safeListen<string>('menu-action', (event) => {
            const action = event.payload;
            console.log("Menu action received:", action);

            if (action === 'toggle_dark_mode') {
                document.documentElement.classList.toggle('dark');
            } else if (action === 'toggle_llm_assistant') {
                setShowAssistant(prev => !prev);
            } else if (action === 'view_settings') {
                setIsSettingsOpen(true);
            } else if (action.startsWith('view_axis_')) {
                safeInvoke('set_camera_view_axis', { axis: action.replace('view_axis_', '') })
                    .catch(console.error);
            } else if (action === 'delete_selected') {
                if (selectedAtoms.length > 0) {
                    safeInvoke('delete_atoms', { indices: selectedAtoms })
                        .then(() => setSelectedAtoms([]))
                        .catch(console.error);
                } else {
                    alert("No atom selected. Please select an atom first.");
                }
            } else if (action === 'open_supercell_dialog') {
                alert("Please use the Supercell Construction panel in the Right Sidebar.");
            } else if (action === 'open_slab_dialog') {
                alert("Please use the Cutting Plane panel in the Right Sidebar.");
            } else if (action === 'open_add_atom_dialog') {
                const input = window.prompt("Enter new element and fractional position (e.g., 'C 0.5 0.5 0.5'):");
                if (input) {
                    const parts = input.trim().split(/\s+/);
                    if (parts.length >= 4) {
                        const elem = parts[0];
                        const x = parseFloat(parts[1]);
                        const y = parseFloat(parts[2]);
                        const z = parseFloat(parts[3]);
                        safeInvoke('add_atom', { element_symbol: elem, atomic_number: 0, fract_pos: [x, y, z] }).catch(console.error);
                    } else {
                        alert("Invalid format. Use 'Symbol X Y Z'.");
                    }
                }
            } else if (action === 'open_replace_element_dialog') {
                if (selectedAtoms.length > 0) {
                    const newElem = window.prompt("Enter new element symbol (e.g., Fe, O, C):");
                    if (newElem && newElem.trim().length > 0) {
                        safeInvoke('substitute_atoms', {
                            indices: selectedAtoms,
                            newElementSymbol: newElem.trim(),
                            newAtomicNumber: 0
                        }).catch(console.error);
                    }
                } else {
                    alert("No atom selected. Please select an atom first.");
                }
            } else if (action.startsWith('show_spacegroup:')) {
                const sg = action.split(':')[1];
                alert(`Space Group Analysis\\n\\nHermann-Mauguin: ${sg}`);
            } else if (action.startsWith('toggle_')) {
                const flag = action.replace('toggle_', '');
                if (flag === 'cell') {
                    renderFlagsRef.current.cell = !renderFlagsRef.current.cell;
                    setShowCell(renderFlagsRef.current.cell);
                }
                else if (flag === 'bonds') {
                    renderFlagsRef.current.bonds = !renderFlagsRef.current.bonds;
                    setShowBonds(renderFlagsRef.current.bonds);
                }
                else if (flag === 'labels') {
                    renderFlagsRef.current.labels = !renderFlagsRef.current.labels;
                    setShowLabels(renderFlagsRef.current.labels);
                }

                safeInvoke('set_render_flags', {
                    showCell: renderFlagsRef.current.cell,
                    showBonds: renderFlagsRef.current.bonds
                }).then(() => {
                    console.log('[App] set_render_flags OK:', { showCell: renderFlagsRef.current.cell, showBonds: renderFlagsRef.current.bonds });
                }).catch(console.error);
            } else if (action === 'show_about') {
                alert("CrystalCanvas\\nVersion 1.0\\nPowered by Tauri, React, wgpu, and C++.\\nLicense: MIT OR Apache-2.0");
            }
        }).then(f => unlistenMenu = f).catch(console.warn);
        // Listen for projection changes explicitly triggered by the Rust backend
        safeListen<{ is_perspective: boolean }>('view_projection_changed', (event) => {
            setIsPerspective(event.payload.is_perspective);
        }).then((f: () => void) => unlistenProjection = f).catch(console.warn);

        return () => {
            unlistenDrop();
            unlistenHover();
            unlistenCancel();
            unlistenMenu();
            unlistenProjection();
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

    // Attach pointer and wheel events directly to the viewport div
    // so that sidebar/topbar clicks don't trigger camera operations
    useEffect(() => {
        const el = viewportRef.current;
        if (!el) return;

        const onPointerDown = (e: PointerEvent) => {
            if (e.button !== 0 && e.button !== 1 && e.button !== 2) return;
            if (e.button === 2) {
                setContextMenu({ x: e.clientX, y: e.clientY });
                return;
            }

            pointerDownPos.current = { x: e.clientX, y: e.clientY };

            if (interactionMode === 'rotate' || interactionMode === 'move' || e.button === 1) {
                isDraggingCamera.current = true;
                lastMousePos.current = { x: e.clientX, y: e.clientY };
                el.setPointerCapture(e.pointerId);
            }
        };

        const onPointerMove = (e: PointerEvent) => {
            if (!isDraggingCamera.current) return;
            const dx = e.clientX - lastMousePos.current.x;
            const dy = e.clientY - lastMousePos.current.y;
            lastMousePos.current = { x: e.clientX, y: e.clientY };
            if (e.buttons === 4 || (e.buttons === 1 && interactionMode === 'move')) {
                safeInvoke('pan_camera', { dx, dy }).catch(console.error);
            } else if (e.buttons === 1 && interactionMode === 'rotate') {
                safeInvoke('rotate_camera', { dx: dx * 1.0, dy: dy * 1.0 }).catch(console.error);
            }
        };

        const onPointerUp = (e: PointerEvent) => {
            if (isDraggingCamera.current) {
                isDraggingCamera.current = false;
                el.releasePointerCapture(e.pointerId);
            }
            if (e.button === 0 && interactionMode === 'select') {
                const ddx = Math.abs(e.clientX - pointerDownPos.current.x);
                const ddy = Math.abs(e.clientY - pointerDownPos.current.y);
                if (ddx < 5 && ddy < 5) {
                    const rect = el.getBoundingClientRect();
                    const dpr = window.devicePixelRatio || 1;
                    const x = (e.clientX - rect.left) * dpr;
                    const y = (e.clientY - rect.top) * dpr;
                    safeInvoke<number | null>('pick_atom', {
                        x, y,
                        screenW: rect.width * dpr,
                        screenH: rect.height * dpr
                    }).then((idx) => {
                        console.log("pick_atom returned:", idx, "at", { x, y });
                        setSelectedAtoms(prev => {
                            let newSel = [...prev];
                            if (idx !== null && idx !== undefined) {
                                if (e.shiftKey) {
                                    if (newSel.includes(idx)) {
                                        newSel = newSel.filter(i => i !== idx); // Toggle off
                                    } else {
                                        newSel.push(idx); // Toggle on
                                    }
                                } else {
                                    newSel = [idx]; // Single selection replaces all
                                }
                            } else {
                                if (!e.shiftKey) {
                                    newSel = []; // Clicking empty space clears selection unless shift is held
                                }
                            }
                            safeInvoke('update_selection', { indices: newSel }).catch(console.error);
                            return newSel;
                        });
                    }).catch((err) => {
                        console.error("pick_atom error:", err);
                    });
                }
            }
        };

        const onWheel = (e: WheelEvent) => {
            e.preventDefault();
            safeInvoke('zoom_camera', { delta: Math.sign(e.deltaY) }).catch(console.error);
        };

        el.addEventListener('pointerdown', onPointerDown);
        el.addEventListener('pointermove', onPointerMove);
        el.addEventListener('pointerup', onPointerUp);
        el.addEventListener('pointercancel', onPointerUp);
        el.addEventListener('wheel', onWheel, { passive: false });

        return () => {
            el.removeEventListener('pointerdown', onPointerDown);
            el.removeEventListener('pointermove', onPointerMove);
            el.removeEventListener('pointerup', onPointerUp);
            el.removeEventListener('pointercancel', onPointerUp);
            el.removeEventListener('wheel', onWheel);
        };
    }, [interactionMode]);

    const handle_context_menu = (e: React.MouseEvent) => {
        e.preventDefault();
        setContextMenu({ x: e.clientX, y: e.clientY });
    };

    const toggle_cell = () => {
        const next = !renderFlagsRef.current.cell;
        renderFlagsRef.current.cell = next;
        setShowCell(next);
        safeInvoke('set_render_flags', { showCell: next, showBonds: renderFlagsRef.current.bonds }).catch(console.error);
    };

    const handle_set_perspective = (perspective: boolean) => {
        setIsPerspective(perspective);
        safeInvoke('set_camera_projection', { is_perspective: perspective }).catch(console.error);
    };

    const toggle_bonds = () => {
        const next = !renderFlagsRef.current.bonds;
        renderFlagsRef.current.bonds = next;
        setShowBonds(next);
        safeInvoke('set_render_flags', { showCell: renderFlagsRef.current.cell, showBonds: next }).catch(console.error);
    };

    const toggle_labels = () => {
        const next = !renderFlagsRef.current.labels;
        renderFlagsRef.current.labels = next;
        setShowLabels(next);
        // Implement when text rendering is ready
    };



    return (
        <div
            onContextMenu={(e) => e.preventDefault()}
            onClick={() => setContextMenu(null)}
            className="w-full h-full touch-none"
        >
            <Shell viewportRef={viewportRef}>

                <TopNavBar
                    showAssistant={showAssistant}
                    onToggleAssistant={() => setShowAssistant(prev => !prev)}
                    showLabels={showLabels}
                    onToggleLabels={() => {
                        const next = !showLabels;
                        setShowLabels(next);
                        // TODO: safeInvoke('set_render_flags', ...) when available
                    }}
                    onOpenSettings={() => setIsSettingsOpen(true)}
                    interactionMode={interactionMode}
                    setInteractionMode={setInteractionMode}
                />

                {/* Middle Section: Sidebars + Spacer */}
                <div className="flex-1 flex justify-between overflow-hidden relative">
                    {/* Left Sidebar overlaying canvas */}
                    <div className="absolute top-16 left-0 bottom-0 pointer-events-none z-10 p-2 pl-3 pb-4">
                        <LeftSidebar
                            crystalState={crystalState}
                            selectedAtoms={selectedAtoms}
                            onSelectionChange={(sel) => {
                                setSelectedAtoms(sel);
                                safeInvoke('update_selection', { indices: sel }).catch(console.error);
                            }}
                        />
                    </div>
                    <div className="absolute top-0 right-0 bottom-0 pointer-events-none z-10 p-2 pr-3 pb-4">
                        <RightSidebar
                            crystalState={crystalState}
                            selectedAtoms={selectedAtoms}
                            onSelectionChange={(sel) => {
                                setSelectedAtoms(sel);
                                safeInvoke('update_selection', { indices: sel }).catch(console.error);
                            }}
                            onBondCountUpdate={setBondCount}
                            onActivePhononModeUpdate={setActivePhononMode}
                        />
                    </div>

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
                <BottomStatusBar
                    crystalState={crystalState}
                    bondCount={bondCount}
                    activePhononMode={activePhononMode}
                />

                {/* Right-Click Context Menu */}
                {contextMenu && (
                    <div
                        className="absolute z-[100] bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 shadow-xl rounded-lg overflow-hidden min-w-[180px] pointer-events-auto flex flex-col p-1 backdrop-blur-md"
                        style={{ top: contextMenu.y, left: contextMenu.x }}
                    >
                        <button
                            onClick={() => {
                                safeInvoke('add_atom', { element_symbol: "C", atomic_number: 6, fract_pos: [0.5, 0.5, 0.5] }).catch(console.error);
                                setContextMenu(null);
                            }}
                            className="text-left px-3 py-2 text-sm hover:bg-emerald-50 dark:hover:bg-slate-700 rounded-md transition-colors text-slate-700 dark:text-slate-300 w-full flex items-center gap-2"
                        >
                            <span className="w-3 h-3 bg-emerald-500 rounded-full shadow-sm" /> Add Atom
                        </button>
                        <div className="h-px bg-slate-200 dark:bg-slate-700 my-1" />
                        <button disabled className="text-left px-3 py-2 text-sm transition-colors text-slate-400 dark:text-slate-500 cursor-not-allowed w-full rounded-md">
                            Properties...
                        </button>
                    </div>
                )}
            </Shell>
            <SettingsModal
                isOpen={isSettingsOpen}
                onClose={() => setIsSettingsOpen(false)}
                elements={crystalState?.elements ? Array.from(new Set(crystalState.elements)) : []}
            />
        </div>
    );
}

export default App;
