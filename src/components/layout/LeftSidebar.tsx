// [Overview: Left sidebar component for structure info and atom management.]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React, { useState } from 'react';
import { cn } from '../../utils/cn';
import { getJmolColor } from '../../utils/colors';
import { renderSpacegroupSymbol } from '../../utils/spacegroup';
import { safeInvoke } from '../../utils/tauri-mock';
import { IpcException, type IpcError } from '../../ipc/contracts';
import { CrystalState } from '../../types/crystal';

interface LeftSidebarProps {
    crystalState: CrystalState | null;
    selectedAtoms: number[];
    onSelectionChange: (indices: number[]) => void;
}

type LatticeError = Pick<IpcError, 'code' | 'message'>;

const formatNumber = (value: number, digits: number): string => (
    Number.isFinite(value) ? value.toFixed(digits) : '—'
);

export const LeftSidebar: React.FC<LeftSidebarProps> = ({
    crystalState,
    selectedAtoms,
    onSelectionChange,
}) => {
    const [latticeError, setLatticeError] = useState<LatticeError | null>(null);
    const numAtoms = crystalState?.intrinsic_sites ?? 0;
    const structureLabel = crystalState && numAtoms > 0 ? 'Structure workspace' : 'No structure loaded';

    if (!crystalState || numAtoms === 0) {
        return null;
    }

    const volume = crystalState.cell_a * crystalState.cell_b * crystalState.cell_c * Math.sqrt(
        1 - Math.cos(crystalState.cell_alpha * Math.PI / 180) ** 2
        - Math.cos(crystalState.cell_beta * Math.PI / 180) ** 2
        - Math.cos(crystalState.cell_gamma * Math.PI / 180) ** 2
        + 2 * Math.cos(crystalState.cell_alpha * Math.PI / 180)
        * Math.cos(crystalState.cell_beta * Math.PI / 180)
        * Math.cos(crystalState.cell_gamma * Math.PI / 180),
    );
    const volumeDisplay = Number.isFinite(volume) ? volume.toFixed(1) : '—';

    return (
        <aside
            className="cc-panel w-[280px] 2xl:w-[320px] shrink-0 pointer-events-auto overflow-hidden"
            data-sidebar-surface="structure-workspace"
            aria-label={structureLabel}
        >
            <section className="p-3" data-sidebar-section="structure" aria-labelledby="structure-sidebar-title">
                <h2 id="structure-sidebar-title" className="text-sm font-semibold text-slate-800 dark:text-slate-200">
                    Structure
                </h2>
                <div className="mt-3 space-y-2 text-xs">
                    <InfoRow label="Atoms:" value={numAtoms.toString()} />
                    <InfoRow
                        label="Space Group:"
                        value={crystalState.spacegroup_number ? renderSpacegroupSymbol(crystalState.spacegroup_number) : 'N/A'}
                    />
                    <div className="grid grid-cols-2 gap-x-3 gap-y-1.5 pt-1">
                        <UnitCellInput label="a" paramKey="a" value={formatNumber(crystalState.cell_a, 2)} unit="Å" crystalState={crystalState} setLatticeError={setLatticeError} />
                        <UnitCellInput label="α" paramKey="alpha" value={formatNumber(crystalState.cell_alpha, 1)} unit="°" crystalState={crystalState} setLatticeError={setLatticeError} />
                        <UnitCellInput label="b" paramKey="b" value={formatNumber(crystalState.cell_b, 2)} unit="Å" crystalState={crystalState} setLatticeError={setLatticeError} />
                        <UnitCellInput label="β" paramKey="beta" value={formatNumber(crystalState.cell_beta, 1)} unit="°" crystalState={crystalState} setLatticeError={setLatticeError} />
                        <UnitCellInput label="c" paramKey="c" value={formatNumber(crystalState.cell_c, 2)} unit="Å" crystalState={crystalState} setLatticeError={setLatticeError} />
                        <UnitCellInput label="γ" paramKey="gamma" value={formatNumber(crystalState.cell_gamma, 1)} unit="°" crystalState={crystalState} setLatticeError={setLatticeError} />
                    </div>
                    <InfoRow label="Volume:" value={`${volumeDisplay} Å³`} className="pt-1 font-medium" />
                    {latticeError && (
                        <div role="alert" className="border-t border-red-200 pt-2 text-[11px] text-red-700 dark:border-red-900/60 dark:text-red-300">
                            <span className="font-mono font-medium">{latticeError.code}</span>
                            <span className="ml-1">{latticeError.message}</span>
                        </div>
                    )}
                </div>
            </section>

            <section className="border-t border-slate-200 dark:border-slate-800" data-sidebar-section="atoms" aria-labelledby="atoms-sidebar-title">
                <div className="px-3 py-2">
                    <h2 id="atoms-sidebar-title" className="text-sm font-semibold text-slate-800 dark:text-slate-200">Atoms</h2>
                </div>
                <div className="max-h-[220px] max-w-full overflow-x-auto overflow-y-auto border-t border-slate-200 dark:border-slate-800 text-[10px] custom-scrollbar">
                    <table className="min-w-[320px] text-left" aria-label="Intrinsic atom fractional coordinates" data-coordinate-system="fractional">
                        <thead className="sticky top-0 z-10 bg-[var(--cc-field)] font-medium text-slate-500 dark:text-slate-400">
                            <tr>
                                <th scope="col" className="px-2 py-1.5 text-center text-xs">ID</th>
                                <th scope="col" className="px-2 py-1.5 text-center text-xs">El</th>
                                <th scope="col" className="px-2 py-1.5 text-right text-xs">x</th>
                                <th scope="col" className="px-2 py-1.5 text-right text-xs">y</th>
                                <th scope="col" className="px-2 py-1.5 text-right text-xs">z</th>
                                <th scope="col" className="px-2 py-1.5 text-right text-xs">Occ.</th>
                                <th scope="col" className="px-2 py-1.5 text-center text-xs">Color</th>
                            </tr>
                        </thead>
                        <tbody className="divide-y divide-slate-200 dark:divide-slate-800">
                            {crystalState.labels.slice(0, crystalState.intrinsic_sites).map((_label, i) => (
                                <AtomRow
                                    key={i}
                                    id={i}
                                    element={crystalState.elements[i]}
                                    x={formatNumber(crystalState.fract_x[i], 2)}
                                    y={formatNumber(crystalState.fract_y[i], 2)}
                                    z={formatNumber(crystalState.fract_z[i], 2)}
                                    occ={formatNumber(crystalState.occupancies[i], 2)}
                                    isSelected={selectedAtoms.includes(i)}
                                    onClick={(event) => {
                                        if (event.shiftKey) {
                                            if (selectedAtoms.includes(i)) {
                                                onSelectionChange(selectedAtoms.filter((index) => index !== i));
                                            } else {
                                                onSelectionChange([...selectedAtoms, i]);
                                            }
                                        } else {
                                            onSelectionChange([i]);
                                        }
                                    }}
                                />
                            ))}
                        </tbody>
                    </table>
                </div>
            </section>
        </aside>
    );
};

const InfoRow = ({ label, value, className }: { label: string; value: React.ReactNode; className?: string }) => (
    <div className={cn('flex items-center justify-between', className)}>
        <span className="text-slate-500 dark:text-slate-400">{label}</span>
        <span className="tabular-nums font-medium text-slate-700 dark:text-slate-300">{value}</span>
    </div>
);

const UnitCellInput = ({
    label,
    paramKey,
    value,
    unit,
    crystalState,
    setLatticeError,
}: {
    label: string;
    paramKey: string;
    value: string;
    unit: string;
    crystalState: CrystalState;
    setLatticeError: React.Dispatch<React.SetStateAction<LatticeError | null>>;
}) => {
    const handleBlur = async (event: React.FocusEvent<HTMLInputElement>) => {
        const nextValue = Number(event.target.value);
        if (!Number.isFinite(nextValue)) {
            setLatticeError({ code: 'invalid_argument', message: `Lattice ${label} must be finite.` });
            return;
        }

        const params = {
            a: crystalState.cell_a,
            b: crystalState.cell_b,
            c: crystalState.cell_c,
            alpha: crystalState.cell_alpha,
            beta: crystalState.cell_beta,
            gamma: crystalState.cell_gamma,
        };

        if (paramKey === 'a') params.a = nextValue;
        else if (paramKey === 'b') params.b = nextValue;
        else if (paramKey === 'c') params.c = nextValue;
        else if (paramKey === 'alpha') params.alpha = nextValue;
        else if (paramKey === 'beta') params.beta = nextValue;
        else if (paramKey === 'gamma') params.gamma = nextValue;

        try {
            await safeInvoke('update_lattice_params', {
                a: params.a,
                b: params.b,
                c: params.c,
                alpha: params.alpha,
                beta: params.beta,
                gamma: params.gamma,
            });
            setLatticeError(null);
        } catch (error) {
            if (error instanceof IpcException) {
                setLatticeError({ code: error.code, message: error.message });
                return;
            }
            setLatticeError({ code: 'internal_error', message: 'Unable to update lattice parameters.' });
        }
    };

    return (
        <label className="grid grid-cols-[1rem_minmax(0,1fr)_auto] items-center gap-1.5">
            <span className="font-medium text-slate-500 dark:text-slate-400">{label}</span>
            <input
                type="text"
                key={value}
                defaultValue={value}
                onBlur={handleBlur}
                aria-label={`Lattice ${label} (${unit})`}
                className="min-w-0 border border-slate-200 bg-[var(--cc-field)] px-1.5 py-0.5 text-right text-xs tabular-nums text-slate-700 outline-none dark:border-slate-700 dark:text-slate-300"
            />
            <span className="shrink-0 text-right text-[10px] text-slate-400" data-unit={unit}>{unit}</span>
        </label>
    );
};

interface AtomRowProps {
    id: number;
    element: string;
    x: string;
    y: string;
    z: string;
    occ: string;
    isSelected: boolean;
    onClick: (event: React.MouseEvent) => void;
}

const AtomRow: React.FC<AtomRowProps> = ({ id, element, x, y, z, occ, isSelected, onClick }) => {
    const hexColor = getJmolColor(element);
    return (
        <tr
            onClick={onClick}
            aria-selected={isSelected}
            className={cn(
                'cursor-pointer transition-colors',
                isSelected
                    ? 'bg-emerald-100 dark:bg-emerald-900/40'
                    : 'hover:bg-slate-100 dark:hover:bg-slate-800/50',
            )}
        >
            <td className="px-2 py-1.5 text-center font-mono text-slate-500">{id + 1}</td>
            <td className="px-2 py-1.5 text-center font-medium">{element}</td>
            <td className="px-2 py-1.5 text-right font-mono tabular-nums">{x}</td>
            <td className="px-2 py-1.5 text-right font-mono tabular-nums">{y}</td>
            <td className="px-2 py-1.5 text-right font-mono tabular-nums">{z}</td>
            <td className="px-2 py-1.5 text-right font-mono tabular-nums">{occ}</td>
            <td className="px-2 py-1.5">
                <div
                    className="mx-auto h-3 w-3 rounded-full border border-black/10 shadow-sm dark:border-white/10"
                    style={{ backgroundColor: hexColor }}
                    title={`${element}: ${hexColor}`}
                />
            </td>
        </tr>
    );
};
