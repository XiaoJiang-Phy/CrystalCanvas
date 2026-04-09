import React, { useState, useEffect } from 'react';
import { cn } from '../../utils/cn';
import { safeInvoke, safeListen, safeDialogOpen } from '../../utils/tauri-mock';
import { CrystalState, BondAnalysisResult, PhononModeSummary } from '../../types/crystal';
import { PromptModal } from './PromptModal';
import { PhononImportModal } from './PhononImportModal';

export const RightSidebar: React.FC<{
    crystalState: CrystalState | null,
    selectedAtoms?: number[],
    onSelectionChange?: (indices: number[]) => void,
    onBondCountUpdate?: (count: number) => void,
    onActivePhononModeUpdate?: (mode: PhononModeSummary | null) => void,
    onStructureUpdate?: () => void
}> = ({ crystalState, selectedAtoms = [], onSelectionChange, onBondCountUpdate, onActivePhononModeUpdate, onStructureUpdate }) => {

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
    const [openAccordion, setOpenAccordion] = useState<string | null>("Structural Analysis");
    const [isPhononModalOpen, setIsPhononModalOpen] = useState(false);

    // Volumetric State
    const [volumetricInfo, setVolumetricInfo] = useState<any | null>(null);
    const [volumetricRange, setVolumetricRange] = useState<{min: number, max: number}>({min: -1.0, max: 1.0});
    const [isovalue, setIsovalue] = useState(0.05);

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

    return (
        <div className="w-[240px] shrink-0 h-full flex flex-col gap-3 p-3 pointer-events-none overflow-y-auto custom-scrollbar">

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
        </div>
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

const Accordion: React.FC<{ title: string; isOpen: boolean; onToggle: () => void; children: React.ReactNode }> = ({ title, isOpen, onToggle, children }) => {
    return (
        <div className="pointer-events-auto shrink-0 bg-white/80 dark:bg-slate-900/80 backdrop-blur-xl border border-white/30 dark:border-slate-700/50 rounded-xl shadow-lg shadow-black/5 dark:shadow-black/20 overflow-hidden">
            <button
                onClick={onToggle}
                className={cn(
                    "w-full px-3 py-2.5 flex justify-between items-center bg-transparent hover:bg-slate-50/50 dark:hover:bg-slate-800/50 transition-colors",
                    isOpen && "border-b border-slate-100 dark:border-slate-800"
                )}
            >
                <span className="font-medium text-sm text-slate-800 dark:text-slate-200">{title}</span>
                <svg
                    className={cn("w-3.5 h-3.5 text-slate-400 transition-transform duration-200", isOpen && "rotate-180")}
                    fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}
                >
                    <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
                </svg>
            </button>

            <div className={cn(
                "transition-all duration-300 ease-in-out overflow-hidden origin-top",
                isOpen ? "max-h-[800px] opacity-100 overflow-y-auto" : "max-h-0 opacity-0"
            )}>
                <div className="px-3 py-3">
                    {children}
                </div>
            </div>
        </div>
    );
};
