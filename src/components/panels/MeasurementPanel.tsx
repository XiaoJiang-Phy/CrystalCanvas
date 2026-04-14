import React, { useState, useEffect } from 'react';
import ReactDOM from 'react-dom';
import { safeInvoke } from '../../utils/tauri-mock';
import { cn } from '../../utils/cn';
import { PanelProps } from './index';

export default function MeasurementPanel({ crystalState, selectedAtoms = [], onSelectionChange, onStructureUpdate }: PanelProps) {
    const [measurementLabels, setMeasurementLabels] = useState<{label: string, x: number, y: number}[]>([]);

    useEffect(() => {
        if (!crystalState?.measurements || crystalState.measurements.length === 0) {
            setMeasurementLabels([]);
            return;
        }
        let active = true;
        const updateLabels = async () => {
            if (!active) return;
            try {
                const w = window.innerWidth;
                const h = window.innerHeight;
                const labels = await safeInvoke<{label: string, x: number, y: number}[]>('get_measurement_labels_screen', { width: w, height: h });
                if (active) setMeasurementLabels(labels || []);
            } catch (e: any) {
                if (active) setMeasurementLabels([]);
            }
            if (active) requestAnimationFrame(updateLabels);
        };
        updateLabels();
        return () => { active = false; };
    }, [crystalState?.measurements]);

    return (
        <>
        <div className="space-y-3">
            <div className="flex justify-between items-center text-xs">
                <span className="text-slate-500 dark:text-slate-400">Total Measurements:</span>
                <span className="font-semibold text-slate-800 dark:text-slate-200">{crystalState?.measurements?.length || 0}</span>
            </div>

            {crystalState?.measurements && crystalState.measurements.length > 0 ? (
                <div className="space-y-2 max-h-48 overflow-y-auto custom-scrollbar">
                    {crystalState.measurements.map((m, i) => (
                        <div key={i} className="text-[11px] bg-slate-50 dark:bg-slate-800/40 p-2 rounded border border-slate-100 dark:border-slate-700">
                            <div className="flex justify-between font-medium text-slate-700 dark:text-slate-300 mb-1">
                                <span>{m.kind}</span>
                                <span className="text-emerald-600 dark:text-emerald-400">
                                    {m.value.toFixed(2)} {m.kind === 'Distance' ? 'Å' : '°'}
                                </span>
                            </div>
                            <div className="text-slate-500 font-mono tracking-tighter">
                                [{m.indices.join('-')}]
                            </div>
                        </div>
                    ))}
                </div>
            ) : (
                <div className="text-xs text-slate-400 italic text-center py-4">No measurements yet</div>
            )}

            <button
                onClick={() => safeInvoke('clear_measurements').then(() => { if (onStructureUpdate) onStructureUpdate(); }).catch((e: any) => alert(e))}
                disabled={!crystalState?.measurements?.length}
                className={cn(
                    "w-full py-1.5 rounded-md text-xs font-medium transition-colors border pointer-events-auto",
                    crystalState?.measurements?.length ? "bg-red-50 text-red-600 border-red-200 hover:bg-red-100 dark:bg-red-500/10 dark:text-red-400 dark:border-red-800/50 dark:hover:bg-red-500/20 active:scale-[0.98]" : "bg-slate-100 dark:bg-slate-800/60 text-slate-400 dark:text-slate-500 cursor-not-allowed border-slate-200 dark:border-slate-700"
                )}
            >
                Clear All Measurements
            </button>
            
            <button
                onClick={() => {
                    if (selectedAtoms.length >= 2 && selectedAtoms.length <= 4) {
                        safeInvoke('add_measurement', { indices: selectedAtoms })
                            .then(() => { if (onStructureUpdate) onStructureUpdate(); if (onSelectionChange) onSelectionChange([]); })
                            .catch((e: any) => alert(e));
                    } else {
                        alert("Please select exactly 2, 3, or 4 atoms first.");
                    }
                }}
                disabled={selectedAtoms.length < 2 || selectedAtoms.length > 4}
                className={cn(
                    "w-full py-1.5 rounded-md text-xs font-medium transition-colors shadow-sm pointer-events-auto",
                    (selectedAtoms.length >= 2 && selectedAtoms.length <= 4) ? "bg-emerald-500 hover:bg-emerald-600 text-white active:scale-[0.98]" : "bg-slate-300 dark:bg-slate-700 text-slate-500 cursor-not-allowed"
                )}
            >
                Add Measurement from Selection
            </button>
            
            <div className="text-[10px] text-slate-400 flex items-center justify-center p-1 bg-amber-50 dark:bg-amber-900/10 text-amber-700 dark:text-amber-500 rounded border border-amber-200/50">
                Shift-click to select 2 (Distance), 3 (Angle), or 4 (Dihedral) atoms, then add.
            </div>
        </div>
        {measurementLabels.length > 0 && ReactDOM.createPortal(
            <div className="fixed inset-0 pointer-events-none z-[50]" style={{fontFamily: "'Inter', 'SF Pro', system-ui, sans-serif"}}>
                {measurementLabels.map((lbl, i) => (
                    <div
                        key={i}
                        className="absolute text-[12px] font-bold whitespace-nowrap bg-slate-900/40 backdrop-blur-[2px] text-orange-400 px-1.5 py-0.5 rounded border border-orange-500/30"
                        style={{
                            left: lbl.x,
                            top: lbl.y,
                            transform: 'translate(-50%, -50%)',
                            boxShadow: '0 2px 4px rgba(0,0,0,0.2)'
                        }}
                    >
                        {lbl.label}
                    </div>
                ))}
            </div>,
            document.body
        )}
        </>
    );
}
