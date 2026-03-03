// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React from 'react';
import { CrystalState } from '../../types/crystal';

export const BottomStatusBar: React.FC<{ crystalState: CrystalState | null }> = ({ crystalState }) => {
    const numAtoms = crystalState?.labels.length || 0;
    const sg = crystalState?.spacegroup_hm || 'N/A';
    const vol = crystalState ?
        (crystalState.cell_a * crystalState.cell_b * crystalState.cell_c *
            Math.sqrt(1 - Math.cos(crystalState.cell_alpha * Math.PI / 180) ** 2
                - Math.cos(crystalState.cell_beta * Math.PI / 180) ** 2
                - Math.cos(crystalState.cell_gamma * Math.PI / 180) ** 2
                + 2 * Math.cos(crystalState.cell_alpha * Math.PI / 180) * Math.cos(crystalState.cell_beta * Math.PI / 180) * Math.cos(crystalState.cell_gamma * Math.PI / 180)
            )).toFixed(1) : 0;
    return (
        <div className="w-full h-7 shrink-0 bg-slate-100/80 dark:bg-slate-900/80 backdrop-blur-md border-t border-slate-200/80 dark:border-slate-700/50 flex items-center justify-between px-4 text-[11px] z-40 pointer-events-auto transition-colors duration-300">
            <div className="flex items-center gap-4 text-slate-500 dark:text-slate-400 tabular-nums">
                <span>SpaceGroup: <span className="font-medium text-slate-700 dark:text-slate-300">{sg}</span></span>
                <span>Volume: <span className="font-medium text-slate-700 dark:text-slate-300">{vol} Å³</span></span>
            </div>
            <div className="flex items-center gap-6 text-slate-500 dark:text-slate-400">
                <span>Total Atoms: <span className="font-medium text-slate-700 dark:text-slate-300">{numAtoms}</span></span>
                <span>Selected: <span className="font-medium text-slate-700 dark:text-slate-300">0</span></span>
            </div>
        </div>
    );
};
