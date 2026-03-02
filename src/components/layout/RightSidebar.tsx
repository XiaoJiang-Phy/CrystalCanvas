// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React, { useState } from 'react';
import { cn } from '../../utils/cn';

import { CrystalState } from '../../types/crystal';

export const RightSidebar: React.FC<{ crystalState: CrystalState | null }> = ({ crystalState }) => {
    return (
        <div className="w-[240px] shrink-0 h-full flex flex-col gap-3 p-3 pointer-events-none overflow-y-auto custom-scrollbar">

            {/* Supercell Accordion */}
            <Accordion title="Supercell Construction" defaultOpen>
                <div className="space-y-3">
                    <div className="flex gap-2 text-xs">
                        <NumberInput label="Nx" defaultValue={1} />
                        <NumberInput label="Ny" defaultValue={1} />
                        <NumberInput label="Nz" defaultValue={1} />
                    </div>
                    <button className="w-full py-1.5 bg-emerald-500 hover:bg-emerald-600 text-white rounded-md text-xs font-medium transition-colors shadow-sm active:scale-[0.98]">
                        Execute Supercell
                    </button>
                </div>
            </Accordion>

            {/* Cutting Plane Accordion */}
            <Accordion title="Cutting Plane (hkl)" defaultOpen>
                <div className="space-y-3">
                    <div className="flex gap-2 text-xs">
                        <NumberInput label="h" defaultValue={0} />
                        <NumberInput label="k" defaultValue={0} />
                        <NumberInput label="l" defaultValue={0} />
                    </div>

                    <div className="space-y-1">
                        <span className="text-[11px] text-slate-500 dark:text-slate-400">Cutting Depth</span>
                        <input type="range" className="w-full h-1 accent-emerald-500 cursor-pointer" />
                    </div>

                    <div className="flex gap-2">
                        <ActionButton label="Cut" />
                        <ActionButton label="Reset" />
                    </div>
                </div>
            </Accordion>

            {/* Atom Operations Accordion */}
            <Accordion title="Atom Operations" defaultOpen>
                <div className="space-y-3">
                    <div className="text-xs space-y-1">
                        <div className="text-slate-500 dark:text-slate-400">
                            Selected: <span className="text-slate-800 dark:text-slate-200 font-medium">None</span>
                        </div>
                        <div className="text-slate-500 dark:text-slate-400">
                            Element: <span className="text-slate-800 dark:text-slate-200 font-medium">-</span>
                        </div>
                    </div>

                    <div className="flex flex-col gap-1.5">
                        <DisabledButton label="Replace Atom" />
                        <DisabledButton label="Delete Atom" />
                        <DisabledButton label="Add Sub-Atom" />
                    </div>
                </div>
            </Accordion>

        </div>
    );
};

// --- Subcomponents ---

const NumberInput = ({ label, defaultValue }: { label: string; defaultValue: number }) => (
    <div className="flex-1 space-y-0.5">
        <label className="text-[11px] text-slate-500 dark:text-slate-400">{label}</label>
        <input
            type="number"
            defaultValue={defaultValue}
            min={label.startsWith('N') ? 1 : undefined}
            className="w-full bg-slate-100 dark:bg-slate-800/60 rounded px-2 py-1 outline-none border border-slate-200 dark:border-slate-700 text-xs focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500/30 transition-all text-slate-700 dark:text-slate-300"
        />
    </div>
);

const ActionButton = ({ label }: { label: string }) => (
    <button className="flex-1 py-1.5 bg-slate-100 dark:bg-slate-800/60 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 rounded-md text-xs font-medium transition-colors border border-slate-200 dark:border-slate-700 active:scale-[0.98]">
        {label}
    </button>
);

const DisabledButton = ({ label }: { label: string }) => (
    <button disabled className="w-full py-1.5 bg-slate-100 dark:bg-slate-800/60 text-slate-400 dark:text-slate-500 cursor-not-allowed rounded-md border border-slate-200 dark:border-slate-700 text-xs">
        {label}
    </button>
);

const Accordion: React.FC<{ title: string; defaultOpen?: boolean; children: React.ReactNode }> = ({ title, defaultOpen = false, children }) => {
    const [isOpen, setIsOpen] = useState(defaultOpen);

    return (
        <div className="pointer-events-auto bg-white/80 dark:bg-slate-900/80 backdrop-blur-xl border border-white/30 dark:border-slate-700/50 rounded-xl shadow-lg shadow-black/5 dark:shadow-black/20 overflow-hidden">
            <button
                onClick={() => setIsOpen(!isOpen)}
                className={cn(
                    "w-full px-3 py-2.5 flex justify-between items-center bg-transparent hover:bg-slate-50/50 dark:hover:bg-slate-800/50 transition-colors",
                    isOpen && "border-b border-slate-100 dark:border-slate-800"
                )}
            >
                <span className="font-medium text-sm text-slate-800 dark:text-slate-200">{title}</span>
                <svg
                    className={cn("w-3.5 h-3.5 text-slate-400 transition-transform duration-200", isOpen && "rotate-180")}
                    fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}
                >
                    <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
                </svg>
            </button>

            <div className={cn(
                "transition-all duration-300 ease-in-out overflow-hidden origin-top",
                isOpen ? "max-h-96 opacity-100" : "max-h-0 opacity-0"
            )}>
                <div className="px-3 py-3">
                    {children}
                </div>
            </div>
        </div>
    );
};
