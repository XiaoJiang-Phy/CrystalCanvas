import React, { useState } from 'react';
import { safeInvoke } from '../../utils/tauri-mock';
import { BondAnalysisResult } from '../../types/crystal';
import { PanelProps } from './index';

export default function BondAnalysisPanel({
    selectedAtoms,
    onBondCountUpdate
}: PanelProps) {
    const [bondAnalysis, setBondAnalysis] = useState<BondAnalysisResult | null>(null);

    const handle_refresh_bonds = () => {
        safeInvoke<BondAnalysisResult>('get_bond_analysis', { thresholdFactor: 1.2 })
            .then(res => {
                if (res) {
                    setBondAnalysis(res);
                    if (onBondCountUpdate) onBondCountUpdate(res.bonds.length);
                }
            })
            .catch(console.error);
    };

    return (
        <div className="space-y-3">
            <button onClick={handle_refresh_bonds} className="flex-1 w-full py-1.5 bg-emerald-50 dark:bg-emerald-500/10 hover:bg-emerald-100 dark:hover:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 rounded-md text-xs font-medium transition-colors border border-emerald-200/50 dark:border-emerald-800/50 active:scale-[0.98] pointer-events-auto">
                Calculate Bonds & Polyhedra
            </button>

            {bondAnalysis && (
                <div className="text-[11px] text-slate-600 dark:text-slate-300 space-y-2">
                    <div className="flex justify-between font-bold border-b border-slate-200 dark:border-slate-700 pb-1">
                        <span>Total Bonds: {bondAnalysis.bonds.length}</span>
                    </div>

                    {/* Bond Length Stats */}
                    <div className="max-h-32 overflow-y-auto custom-scrollbar space-y-1">
                        {bondAnalysis.bond_length_stats.map((stat, i) => (
                            <div key={i} className="flex justify-between items-center bg-slate-50 dark:bg-slate-800/40 p-1 rounded">
                                <span className="font-medium">{stat.element_a}-{stat.element_b}</span>
                                <span className="tabular-nums">
                                    {stat.count} pair | {stat.mean.toFixed(2)} Å
                                </span>
                            </div>
                        ))}
                    </div>

                    {/* Selected Atom Distortion Index */}
                    {selectedAtoms?.length === 1 && bondAnalysis.coordination[selectedAtoms[0]] && (
                        <div className="mt-2 p-2 bg-emerald-50 dark:bg-emerald-900/20 rounded-md border border-emerald-100 dark:border-emerald-800/30">
                            <div className="font-medium text-emerald-800 dark:text-emerald-300 mb-1">
                                Atom #{selectedAtoms[0]} ({bondAnalysis.coordination[selectedAtoms[0]].element})
                            </div>
                            <div>Coordination: {bondAnalysis.coordination[selectedAtoms[0]].coordination_number}</div>
                            {bondAnalysis.coordination[selectedAtoms[0]].polyhedron_type && (
                                <div>Polyhedron: {bondAnalysis.coordination[selectedAtoms[0]].polyhedron_type}</div>
                            )}
                            {bondAnalysis.distortion_indices[selectedAtoms[0]] > 0 && (
                                <div>Distortion Δ: {bondAnalysis.distortion_indices[selectedAtoms[0]].toFixed(4)}</div>
                            )}
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
