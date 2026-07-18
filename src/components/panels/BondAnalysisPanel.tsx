import React, { useState } from 'react';
import { safeInvoke } from '../../utils/tauri-mock';
import { IpcException, type IpcError } from '../../ipc/contracts';
import { BondAnalysisResult } from '../../types/crystal';
import { PanelProps } from './index';
import { ActionButton, PanelError } from './shared';

export default function BondAnalysisPanel({
    selectedAtoms,
    onBondCountUpdate
}: PanelProps) {
    const [bondAnalysis, setBondAnalysis] = useState<BondAnalysisResult | null>(null);
    const [isLoading, setIsLoading] = useState(false);
    const [error, setError] = useState<IpcError | null>(null);

    const setPanelError = (cause: unknown, fallback: string) => {
        if (cause instanceof IpcException) {
            setError({ code: cause.code, message: cause.message, recoverable: cause.recoverable });
            return;
        }
        setError({ code: 'internal_error', message: fallback, recoverable: false });
    };

    const handle_refresh_bonds = async () => {
        if (isLoading) return;
        setError(null);
        setIsLoading(true);
        try {
            const res = await safeInvoke('get_bond_analysis', { thresholdFactor: 1.2 });
            if (res) {
                setBondAnalysis(res);
                if (onBondCountUpdate) onBondCountUpdate(res.bonds.length);
            }
        } catch (cause) {
            setPanelError(cause, 'Unable to calculate bonds and polyhedra.');
        } finally {
            setIsLoading(false);
        }
    };

    return (
        <div className="space-y-3" aria-busy={isLoading}>
            <ActionButton label="Calculate Bonds & Polyhedra" onClick={handle_refresh_bonds} disabled={isLoading} busy={isLoading} />

            {error && <PanelError error={error} message={error.message} />}
            {!bondAnalysis && !isLoading && !error && <div role="status" className="text-xs text-[var(--cc-muted)]">No bond analysis has been calculated.</div>}

            {bondAnalysis && (
                <div className="space-y-2 text-[11px] text-[var(--cc-text)]">
                    <div className="flex justify-between border-b border-[var(--cc-border)] pb-1 font-bold">
                        <span>Total Bonds: {bondAnalysis.bonds.length}</span>
                    </div>

                    {/* Bond Length Stats */}
                    <div className="max-h-32 overflow-y-auto custom-scrollbar space-y-1">
                        {bondAnalysis.bond_length_stats.map((stat, i) => (
                            <div key={i} className="flex items-center justify-between rounded bg-[var(--cc-field)] p-1">
                                <span className="font-medium">{stat.element_a}-{stat.element_b}</span>
                                <span className="tabular-nums">
                                    {stat.count} pair | {stat.mean.toFixed(2)} Å
                                </span>
                            </div>
                        ))}
                    </div>

                    {/* Selected Atom Distortion Index */}
                    {selectedAtoms?.length === 1 && bondAnalysis.coordination[selectedAtoms[0]] && (
                        <div className="mt-2 rounded border border-[var(--cc-border)] bg-[var(--cc-panel)] p-2">
                            <div className="mb-1 font-medium text-[var(--cc-accent)]">
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
