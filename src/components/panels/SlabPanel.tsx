import React, { useState } from 'react';
import { safeInvoke } from '../../utils/tauri-mock';
import { NumberInput, ActionButton } from './shared';
import { PanelProps } from './index';

export default function SlabPanel({ onStructureUpdate }: PanelProps) {
    const [slab, setSlab] = useState({ h: 1, k: 1, l: 1, layers: 3, vacuum: 15.0 });

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

    return (
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
    );
}
