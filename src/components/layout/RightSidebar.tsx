import React, { useState, useEffect, useCallback, useRef } from 'react';
import ReactDOM from 'react-dom';
import { cn } from '../../utils/cn';
import { safeInvoke, safeListen, safeDialogOpen, safeDialogSave } from '../../utils/tauri-mock';
import { CrystalState, BondAnalysisResult, PhononModeSummary, BzInfo, WannierInfo } from '../../types/crystal';
import { PromptModal } from './PromptModal';
import { PhononImportModal } from './PhononImportModal';

export const RightSidebar: React.FC<{
    crystalState: CrystalState | null,
    selectedAtoms?: number[],
    onSelectionChange?: (indices: number[]) => void,
    onBondCountUpdate?: (count: number) => void,
    onActivePhononModeUpdate?: (mode: PhononModeSummary | null) => void,
    onStructureUpdate?: () => void,
    interactionMode?: 'select' | 'move' | 'rotate' | 'measure',
    setInteractionMode?: (mode: 'select' | 'move' | 'rotate' | 'measure') => void
}> = ({ crystalState, selectedAtoms = [], onSelectionChange, onBondCountUpdate, onActivePhononModeUpdate, onStructureUpdate, interactionMode, setInteractionMode }) => {

    const [sc, setSc] = useState({ nx: 1, ny: 1, nz: 1 });
    const [slab, setSlab] = useState({ h: 1, k: 1, l: 1, layers: 3, vacuum: 15.0 });
    const [promptConfig, setPromptConfig] = useState<{
        isOpen: boolean;
        title: string;
        description?: string;
        placeholder?: string;
        initialValue?: string;
        onSubmit: (value: string) => void;
    }>({ isOpen: false, title: "", onSubmit: () => { } });

    const [bondAnalysis, setBondAnalysis] = useState<BondAnalysisResult | null>(null);
    const [phononModes, setPhononModes] = useState<PhononModeSummary[] | null>(null);
    const [activeModeIdx, setActiveModeIdx] = useState<number | null>(null);
    const [isAnimating, setIsAnimating] = useState(false);
    const [amplitude, setAmplitude] = useState(1.0);
    const [openAccordion, setOpenAccordionRaw] = useState<string | null>(null);
    const [isPhononModalOpen, setIsPhononModalOpen] = useState(false);
    const previousModeRef = useRef<'select' | 'move' | 'rotate' | 'measure'>('rotate');

    const setOpenAccordion = useCallback((key: string | null) => {
        setOpenAccordionRaw(prev => {
            if (key === 'Measurements' && prev !== 'Measurements' && setInteractionMode) {
                previousModeRef.current = interactionMode || 'rotate';
                setInteractionMode('measure');
            } else if (prev === 'Measurements' && key !== 'Measurements' && setInteractionMode) {
                setInteractionMode(previousModeRef.current);
            }
            return key;
        });
    }, [setInteractionMode, interactionMode]);

    // Volumetric State
    const [volumetricInfo, setVolumetricInfo] = useState<any | null>(null);
    const [volumetricRange, setVolumetricRange] = useState<{min: number, max: number}>({min: -1.0, max: 1.0});
    const [isovalue, setIsovalue] = useState(0.05);

    // Reciprocal Space State
    const [bzInfo, setBzInfo] = useState<BzInfo | null>(null);
    const [isBzVisible, setIsBzVisible] = useState(false);
    const [bzScale, setBzScale] = useState(0.35);
    const [bzLabels, setBzLabels] = useState<{label: string, x: number, y: number}[]>([]);

    // Wannier Hopping State
    const [wannierInfo, setWannierInfo] = useState<WannierInfo | null>(null);
    const [tMin, setTMin] = useState<number>(0.01);
    const [activeRShells, setActiveRShells] = useState<boolean[]>([]);
    const [activeOrbitals, setActiveOrbitals] = useState<boolean[]>([]);
    const [showOnSite, setShowOnSite] = useState<boolean>(false);
    const [isWannierVisible, setIsWannierVisible] = useState<boolean>(false);

    // Measurement Render State
    const [measurementLabels, setMeasurementLabels] = useState<{label: string, x: number, y: number}[]>([]);


    const fetch_bz_labels = useCallback(async () => {
        const w = window.innerWidth;
        const h = window.innerHeight;
        try {
            const labels = await safeInvoke<{label: string, x: number, y: number}[]>('get_bz_label_positions', { width: w, height: h });
            if (labels) setBzLabels(labels);
        } catch (_e) {
            setBzLabels([]);
        }
    }, []);

    // Live update measurement labels
    useEffect(() => {
        if (!crystalState?.measurements || crystalState.measurements.length === 0) {
            setMeasurementLabels([]);
            return;
        }
        
        let active = true;
        
        const updateLabels = async () => {
            if (!active) return;
            try {
                const w = window.innerWidth;
                const h = window.innerHeight;
                const labels = await safeInvoke<{label: string, x: number, y: number}[]>('get_measurement_labels_screen', { width: w, height: h });
                if (active) setMeasurementLabels(labels || []);
            } catch (e) {
                if (active) setMeasurementLabels([]);
            }
            if (active) {
                requestAnimationFrame(updateLabels);
            }
        };
        
        updateLabels();
        
        return () => {
            active = false;
        };
    }, [crystalState?.measurements]);

    useEffect(() => {
        let unlisten = () => {};
        safeListen<any>('volumetric_loaded', (event) => {
            const info = event.payload;
            if (info) {
                setVolumetricInfo(info);
                let dMin = info.data_min;
                let dMax = info.data_max;
                setVolumetricRange({ min: dMin, max: dMax });
                
                // For signed data (like charge diff or orbitals), use 10% of the absolute max
                // For positive-only data, use 10% of the max
                let defaultIsovalue = 0.05;
                if (dMin < 0.0) {
                    defaultIsovalue = Math.max(Math.abs(dMax), Math.abs(dMin)) * 0.1;
                } else {
                    defaultIsovalue = dMax * 0.1;
                    if (defaultIsovalue < dMin) defaultIsovalue = dMin + (dMax - dMin) * 0.1;
                }
                setIsovalue(defaultIsovalue);
                setOpenAccordion('Volumetric');
                // Dispatch initial defaults to backend
                safeInvoke('set_isovalue', { value: defaultIsovalue }).catch(console.warn);
                safeInvoke('set_volume_render_mode', { mode: 'both' }).catch(console.warn);
                safeInvoke('set_isosurface_sign_mode', { mode: 'both' }).catch(console.warn);
            }
        }).then(f => unlisten = f).catch(console.warn);
        
        return () => {
            unlisten();
        };
    }, []);

    const handle_supercell = () => {
        const matrix = [
            [sc.nx, 0, 0],
            [0, sc.ny, 0],
            [0, 0, sc.nz]
        ];
        safeInvoke('apply_supercell', { matrix })
            .then(() => { if (onStructureUpdate) onStructureUpdate(); })
            .catch(console.error);
    };


    const handle_slab_cut = () => {
        if (slab.h === 0 && slab.k === 0 && slab.l === 0) {
            alert("Invalid Miller indices: returning to default (1, 1, 1).");
            setSlab(s => ({ ...s, h: 1, k: 1, l: 1 }));
            return;
        }
        safeInvoke('apply_slab', {
            miller: [slab.h, slab.k, slab.l],
            layers: slab.layers,
            vacuumA: slab.vacuum
        }).then(() => {
            if (onStructureUpdate) onStructureUpdate();
        }).catch(console.error);
    };


    const handle_delete_atom = () => {
        if (selectedAtoms.length === 0) return;
        safeInvoke('delete_atoms', { indices: selectedAtoms }).then(() => {
            if (onSelectionChange) onSelectionChange([]);
        }).catch(console.error);
    };

    const handle_replace_atom = () => {
        if (selectedAtoms.length === 0) return;
        setPromptConfig({
            isOpen: true,
            title: "Replace Atom(s)",
            description: "Enter new element symbol (e.g., Fe, O, C):",
            placeholder: "Element symbol",
            onSubmit: (newElem) => {
                if (newElem && newElem.trim().length > 0) {
                    safeInvoke('substitute_atoms', {
                        indices: selectedAtoms,
                        newElementSymbol: newElem.trim(),
                        newAtomicNumber: 0 // Backend can map symbol to number
                    })
                        .then(() => {
                            // Immediately fetch crystal state after substitute
                            safeInvoke('get_crystal_state').catch(console.error);
                        })
                        .catch(e => alert(e));
                }
            }
        });
    };

    const handle_refresh_bonds = () => {
        safeInvoke<BondAnalysisResult>('get_bond_analysis', { thresholdFactor: 1.2 })
            .then(res => {
                if (res) {
                    setBondAnalysis(res);
                    if (onBondCountUpdate) onBondCountUpdate(res.bonds.length);
                }
            })
            .catch(console.error);
    };

    const handle_load_phonon = () => {
        setIsPhononModalOpen(true);
    };

    const handleSubmitPhonon = async (paths: { scfIn: string, scfOut: string, modes: string, axsf: string }) => {
        try {
            setIsPhononModalOpen(false);
            let modesData;
            if (paths.axsf) {
                modesData = await safeInvoke<PhononModeSummary[]>('load_axsf_phonon', { path: paths.axsf });
            } else {
                modesData = await safeInvoke<PhononModeSummary[]>('load_phonon_interactive', {
                    scfIn: paths.scfIn,
                    scfOut: paths.scfOut,
                    modes: paths.modes
                });
            }
            if (modesData) {
                setPhononModes(modesData);
                setActiveModeIdx(null);
                setIsAnimating(false);
            }
        } catch (error) {
            console.error(error);
            alert(String(error));
        }
    };

    const handle_select_mode = (idx: number) => {
        setActiveModeIdx(idx);
        if (phononModes && onActivePhononModeUpdate) {
            const mode = phononModes.find(m => m.index === idx);
            onActivePhononModeUpdate(mode || null);
        }
        safeInvoke('set_phonon_mode', { modeIndex: idx }).catch(console.error);
    };

    const handle_compute_bz = () => {
        safeInvoke<BzInfo>('compute_brillouin_zone')
            .then(res => {
                if (res) {
                    setBzInfo(res);
                    return safeInvoke('toggle_bz_display', { show: true }).then(() => {
                        setIsBzVisible(true);
                        setTimeout(fetch_bz_labels, 150);
                    });
                }
            })
            .catch(console.error);
    };

    const handle_toggle_bz = () => {
        const next = !isBzVisible;
        safeInvoke('toggle_bz_display', { show: next })
            .then(() => {
                setIsBzVisible(next);
                if (next) {
                    setTimeout(fetch_bz_labels, 200);
                } else {
                    setBzLabels([]);
                }
            })
            .catch(console.error);
    };

    const handle_bz_resize = (delta: number) => {
        const next = Math.min(1.0, Math.max(0.15, bzScale + delta));
        setBzScale(next);
        safeInvoke('set_bz_scale', { scale: next }).catch(console.error);
    };

    const handle_bz_close = () => {
        setIsBzVisible(false);
        safeInvoke('toggle_bz_display', { show: false }).catch(console.error);
    };

    const handle_load_wannier = async () => {
        try {
            const file = await safeDialogOpen({
                title: 'Open wannier90_hr.dat',
                filters: [{ name: 'Wannier Hopping', extensions: ['dat'] }]
            });
            if (file && typeof file === 'string') {
                const info = await safeInvoke<WannierInfo>('load_wannier_hr', { path: file });
                if (info) {
                    setWannierInfo(info);
                    setActiveRShells(new Array(info.r_shells.length).fill(true));
                    setActiveOrbitals(new Array(info.num_wann).fill(true));
                    setIsWannierVisible(true);
                }
            }
        } catch (e) {
            alert(String(e));
        }
    };

    const handle_clear_wannier = () => {
        safeInvoke('clear_wannier').then(() => {
            setWannierInfo(null);
            setIsWannierVisible(false);
        }).catch(console.error);
    };


    useEffect(() => {
        if (!isAnimating) return;
        let animationFrameId: number;
        const start = performance.now();

        const render = (time: number) => {
            // Full cycle every 1000ms -> 2pi
            const phase = ((time - start) / 1000.0) * 2.0 * Math.PI;
            safeInvoke('set_phonon_phase', { phase, amplitude }).catch(console.error);
            animationFrameId = requestAnimationFrame(render);
        };
        animationFrameId = requestAnimationFrame(render);
        return () => cancelAnimationFrame(animationFrameId);
    }, [isAnimating, amplitude]);

    // Icon definitions for the toolbar — domain-specific SVGs
    const TOOL_SECTIONS = [
        { key: 'Structural Analysis', label: 'Bonds & Polyhedra', icon: (
            // Two circles connected by a line (bond) + a small ruler tick
            <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round">
                <circle cx="7" cy="12" r="3.5" />
                <circle cx="17" cy="12" r="3.5" />
                <line x1="10.5" y1="12" x2="13.5" y2="12" />
                <line x1="12" y1="9" x2="12" y2="10.5" />
            </svg>
        ) },
        { key: 'Volumetric', label: 'Isosurface / Volume', icon: (
            // Cloud-like isosurface blob
            <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
                <path d="M6 19a4 4 0 01-.78-7.93A7 7 0 0118.5 10.5a4.5 4.5 0 01-.36 8.5H6z" />
            </svg>
        ) },
        { key: 'Phonon Animation', label: 'Phonon Modes', icon: (
            // Sine wave (lattice vibration)
            <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round">
                <path d="M2 12c2-4 4-4 6 0s4 4 6 0 4-4 6 0" />
                <circle cx="5" cy="12" r="1.2" fill="currentColor" stroke="none" />
                <circle cx="11" cy="12" r="1.2" fill="currentColor" stroke="none" />
                <circle cx="17" cy="12" r="1.2" fill="currentColor" stroke="none" />
            </svg>
        ) },
        { key: 'Reciprocal Space', label: 'Brillouin Zone', icon: (
            // Hexagonal Brillouin zone with k-path segment
            <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
                <polygon points="12,3 19.5,7.5 19.5,16.5 12,21 4.5,16.5 4.5,7.5" />
                <circle cx="12" cy="12" r="1.5" fill="currentColor" stroke="none" />
                <line x1="12" y1="12" x2="19.5" y2="7.5" strokeDasharray="2 2" />
            </svg>
        ) },
        { key: 'Tight-Binding', label: 'Wannier / Hopping', icon: (
            // Two atoms with a hopping arrow between them
            <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round">
                <circle cx="6" cy="16" r="3" />
                <circle cx="18" cy="8" r="3" />
                <path d="M9 14l3.5-3.5M12.5 10.5L10.5 10M12.5 10.5L13 12.5" />
            </svg>
        ) },
        { key: 'Supercell', label: 'Supercell', icon: (
            // 2×2 unit cell grid
            <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round">
                <rect x="3" y="3" width="8" height="8" rx="1" />
                <rect x="13" y="3" width="8" height="8" rx="1" strokeDasharray="3 2" />
                <rect x="3" y="13" width="8" height="8" rx="1" strokeDasharray="3 2" />
                <rect x="13" y="13" width="8" height="8" rx="1" strokeDasharray="3 2" />
            </svg>
        ) },
        { key: 'Cutting Plane', label: 'Slab (hkl)', icon: (
            // Layered slab with a cutting line through it
            <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round">
                <rect x="4" y="4" width="16" height="4" rx="1" />
                <rect x="4" y="10" width="16" height="4" rx="1" />
                <rect x="4" y="16" width="16" height="4" rx="1" />
                <line x1="2" y1="9" x2="22" y2="9" strokeWidth={2} stroke="#ef4444" strokeDasharray="4 2" />
            </svg>
        ) },
        { key: 'Atom Operations', label: 'Add / Delete Atoms', icon: (
            // Atom circle with a small "+" badge
            <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round">
                <circle cx="11" cy="13" r="5" />
                <circle cx="11" cy="13" r="1.8" fill="currentColor" stroke="none" />
                <circle cx="17.5" cy="6.5" r="4" fill="white" stroke="currentColor" strokeWidth={1.5} />
                <line x1="17.5" y1="4.5" x2="17.5" y2="8.5" strokeWidth={1.5} />
                <line x1="15.5" y1="6.5" x2="19.5" y2="6.5" strokeWidth={1.5} />
            </svg>
        ) },
        { key: 'Measurements', label: 'Measurements Tool', icon: (
            // Ruler icon
            <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
                <path d="M21.17 3.25l-2.42-2.42a1 1 0 00-1.42 0L2.17 16a1 1 0 000 1.42l2.42 2.42a1 1 0 001.42 0l15.16-15.16a1 1 0 000-1.43zM6.5 17L5 15.5M10.5 13L9 11.5M14.5 9L13 7.5M18.5 5L17 3.5" />
            </svg>
        ) },
    ];

    return (
        <>
        <div className="shrink-0 h-full flex flex-row pointer-events-none">
            {/* Sliding Panel */}
            <div className={cn(
                "transition-all duration-300 ease-in-out overflow-hidden",
                openAccordion ? "w-[240px] opacity-100" : "w-0 opacity-0"
            )}>
                <div className="w-[240px] h-full flex flex-col gap-3 p-3 overflow-y-auto custom-scrollbar pointer-events-none">

            {/* Bond Analysis Accordion */}
            <Accordion title="Structural Analysis" isOpen={openAccordion === 'Structural Analysis'} onToggle={() => setOpenAccordion(openAccordion === 'Structural Analysis' ? null : 'Structural Analysis')}>
                <div className="space-y-3">
                    <ActionButton label="Calculate Bonds & Polyhedra" onClick={handle_refresh_bonds} />

                    {bondAnalysis && (
                        <div className="text-[11px] text-slate-600 dark:text-slate-300 space-y-2">
                            <div className="flex justify-between font-bold border-b border-slate-200 dark:border-slate-700 pb-1">
                                <span>Total Bonds: {bondAnalysis.bonds.length}</span>
                            </div>

                            {/* Bond Length Stats */}
                            <div className="max-h-32 overflow-y-auto custom-scrollbar space-y-1">
                                {bondAnalysis.bond_length_stats.map((stat, i) => (
                                    <div key={i} className="flex justify-between items-center bg-slate-50 dark:bg-slate-800/40 p-1 rounded">
                                        <span className="font-medium">{stat.element_a}-{stat.element_b}</span>
                                        <span className="tabular-nums">
                                            {stat.count} pair | {stat.mean.toFixed(2)} Å
                                        </span>
                                    </div>
                                ))}
                            </div>

                            {/* Selected Atom Distortion Index */}
                            {selectedAtoms.length === 1 && bondAnalysis.coordination[selectedAtoms[0]] && (
                                <div className="mt-2 p-2 bg-emerald-50 dark:bg-emerald-900/20 rounded-md border border-emerald-100 dark:border-emerald-800/30">
                                    <div className="font-medium text-emerald-800 dark:text-emerald-300 mb-1">
                                        Atom #{selectedAtoms[0]} ({bondAnalysis.coordination[selectedAtoms[0]].element})
                                    </div>
                                    <div>Coordination: {bondAnalysis.coordination[selectedAtoms[0]].coordination_number}</div>
                                    {bondAnalysis.coordination[selectedAtoms[0]].polyhedron_type && (
                                        <div>Polyhedron: {bondAnalysis.coordination[selectedAtoms[0]].polyhedron_type}</div>
                                    )}
                                    {bondAnalysis.distortion_indices[selectedAtoms[0]] > 0 && (
                                        <div>Distortion Δ: {bondAnalysis.distortion_indices[selectedAtoms[0]].toFixed(4)}</div>
                                    )}
                                </div>
                            )}
                        </div>
                    )}
                </div>
            </Accordion>

            {/* Volumetric Data Accordion */}
            <Accordion title="Volumetric Data" isOpen={openAccordion === 'Volumetric'} onToggle={() => setOpenAccordion(openAccordion === 'Volumetric' ? null : 'Volumetric')}>
                <div className="space-y-3">
                    <ActionButton label="Load Volumetric Data..." onClick={async () => {
                        try {
                            const file = await safeDialogOpen({
                                title: 'Open Volumetric File'
                            });
                            if (file && typeof file === 'string') {
                                const info = await safeInvoke<any>('load_volumetric_file', { path: file });
                                if (info) {
                                    setVolumetricInfo(info);
                                    let dMin = info.data_min;
                                    let dMax = info.data_max;
                                    setVolumetricRange({ min: dMin, max: dMax });
                                    setIsovalue((dMax - dMin) * 0.1 + dMin);
                                }
                                if (onStructureUpdate) onStructureUpdate();
                            }
                        } catch (e) {
                            alert(String(e));
                        }
                    }} />

                    {volumetricInfo && (
                        <div className="bg-slate-100 dark:bg-slate-800/50 rounded flex flex-col p-2 space-y-1 text-[10px] text-slate-500 dark:text-slate-400 font-mono">
                            <div className="flex justify-between items-center text-xs">
                                <span className="font-semibold text-slate-700 dark:text-slate-300">Data Info</span>
                                <span className="bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 px-1.5 py-0.5 rounded uppercase">{volumetricInfo.format}</span>
                            </div>
                            <div className="flex justify-between">
                                <span>Grid Size:</span>
                                <span>{volumetricInfo.grid_dims[0]}×{volumetricInfo.grid_dims[1]}×{volumetricInfo.grid_dims[2]}</span>
                            </div>
                            <div className="flex justify-between">
                                <span>Min Den:</span>
                                <span>{volumetricInfo.data_min.toExponential(2)}</span>
                            </div>
                            <div className="flex justify-between">
                                <span>Max Den:</span>
                                <span>{volumetricInfo.data_max.toExponential(2)}</span>
                            </div>
                        </div>
                    )}
                    
                    <div className="space-y-1">
                        <label className="text-[11px] text-slate-500 dark:text-slate-400">Render Mode</label>
                        <select
                            className="w-full bg-slate-100 dark:bg-slate-800/60 rounded px-2 py-1.5 outline-none border border-slate-200 dark:border-slate-700 text-xs text-slate-700 dark:text-slate-300 pointer-events-auto"
                            onChange={(e) => safeInvoke('set_volume_render_mode', { mode: e.target.value }).catch(e => alert(String(e)))}
                            defaultValue="both"
                        >
                            <option value="both">Both (Isosurface + Volume)</option>
                            <option value="isosurface">Isosurface Only</option>
                            <option value="volume">Volume Only</option>
                        </select>
                    </div>

                    <div className="space-y-1">
                        <div className="flex justify-between items-center text-[11px] text-slate-500 dark:text-slate-400">
                            <span>Isovalue</span>
                            <span>{isovalue.toExponential(2)}</span>
                        </div>
                        <input
                            type="range" min={0} max={Math.max(Math.abs(volumetricRange.min), Math.abs(volumetricRange.max))} step={Math.max(Math.abs(volumetricRange.min), Math.abs(volumetricRange.max)) / 1000.0}
                            value={isovalue}
                            onChange={(e) => {
                                let v = parseFloat(e.target.value);
                                setIsovalue(v);
                                safeInvoke('set_isovalue', { value: v }).catch(e => alert(String(e)));
                            }}
                            className="w-full h-1 accent-emerald-500 cursor-pointer pointer-events-auto"
                        />
                    </div>



                    <div className="space-y-1">
                        <div className="flex justify-between items-center text-[11px] text-slate-500 dark:text-slate-400">
                            <span>Surface Opacity</span>
                        </div>
                        <input
                            type="range" min={0.0} max={1.0} step={0.05}
                            defaultValue={0.5}
                            onChange={(e) => safeInvoke('set_isosurface_opacity', { opacity: parseFloat(e.target.value) }).catch(e => alert(String(e)))}
                            className="w-full h-1 accent-emerald-500 cursor-pointer pointer-events-auto"
                        />
                    </div>

                    <div className="space-y-1">
                        <label className="text-[11px] text-slate-500 dark:text-slate-400">Sign Mode (Charge Diff)</label>
                        <select
                            className="w-full bg-slate-100 dark:bg-slate-800/60 rounded px-2 py-1.5 outline-none border border-slate-200 dark:border-slate-700 text-xs text-slate-700 dark:text-slate-300 pointer-events-auto"
                            onChange={(e) => {
                                safeInvoke('set_isosurface_sign_mode', { mode: e.target.value }).catch(err => alert(String(err)));
                            }}
                            defaultValue="positive"
                        >
                            <option value="positive">Positive Only</option>
                            <option value="negative">Negative Only</option>
                            <option value="both">Both (±)</option>
                        </select>
                    </div>

                    <div className="space-y-1">
                        <label className="text-[11px] text-slate-500 dark:text-slate-400">Volume Colormap</label>
                        <select
                            className="w-full bg-slate-100 dark:bg-slate-800/60 rounded px-2 py-1.5 outline-none border border-slate-200 dark:border-slate-700 text-xs text-slate-700 dark:text-slate-300 pointer-events-auto"
                            onChange={(e) => safeInvoke('set_volume_colormap', { mode: e.target.value }).catch(err => alert(String(err)))}
                            defaultValue="viridis"
                        >
                            <option value="viridis">Viridis</option>
                            <option value="inferno">Inferno</option>
                            <option value="plasma">Plasma</option>
                            <option value="magma">Magma</option>
                            <option value="cividis">Cividis</option>
                            <option value="turbo">Turbo (Rainbow)</option>
                            <option value="hot">Hot</option>
                            <option value="coolwarm">Coolwarm (± diverging)</option>
                            <option value="rdylbu">RdYlBu (± diverging)</option>
                            <option value="grayscale">Grayscale</option>
                        </select>
                    </div>

                    <div className="space-y-1">
                        <div className="flex justify-between items-center text-[11px] text-slate-500 dark:text-slate-400">
                            <span>Volume Density Cutoff</span>
                        </div>
                        <input
                            type="range" min={0} max={Math.max(Math.abs(volumetricRange.min), Math.abs(volumetricRange.max))} step={Math.max(Math.abs(volumetricRange.min), Math.abs(volumetricRange.max)) / 500.0}
                            defaultValue={0}
                            onChange={(e) => safeInvoke('set_volume_density_cutoff', { cutoff: parseFloat(e.target.value) }).catch(err => alert(String(err)))}
                            className="w-full h-1 accent-emerald-500 cursor-pointer pointer-events-auto"
                        />
                    </div>

                    <div className="space-y-1">
                        <div className="flex justify-between items-center text-[11px] text-slate-500 dark:text-slate-400">
                            <span>Volume Opacity Scale</span>
                        </div>
                        <input
                            type="range" min={0.1} max={5.0} step={0.1}
                            defaultValue={1.0}
                            onChange={(e) => safeInvoke('set_volume_opacity_range', { min: volumetricRange.min, max: volumetricRange.max, opacityScale: parseFloat(e.target.value) }).catch(err => alert(String(err)))}
                            className="w-full h-1 accent-emerald-500 cursor-pointer pointer-events-auto"
                        />
                    </div>

                </div>
            </Accordion>

            {/* Phonon Animation Accordion */}
            <Accordion title="Phonon Animation" isOpen={openAccordion === 'Phonon Animation'} onToggle={() => setOpenAccordion(openAccordion === 'Phonon Animation' ? null : 'Phonon Animation')}>
                <div className="space-y-3">
                    <ActionButton label="Load Phonon Data (.mold/.dat)" onClick={handle_load_phonon} />

                    {phononModes && (
                        <>
                            <div className="space-y-1">
                                <label className="text-[11px] text-slate-500 dark:text-slate-400">Select Mode</label>
                                <select
                                    className="w-full bg-slate-100 dark:bg-slate-800/60 rounded px-2 py-1.5 outline-none border border-slate-200 dark:border-slate-700 text-xs text-slate-700 dark:text-slate-300 pointer-events-auto"
                                    value={activeModeIdx ?? ""}
                                    onChange={(e) => handle_select_mode(parseInt(e.target.value))}
                                >
                                    <option value="" disabled>-- Select Mode --</option>
                                    {Array.from(new Set(phononModes.map(m => m.q_point.join(',')))).map(qStr => {
                                        const qModes = phononModes.filter(m => m.q_point.join(',') === qStr);
                                        const [qx, qy, qz] = qStr.split(',').map(Number);
                                        const isGamma = qx === 0 && qy === 0 && qz === 0;
                                        return (
                                            <optgroup key={qStr} label={`q = (${qx.toFixed(3)}, ${qy.toFixed(3)}, ${qz.toFixed(3)})${isGamma ? ' [Γ]' : ''}`}>
                                                {qModes.map(m => (
                                                    <option key={m.index} value={m.index}>
                                                        Mode {m.index + 1}: {m.frequency_cm1.toFixed(2)} cm⁻¹ {m.is_imaginary ? '(i)' : ''}
                                                    </option>
                                                ))}
                                            </optgroup>
                                        );
                                    })}
                                </select>
                            </div>

                            <div className="space-y-1">
                                <div className="flex justify-between items-center text-[11px] text-slate-500 dark:text-slate-400">
                                    <span>Amplitude: {amplitude.toFixed(1)}</span>
                                </div>
                                <input
                                    type="range" min={0.1} max={5.0} step={0.1}
                                    value={amplitude}
                                    onChange={e => setAmplitude(parseFloat(e.target.value))}
                                    className="w-full h-1 accent-emerald-500 cursor-pointer pointer-events-auto"
                                />
                            </div>

                            <button
                                onClick={() => setIsAnimating(!isAnimating)}
                                disabled={activeModeIdx === null}
                                className={cn(
                                    "w-full py-1.5 text-white rounded-md text-xs font-medium transition-colors shadow-sm pointer-events-auto",
                                    activeModeIdx === null ? "bg-slate-300 dark:bg-slate-700 text-slate-500 cursor-not-allowed" :
                                        isAnimating ? "bg-amber-500 hover:bg-amber-600" : "bg-emerald-500 hover:bg-emerald-600"
                                )}
                            >
                                {isAnimating ? "⏸ Pause Animation" : "▶ Play Animation"}
                            </button>
                        </>
                    )}
                </div>
            </Accordion>

            {/* Reciprocal Space Accordion */}
            <Accordion title="Reciprocal Space" isOpen={openAccordion === 'Reciprocal Space'} onToggle={() => setOpenAccordion(openAccordion === 'Reciprocal Space' ? null : 'Reciprocal Space')}>
                <div className="space-y-3">
                    <ActionButton label="Compute Brillouin Zone" onClick={handle_compute_bz} />
                    
                    <button 
                        onClick={handle_toggle_bz} 
                        disabled={!bzInfo}
                        className={cn(
                            "w-full py-1.5 rounded-md text-xs font-medium transition-colors shadow-sm pointer-events-auto",
                            !bzInfo ? "bg-slate-300 dark:bg-slate-700 text-slate-500 cursor-not-allowed" :
                            isBzVisible 
                                ? "bg-amber-500 hover:bg-amber-600 text-white" 
                                : "bg-slate-500 hover:bg-slate-600 text-white"
                        )}
                    >
                        {isBzVisible ? "◀ Back to Crystal View" : "View Brillouin Zone"}
                    </button>

                    {bzInfo && (
                        <div className="text-[11px] text-slate-600 dark:text-slate-300 bg-slate-50 dark:bg-slate-800/40 p-2 rounded border border-slate-100 dark:border-slate-700 space-y-1">
                            <div className="flex justify-between">
                                <span className="text-slate-500">Bravais Type:</span>
                                <span className="font-semibold">{bzInfo.bravais_type}</span>
                            </div>
                            <div className="flex justify-between">
                                <span className="text-slate-500">Geometry:</span>
                                <span>
                                    {bzInfo.is_2d ? `${bzInfo.edges_count} edges, ` : `${bzInfo.faces_count} faces, `} 
                                    {bzInfo.vertices_count} vertices
                                </span>
                            </div>
                        </div>
                    )}

                    {bzInfo && (
                        <>
                        <div className="border-t border-slate-200 dark:border-slate-700 my-2"></div>
                        <div className="text-[11px] font-medium text-slate-500 dark:text-slate-400 mb-1">Band Path Generator</div>
                        <div className="flex items-center gap-2 mb-1">
                            <label className="text-[11px] text-slate-500 dark:text-slate-400 whitespace-nowrap">N<sub>k</sub></label>
                            <input 
                                type="number" min={5} max={100} defaultValue={20}
                                id="kpath-npoints"
                                className="w-14 bg-slate-100 dark:bg-slate-800/60 rounded px-2 py-0.5 text-xs border border-slate-200 dark:border-slate-700 focus:border-emerald-500 outline-none text-slate-700 dark:text-slate-300 pointer-events-auto"
                            />
                            <select
                                id="kpath-format"
                                defaultValue="qe"
                                className="flex-1 bg-slate-100 dark:bg-slate-800/60 rounded px-2 py-0.5 text-xs border border-slate-200 dark:border-slate-700 focus:border-emerald-500 outline-none text-slate-700 dark:text-slate-300 pointer-events-auto"
                            >
                                <option value="qe">QE (crystal)</option>
                                <option value="vasp">VASP (KPOINTS)</option>
                            </select>
                        </div>
                        <button
                            onClick={async () => {
                                const nEl = document.getElementById('kpath-npoints') as HTMLInputElement;
                                const fmtEl = document.getElementById('kpath-format') as HTMLSelectElement;
                                const npoints = parseInt(nEl?.value) || 20;
                                const fmt = fmtEl?.value || 'qe';
                                try {
                                    const res = await safeInvoke<{qe: string, vasp: string}>('generate_kpath_text', { npoints });
                                    if (!res) return;
                                    const text = fmt === 'qe' ? res.qe : res.vasp;
                                    const preEl = document.getElementById('kpath-preview');
                                    if (preEl) preEl.textContent = text;
                                    const defaultName = fmt === 'qe' ? 'kpath_qe.txt' : 'KPOINTS';
                                    const savePath = await safeDialogSave({
                                        title: 'Save K-Path',
                                        defaultPath: defaultName,
                                        filters: [{ name: 'Text', extensions: ['txt'] }],
                                    });
                                    if (savePath) {
                                        await safeInvoke('write_text_file', { path: String(savePath), content: text });
                                    }
                                } catch (e) { alert(String(e)); }
                            }}
                            className="w-full py-1.5 bg-indigo-50 dark:bg-indigo-500/10 hover:bg-indigo-100 dark:hover:bg-indigo-500/20 text-indigo-600 dark:text-indigo-400 rounded-md text-xs font-medium transition-colors border border-indigo-200/50 dark:border-indigo-800/50 active:scale-[0.98] pointer-events-auto"
                        >
                            💾 Generate &amp; Save K-Path
                        </button>
                        <pre 
                            id="kpath-preview"
                            className="mt-1 max-h-32 overflow-y-auto text-[10px] bg-slate-900 text-green-400 p-2 rounded font-mono whitespace-pre custom-scrollbar empty:hidden pointer-events-auto select-text cursor-text"
                        ></pre>
                        </>
                    )}

                    <div className="border-t border-slate-200 dark:border-slate-700 my-2"></div>
                    
                    <div className="text-[11px] font-medium text-slate-500 dark:text-slate-400 mb-1">Standardization</div>
                    <div className="grid grid-cols-2 gap-2">
                        <ActionButton label="Niggli Reduce" onClick={() => {
                            safeInvoke('apply_niggli_reduce').then(() => { if (onStructureUpdate) onStructureUpdate(); }).catch(e => alert(e));
                        }} />
                        <ActionButton label="Primitive" onClick={() => {
                            safeInvoke('apply_cell_standardize', { toPrimitive: true }).then(() => { if (onStructureUpdate) onStructureUpdate(); }).catch(e => alert(e));
                        }} />
                        <ActionButton label="Conventional" onClick={() => {
                            safeInvoke('apply_cell_standardize', { toPrimitive: false }).then(() => { if (onStructureUpdate) onStructureUpdate(); }).catch(e => alert(e));
                        }} />
                    </div>
                </div>
            </Accordion>

            {/* Tight-Binding (Wannier) Accordion */}
            <Accordion title="Tight-Binding (Wannier)" isOpen={openAccordion === 'Tight-Binding'} onToggle={() => setOpenAccordion(openAccordion === 'Tight-Binding' ? null : 'Tight-Binding')}>
                <div className="space-y-3">
                    <ActionButton label="Load wannier90_hr.dat..." onClick={handle_load_wannier} />
                    
                    {wannierInfo && (
                        <>
                            <div className="text-[11px] text-slate-600 dark:text-slate-300 bg-slate-50 dark:bg-slate-800/40 p-2 rounded border border-slate-100 dark:border-slate-700 space-y-1">
                                <div className="flex justify-between">
                                    <span className="text-slate-500">Orbitals:</span>
                                    <span className="font-semibold">{wannierInfo.num_wann}</span>
                                </div>
                                <div className="flex justify-between">
                                    <span className="text-slate-500">R-Shells:</span>
                                    <span className="font-semibold">{wannierInfo.r_shells.length}</span>
                                </div>
                                <div className="flex justify-between">
                                    <span className="text-slate-500">Max |t|:</span>
                                    <span className="font-semibold">{wannierInfo.t_max.toFixed(4)} eV</span>
                                </div>
                            </div>
                            
                            <div className="space-y-1">
                                <div className="flex justify-between items-center text-[11px] text-slate-500 dark:text-slate-400">
                                    <span>|t| Threshold:</span>
                                    <span>{tMin.toFixed(3)} eV</span>
                                </div>
                                <input
                                    type="range" min={0.0} max={wannierInfo.t_max} step={wannierInfo.t_max / 100}
                                    value={tMin}
                                    onChange={(e) => {
                                        const v = parseFloat(e.target.value);
                                        setTMin(v);
                                        safeInvoke('set_wannier_t_min', { tMin: v }).catch(console.error);
                                    }}
                                    className="w-full h-1 accent-emerald-500 cursor-pointer pointer-events-auto"
                                />
                            </div>

                            <div className="space-y-2 max-h-32 overflow-y-auto custom-scrollbar p-1">
                                <div className="text-[11px] font-medium text-slate-500 dark:text-slate-400 border-b border-slate-200 dark:border-slate-700 pb-1">Orbitals (m, n)</div>
                                <div className="flex flex-wrap gap-2">
                                    {activeOrbitals.map((active, i) => (
                                        <label key={`orb-${i}`} className="flex items-center gap-1 text-[10px] text-slate-600 dark:text-slate-300 cursor-pointer pointer-events-auto">
                                            <input type="checkbox" checked={active} onChange={(e) => {
                                                const checked = e.target.checked;
                                                const next = [...activeOrbitals];
                                                next[i] = checked;
                                                setActiveOrbitals(next);
                                                safeInvoke('set_wannier_orbital', { orbIdx: i, active: checked }).catch(console.error);
                                            }} className="accent-emerald-500 rounded-sm" />
                                            Orb {i+1}
                                        </label>
                                    ))}
                                </div>
                            </div>

                            <div className="space-y-2 max-h-32 overflow-y-auto custom-scrollbar p-1">
                                <div className="text-[11px] font-medium text-slate-500 dark:text-slate-400 border-b border-slate-200 dark:border-slate-700 pb-1">Translation & On-site</div>
                                
                                <label className="flex items-center gap-1 text-[10px] text-slate-600 dark:text-slate-300 cursor-pointer pointer-events-auto mb-2">
                                    <input type="checkbox" checked={showOnSite} onChange={(e) => {
                                        const checked = e.target.checked;
                                        setShowOnSite(checked);
                                        safeInvoke('toggle_wannier_onsite', { show: checked }).catch(console.error);
                                    }} className="accent-emerald-500 rounded-sm" />
                                    Show On-site (R=0, m=n)
                                </label>

                                <div className="flex flex-wrap gap-x-3 gap-y-2">
                                    {activeRShells.map((active, i) => {
                                        const r = wannierInfo.r_shells[i];
                                        return (
                                            <label key={`r-${i}`} className="flex items-center gap-1 text-[10px] text-slate-600 dark:text-slate-300 cursor-pointer pointer-events-auto">
                                                <input type="checkbox" checked={active} onChange={(e) => {
                                                    const checked = e.target.checked;
                                                    const next = [...activeRShells];
                                                    next[i] = checked;
                                                    setActiveRShells(next);
                                                    safeInvoke('set_wannier_r_shell', { shellIdx: i, active: checked }).catch(console.error);
                                                }} className="accent-emerald-500 rounded-sm" />
                                                [{r[0]}, {r[1]}, {r[2]}]
                                            </label>
                                        );
                                    })}
                                </div>
                            </div>

                            <div className="flex gap-2">
                                <button
                                    onClick={() => {
                                        const next = !isWannierVisible;
                                        setIsWannierVisible(next);
                                        safeInvoke('toggle_hopping_display', { show: next }).catch(console.error);
                                    }}
                                    className={cn(
                                        "flex-1 py-1.5 rounded-md text-xs font-medium transition-colors shadow-sm pointer-events-auto",
                                        isWannierVisible ? "bg-amber-500 hover:bg-amber-600 text-white" : "bg-emerald-500 hover:bg-emerald-600 text-white"
                                    )}
                                >
                                    {isWannierVisible ? "Hide Hoppings" : "Show Hoppings"}
                                </button>
                                <button
                                    onClick={handle_clear_wannier}
                                    className="flex-[0.5] py-1.5 bg-red-50 dark:bg-red-500/10 hover:bg-red-100 dark:hover:bg-red-500/20 text-red-600 dark:text-red-400 rounded-md text-xs font-medium transition-colors border border-red-200/50 dark:border-red-800/50 pointer-events-auto"
                                >
                                    Clear
                                </button>
                            </div>
                        </>
                    )}
                </div>
            </Accordion>

            {/* Supercell Accordion */}
            <Accordion title="Supercell Construction" isOpen={openAccordion === 'Supercell'} onToggle={() => setOpenAccordion(openAccordion === 'Supercell' ? null : 'Supercell')}>
                <div className="space-y-3">
                    <div className="flex gap-2 text-xs">
                        <NumberInput label="Nx" value={sc.nx} onChange={v => setSc(s => ({ ...s, nx: v }))} />
                        <NumberInput label="Ny" value={sc.ny} onChange={v => setSc(s => ({ ...s, ny: v }))} />
                        <NumberInput label="Nz" value={sc.nz} onChange={v => setSc(s => ({ ...s, nz: v }))} />
                    </div>
                    <button onClick={handle_supercell} className="w-full py-1.5 bg-emerald-500 hover:bg-emerald-600 text-white rounded-md text-xs font-medium transition-colors shadow-sm active:scale-[0.98] pointer-events-auto">
                        Execute Supercell
                    </button>
                    <button
                        onClick={() => {
                            safeInvoke('restore_unitcell')
                                .then(() => { if (onStructureUpdate) onStructureUpdate(); })
                                .catch(e => alert(`Restore failed: ${e}`));
                        }}
                        className="w-full py-1.5 bg-slate-50 dark:bg-slate-800/40 hover:bg-slate-100 dark:hover:bg-slate-700/60 text-slate-600 dark:text-slate-300 rounded-md text-xs font-medium transition-colors border border-slate-200 dark:border-slate-700 active:scale-[0.98] pointer-events-auto"
                    >
                        Restore Original Cell
                    </button>


                </div>
            </Accordion>

            {/* Cutting Plane Accordion */}
            <Accordion title="Cutting Plane (hkl)" isOpen={openAccordion === 'Cutting Plane'} onToggle={() => setOpenAccordion(openAccordion === 'Cutting Plane' ? null : 'Cutting Plane')}>
                <div className="space-y-3">
                    <div className="flex gap-2 text-xs">
                        <NumberInput label="h" value={slab.h} onChange={v => setSlab(s => ({ ...s, h: v }))} />
                        <NumberInput label="k" value={slab.k} onChange={v => setSlab(s => ({ ...s, k: v }))} />
                        <NumberInput label="l" value={slab.l} onChange={v => setSlab(s => ({ ...s, l: v }))} />
                    </div>

                    <div className="space-y-1">
                        <div className="flex justify-between items-center text-[11px] text-slate-500 dark:text-slate-400">
                            <span>Layers: {slab.layers}</span>
                        </div>
                        <input type="range" min={1} max={10} step={1} value={slab.layers} onChange={e => setSlab(s => ({ ...s, layers: parseInt(e.target.value) }))} className="w-full h-1 accent-emerald-500 cursor-pointer pointer-events-auto" />
                    </div>

                    <div className="space-y-1">
                        <div className="flex justify-between items-center text-[11px] text-slate-500 dark:text-slate-400">
                            <span>Vacuum: {slab.vacuum} Å</span>
                        </div>
                        <input type="range" min={0} max={30} step={1} value={slab.vacuum} onChange={e => setSlab(s => ({ ...s, vacuum: parseFloat(e.target.value) }))} className="w-full h-1 accent-emerald-500 cursor-pointer pointer-events-auto" />
                    </div>

                    <div className="flex gap-2">
                        <ActionButton label="Cut" onClick={handle_slab_cut} />
                        <ActionButton label="Reset" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'reset' })} />
                    </div>
                </div>
            </Accordion>

            {/* Atom Operations Accordion */}
            <Accordion title="Atom Operations" isOpen={openAccordion === 'Atom Operations'} onToggle={() => setOpenAccordion(openAccordion === 'Atom Operations' ? null : 'Atom Operations')}>
                <div className="space-y-3">
                    <div className="text-xs space-y-1">
                        <div className="text-slate-500 dark:text-slate-400">
                            Selected: <span className="text-slate-800 dark:text-slate-200 font-medium">
                                {selectedAtoms.length > 0 ? (selectedAtoms.length === 1 ? `Atom #${selectedAtoms[0]}` : `${selectedAtoms.length} atoms`) : "None"}
                            </span>
                        </div>
                        <div className="text-slate-500 dark:text-slate-400">
                            Element: <span className="text-slate-800 dark:text-slate-200 font-medium">
                                {selectedAtoms.length === 1 && crystalState ? crystalState.elements[selectedAtoms[0]] : (selectedAtoms.length > 1 ? "Mixed" : "-")}
                            </span>
                        </div>
                    </div>

                    <div className="flex flex-col gap-1.5">
                        {selectedAtoms.length > 0 ? (
                            <>
                                <ActionButton label="Replace Atom(s)" onClick={handle_replace_atom} />
                                <button onClick={handle_delete_atom} className="w-full py-1.5 bg-red-500/10 hover:bg-red-500/20 text-red-600 dark:text-red-400 rounded-md text-xs font-medium transition-colors border border-red-200 dark:border-red-900 active:scale-[0.98] pointer-events-auto">
                                    Delete Atom(s)
                                </button>
                                <DisabledButton label="Add Sub-Atom" />
                            </>
                        ) : (
                            <>
                                <DisabledButton label="Replace Atom(s)" />
                                <DisabledButton label="Delete Atom(s)" />
                                <DisabledButton label="Add Sub-Atom" />
                            </>
                        )}
                    </div>
                </div>
            </Accordion>

            {/* Measurements Accordion */}
            <Accordion title="Measurements Library" isOpen={openAccordion === 'Measurements'} onToggle={() => setOpenAccordion(openAccordion === 'Measurements' ? null : 'Measurements')}>
                <div className="space-y-3">
                    <div className="flex justify-between items-center text-xs">
                        <span className="text-slate-500 dark:text-slate-400">Total Measurements:</span>
                        <span className="font-semibold text-slate-800 dark:text-slate-200">{crystalState?.measurements?.length || 0}</span>
                    </div>

                    {crystalState?.measurements && crystalState.measurements.length > 0 ? (
                        <div className="space-y-2 max-h-48 overflow-y-auto custom-scrollbar">
                            {crystalState.measurements.map((m, i) => (
                                <div key={i} className="text-[11px] bg-slate-50 dark:bg-slate-800/40 p-2 rounded border border-slate-100 dark:border-slate-700">
                                    <div className="flex justify-between font-medium text-slate-700 dark:text-slate-300 mb-1">
                                        <span>{m.kind}</span>
                                        <span className="text-emerald-600 dark:text-emerald-400">
                                            {m.value.toFixed(2)} {m.kind === 'Distance' ? 'Å' : '°'}
                                        </span>
                                    </div>
                                    <div className="text-slate-500 font-mono tracking-tighter">
                                        [{m.indices.join('-')}]
                                    </div>
                                </div>
                            ))}
                        </div>
                    ) : (
                        <div className="text-xs text-slate-400 italic text-center py-4">No measurements yet</div>
                    )}

                    <button
                        onClick={() => safeInvoke('clear_measurements').then(() => { if (onStructureUpdate) onStructureUpdate(); }).catch(e => alert(e))}
                        disabled={!crystalState?.measurements?.length}
                        className={cn(
                            "w-full py-1.5 rounded-md text-xs font-medium transition-colors border pointer-events-auto",
                            crystalState?.measurements?.length ? "bg-red-50 text-red-600 border-red-200 hover:bg-red-100 dark:bg-red-500/10 dark:text-red-400 dark:border-red-800/50 dark:hover:bg-red-500/20 active:scale-[0.98]" : "bg-slate-100 dark:bg-slate-800/60 text-slate-400 dark:text-slate-500 cursor-not-allowed border-slate-200 dark:border-slate-700"
                        )}
                    >
                        Clear All Measurements
                    </button>
                    
                    <button
                        onClick={() => {
                            if (selectedAtoms.length >= 2 && selectedAtoms.length <= 4) {
                                safeInvoke('add_measurement', { indices: selectedAtoms })
                                    .then(() => { if (onStructureUpdate) onStructureUpdate(); if (onSelectionChange) onSelectionChange([]); })
                                    .catch(e => alert(e));
                            } else {
                                alert("Please select exactly 2, 3, or 4 atoms first.");
                            }
                        }}
                        disabled={selectedAtoms.length < 2 || selectedAtoms.length > 4}
                        className={cn(
                            "w-full py-1.5 rounded-md text-xs font-medium transition-colors shadow-sm pointer-events-auto",
                            (selectedAtoms.length >= 2 && selectedAtoms.length <= 4) ? "bg-emerald-500 hover:bg-emerald-600 text-white active:scale-[0.98]" : "bg-slate-300 dark:bg-slate-700 text-slate-500 cursor-not-allowed"
                        )}
                    >
                        Add Measurement from Selection
                    </button>
                    
                    <div className="text-[10px] text-slate-400 flex items-center justify-center p-1 bg-amber-50 dark:bg-amber-900/10 text-amber-700 dark:text-amber-500 rounded border border-amber-200/50">
                        Shift-click to select 2 (Distance), 3 (Angle), or 4 (Dihedral) atoms, then add.
                    </div>
                </div>
            </Accordion>

            {/* Modals */}
            <PhononImportModal
                isOpen={isPhononModalOpen}
                onClose={() => setIsPhononModalOpen(false)}
                onSubmit={handleSubmitPhonon}
            />

            <PromptModal
                {...promptConfig}
                onClose={() => setPromptConfig(prev => ({ ...prev, isOpen: false }))}
            />
                </div>{/* end w-[240px] inner */}
            </div>{/* end sliding panel */}

            {/* Icon Toolbar */}
            <div className="w-[44px] shrink-0 h-full flex flex-col items-center pt-2 pb-2 gap-1 pointer-events-auto">
                {TOOL_SECTIONS.map((section) => (
                    <button
                        key={section.key}
                        title={section.label}
                        onClick={() => setOpenAccordion(openAccordion === section.key ? null : section.key)}
                        className={cn(
                            "w-9 h-9 flex items-center justify-center rounded-lg transition-all duration-200",
                            openAccordion === section.key
                                ? "bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 shadow-sm ring-1 ring-emerald-500/30"
                                : "text-slate-500 dark:text-slate-400 hover:bg-white/60 dark:hover:bg-slate-800/60 hover:text-slate-700 dark:hover:text-slate-200"
                        )}
                    >
                        {section.icon}
                    </button>
                ))}
            </div>
        </div>{/* end flex-row container */}

        {/* BZ k-point label overlay (portal, outside main tree) */}
        {isBzVisible && bzLabels.length > 0 && ReactDOM.createPortal(
            <div className="fixed inset-0 pointer-events-none z-[60]" style={{fontFamily: "'Inter', 'SF Pro', system-ui, sans-serif"}}>
                {(() => {
                    const pad = 32;
                    const minDist = 22;
                    const positioned = bzLabels.map(l => ({ ...l, dx: 0, dy: -18 }));
                    for (let pass = 0; pass < 3; pass++) {
                        for (let i = 0; i < positioned.length; i++) {
                            for (let j = i + 1; j < positioned.length; j++) {
                                const ax = positioned[i].x + positioned[i].dx;
                                const ay = positioned[i].y + positioned[i].dy;
                                const bx = positioned[j].x + positioned[j].dx;
                                const by = positioned[j].y + positioned[j].dy;
                                const dist = Math.sqrt((ax - bx) ** 2 + (ay - by) ** 2);
                                if (dist < minDist) {
                                    const nudge = (minDist - dist) / 2 + 2;
                                    positioned[j].dy += nudge;
                                    positioned[i].dy -= nudge;
                                }
                            }
                        }
                    }
                    return positioned.map((lbl, i) => {
                        const cx = Math.max(pad, Math.min(lbl.x + lbl.dx, window.innerWidth - pad));
                        const cy = Math.max(pad, Math.min(lbl.y + lbl.dy, window.innerHeight - pad));
                        return (
                            <span
                                key={i}
                                className="absolute text-[12px] font-bold whitespace-nowrap"
                                style={{
                                    left: cx,
                                    top: cy,
                                    transform: 'translate(-50%, -50%)',
                                    color: '#f59e0b',
                                    textShadow: '0 0 4px rgba(0,0,0,0.8), 0 0 2px rgba(0,0,0,0.6)',
                                    letterSpacing: '0.02em',
                                }}
                            >
                                {lbl.label}
                            </span>
                        );
                    });
                })()}
            </div>,
            document.body
        )}

        {/* Measurement floating labels */}
        {measurementLabels.length > 0 && ReactDOM.createPortal(
            <div className="fixed inset-0 pointer-events-none z-[50]" style={{fontFamily: "'Inter', 'SF Pro', system-ui, sans-serif"}}>
                {measurementLabels.map((lbl, i) => (
                    <div
                        key={i}
                        className="absolute text-[12px] font-bold whitespace-nowrap bg-slate-900/40 backdrop-blur-[2px] text-orange-400 px-1.5 py-0.5 rounded border border-orange-500/30"
                        style={{
                            left: lbl.x,
                            top: lbl.y,
                            transform: 'translate(-50%, -50%)',
                            boxShadow: '0 2px 4px rgba(0,0,0,0.2)'
                        }}
                    >
                        {lbl.label}
                    </div>
                ))}
            </div>,
            document.body
        )}
        </>
    );
};

// --- Subcomponents ---

const NumberInput = ({ label, value, onChange }: { label: string; value: number; onChange: (v: number) => void }) => (
    <div className="flex-1 space-y-0.5">
        <label className="text-[11px] text-slate-500 dark:text-slate-400">{label}</label>
        <input
            type="number"
            value={value}
            onChange={(e) => onChange(parseInt(e.target.value) || 0)}
            min={label.startsWith('N') ? 1 : undefined}
            className="w-full bg-slate-100 dark:bg-slate-800/60 rounded px-2 py-1 outline-none border border-slate-200 dark:border-slate-700 text-xs focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500/30 transition-all text-slate-700 dark:text-slate-300 pointer-events-auto"
        />
    </div>
);

const ActionButton = ({ label, onClick }: { label: string; onClick?: () => void }) => (
    <button onClick={onClick} className="flex-1 w-full py-1.5 bg-emerald-50 dark:bg-emerald-500/10 hover:bg-emerald-100 dark:hover:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 rounded-md text-xs font-medium transition-colors border border-emerald-200/50 dark:border-emerald-800/50 active:scale-[0.98] pointer-events-auto">
        {label}
    </button>
);

const DisabledButton = ({ label }: { label: string }) => (
    <button disabled className="w-full py-1.5 bg-slate-100 dark:bg-slate-800/60 text-slate-400 dark:text-slate-500 cursor-not-allowed rounded-md border border-slate-200 dark:border-slate-700 text-xs">
        {label}
    </button>
);

const Accordion: React.FC<{ title: string; isOpen: boolean; onToggle: () => void; children: React.ReactNode }> = ({ title, isOpen, children }) => {
    if (!isOpen) return null;
    return (
        <div className="pointer-events-auto shrink-0 bg-white/80 dark:bg-slate-900/80 backdrop-blur-xl border border-white/30 dark:border-slate-700/50 rounded-xl shadow-lg shadow-black/5 dark:shadow-black/20 overflow-hidden">
            <div className="px-3 py-2 border-b border-slate-100 dark:border-slate-800">
                <span className="font-medium text-sm text-slate-800 dark:text-slate-200">{title}</span>
            </div>
            <div className="px-3 py-3">
                {children}
            </div>
        </div>
    );
};

