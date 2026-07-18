import React, { useState, useEffect } from 'react';
import { safeInvoke } from '../../utils/tauri-mock';
import { IpcException, type IpcError } from '../../ipc/contracts';
import { PhononModeSummary } from '../../types/crystal';
import { PhononImportModal } from '../layout/PhononImportModal';
import { PanelProps } from './index';
import { ActionButton, PanelError, RangeInput, SelectInput } from './shared';

export default function PhononPanel({ onActivePhononModeUpdate }: PanelProps) {
    const [phononModes, setPhononModes] = useState<PhononModeSummary[] | null>(null);
    const [activeModeIdx, setActiveModeIdx] = useState<number | null>(null);
    const [isAnimating, setIsAnimating] = useState(false);
    const [amplitude, setAmplitude] = useState(1.0);
    const [isPhononModalOpen, setIsPhononModalOpen] = useState(false);
    const [isLoading, setIsLoading] = useState(false);
    const [isSelectingMode, setIsSelectingMode] = useState(false);
    const [error, setError] = useState<IpcError | null>(null);

    const setPanelError = (cause: unknown, fallback: string) => {
        if (cause instanceof IpcException) {
            setError({ code: cause.code, message: cause.message, recoverable: cause.recoverable });
            return;
        }
        setError({ code: 'internal_error', message: fallback, recoverable: false });
    };

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
        if (isLoading) return;
        setError(null);
        setIsLoading(true);
        try {
            let modesData;
            if (paths.axsf) {
                modesData = await safeInvoke('load_axsf_phonon', { path: paths.axsf });
            } else {
                modesData = await safeInvoke('load_phonon_interactive', {
                    scfIn: paths.scfIn,
                    scfOut: paths.scfOut,
                    modes: paths.modes
                });
            }
            if (modesData) {
                setPhononModes(modesData);
                setActiveModeIdx(null);
                setIsAnimating(false);
                setIsPhononModalOpen(false);
            }
        } catch (cause) {
            setPanelError(cause, 'Unable to load phonon data.');
            throw cause;
        } finally {
            setIsLoading(false);
        }
    };

    const handle_select_mode = async (idx: number) => {
        if (isSelectingMode) return;
        setError(null);
        setIsSelectingMode(true);
        try {
            await safeInvoke('set_phonon_mode', { modeIndex: idx });
            setActiveModeIdx(idx);
            if (phononModes && onActivePhononModeUpdate) {
                const mode = phononModes.find(m => m.index === idx);
                onActivePhononModeUpdate(mode || null);
            }
        } catch (cause) {
            setPanelError(cause, 'Unable to select the phonon mode.');
        } finally {
            setIsSelectingMode(false);
        }
    };

    return (
        <div className="space-y-3" aria-busy={isLoading || isSelectingMode}>
            <ActionButton label="Load Phonon Data (.mold/.dat)" onClick={handle_load_phonon} disabled={isLoading || isSelectingMode} busy={isLoading} />

            {error && <PanelError error={error} message={error.message} />}
            {!phononModes && !isLoading && !error && <div role="status" className="text-xs text-[var(--cc-muted)]">No phonon modes are loaded.</div>}

            {phononModes && (
                <>
                    <SelectInput
                        label="Select Mode"
                        value={activeModeIdx?.toString() ?? ''}
                        onChange={(value) => handle_select_mode(parseInt(value, 10))}
                        disabled={isLoading || isSelectingMode}
                        busy={isSelectingMode}
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
                    </SelectInput>

                    <RangeInput
                        label="Amplitude"
                        value={amplitude}
                        displayValue={amplitude.toFixed(1)}
                        min={0.1}
                        max={5.0}
                        step={0.1}
                        onChange={setAmplitude}
                        disabled={isLoading || isSelectingMode}
                    />

                    <ActionButton
                        label={isAnimating ? 'Pause Animation' : 'Play Animation'}
                        onClick={() => setIsAnimating(!isAnimating)}
                        disabled={activeModeIdx === null || isLoading || isSelectingMode}
                        tone={isAnimating ? 'secondary' : 'primary'}
                    />
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
