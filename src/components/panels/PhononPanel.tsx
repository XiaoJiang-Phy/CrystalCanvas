import React, { useState, useEffect } from 'react';
import { safeInvoke } from '../../utils/tauri-mock';
import { PhononModeSummary } from '../../types/crystal';
import { PhononImportModal } from '../layout/PhononImportModal';
import { PanelProps } from './index';

export default function PhononPanel({ onActivePhononModeUpdate }: PanelProps) {
    const [phononModes, setPhononModes] = useState<PhononModeSummary[] | null>(null);
    const [activeModeIdx, setActiveModeIdx] = useState<number | null>(null);
    const [isAnimating, setIsAnimating] = useState(false);
    const [amplitude, setAmplitude] = useState(1.0);
    const [isPhononModalOpen, setIsPhononModalOpen] = useState(false);

    useEffect(() => {
        if (!isAnimating) return;
        let animationFrameId: number;
        const start = performance.now();

        const render = (time: number) => {
            const phase = ((time - start) / 1000.0) * 2.0 * Math.PI;
            safeInvoke('set_phonon_phase', { phase, amplitude }).catch(console.error);
            animationFrameId = requestAnimationFrame(render);
        };
        animationFrameId = requestAnimationFrame(render);
        return () => cancelAnimationFrame(animationFrameId);
    }, [isAnimating, amplitude]);

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
        } catch (error: any) {
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

    return (
        <div className="space-y-3">
            <button onClick={handle_load_phonon} className="flex-1 w-full py-1.5 bg-emerald-50 dark:bg-emerald-500/10 hover:bg-emerald-100 dark:hover:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 rounded-md text-xs font-medium transition-colors border border-emerald-200/50 dark:border-emerald-800/50 active:scale-[0.98] pointer-events-auto">
                Load Phonon Data (.mold/.dat)
            </button>

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
                        className={`w-full py-1.5 text-white rounded-md text-xs font-medium transition-colors shadow-sm pointer-events-auto ${
                            activeModeIdx === null ? "bg-slate-300 dark:bg-slate-700 text-slate-500 cursor-not-allowed" :
                                isAnimating ? "bg-amber-500 hover:bg-amber-600" : "bg-emerald-500 hover:bg-emerald-600"
                        }`}
                    >
                        {isAnimating ? "⏸ Pause Animation" : "▶ Play Animation"}
                    </button>
                </>
            )}

            <PhononImportModal
                isOpen={isPhononModalOpen}
                onClose={() => setIsPhononModalOpen(false)}
                onSubmit={handleSubmitPhonon}
            />
        </div>
    );
}
