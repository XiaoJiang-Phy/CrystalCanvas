import React, { useState } from 'react';
import { safeInvoke, safeDialogOpen } from '../../utils/tauri-mock';
import { IpcException, type IpcError } from '../../ipc/contracts';
import { WannierInfo } from '../../types/crystal';
import { PanelProps } from './index';
import { ActionButton, PanelError, RangeInput } from './shared';

export default function WannierPanel({}: PanelProps) {
    const [wannierInfo, setWannierInfo] = useState<WannierInfo | null>(null);
    const [tMin, setTMin] = useState<number>(0.01);
    const [activeRShells, setActiveRShells] = useState<boolean[]>([]);
    const [activeOrbitals, setActiveOrbitals] = useState<boolean[]>([]);
    const [showOnSite, setShowOnSite] = useState(false);
    const [isWannierVisible, setIsWannierVisible] = useState(false);
    const [activeOperation, setActiveOperation] = useState<'load' | 'clear' | null>(null);
    const [pendingControl, setPendingControl] = useState<string | null>(null);
    const [error, setError] = useState<IpcError | null>(null);

    const setPanelError = (cause: unknown, fallback: string) => {
        if (cause instanceof IpcException) {
            setError({ code: cause.code, message: cause.message, recoverable: cause.recoverable });
            return;
        }
        setError({ code: 'internal_error', message: fallback, recoverable: false });
    };

    const handle_load_wannier = async () => {
        if (activeOperation) return;
        setError(null);
        setActiveOperation('load');
        try {
            const file = await safeDialogOpen({
                title: 'Open wannier90_hr.dat',
                filters: [{ name: 'Wannier Hopping', extensions: ['dat'] }]
            });
            if (file && typeof file === 'string') {
                const info = await safeInvoke('load_wannier_hr', { path: file });
                if (info) {
                    setWannierInfo(info);
                    setTMin(0.01);
                    setActiveRShells(new Array(info.r_shells.length).fill(true));
                    setActiveOrbitals(new Array(info.num_wann).fill(true));
                    setShowOnSite(false);
                    setIsWannierVisible(true);
                }
            }
        } catch (cause) {
            setPanelError(cause, 'Unable to load Wannier hopping data.');
        } finally {
            setActiveOperation(null);
        }
    };

    const handle_clear_wannier = async () => {
        if (activeOperation) return;
        setError(null);
        setActiveOperation('clear');
        try {
            await safeInvoke('clear_wannier');
            setWannierInfo(null);
            setIsWannierVisible(false);
        } catch (cause) {
            setPanelError(cause, 'Unable to clear the Wannier overlay.');
        } finally {
            setActiveOperation(null);
        }
    };

    const handleSetOrbital = async (orbIdx: number, checked: boolean) => {
        if (activeOperation || pendingControl) return;
        const next = [...activeOrbitals];
        next[orbIdx] = checked;
        setError(null);
        setPendingControl('orbital');
        try {
            await safeInvoke('set_wannier_orbital', { orbIdx, active: checked });
            setActiveOrbitals(next);
        } catch (cause) {
            setPanelError(cause, 'Unable to change the active orbital.');
        } finally {
            setPendingControl(null);
        }
    };

    const handleToggleOnSite = async (checked: boolean) => {
        if (activeOperation || pendingControl) return;
        setError(null);
        setPendingControl('onsite');
        try {
            await safeInvoke('toggle_wannier_onsite', { show: checked });
            setShowOnSite(checked);
        } catch (cause) {
            setPanelError(cause, 'Unable to change on-site hopping visibility.');
        } finally {
            setPendingControl(null);
        }
    };

    const handleSetRShell = async (shellIdx: number, checked: boolean) => {
        if (activeOperation || pendingControl) return;
        const next = [...activeRShells];
        next[shellIdx] = checked;
        setError(null);
        setPendingControl('shell');
        try {
            await safeInvoke('set_wannier_r_shell', { shellIdx, active: checked });
            setActiveRShells(next);
        } catch (cause) {
            setPanelError(cause, 'Unable to change the active translation shell.');
        } finally {
            setPendingControl(null);
        }
    };

    const handleToggleVisibility = async () => {
        if (activeOperation || pendingControl) return;
        const next = !isWannierVisible;
        setError(null);
        setPendingControl('visibility');
        try {
            await safeInvoke('toggle_hopping_display', { show: next });
            setIsWannierVisible(next);
        } catch (cause) {
            setPanelError(cause, 'Unable to change hopping-network visibility.');
        } finally {
            setPendingControl(null);
        }
    };

    const isBusy = activeOperation !== null;
    const isPanelBusy = isBusy || pendingControl !== null;
    const loadAction = <ActionButton label="Load wannier90_hr.dat..." busyLabel="Loading Wannier data…" onClick={handle_load_wannier} disabled={isPanelBusy} busy={activeOperation === 'load'} />;

    if (!wannierInfo) {
        return (
            <div className="space-y-3" aria-busy={isPanelBusy}>
                {loadAction}
                {error && <PanelError error={error} message={error.message} />}
                {!isPanelBusy && !error && <div role="status" className="text-xs text-[var(--cc-muted)]">No Wannier hopping data is loaded.</div>}
            </div>
        );
    }

    const hasUsableHoppingRange = Number.isFinite(wannierInfo.t_max) && wannierInfo.t_max > 0;
    const hoppingStep = wannierInfo.t_max / 100;
    const hasUsableHoppingStep = hasUsableHoppingRange && Number.isFinite(hoppingStep) && hoppingStep > 0;
    const hasUsableHoppingThreshold = hasUsableHoppingStep && tMin <= wannierInfo.t_max;
    const hoppingThreshold = hasUsableHoppingThreshold ? tMin : 0;

    return (
        <div className="space-y-3" aria-busy={isPanelBusy}>
            {loadAction}
            {error && <PanelError error={error} message={error.message} />}
            
                    <div className="space-y-1 rounded border border-[var(--cc-border)] bg-[var(--cc-panel)] p-2 text-[11px] text-[var(--cc-text)]">
                        <div className="flex justify-between">
                            <span className="text-[var(--cc-muted)]">Orbitals:</span>
                            <span className="font-semibold">{wannierInfo.num_wann}</span>
                        </div>
                        <div className="flex justify-between">
                            <span className="text-[var(--cc-muted)]">R-Shells:</span>
                            <span className="font-semibold">{wannierInfo.r_shells.length}</span>
                        </div>
                        <div className="flex justify-between">
                            <span className="text-[var(--cc-muted)]">Max |t|:</span>
                            <span className="font-semibold">{hasUsableHoppingRange ? `${wannierInfo.t_max.toFixed(4)} eV` : 'Unavailable'}</span>
                        </div>
                    </div>
                    
                    {hasUsableHoppingThreshold ? (
                        <RangeInput
                            label="|t| Threshold"
                            value={hoppingThreshold}
                            displayValue={`${hoppingThreshold.toFixed(3)} eV`}
                            min={0}
                            max={wannierInfo.t_max}
                            step={hoppingStep}
                            onChange={(value) => {
                                const previous = tMin;
                                setError(null);
                                setTMin(value);
                                safeInvoke('set_wannier_t_min', { tMin: value }).catch((cause) => {
                                    setTMin((current) => current === value ? previous : current);
                                    setPanelError(cause, 'Unable to change the hopping threshold.');
                                });
                            }}
                            disabled={isPanelBusy}
                        />
                    ) : (
                        <div role="status" className="text-xs text-[var(--cc-muted)]">Hopping threshold is unavailable because the loaded range cannot retain the current threshold.</div>
                    )}

                    <div className="max-h-32 space-y-2 overflow-y-auto p-1 custom-scrollbar" aria-busy={pendingControl === 'orbital'}>
                        <div className="border-b border-[var(--cc-border)] pb-1 text-[11px] font-medium text-[var(--cc-muted)]">Orbitals (m, n)</div>
                        <div className="flex flex-wrap gap-2">
                            {activeOrbitals.map((active, i) => (
                                <label key={`orb-${i}`} className="pointer-events-auto flex cursor-pointer items-center gap-1 text-[10px] text-[var(--cc-text)]">
                                    <input
                                        type="checkbox"
                                        checked={active}
                                        disabled={isPanelBusy}
                                        onChange={(event) => void handleSetOrbital(i, event.target.checked)}
                                        className="rounded-sm accent-[var(--cc-accent)]"
                                    />
                                    Orb {i+1}
                                </label>
                            ))}
                        </div>
                    </div>

                    <div className="max-h-32 space-y-2 overflow-y-auto p-1 custom-scrollbar" aria-busy={pendingControl === 'onsite' || pendingControl === 'shell'}>
                        <div className="border-b border-[var(--cc-border)] pb-1 text-[11px] font-medium text-[var(--cc-muted)]">Translation & On-site</div>
                        
                        <label className="pointer-events-auto mb-2 flex cursor-pointer items-center gap-1 text-[10px] text-[var(--cc-text)]">
                            <input
                                type="checkbox"
                                checked={showOnSite}
                                disabled={isPanelBusy}
                                onChange={(event) => void handleToggleOnSite(event.target.checked)}
                                className="rounded-sm accent-[var(--cc-accent)]"
                            />
                            Show On-site Energies (R=0)
                        </label>

                        <div className="flex flex-col gap-1">
                            {wannierInfo.r_shells.map(([rx, ry, rz], shellIdx) => {
                                if (rx === 0 && ry === 0 && rz === 0) return null;
                                return (
                                <label key={`R-${shellIdx}`} className="pointer-events-auto flex cursor-pointer items-center gap-1 text-[10px] text-[var(--cc-text)]">
                                    <input
                                        type="checkbox"
                                        checked={activeRShells[shellIdx]}
                                        disabled={isPanelBusy}
                                        onChange={(event) => void handleSetRShell(shellIdx, event.target.checked)}
                                        className="rounded-sm accent-[var(--cc-accent)]"
                                    />
                                    R = [{rx}, {ry}, {rz}]
                                </label>
                            )})}
                        </div>
                    </div>

                    <ActionButton
                        label="Clear Wannier Overlay"
                        busyLabel="Clearing Wannier overlay…"
                        onClick={handle_clear_wannier}
                        disabled={isPanelBusy}
                        busy={activeOperation === 'clear'}
                        tone="danger"
                    />
                    
                    <ActionButton
                        label={isWannierVisible ? 'Hide Hopping Network' : 'Show Hopping Network'}
                        busyLabel="Updating hopping network…"
                        onClick={handleToggleVisibility}
                        disabled={isPanelBusy}
                        busy={pendingControl === 'visibility'}
                        tone="secondary"
                    />
        </div>
    );
}
