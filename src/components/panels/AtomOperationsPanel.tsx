import React, { useState } from 'react';
import { safeInvoke } from '../../utils/tauri-mock';
import { PromptModal } from '../layout/PromptModal';
import { ActionButton } from './shared';
import { PanelProps } from './index';

const DisabledButton = ({ label }: { label: string }) => (
    <button disabled className="w-full py-1.5 bg-slate-100 dark:bg-slate-800/60 text-slate-400 dark:text-slate-500 cursor-not-allowed rounded-md border border-slate-200 dark:border-slate-700 text-xs">
        {label}
    </button>
);

export default function AtomOperationsPanel({ crystalState, selectedAtoms = [], onSelectionChange, onStructureUpdate }: PanelProps) {
    const [promptConfig, setPromptConfig] = useState<{
        isOpen: boolean;
        title: string;
        description?: string;
        placeholder?: string;
        initialValue?: string;
        onSubmit: (value: string) => void;
    }>({ isOpen: false, title: "", onSubmit: () => { } });

    const handle_delete_atom = () => {
        if (selectedAtoms.length === 0) return;
        safeInvoke('delete_atoms', { indices: selectedAtoms }).then(() => {
            if (onSelectionChange) onSelectionChange([]);
            if (onStructureUpdate) onStructureUpdate();
        }).catch(console.error);
    };

    const handle_replace_atom = () => {
        if (selectedAtoms.length === 0) return;
        setPromptConfig({
            isOpen: true,
            title: "Replace Atom(s)",
            description: "Enter new element symbol (e.g., Fe, O, C):",
            placeholder: "Element symbol",
            onSubmit: (newElem) => {
                if (newElem && newElem.trim().length > 0) {
                    safeInvoke('substitute_atoms', {
                        indices: selectedAtoms,
                        newElementSymbol: newElem.trim(),
                        newAtomicNumber: 0
                    })
                        .then(() => {
                            safeInvoke('get_crystal_state').catch(console.error);
                            if (onStructureUpdate) onStructureUpdate();
                        })
                        .catch((e: any) => alert(e));
                }
            }
        });
    };

    return (
        <div className="space-y-3">
            <div className="text-xs space-y-1">
                <div className="text-slate-500 dark:text-slate-400">
                    Selected: <span className="text-slate-800 dark:text-slate-200 font-medium">
                        {selectedAtoms.length > 0 ? (selectedAtoms.length === 1 ? `Atom #${selectedAtoms[0]}` : `${selectedAtoms.length} atoms`) : "None"}
                    </span>
                </div>
                <div className="text-slate-500 dark:text-slate-400">
                    Element: <span className="text-slate-800 dark:text-slate-200 font-medium">
                        {selectedAtoms.length === 1 && crystalState ? crystalState.elements?.[selectedAtoms[0]] : (selectedAtoms.length > 1 ? "Mixed" : "-")}
                    </span>
                </div>
            </div>

            <div className="flex flex-col gap-1.5">
                {selectedAtoms.length > 0 ? (
                    <>
                        <ActionButton label="Replace Atom(s)" onClick={handle_replace_atom} />
                        <button onClick={handle_delete_atom} className="w-full py-1.5 bg-red-500/10 hover:bg-red-500/20 text-red-600 dark:text-red-400 rounded-md text-xs font-medium transition-colors border border-red-200 dark:border-red-900 active:scale-[0.98] pointer-events-auto">
                            Delete Atom(s)
                        </button>
                        <DisabledButton label="Add Sub-Atom" />
                    </>
                ) : (
                    <>
                        <DisabledButton label="Replace Atom(s)" />
                        <DisabledButton label="Delete Atom(s)" />
                        <DisabledButton label="Add Sub-Atom" />
                    </>
                )}
            </div>

            <PromptModal
                {...promptConfig}
                onClose={() => setPromptConfig(prev => ({ ...prev, isOpen: false }))}
            />
        </div>
    );
}
