import React, { useState } from 'react';
import { safeInvoke } from '../../utils/tauri-mock';
import { NumberInput } from './shared';
import { PanelProps } from './index';

export default function SupercellPanel({ onStructureUpdate }: PanelProps) {
    const [sc, setSc] = useState({ nx: 1, ny: 1, nz: 1 });

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

    return (
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
                        .catch((e: any) => alert(`Restore failed: ${e}`));
                }}
                className="w-full py-1.5 bg-slate-50 dark:bg-slate-800/40 hover:bg-slate-100 dark:hover:bg-slate-700/60 text-slate-600 dark:text-slate-300 rounded-md text-xs font-medium transition-colors border border-slate-200 dark:border-slate-700 active:scale-[0.98] pointer-events-auto"
            >
                Restore Original Cell
            </button>
        </div>
    );
}
