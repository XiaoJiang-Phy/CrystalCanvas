import React, { useState } from 'react';
import { IpcException, type IpcError } from '../../ipc/contracts';
import { safeInvoke } from '../../utils/tauri-mock';
import { ActionButton, NumberInput, PanelError } from './shared';

export default function SupercellPanel() {
    const [sc, setSc] = useState({ nx: 1, ny: 1, nz: 1 });
    const [error, setError] = useState<IpcError | null>(null);
    const [activeOperation, setActiveOperation] = useState<'supercell' | 'restore' | null>(null);

    const setMutationError = (cause: unknown, fallback: string) => {
        if (cause instanceof IpcException) {
            setError({ code: cause.code, message: cause.message, recoverable: cause.recoverable });
            return;
        }
        setError({ code: 'internal_error', message: fallback, recoverable: false });
    };

    const handleSupercell = async () => {
        if (activeOperation) return;
        const matrix: [[number, number, number], [number, number, number], [number, number, number]] = [
            [sc.nx, 0, 0],
            [0, sc.ny, 0],
            [0, 0, sc.nz],
        ];
        setError(null);
        setActiveOperation('supercell');
        try {
            await safeInvoke('apply_supercell', { matrix });
        } catch (cause) {
            setMutationError(cause, 'Unable to create the supercell.');
        } finally {
            setActiveOperation(null);
        }
    };

    const handleRestore = async () => {
        if (activeOperation) return;
        setError(null);
        setActiveOperation('restore');
        try {
            await safeInvoke('restore_unitcell');
        } catch (cause) {
            setMutationError(cause, 'Unable to restore the original cell.');
        } finally {
            setActiveOperation(null);
        }
    };

    const isBusy = activeOperation !== null;

    return (
        <div className="space-y-3">
            <div className="grid grid-cols-3 gap-2">
                <NumberInput label="Nx" value={sc.nx} onChange={(value) => setSc((current) => ({ ...current, nx: value }))} disabled={isBusy} invalid={sc.nx < 1} />
                <NumberInput label="Ny" value={sc.ny} onChange={(value) => setSc((current) => ({ ...current, ny: value }))} disabled={isBusy} invalid={sc.ny < 1} />
                <NumberInput label="Nz" value={sc.nz} onChange={(value) => setSc((current) => ({ ...current, nz: value }))} disabled={isBusy} invalid={sc.nz < 1} />
            </div>
            {error && <PanelError error={error} message={error.message} />}
            <ActionButton label="Execute Supercell" onClick={handleSupercell} disabled={isBusy} busy={activeOperation === 'supercell'} />
            <ActionButton label="Restore Original Cell" onClick={handleRestore} disabled={isBusy} busy={activeOperation === 'restore'} tone="secondary" />
        </div>
    );
}
