import { useEffect } from 'react';
import { safeInvoke, safeListen } from '../utils/tauri-mock';

interface UseTauriMenuProps {
    setShowAssistant: React.Dispatch<React.SetStateAction<boolean>>;
    setIsSettingsOpen: (isOpen: boolean) => void;
    setIsExportImageOpen: (isOpen: boolean) => void;
    selectedAtomsRef: React.MutableRefObject<number[]>;
    updateSelection: (sel: number[]) => void;
    setPromptConfig: (config: any) => void;
    onStateChange: () => void;
    renderFlagsRef: React.MutableRefObject<{ cell: boolean, bonds: boolean, labels: boolean }>;
    setShowCell: (show: boolean) => void;
    setShowBonds: (show: boolean) => void;
    setShowLabels: (show: boolean) => void;
    setIsPerspective: (isPerspective: boolean) => void;
}

export function useTauriMenu({
    setShowAssistant,
    setIsSettingsOpen,
    setIsExportImageOpen,
    selectedAtomsRef,
    updateSelection,
    setPromptConfig,
    onStateChange,
    renderFlagsRef,
    setShowCell,
    setShowBonds,
    setShowLabels,
    setIsPerspective
}: UseTauriMenuProps) {
    useEffect(() => {
        let unlistenMenu = () => { };
        let unlistenProjection = () => { };

        safeListen<string>('menu-action', (event) => {
            const action = event.payload;
            console.log("Menu action received:", action);

            if (action === 'toggle_dark_mode') {
                document.documentElement.classList.toggle('dark');
            } else if (action === 'toggle_llm_assistant') {
                setShowAssistant(prev => !prev);
            } else if (action === 'view_settings') {
                setIsSettingsOpen(true);
            } else if (action === 'export_image') {
                setIsExportImageOpen(true);
            } else if (action.startsWith('view_axis_')) {
                safeInvoke('set_camera_view_axis', { axis: action.replace('view_axis_', '') })
                    .catch(console.error);
            } else if (action === 'delete_selected') {
                if (selectedAtomsRef.current.length > 0) {
                    safeInvoke('delete_atoms', { indices: selectedAtomsRef.current })
                        .then(() => updateSelection([]))
                        .catch(console.error);
                } else {
                    alert("No atom selected. Please select an atom first.");
                }
            } else if (action === 'open_supercell_dialog') {
                alert("Please use the Supercell Construction panel in the Right Sidebar.");
            } else if (action === 'open_slab_dialog') {
                alert("Please use the Cutting Plane panel in the Right Sidebar.");
            } else if (action === 'open_add_atom_dialog') {
                setPromptConfig({
                    isOpen: true,
                    title: "Add Atom",
                    description: "Enter new element and fractional position (e.g., 'C 0.5 0.5 0.5'):",
                    placeholder: "C 0.5 0.5 0.5",
                    onSubmit: (input: string) => {
                        const parts = input.trim().split(/\s+/);
                        if (parts.length >= 4) {
                            const elem = parts[0];
                            const x = parseFloat(parts[1]);
                            const y = parseFloat(parts[2]);
                            const z = parseFloat(parts[3]);
                            safeInvoke('add_atom', { elementSymbol: elem, atomicNumber: 0, fractPos: [x, y, z] })
                                .then(onStateChange)
                                .catch(e => alert(e));
                        } else {
                            alert("Invalid format. Use 'Symbol X Y Z'.");
                        }
                    }
                });
            } else if (action === 'open_replace_element_dialog') {
                if (selectedAtomsRef.current.length > 0) {
                    setPromptConfig({
                        isOpen: true,
                        title: "Replace Atom(s)",
                        description: "Enter new element symbol (e.g., Fe, O, C):",
                        placeholder: "Element symbol",
                        onSubmit: (newElem: string) => {
                            if (newElem && newElem.trim().length > 0) {
                                safeInvoke('substitute_atoms', {
                                    indices: selectedAtomsRef.current,
                                    newElementSymbol: newElem.trim(),
                                    newAtomicNumber: 0
                                }).then(onStateChange).catch(e => alert(e));
                            }
                        }
                    });
                } else {
                    alert("No atom selected. Please select an atom first.");
                }
            } else if (action.startsWith('show_spacegroup:')) {
                const sg = action.split(':')[1];
                alert(`Space Group Analysis\n\nHermann-Mauguin: ${sg}`);
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
                alert("CrystalCanvas\nVersion 1.0\nPowered by Tauri, React, wgpu, and C++.\nLicense: MIT OR Apache-2.0");
            }
        }).then(f => unlistenMenu = f).catch(console.warn);

        const handleKeyDown = (e: KeyboardEvent) => {
            const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
            const cmdOrCtrl = isMac ? e.metaKey : e.ctrlKey;
            
            if (cmdOrCtrl && e.key.toLowerCase() === 'z') {
                e.preventDefault();
                if (e.shiftKey) {
                    safeInvoke('redo').then(onStateChange).catch(console.error);
                } else {
                    safeInvoke('undo').then(onStateChange).catch(console.error);
                }
            }
        };
        window.addEventListener('keydown', handleKeyDown);

        safeListen<{ is_perspective: boolean }>('view_projection_changed', (event) => {
            setIsPerspective(event.payload.is_perspective);
        }).then(f => unlistenProjection = f).catch(console.warn);

        return () => {
            unlistenMenu();
            unlistenProjection();
            window.removeEventListener('keydown', handleKeyDown);
        };
    }, [
        setShowAssistant, setIsSettingsOpen, setIsExportImageOpen,
        selectedAtomsRef, updateSelection, setPromptConfig,
        onStateChange, renderFlagsRef, setShowCell, setShowBonds, setShowLabels, setIsPerspective
    ]);
}
