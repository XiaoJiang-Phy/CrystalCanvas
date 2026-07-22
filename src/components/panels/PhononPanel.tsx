import React, { useState } from 'react';
import { safeInvoke } from '../../utils/tauri-mock';
import { IpcException, type IpcError } from '../../ipc/contracts';
import { PhononModeSummary } from '../../types/crystal';
import { PhononImportModal } from '../layout/PhononImportModal';
import { PanelProps } from './index';
import { ActionButton, PanelError, RangeInput, SelectInput } from './shared';

type PhononControlOperation = 'mode' | 'playback' | 'scale' | 'reset';

export default function PhononPanel({ onActivePhononModeUpdate }: PanelProps) {
    const [phononModes, setPhononModes] = useState<PhononModeSummary[] | null>(null);
    const [activeModeIdx, setActiveModeIdx] = useState<number | null>(null);
    const [isAnimating, setIsAnimating] = useState(false);
    const [amplitude, setAmplitude] = useState(1.0);
    const [isPhononModalOpen, setIsPhononModalOpen] = useState(false);
    const [isLoading, setIsLoading] = useState(false);
    const [activeControlOperation, setActiveControlOperation] = useState<PhononControlOperation | null>(null);
    const [error, setError] = useState<IpcError | null>(null);
    const isControlBusy = activeControlOperation !== null;

    const setPanelError = (cause: unknown, fallback: string) => {
        if (cause instanceof IpcException) {
            setError({ code: cause.code, message: cause.message, recoverable: cause.recoverable });
            return;
        }
        setError({ code: 'internal_error', message: fallback, recoverable: false });
    };

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
        if (isControlBusy) return;
        setError(null);
        setActiveControlOperation('mode');
        try {
            await safeInvoke('set_phonon_mode', { modeIndex: idx });
            setActiveModeIdx(idx);
            setIsAnimating(false);
            setAmplitude(1.0);
            if (phononModes && onActivePhononModeUpdate) {
                const mode = phononModes.find(m => m.index === idx);
                onActivePhononModeUpdate(mode || null);
            }
        } catch (cause) {
            setPanelError(cause, 'Unable to select the phonon mode.');
        } finally {
            setActiveControlOperation(null);
        }
    };

    const handle_toggle_animation = async () => {
        if (isControlBusy) return;
        const playing = !isAnimating;
        setError(null);
        setActiveControlOperation('playback');
        try {
            await safeInvoke('set_phonon_playing', { playing });
            setIsAnimating(playing);
        } catch (cause) {
            setPanelError(cause, 'Unable to change phonon playback.');
        } finally {
            setActiveControlOperation(null);
        }
    };

    const handle_set_amplitude = async (displayScale: number) => {
        if (isControlBusy) return;
        setError(null);
        setActiveControlOperation('scale');
        try {
            await safeInvoke('set_phonon_display_scale', { displayScale });
            setAmplitude(displayScale);
        } catch (cause) {
            setPanelError(cause, 'Unable to change phonon display scale.');
        } finally {
            setActiveControlOperation(null);
        }
    };

    const handle_reset_animation = async () => {
        if (isControlBusy) return;
        setError(null);
        setActiveControlOperation('reset');
        try {
            await safeInvoke('set_phonon_phase', { phase: 0, amplitude });
        } catch (cause) {
            setPanelError(cause, 'Unable to reset phonon animation.');
        } finally {
            setActiveControlOperation(null);
        }
    };

    return (
        <div className="space-y-3" aria-busy={isLoading || isControlBusy}>
            <ActionButton label="Load Phonon Data (.mold/.dat)" onClick={handle_load_phonon} disabled={isLoading || isControlBusy} busy={isLoading} />

            {error && <PanelError error={error} message={error.message} />}
            {!phononModes && !isLoading && !error && <div role="status" className="text-xs text-[var(--cc-muted)]">No phonon modes are loaded.</div>}

            {phononModes && (
                <>
                    <SelectInput
                        label="Select Mode"
                        value={activeModeIdx?.toString() ?? ''}
                        onChange={(value) => handle_select_mode(parseInt(value, 10))}
                        disabled={isLoading || isControlBusy}
                        busy={activeControlOperation === 'mode'}
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
                        onChange={handle_set_amplitude}
                        disabled={isLoading || isControlBusy}
                        busy={activeControlOperation === 'scale'}
                    />

                    <ActionButton
                        label={isAnimating ? 'Pause Animation' : 'Play Animation'}
                        onClick={handle_toggle_animation}
                        disabled={activeModeIdx === null || isLoading || isControlBusy}
                        busy={activeControlOperation === 'playback'}
                        busyLabel="Updating Animation…"
                        tone={isAnimating ? 'secondary' : 'primary'}
                    />

                    <ActionButton
                        label="Reset Animation"
                        onClick={handle_reset_animation}
                        disabled={activeModeIdx === null || isLoading || isControlBusy}
                        busy={activeControlOperation === 'reset'}
                        busyLabel="Resetting Animation…"
                        tone="secondary"
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
