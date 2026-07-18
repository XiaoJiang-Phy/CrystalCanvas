import React, { useState } from 'react';
import { IpcException, type IpcError } from '../../ipc/contracts';
import { safeInvoke } from '../../utils/tauri-mock';
import { ActionButton, NumberInput, PanelError, RangeInput } from './shared';

export default function SlabPanel() {
    const [slab, setSlab] = useState({ h: 1, k: 1, l: 1, layers: 3, vacuum: 15.0 });
    const [error, setError] = useState<IpcError | null>(null);
    const [activeOperation, setActiveOperation] = useState<'cut' | 'reset' | null>(null);

    const setMutationError = (cause: unknown, fallback: string) => {
        if (cause instanceof IpcException) {
            setError({ code: cause.code, message: cause.message, recoverable: cause.recoverable });
            return;
        }
        setError({ code: 'internal_error', message: fallback, recoverable: false });
    };

    const handleSlabCut = async () => {
        if (activeOperation) return;
        if (slab.h === 0 && slab.k === 0 && slab.l === 0) {
            setError({ code: 'invalid_argument', message: 'Invalid Miller indices: returning to default (1, 1, 1).', recoverable: true });
            setSlab((current) => ({ ...current, h: 1, k: 1, l: 1 }));
            return;
        }
        setError(null);
        setActiveOperation('cut');
        try {
            await safeInvoke('apply_slab', {
                miller: [slab.h, slab.k, slab.l],
                layers: slab.layers,
                vacuumA: slab.vacuum
            });
        } catch (cause) {
            setMutationError(cause, 'Unable to create the cutting plane.');
        } finally {
            setActiveOperation(null);
        }
    };

    const handleReset = async () => {
        if (activeOperation) return;
        setError(null);
        setActiveOperation('reset');
        try {
            await safeInvoke('set_camera_view_axis', { axis: 'reset' });
        } catch (cause) {
            setMutationError(cause, 'Unable to reset the camera view.');
        } finally {
            setActiveOperation(null);
        }
    };

    const isBusy = activeOperation !== null;
    const invalidMiller = slab.h === 0 && slab.k === 0 && slab.l === 0;

    return (
        <div className="space-y-3">
            <div className="grid grid-cols-3 gap-2">
                <NumberInput label="h" value={slab.h} onChange={(value) => setSlab((current) => ({ ...current, h: value }))} disabled={isBusy} invalid={invalidMiller} />
                <NumberInput label="k" value={slab.k} onChange={(value) => setSlab((current) => ({ ...current, k: value }))} disabled={isBusy} invalid={invalidMiller} />
                <NumberInput label="l" value={slab.l} onChange={(value) => setSlab((current) => ({ ...current, l: value }))} disabled={isBusy} invalid={invalidMiller} />
            </div>
            <RangeInput label="Layers" value={slab.layers} displayValue={String(slab.layers)} min={1} max={10} step={1} onChange={(value) => setSlab((current) => ({ ...current, layers: value }))} disabled={isBusy} />
            <RangeInput label="Vacuum" value={slab.vacuum} displayValue={`${slab.vacuum} Å`} min={0} max={30} step={1} onChange={(value) => setSlab((current) => ({ ...current, vacuum: value }))} disabled={isBusy} />
            {error && <PanelError error={error} message={error.message} />}
            <div className="grid grid-cols-2 gap-2">
                <ActionButton label="Cut" onClick={handleSlabCut} disabled={isBusy} busy={activeOperation === 'cut'} />
                <ActionButton label="Reset" onClick={handleReset} disabled={isBusy} busy={activeOperation === 'reset'} tone="secondary" />
            </div>
        </div>
    );
}
