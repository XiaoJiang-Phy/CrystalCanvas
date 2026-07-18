import React, { useState, useCallback } from 'react';
import ReactDOM from 'react-dom';
import { safeInvoke, safeDialogSave } from '../../utils/tauri-mock';
import { IpcException, type IpcError } from '../../ipc/contracts';
import { BzInfo } from '../../types/crystal';
import { PanelProps } from './index';
import { ActionButton, PanelError, SelectInput } from './shared';

export default function BrillouinZonePanel({}: PanelProps) {
    const [bzInfo, setBzInfo] = useState<BzInfo | null>(null);
    const [isBzVisible, setIsBzVisible] = useState(false);
    const [bzLabels, setBzLabels] = useState<{label: string, x: number, y: number}[]>([]);
    const [kPathFormat, setKPathFormat] = useState<'qe' | 'vasp'>('qe');
    const [activeOperation, setActiveOperation] = useState<'compute' | 'toggle' | 'save' | null>(null);
    const [error, setError] = useState<IpcError | null>(null);

    const setPanelError = (cause: unknown, fallback: string) => {
        if (cause instanceof IpcException) {
            setError({ code: cause.code, message: cause.message, recoverable: cause.recoverable });
            return;
        }
        setError({ code: 'internal_error', message: fallback, recoverable: false });
    };

    const fetch_bz_labels = useCallback(async () => {
        const w = window.innerWidth;
        const h = window.innerHeight;
        try {
            const labels = await safeInvoke('get_bz_label_positions', { width: w, height: h });
            if (labels) setBzLabels(labels);
        } catch (cause) {
            setBzLabels([]);
            setPanelError(cause, 'Unable to position Brillouin-zone labels.');
        }
    }, []);

    const handle_compute_bz = async () => {
        if (activeOperation) return;
        setError(null);
        setActiveOperation('compute');
        try {
            const res = await safeInvoke('compute_brillouin_zone');
            if (res) {
                setBzInfo(res);
                await safeInvoke('toggle_bz_display', { show: true });
                setIsBzVisible(true);
                setTimeout(fetch_bz_labels, 150);
            }
        } catch (cause) {
            setPanelError(cause, 'Unable to compute the Brillouin zone.');
        } finally {
            setActiveOperation(null);
        }
    };

    const handle_toggle_bz = async () => {
        if (activeOperation) return;
        const next = !isBzVisible;
        setError(null);
        setActiveOperation('toggle');
        try {
            await safeInvoke('toggle_bz_display', { show: next });
            setIsBzVisible(next);
            if (next) {
                setTimeout(fetch_bz_labels, 200);
            } else {
                setBzLabels([]);
            }
        } catch (cause) {
            setPanelError(cause, 'Unable to change Brillouin-zone visibility.');
        } finally {
            setActiveOperation(null);
        }
    };

    const isBusy = activeOperation !== null;

    return (
        <>
        <div className="space-y-3" aria-busy={isBusy}>
            <ActionButton label="Compute Brillouin Zone" onClick={handle_compute_bz} disabled={isBusy} busy={activeOperation === 'compute'} />

            {error && <PanelError error={error} message={error.message} />}
            {!bzInfo && !isBusy && !error && <div role="status" className="text-xs text-[var(--cc-muted)]">Brillouin-zone data is unavailable until it is computed.</div>}
            
            <ActionButton
                label={isBzVisible ? 'Back to Crystal View' : 'View Brillouin Zone'}
                onClick={handle_toggle_bz}
                disabled={!bzInfo || isBusy}
                busy={activeOperation === 'toggle'}
                tone="secondary"
            />

            {bzInfo && (
                <div className="space-y-1 rounded border border-[var(--cc-border)] bg-[var(--cc-panel)] p-2 text-[11px] text-[var(--cc-text)]">
                    <div className="flex justify-between">
                        <span className="text-[var(--cc-muted)]">Bravais Type:</span>
                        <span className="font-semibold">{bzInfo.bravais_type}</span>
                    </div>
                    <div className="flex justify-between">
                        <span className="text-[var(--cc-muted)]">Geometry:</span>
                        <span>
                            {bzInfo.is_2d ? `${bzInfo.edges_count} edges, ` : `${bzInfo.faces_count} faces, `} 
                            {bzInfo.vertices_count} vertices
                        </span>
                    </div>
                </div>
            )}

            {bzInfo && (
                <>
                <div className="my-2 border-t border-[var(--cc-border)]"></div>
                <div className="mb-1 text-[11px] font-medium text-[var(--cc-muted)]">Band Path Generator</div>
                <div className="flex items-center gap-2 mb-1">
                    <label className="whitespace-nowrap text-[11px] text-[var(--cc-muted)]">N<sub>k</sub></label>
                    <input
                        type="number" min={5} max={100} defaultValue={20}
                        id="kpath-npoints"
                        className="w-14 rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-2 py-1.5 text-xs text-[var(--cc-text)] outline-none focus-visible:border-[var(--cc-accent)] focus-visible:ring-1 focus-visible:ring-[var(--cc-accent)]"
                    />
                    <div className="flex-1">
                        <SelectInput
                            label="Format"
                            value={kPathFormat}
                            onChange={(value) => setKPathFormat(value === 'vasp' ? 'vasp' : 'qe')}
                            disabled={isBusy}
                        >
                            <option value="qe">QE (crystal)</option>
                            <option value="vasp">VASP (KPOINTS)</option>
                        </SelectInput>
                    </div>
                </div>
                <ActionButton
                    label="Generate & Save"
                    disabled={isBusy}
                    busy={activeOperation === 'save'}
                    onClick={async () => {
                        if (isBusy) return;
                        const nEl = document.getElementById('kpath-npoints') as HTMLInputElement;
                        const npoints = parseInt(nEl?.value) || 20;
                        const fmt = kPathFormat;
                        setError(null);
                        setActiveOperation('save');
                        try {
                            const res = await safeInvoke('generate_kpath_text', { npoints });
                            if (!res) return;
                            const text = fmt === 'qe' ? res.qe : res.vasp;
                            const preEl = document.getElementById('kpath-preview');
                            if (preEl) preEl.textContent = text;
                            const defaultName = fmt === 'qe' ? 'kpath_qe.txt' : 'KPOINTS';
                            const savePath = await safeDialogSave({
                                title: 'Save K-Path',
                                defaultPath: defaultName
                            });
                            if (savePath) {
                                await safeInvoke('write_text_file', { path: savePath, content: text });
                            }
                        } catch (cause) {
                            setPanelError(cause, 'Unable to generate or save the k-path.');
                        } finally {
                            setActiveOperation(null);
                        }
                    }}
                />
                <div className="bg-slate-900 text-emerald-400 p-2 rounded text-[10px] font-mono overflow-x-auto max-h-32 custom-scrollbar select-text pointer-events-auto">
                    <pre id="kpath-preview">Press generate to preview...</pre>
                </div>
                </>
            )}
        </div>
        {isBzVisible && bzLabels.length > 0 && ReactDOM.createPortal(
            <div className="fixed inset-0 pointer-events-none z-[60]" style={{fontFamily: "'Inter', 'SF Pro', system-ui, sans-serif"}}>
                {(() => {
                    const pad = 32;
                    const minDist = 22;
                    const positioned = bzLabels.map(l => ({ ...l, dx: 0, dy: -18 }));
                    for (let pass = 0; pass < 3; pass++) {
                        for (let i = 0; i < positioned.length; i++) {
                            for (let j = i + 1; j < positioned.length; j++) {
                                const ax = positioned[i].x + positioned[i].dx;
                                const ay = positioned[i].y + positioned[i].dy;
                                const bx = positioned[j].x + positioned[j].dx;
                                const by = positioned[j].y + positioned[j].dy;
                                const dist = Math.sqrt((ax - bx) ** 2 + (ay - by) ** 2);
                                if (dist < minDist) {
                                    const nudge = (minDist - dist) / 2 + 2;
                                    positioned[j].dy += nudge;
                                    positioned[i].dy -= nudge;
                                }
                            }
                        }
                    }
                    return positioned.map((lbl, i) => {
                        const cx = Math.max(pad, Math.min(lbl.x + lbl.dx, window.innerWidth - pad));
                        const cy = Math.max(pad, Math.min(lbl.y + lbl.dy, window.innerHeight - pad));
                        return (
                            <span
                                key={i}
                                className="absolute text-[12px] font-bold whitespace-nowrap"
                                style={{
                                    left: cx,
                                    top: cy,
                                    transform: 'translate(-50%, -50%)',
                                    color: '#f59e0b',
                                    textShadow: '0 0 4px rgba(0,0,0,0.8), 0 0 2px rgba(0,0,0,0.6)',
                                    letterSpacing: '0.02em',
                                }}
                            >
                                {lbl.label}
                            </span>
                        );
                    });
                })()}
            </div>,
            document.body
        )}
        </>
    );
}
