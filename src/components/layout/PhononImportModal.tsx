import React, { useState } from 'react';
import { safeDialogOpen } from '../../utils/tauri-mock';

interface PhononImportModalProps {
    isOpen: boolean;
    onClose: () => void;
    onSubmit: (paths: { scfIn: string, scfOut: string, modes: string, axsf: string }) => void;
}

export const PhononImportModal: React.FC<PhononImportModalProps> = ({ isOpen, onClose, onSubmit }) => {
    const [scfIn, setScfIn] = useState('');
    const [scfOut, setScfOut] = useState('');
    const [modes, setModes] = useState('');
    const [axsf, setAxsf] = useState('');

    if (!isOpen) return null;

    const handleSelectFile = async (setter: (val: string) => void, title: string, extensions: string[]) => {
        const path = await safeDialogOpen({
            title,
            filters: [{ name: title, extensions }],
            multiple: false,
            directory: false
        });
        if (path && typeof path === 'string') {
            setter(path);
        }
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 pointer-events-auto">
            <div className="bg-white dark:bg-slate-900 rounded-lg shadow-xl border border-slate-200 dark:border-slate-700 w-[500px] overflow-hidden">
                <div className="px-5 py-4 border-b border-slate-200 dark:border-slate-800">
                    <h3 className="text-sm font-semibold text-slate-800 dark:text-slate-200">Load Phonon Data (Quantum ESPRESSO)</h3>
                    <p className="text-xs text-slate-500 mt-1">Select the required files to reconstruct the structure and modes.</p>
                </div>

                <div className="px-5 py-5 space-y-4">
                    <div className="space-y-1.5">
                        <label className="text-xs font-medium text-slate-700 dark:text-slate-300">scf.in (Input)</label>
                        <div className="flex gap-2">
                            <input type="text" readOnly value={scfIn} placeholder="Select scf.in file..." className="flex-1 bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded px-3 text-xs dark:text-slate-300" />
                            <button onClick={() => handleSelectFile(setScfIn, "Select scf.in", ["in", "txt", ""])} className="px-3 py-1.5 bg-slate-200 dark:bg-slate-700 hover:bg-slate-300 dark:hover:bg-slate-600 text-xs rounded transition-colors text-slate-800 dark:text-slate-200">Browse</button>
                        </div>
                    </div>

                    <div className="space-y-1.5">
                        <label className="text-xs font-medium text-slate-700 dark:text-slate-300">scf.out (Output)</label>
                        <div className="flex gap-2">
                            <input type="text" readOnly value={scfOut} placeholder="Select scf.out file..." className="flex-1 bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded px-3 text-xs dark:text-slate-300" />
                            <button onClick={() => handleSelectFile(setScfOut, "Select scf.out", ["out", "txt", ""])} className="px-3 py-1.5 bg-slate-200 dark:bg-slate-700 hover:bg-slate-300 dark:hover:bg-slate-600 text-xs rounded transition-colors text-slate-800 dark:text-slate-200">Browse</button>
                        </div>
                    </div>

                    <div className="space-y-1.5">
                        <label className="text-xs font-medium text-slate-700 dark:text-slate-300">matdyn.modes or dynmat.dat</label>
                        <div className="flex gap-2">
                            <input type="text" readOnly value={modes} placeholder="Select modes file..." className="flex-1 bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded px-3 text-xs dark:text-slate-300" />
                            <button onClick={() => handleSelectFile(setModes, "Select modes file", ["modes", "dat", "mold", ""])} className="px-3 py-1.5 bg-slate-200 dark:bg-slate-700 hover:bg-slate-300 dark:hover:bg-slate-600 text-xs rounded transition-colors text-slate-800 dark:text-slate-200">Browse</button>
                        </div>
                    </div>
                </div>

                <div className="flex items-center gap-4 py-2">
                    <div className="flex-1 h-px bg-slate-200 dark:bg-slate-700"></div>
                    <span className="text-[10px] text-slate-400 font-medium uppercase tracking-wider">OR</span>
                    <div className="flex-1 h-px bg-slate-200 dark:bg-slate-700"></div>
                </div>

                <div className="px-5 pb-5 space-y-4">
                    <div className="space-y-1.5">
                        <label className="text-xs font-medium text-slate-700 dark:text-slate-300">AXSF Animation File</label>
                        <div className="flex gap-2">
                            <input type="text" readOnly value={axsf} placeholder="Select .axsf file..." className="flex-1 bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded px-3 text-xs dark:text-slate-300" />
                            <button onClick={() => handleSelectFile(setAxsf, "Select AXSF file", ["axsf", ""])} className="px-3 py-1.5 bg-slate-200 dark:bg-slate-700 hover:bg-slate-300 dark:hover:bg-slate-600 text-xs rounded transition-colors text-slate-800 dark:text-slate-200">Browse</button>
                        </div>
                        <p className="text-[10px] text-slate-500 mt-1">If an AXSF file is provided, the inputs above will be ignored.</p>
                    </div>
                </div>

                <div className="px-5 py-4 border-t border-slate-200 dark:border-slate-800 flex justify-end gap-2 bg-slate-50 dark:bg-slate-800/30">
                    <button onClick={onClose} className="px-4 py-1.5 text-xs text-slate-600 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200 font-medium transition-colors">
                        Cancel
                    </button>
                    <button
                        onClick={() => onSubmit({ scfIn, scfOut, modes, axsf })}
                        disabled={!axsf && (!scfIn || !scfOut || !modes)}
                        className="px-4 py-1.5 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 disabled:cursor-not-allowed text-white text-xs font-medium rounded transition-colors shadow-sm"
                    >
                        Load Data
                    </button>
                </div>
            </div>
        </div>
    );
};
