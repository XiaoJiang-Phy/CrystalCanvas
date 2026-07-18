import React, { useEffect, useState } from 'react';
import ReactDOM from 'react-dom';
import { IpcException, type IpcError } from '../../ipc/contracts';
import { safeInvoke } from '../../utils/tauri-mock';
import { PanelProps } from './index';
import { ActionButton, PanelError } from './shared';

export default function MeasurementPanel({ crystalState, selectedAtoms = [], onSelectionChange }: PanelProps) {
    const [measurementLabels, setMeasurementLabels] = useState<{ label: string; x: number; y: number }[]>([]);
    const [error, setError] = useState<IpcError | null>(null);
    const [activeOperation, setActiveOperation] = useState<'clear' | 'add' | null>(null);

    useEffect(() => {
        if (!crystalState?.measurements || crystalState.measurements.length === 0) {
            setMeasurementLabels([]);
            return;
        }
        let active = true;
        const updateLabels = async () => {
            if (!active) return;
            try {
                const labels = await safeInvoke('get_measurement_labels_screen', { width: window.innerWidth, height: window.innerHeight });
                if (active) setMeasurementLabels(labels || []);
            } catch {
                if (active) setMeasurementLabels([]);
            }
            if (active) requestAnimationFrame(updateLabels);
        };
        void updateLabels();
        return () => { active = false; };
    }, [crystalState?.measurements]);

    const setMutationError = (cause: unknown, fallback: string) => {
        if (cause instanceof IpcException) {
            setError({ code: cause.code, message: cause.message, recoverable: cause.recoverable });
            return;
        }
        setError({ code: 'internal_error', message: fallback, recoverable: false });
    };

    const handleClearMeasurements = async () => {
        if (activeOperation) return;
        setError(null);
        setActiveOperation('clear');
        try {
            await safeInvoke('clear_measurements');
        } catch (cause) {
            setMutationError(cause, 'Unable to clear measurements.');
        } finally {
            setActiveOperation(null);
        }
    };

    const handleAddMeasurement = async () => {
        if (activeOperation) return;
        if (selectedAtoms.length < 2 || selectedAtoms.length > 4) {
            setError({ code: 'invalid_argument', message: 'Select exactly 2, 3, or 4 atoms before adding a measurement.', recoverable: true });
            return;
        }
        setError(null);
        setActiveOperation('add');
        try {
            await safeInvoke('add_measurement', { indices: selectedAtoms });
            if (onSelectionChange) onSelectionChange([]);
        } catch (cause) {
            setMutationError(cause, 'Unable to add a measurement.');
        } finally {
            setActiveOperation(null);
        }
    };

    const measurementCount = crystalState?.measurements?.length || 0;
    const canAddMeasurement = selectedAtoms.length >= 2 && selectedAtoms.length <= 4;
    const isBusy = activeOperation !== null;

    return (
        <>
            <div className="space-y-3">
                <div className="flex items-center justify-between text-xs">
                    <span className="text-[var(--cc-muted)]">Total Measurements</span>
                    <span className="font-semibold tabular-nums text-[var(--cc-text)]">{measurementCount}</span>
                </div>

                {measurementCount > 0 ? (
                    <div className="max-h-48 space-y-2 overflow-y-auto custom-scrollbar">
                        {crystalState?.measurements?.map((measurement, index) => (
                            <div key={index} className="rounded border border-[var(--cc-border)] bg-[var(--cc-field)] p-2 text-[11px]">
                                <div className="flex justify-between font-medium text-[var(--cc-text)]">
                                    <span>{measurement.kind}</span>
                                    <span className="tabular-nums text-[var(--cc-accent)]">
                                        {measurement.value.toFixed(2)} {measurement.kind === 'Distance' ? 'Å' : '°'}
                                    </span>
                                </div>
                                <div className="mt-1 font-mono text-[var(--cc-muted)]">[{measurement.indices.join('-')}]</div>
                            </div>
                        ))}
                    </div>
                ) : (
                    <div role="status" className="py-3 text-center text-xs text-[var(--cc-muted)]">No measurements yet</div>
                )}

                {error && <PanelError error={error} message={error.message} />}

                <ActionButton label="Clear All Measurements" onClick={handleClearMeasurements} disabled={measurementCount === 0 || isBusy} busy={activeOperation === 'clear'} tone="danger" />
                <ActionButton label="Add Measurement from Selection" onClick={handleAddMeasurement} disabled={!canAddMeasurement || isBusy} busy={activeOperation === 'add'} />

                <p className="rounded border border-[var(--cc-border)] bg-[var(--cc-field)] p-2 text-[10px] text-[var(--cc-muted)]">
                    Shift-click to select 2 (Distance), 3 (Angle), or 4 (Dihedral) atoms, then add.
                </p>
            </div>
            {measurementLabels.length > 0 && ReactDOM.createPortal(
                <div className="fixed inset-0 z-[50] pointer-events-none" style={{ fontFamily: 'system-ui, -apple-system, sans-serif' }}>
                    {measurementLabels.map((label, index) => (
                        <div
                            key={index}
                            className="absolute whitespace-nowrap rounded border border-orange-500 bg-slate-900 px-1.5 py-0.5 text-[12px] font-bold text-orange-400"
                            style={{ left: label.x, top: label.y, transform: 'translate(-50%, -50%)' }}
                        >
                            {label.label}
                        </div>
                    ))}
                </div>,
                document.body,
            )}
        </>
    );
}
