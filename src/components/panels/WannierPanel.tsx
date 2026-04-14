import React, { useState } from 'react';
import { safeInvoke, safeDialogOpen } from '../../utils/tauri-mock';
import { WannierInfo } from '../../types/crystal';
import { PanelProps } from './index';

export default function WannierPanel({}: PanelProps) {
    const [wannierInfo, setWannierInfo] = useState<WannierInfo | null>(null);
    const [tMin, setTMin] = useState<number>(0.01);
    const [activeRShells, setActiveRShells] = useState<boolean[]>([]);
    const [activeOrbitals, setActiveOrbitals] = useState<boolean[]>([]);
    const [showOnSite, setShowOnSite] = useState<boolean>(false);
    const [isWannierVisible, setIsWannierVisible] = useState<boolean>(false);

    const handle_load_wannier = async () => {
        try {
            const file = await safeDialogOpen({
                title: 'Open wannier90_hr.dat',
                filters: [{ name: 'Wannier Hopping', extensions: ['dat'] }]
            });
            if (file && typeof file === 'string') {
                const info = await safeInvoke<WannierInfo>('load_wannier_hr', { path: file });
                if (info) {
                    setWannierInfo(info);
                    setActiveRShells(new Array(info.r_shells.length).fill(true));
                    setActiveOrbitals(new Array(info.num_wann).fill(true));
                    setIsWannierVisible(true);
                }
            }
        } catch (e: any) {
            alert(String(e));
        }
    };

    const handle_clear_wannier = () => {
        safeInvoke('clear_wannier').then(() => {
            setWannierInfo(null);
            setIsWannierVisible(false);
        }).catch(console.error);
    };

    return (
        <div className="space-y-3">
            <button onClick={handle_load_wannier} className="flex-1 w-full py-1.5 bg-emerald-50 dark:bg-emerald-500/10 hover:bg-emerald-100 dark:hover:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 rounded-md text-xs font-medium transition-colors border border-emerald-200/50 dark:border-emerald-800/50 active:scale-[0.98] pointer-events-auto">
                Load wannier90_hr.dat...
            </button>
            
            {wannierInfo && (
                <>
                    <div className="text-[11px] text-slate-600 dark:text-slate-300 bg-slate-50 dark:bg-slate-800/40 p-2 rounded border border-slate-100 dark:border-slate-700 space-y-1">
                        <div className="flex justify-between">
                            <span className="text-slate-500">Orbitals:</span>
                            <span className="font-semibold">{wannierInfo.num_wann}</span>
                        </div>
                        <div className="flex justify-between">
                            <span className="text-slate-500">R-Shells:</span>
                            <span className="font-semibold">{wannierInfo.r_shells.length}</span>
                        </div>
                        <div className="flex justify-between">
                            <span className="text-slate-500">Max |t|:</span>
                            <span className="font-semibold">{wannierInfo.t_max.toFixed(4)} eV</span>
                        </div>
                    </div>
                    
                    <div className="space-y-1">
                        <div className="flex justify-between items-center text-[11px] text-slate-500 dark:text-slate-400">
                            <span>|t| Threshold:</span>
                            <span>{tMin.toFixed(3)} eV</span>
                        </div>
                        <input
                            type="range" min={0.0} max={wannierInfo.t_max} step={wannierInfo.t_max / 100}
                            value={tMin}
                            onChange={(e) => {
                                const v = parseFloat(e.target.value);
                                setTMin(v);
                                safeInvoke('set_wannier_t_min', { tMin: v }).catch(console.error);
                            }}
                            className="w-full h-1 accent-emerald-500 cursor-pointer pointer-events-auto"
                        />
                    </div>

                    <div className="space-y-2 max-h-32 overflow-y-auto custom-scrollbar p-1">
                        <div className="text-[11px] font-medium text-slate-500 dark:text-slate-400 border-b border-slate-200 dark:border-slate-700 pb-1">Orbitals (m, n)</div>
                        <div className="flex flex-wrap gap-2">
                            {activeOrbitals.map((active, i) => (
                                <label key={`orb-${i}`} className="flex items-center gap-1 text-[10px] text-slate-600 dark:text-slate-300 cursor-pointer pointer-events-auto">
                                    <input type="checkbox" checked={active} onChange={(e) => {
                                        const checked = e.target.checked;
                                        const next = [...activeOrbitals];
                                        next[i] = checked;
                                        setActiveOrbitals(next);
                                        safeInvoke('set_wannier_orbital', { orbIdx: i, active: checked }).catch(console.error);
                                    }} className="accent-emerald-500 rounded-sm" />
                                    Orb {i+1}
                                </label>
                            ))}
                        </div>
                    </div>

                    <div className="space-y-2 max-h-32 overflow-y-auto custom-scrollbar p-1">
                        <div className="text-[11px] font-medium text-slate-500 dark:text-slate-400 border-b border-slate-200 dark:border-slate-700 pb-1">Translation & On-site</div>
                        
                        <label className="flex items-center gap-1 text-[10px] text-slate-600 dark:text-slate-300 cursor-pointer pointer-events-auto mb-2">
                            <input type="checkbox" checked={showOnSite} onChange={(e) => {
                                const checked = e.target.checked;
                                setShowOnSite(checked);
                                safeInvoke('toggle_wannier_onsite', { show: checked }).catch(console.error);
                            }} className="accent-emerald-500 rounded-sm" />
                            Show On-site Energies (R=0)
                        </label>

                        <div className="flex flex-col gap-1">
                            {wannierInfo.r_shells.filter((r: any) => r.rx !== 0 || r.ry !== 0 || r.rz !== 0).map((r: any, i: number) => {
                                const offset = wannierInfo.r_shells.findIndex((x: any) => x.rx===0 && x.ry===0 && x.rz===0) > -1 ? 1 : 0;
                                const originalIdx = wannierInfo.r_shells.findIndex((x: any) => x.rx === r.rx && x.ry === r.ry && x.rz === r.rz);
                                return (
                                <label key={`R-${i}`} className="flex items-center gap-1 text-[10px] text-slate-600 dark:text-slate-300 cursor-pointer pointer-events-auto">
                                    <input type="checkbox" checked={activeRShells[originalIdx]} onChange={(e) => {
                                        const checked = e.target.checked;
                                        const next = [...activeRShells];
                                        next[originalIdx] = checked;
                                        setActiveRShells(next);
                                        safeInvoke('set_wannier_r_shell', { shellIdx: originalIdx, active: checked }).catch(console.error);
                                    }} className="accent-emerald-500 rounded-sm" />
                                    R = [{r.rx}, {r.ry}, {r.rz}] 
                                    <span className="text-slate-400 text-[9px] ml-auto">({r.hopping_count} hops)</span>
                                </label>
                            )})}
                        </div>
                    </div>

                    <button
                        onClick={handle_clear_wannier}
                        className="w-full py-1.5 text-rose-600 bg-rose-50 hover:bg-rose-100 dark:text-rose-400 dark:bg-rose-900/20 dark:hover:bg-rose-900/40 rounded-md text-xs font-medium transition-colors border border-rose-200 dark:border-rose-800/50 active:scale-[0.98] pointer-events-auto mt-2"
                    >
                        Clear Wannier Overlay
                    </button>
                    
                    <button
                        onClick={() => {
                            const next = !isWannierVisible;
                            setIsWannierVisible(next);
                            safeInvoke('toggle_hopping_display', { show: next }).catch(console.error);
                        }}
                        className={`w-full py-1.5 rounded-md text-xs font-medium transition-colors border shadow-sm pointer-events-auto mt-1 ${
                            isWannierVisible ? 'bg-amber-500 hover:bg-amber-600 text-white border-amber-600' : 'bg-emerald-500 hover:bg-emerald-600 text-white border-emerald-600'
                        }`}
                    >
                        {isWannierVisible ? "Hide Hopping Network" : "Show Hopping Network"}
                    </button>
                </>
            )}
        </div>
    );
}
