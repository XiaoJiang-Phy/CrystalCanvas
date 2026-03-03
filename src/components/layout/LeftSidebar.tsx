// [Overview: Left sidebar component for structure info and atom management.]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React from 'react';
import { cn } from '../../utils/cn';
import { getJmolColor } from '../../utils/colors';

import { safeInvoke } from '../../utils/tauri-mock';
import { CrystalState } from '../../types/crystal';
interface LeftSidebarProps {
    crystalState: CrystalState | null;
    selectedAtomIdx: number | null;
    onSelectionChange: (idx: number | null) => void;
}

export const LeftSidebar: React.FC<LeftSidebarProps> = ({
    crystalState,
    selectedAtomIdx,
    onSelectionChange
}) => {
    const numAtoms = crystalState ? crystalState.labels.length : 0;
    const vol = crystalState ?
        (crystalState.cell_a * crystalState.cell_b * crystalState.cell_c *
            Math.sqrt(1 - Math.cos(crystalState.cell_alpha * Math.PI / 180) ** 2
                - Math.cos(crystalState.cell_beta * Math.PI / 180) ** 2
                - Math.cos(crystalState.cell_gamma * Math.PI / 180) ** 2
                + 2 * Math.cos(crystalState.cell_alpha * Math.PI / 180) * Math.cos(crystalState.cell_beta * Math.PI / 180) * Math.cos(crystalState.cell_gamma * Math.PI / 180)
            )).toFixed(1) : 0;
    return (
        <div className="w-[240px] shrink-0 h-full flex flex-col gap-3 p-3 pb-10 pointer-events-none overflow-y-auto custom-scrollbar">
            <Panel title="Structure Info">
                <div className="space-y-2 text-xs">
                    <InfoRow label="Atoms:" value={numAtoms.toString()} />
                    <InfoRow label="Space Group:" value={crystalState?.spacegroup_hm || "N/A"} />
                    <div className="grid grid-cols-2 gap-x-4 gap-y-1.5 pt-2">
                        <UnitCellInput label="a" paramKey="a" value={crystalState?.cell_a.toFixed(2) || "0.00"} unit="Å" />
                        <UnitCellInput label="α" paramKey="alpha" value={crystalState?.cell_alpha.toFixed(1) || "0.0"} unit="°" />
                        <UnitCellInput label="b" paramKey="b" value={crystalState?.cell_b.toFixed(2) || "0.00"} unit="Å" />
                        <UnitCellInput label="β" paramKey="beta" value={crystalState?.cell_beta.toFixed(1) || "0.0"} unit="°" />
                        <UnitCellInput label="c" paramKey="c" value={crystalState?.cell_c.toFixed(2) || "0.00"} unit="Å" />
                        <UnitCellInput label="γ" paramKey="gamma" value={crystalState?.cell_gamma.toFixed(1) || "0.0"} unit="°" />
                    </div>
                    <InfoRow label="Volume:" value={`${vol} Å³`} className="pt-1.5 font-medium" />
                </div>
            </Panel>

            <Panel title="Atom Management">
                <div className="w-full bg-slate-50 dark:bg-slate-900/50 rounded-lg border border-slate-200 dark:border-slate-800 text-[10px] max-h-[200px] overflow-x-auto overflow-y-auto custom-scrollbar">
                    <table className="w-full text-left">
                        <thead className="bg-slate-100 dark:bg-slate-800/80 font-medium text-slate-500 dark:text-slate-400">
                            <tr>
                                <th className="px-2 py-1.5">ID</th>
                                <th className="px-2 py-1.5">El</th>
                                <th className="px-2 py-1.5">x</th>
                                <th className="px-2 py-1.5">y</th>
                                <th className="px-2 py-1.5">z</th>
                                <th className="px-2 py-1.5 text-center">Color</th>
                            </tr>
                        </thead>
                        <tbody className="divide-y divide-slate-200 dark:divide-slate-800">
                            {crystalState && crystalState.labels.map((_label, i) => (
                                <AtomRow
                                    key={i}
                                    id={i}
                                    element={crystalState.elements[i]}
                                    x={crystalState.fract_x[i].toFixed(2)}
                                    y={crystalState.fract_y[i].toFixed(2)}
                                    z={crystalState.fract_z[i].toFixed(2)}
                                    isSelected={selectedAtomIdx === i}
                                    onClick={() => onSelectionChange(i)}
                                />
                            ))}
                        </tbody>
                    </table>
                </div>
            </Panel>

        </div>
    );
};

// --- Subcomponents ---

const Panel: React.FC<{ title: string; badge?: string; children: React.ReactNode }> = ({ title, badge, children }) => (
    <div className="pointer-events-auto bg-white/80 dark:bg-slate-900/80 backdrop-blur-xl border border-white/30 dark:border-slate-700/50 rounded-xl shadow-lg shadow-black/5 dark:shadow-black/20 flex flex-col overflow-hidden relative">
        {/* Decorative header gradient */}
        <div className="absolute top-0 left-0 right-0 h-8 bg-gradient-to-r from-emerald-500/10 to-transparent pointer-events-none" />

        <div className="px-3 py-2.5 flex justify-between items-center border-b border-slate-100 dark:border-slate-800">
            <h3 className="font-semibold text-sm text-slate-800 dark:text-slate-200">{title}</h3>
            {badge && <span className="text-[10px] font-mono text-slate-400">{badge}</span>}
        </div>

        <div className="px-3 py-3">
            {children}
        </div>
    </div>
);

const InfoRow = ({ label, value, className }: { label: string; value: string; className?: string }) => (
    <div className={cn("flex justify-between items-center", className)}>
        <span className="text-slate-500 dark:text-slate-400">{label}</span>
        <span className="font-medium text-slate-700 dark:text-slate-300">{value}</span>
    </div>
);

const UnitCellInput = ({ label, paramKey, value, unit }: { label: string; paramKey: string; value: string; unit: string }) => {
    const handleBlur = (e: React.FocusEvent<HTMLInputElement>) => {
        const val = parseFloat(e.target.value);
        if (!isNaN(val)) {
            // Note: In a complete implementation, we'd gather all 6 parameters and call update_lattice_params.
            // For now, emit a log or call a partial safeInvoke.
            console.log(`Update lattice ${paramKey} -> ${val}`);
        }
    };
    return (
        <div className="flex items-center gap-1.5">
            <span className="w-3 text-slate-500 dark:text-slate-400 font-medium">{label}</span>
            <div className="flex-1 flex items-center bg-slate-100 dark:bg-slate-800/50 rounded border border-slate-200 dark:border-slate-700 px-1.5 py-0.5">
                <input
                    type="text"
                    key={value} // Force re-render on value change from outside
                    defaultValue={value}
                    onBlur={handleBlur}
                    className="w-full bg-transparent outline-none text-slate-700 dark:text-slate-300 min-w-0 text-xs"
                />
                <span className="text-slate-400 ml-0.5 text-[10px]">{unit}</span>
            </div>
        </div>
    );
};

const AtomRow = ({ id, element, x, y, z, isSelected, onClick }: { id: number; element: string; x: string; y: string; z: string; isSelected?: boolean; onClick?: () => void }) => {
    const hexColor = getJmolColor(element);
    return (
        <tr
            onClick={onClick}
            className={cn(
                "transition-colors cursor-pointer",
                isSelected
                    ? "bg-emerald-100 dark:bg-emerald-900/40"
                    : "hover:bg-slate-100 dark:hover:bg-slate-800/50"
            )}
        >
            <td className="px-2 py-1.5 text-slate-500">{id + 1}</td>
            <td className="px-2 py-1.5 font-medium">{element}</td>
            <td className="px-2 py-1.5 tabular-nums">{x}</td>
            <td className="px-2 py-1.5 tabular-nums">{y}</td>
            <td className="px-2 py-1.5 tabular-nums">{z}</td>
            <td className="px-2 py-1.5">
                <div
                    className="w-3 h-3 rounded-full shadow-sm mx-auto border border-black/10 dark:border-white/10"
                    style={{ backgroundColor: hexColor }}
                    title={`${element}: ${hexColor}`}
                />
            </td>
        </tr>
    )
};
