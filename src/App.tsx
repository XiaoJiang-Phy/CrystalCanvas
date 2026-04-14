// [Overview: Root React application component managing global state and layout interactions.]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React, { useEffect, useRef, useState, useCallback } from 'react';
import { safeInvoke, safeListen } from './utils/tauri-mock';
import { CrystalState, PhononModeSummary } from './types/crystal';
import { Shell } from './components/layout/Shell';
import { TopNavBar } from './components/layout/TopNavBar';
import { LeftSidebar } from './components/layout/LeftSidebar';
import { RightSidebar } from './components/layout/RightSidebar';
import { BottomStatusBar } from './components/layout/BottomStatusBar';
import { LlmAssistant } from './components/layout/LlmAssistant';
import { SettingsModal } from './components/layout/SettingsModal';
import { PromptModal } from './components/layout/PromptModal';
import { ExportImageModal } from './components/layout/ExportImageModal';
import { useFileDrop } from './hooks/useFileDrop';
import { useTauriMenu } from './hooks/useTauriMenu';
import { useCameraInteraction } from './hooks/useCameraInteraction';
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
    const [isExportImageOpen, setIsExportImageOpen] = useState(false);
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
    const selectedAtomsRef = useRef<number[]>([]);

    const updateSelection = useCallback((sel: number[] | ((prev: number[]) => number[])) => {
        setSelectedAtoms(prev => {
            const next = typeof sel === 'function' ? sel(prev) : sel;
            selectedAtomsRef.current = next;
            return next;
        });
    }, []);

    const [bondCount, setBondCount] = useState<number | undefined>(undefined);
    const [activePhononMode, setActivePhononMode] = useState<PhononModeSummary | null>(null);

    const [promptConfig, setPromptConfig] = useState<{
        isOpen: boolean;
        title: string;
        description?: string;
        placeholder?: string;
        initialValue?: string;
        onSubmit: (value: string) => void;
    }>({ isOpen: false, title: "", onSubmit: () => { } });

    const fetch_crystal_state = useCallback(async () => {
        try {
            const state = await safeInvoke<CrystalState>('get_crystal_state');
            if (state) setCrystalState(state);
        } catch (e) {
            console.error(e);
        }
    }, []);

    useEffect(() => {
        fetch_crystal_state();
        let unlistenStateChanged = () => { };
        safeListen('state_changed', () => {
            fetch_crystal_state();
        }).then(f => unlistenStateChanged = f).catch(console.warn);

        return () => {
            unlistenStateChanged();
        };
    }, []);

    // Menu and File drop event listener
    useFileDrop({ setIsDragging, onFileLoaded: fetch_crystal_state });

    useTauriMenu({
        setShowAssistant,
        setIsSettingsOpen,
        setIsExportImageOpen,
        selectedAtomsRef,
        updateSelection,
        setPromptConfig,
        onStateChange: fetch_crystal_state,
        renderFlagsRef,
        setShowCell,
        setShowBonds,
        setShowLabels,
        setIsPerspective
    });

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

    useCameraInteraction({
        viewportRef,
        interactionMode,
        selectedAtoms,
        updateSelection,
        setContextMenu,
        onStateChange: fetch_crystal_state
    });

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
        safeInvoke('set_camera_projection', { isPerspective: perspective }).catch(console.error);
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
                                updateSelection(sel);
                                safeInvoke('update_selection', { indices: sel }).catch(console.error);
                            }}
                        />
                    </div>
                    <div className="absolute top-0 right-0 bottom-0 pointer-events-none z-10 p-2 pr-3 pb-4">
                        <RightSidebar
                            crystalState={crystalState}
                            selectedAtoms={selectedAtoms}
                            onSelectionChange={(sel) => {
                                updateSelection(sel);
                                safeInvoke('update_selection', { indices: sel }).catch(console.error);
                            }}
                            onBondCountUpdate={setBondCount}
                            onActivePhononModeUpdate={setActivePhononMode}
                            onStructureUpdate={fetch_crystal_state}
                            interactionMode={interactionMode}
                            setInteractionMode={setInteractionMode}
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
                    selectedCount={selectedAtoms.length}
                    interactionMode={interactionMode}
                />

                {/* Right-Click Context Menu */}
                {contextMenu && (
                    <div
                        className="absolute z-[100] bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 shadow-xl rounded-lg overflow-hidden min-w-[180px] pointer-events-auto flex flex-col p-1 backdrop-blur-md"
                        style={{ top: contextMenu.y, left: contextMenu.x }}
                    >
                        <button
                            onClick={() => {
                                setContextMenu(null);
                                setPromptConfig({
                                    isOpen: true,
                                    title: "Add Atom",
                                    description: "What element do you want to add?",
                                    placeholder: "e.g., C, Fe, O",
                                    onSubmit: (elem) => {
                                        if (elem && elem.trim()) {
                                            safeInvoke('add_atom', {
                                                elementSymbol: elem.trim(),
                                                atomicNumber: 0,
                                                fractPos: [0.5, 0.5, 0.5]
                                            })
                                                .then(fetch_crystal_state)
                                                .catch(e => alert(e));
                                        }
                                    }
                                });
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

            <PromptModal
                {...promptConfig}
                onClose={() => setPromptConfig(prev => ({ ...prev, isOpen: false }))}
            />

            <SettingsModal
                isOpen={isSettingsOpen}
                onClose={() => setIsSettingsOpen(false)}
                elements={crystalState?.elements ? Array.from(new Set(crystalState.elements)) : []}
            />

            <ExportImageModal
                isOpen={isExportImageOpen}
                onClose={() => setIsExportImageOpen(false)}
                viewportWidth={viewportRef.current?.clientWidth ?? 1280}
                viewportHeight={viewportRef.current?.clientHeight ?? 800}
            />
        </div>
    );
}

export default App;
