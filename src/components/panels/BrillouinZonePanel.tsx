import React, { useState, useCallback } from 'react';
import ReactDOM from 'react-dom';
import { safeInvoke, safeDialogSave } from '../../utils/tauri-mock';
import { BzInfo } from '../../types/crystal';
import { PanelProps } from './index';

export default function BrillouinZonePanel({}: PanelProps) {
    const [bzInfo, setBzInfo] = useState<BzInfo | null>(null);
    const [isBzVisible, setIsBzVisible] = useState(false);
    const [bzLabels, setBzLabels] = useState<{label: string, x: number, y: number}[]>([]);

    const fetch_bz_labels = useCallback(async () => {
        const w = window.innerWidth;
        const h = window.innerHeight;
        try {
            const labels = await safeInvoke<{label: string, x: number, y: number}[]>('get_bz_label_positions', { width: w, height: h });
            if (labels) setBzLabels(labels);
        } catch (_e: any) {
            setBzLabels([]);
        }
    }, []);

    const handle_compute_bz = () => {
        safeInvoke<BzInfo>('compute_brillouin_zone')
            .then(res => {
                if (res) {
                    setBzInfo(res);
                    return safeInvoke('toggle_bz_display', { show: true }).then(() => {
                        setIsBzVisible(true);
                        setTimeout(fetch_bz_labels, 150);
                    });
                }
            })
            .catch(console.error);
    };

    const handle_toggle_bz = () => {
        const next = !isBzVisible;
        safeInvoke('toggle_bz_display', { show: next })
            .then(() => {
                setIsBzVisible(next);
                if (next) {
                    setTimeout(fetch_bz_labels, 200);
                } else {
                    setBzLabels([]);
                }
            })
            .catch(console.error);
    };

    return (
        <>
        <div className="space-y-3">
            <button onClick={handle_compute_bz} className="flex-1 w-full py-1.5 bg-emerald-50 dark:bg-emerald-500/10 hover:bg-emerald-100 dark:hover:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 rounded-md text-xs font-medium transition-colors border border-emerald-200/50 dark:border-emerald-800/50 active:scale-[0.98] pointer-events-auto">
                Compute Brillouin Zone
            </button>
            
            <button 
                onClick={handle_toggle_bz} 
                disabled={!bzInfo}
                className={`w-full py-1.5 rounded-md text-xs font-medium transition-colors shadow-sm pointer-events-auto ${
                    !bzInfo ? "bg-slate-300 dark:bg-slate-700 text-slate-500 cursor-not-allowed" :
                    isBzVisible 
                        ? "bg-amber-500 hover:bg-amber-600 text-white" 
                        : "bg-slate-500 hover:bg-slate-600 text-white"
                }`}
            >
                {isBzVisible ? "◀ Back to Crystal View" : "View Brillouin Zone"}
            </button>

            {bzInfo && (
                <div className="text-[11px] text-slate-600 dark:text-slate-300 bg-slate-50 dark:bg-slate-800/40 p-2 rounded border border-slate-100 dark:border-slate-700 space-y-1">
                    <div className="flex justify-between">
                        <span className="text-slate-500">Bravais Type:</span>
                        <span className="font-semibold">{bzInfo.bravais_type}</span>
                    </div>
                    <div className="flex justify-between">
                        <span className="text-slate-500">Geometry:</span>
                        <span>
                            {bzInfo.is_2d ? `${bzInfo.edges_count} edges, ` : `${bzInfo.faces_count} faces, `} 
                            {bzInfo.vertices_count} vertices
                        </span>
                    </div>
                </div>
            )}

            {bzInfo && (
                <>
                <div className="border-t border-slate-200 dark:border-slate-700 my-2"></div>
                <div className="text-[11px] font-medium text-slate-500 dark:text-slate-400 mb-1">Band Path Generator</div>
                <div className="flex items-center gap-2 mb-1">
                    <label className="text-[11px] text-slate-500 dark:text-slate-400 whitespace-nowrap">N<sub>k</sub></label>
                    <input 
                        type="number" min={5} max={100} defaultValue={20}
                        id="kpath-npoints"
                        className="w-14 bg-slate-100 dark:bg-slate-800/60 rounded px-2 py-0.5 text-xs border border-slate-200 dark:border-slate-700 focus:border-emerald-500 outline-none text-slate-700 dark:text-slate-300 pointer-events-auto"
                    />
                    <select
                        id="kpath-format"
                        defaultValue="qe"
                        className="flex-1 bg-slate-100 dark:bg-slate-800/60 rounded px-2 py-0.5 text-xs border border-slate-200 dark:border-slate-700 focus:border-emerald-500 outline-none text-slate-700 dark:text-slate-300 pointer-events-auto"
                    >
                        <option value="qe">QE (crystal)</option>
                        <option value="vasp">VASP (KPOINTS)</option>
                    </select>
                </div>
                <button
                    onClick={async () => {
                        const nEl = document.getElementById('kpath-npoints') as HTMLInputElement;
                        const fmtEl = document.getElementById('kpath-format') as HTMLSelectElement;
                        const npoints = parseInt(nEl?.value) || 20;
                        const fmt = fmtEl?.value || 'qe';
                        try {
                            const res = await safeInvoke<{qe: string, vasp: string}>('generate_kpath_text', { npoints });
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
                                await safeInvoke('save_text_file', { path: savePath, content: text });
                            }
                        } catch (e: any) {
                            alert(String(e));
                        }
                    }}
                    className="w-full py-1 text-emerald-600 bg-emerald-50 hover:bg-emerald-100 dark:text-emerald-400 dark:bg-emerald-900/20 dark:hover:bg-emerald-900/40 border border-emerald-200 dark:border-emerald-800/50 rounded text-[11px] mb-2 pointer-events-auto"
                >
                    Generate & Save
                </button>
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
