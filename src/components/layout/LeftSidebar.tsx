// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React from 'react';
import { cn } from '../../utils/cn';
import { getJmolColor } from '../../utils/colors';

export const LeftSidebar: React.FC = () => {
    return (
        <div className="w-[300px] shrink-0 h-full flex flex-col gap-3 p-3 pb-10 pointer-events-none overflow-y-auto custom-scrollbar">
            <Panel title="Structure Info">
                <div className="space-y-2 text-xs">
                    <InfoRow label="Atoms:" value="512" />
                    <InfoRow label="Space Group:" value="F m -3 m" />
                    <div className="grid grid-cols-2 gap-x-4 gap-y-1.5 pt-2">
                        <UnitCellInput label="a" value="10.2" unit="Å" />
                        <UnitCellInput label="α" value="90" unit="°" />
                        <UnitCellInput label="b" value="10.2" unit="Å" />
                        <UnitCellInput label="β" value="90" unit="°" />
                        <UnitCellInput label="c" value="14.5" unit="Å" />
                        <UnitCellInput label="γ" value="90" unit="°" />
                    </div>
                    <InfoRow label="Volume:" value="1508.6 Å³" className="pt-1.5 font-medium" />
                </div>
            </Panel>

            <Panel title="Atom Management" badge="[TDD 1.3]">
                <div className="w-full bg-slate-50 dark:bg-slate-900/50 rounded-lg border border-slate-200 dark:border-slate-800 overflow-hidden text-xs max-h-[90px] overflow-y-auto custom-scrollbar">
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
                            <AtomRow id={12} element="Na" x="1.23" y="0.00" z="0.54" />
                            <AtomRow id={13} element="Na" x="0.03" y="0.00" z="0.50" />
                            <AtomRow id={14} element="Cl" x="0.53" y="0.50" z="0.50" />
                            <AtomRow id={15} element="Cl" x="0.03" y="0.50" z="0.00" />
                        </tbody>
                    </table>
                </div>
            </Panel>

            <Panel title="Visual Settings">
                <div className="space-y-3 text-xs">
                    <SliderRow label="Atomic Size" />
                    <SliderRow label="Bond Length" />
                    <div className="space-y-2 pt-1">
                        <CheckboxRow label="Show Cell" checked />
                        <CheckboxRow label="Show Bonds" checked />
                        <CheckboxRow label="Show Labels" checked={false} />
                    </div>
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

const UnitCellInput = ({ label, value, unit }: { label: string; value: string; unit: string }) => (
    <div className="flex items-center gap-1.5">
        <span className="w-3 text-slate-500 dark:text-slate-400 font-medium">{label}</span>
        <div className="flex-1 flex items-center bg-slate-100 dark:bg-slate-800/50 rounded border border-slate-200 dark:border-slate-700 px-1.5 py-0.5">
            <input
                type="text"
                defaultValue={value}
                className="w-full bg-transparent outline-none text-slate-700 dark:text-slate-300 min-w-0 text-xs"
            />
            <span className="text-slate-400 ml-0.5 text-[10px]">{unit}</span>
        </div>
    </div>
);

const AtomRow = ({ id, element, x, y, z }: { id: number; element: string; x: string; y: string; z: string }) => {
    const hexColor = getJmolColor(element);
    return (
        <tr className="hover:bg-slate-100 dark:hover:bg-slate-800/50 transition-colors">
            <td className="px-2 py-1.5 text-slate-500">{id}</td>
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

const SliderRow = ({ label }: { label: string }) => (
    <div className="space-y-1">
        <div className="flex justify-between">
            <span className="text-slate-600 dark:text-slate-400">{label}</span>
        </div>
        <input
            type="range"
            className="w-full h-1 accent-emerald-500 cursor-pointer"
        />
    </div>
);

const CheckboxRow = ({ label, checked }: { label: string; checked?: boolean }) => (
    <label className="flex items-center gap-2 cursor-pointer group">
        <div className={cn(
            "w-3.5 h-3.5 rounded-sm flex items-center justify-center transition-colors border",
            checked
                ? "bg-emerald-500 border-emerald-500 text-white"
                : "border-slate-300 dark:border-slate-600 group-hover:border-emerald-500"
        )}>
            {checked && (
                <svg fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3} className="w-2.5 h-2.5">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                </svg>
            )}
        </div>
        <span className="text-slate-700 dark:text-slate-300 group-hover:text-emerald-600 dark:group-hover:text-emerald-400 transition-colors">
            {label}
        </span>
    </label>
);
