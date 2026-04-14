import React, { useState, useEffect } from 'react';
import { safeInvoke, safeListen, safeDialogOpen } from '../../utils/tauri-mock';
import { PanelProps } from './index';

export default function VolumetricPanel({ onStructureUpdate, setOpenAccordion }: PanelProps) {
    const [volumetricInfo, setVolumetricInfo] = useState<any | null>(null);
    const [volumetricRange, setVolumetricRange] = useState<{min: number, max: number}>({min: -1.0, max: 1.0});
    const [isovalue, setIsovalue] = useState(0.05);

    useEffect(() => {
        let unlisten = () => {};
        safeListen<any>('volumetric_loaded', (event: any) => {
            const info = event.payload;
            if (info) {
                setVolumetricInfo(info);
                let dMin = info.data_min;
                let dMax = info.data_max;
                setVolumetricRange({ min: dMin, max: dMax });
                
                let defaultIsovalue = 0.05;
                if (dMin < 0.0) {
                    defaultIsovalue = Math.max(Math.abs(dMax), Math.abs(dMin)) * 0.1;
                } else {
                    defaultIsovalue = dMax * 0.1;
                    if (defaultIsovalue < dMin) defaultIsovalue = dMin + (dMax - dMin) * 0.1;
                }
                setIsovalue(defaultIsovalue);
                
                if (setOpenAccordion) {
                    setOpenAccordion('Volumetric');
                }
                
                safeInvoke('set_isovalue', { value: defaultIsovalue }).catch(console.warn);
                safeInvoke('set_volume_render_mode', { mode: 'both' }).catch(console.warn);
                safeInvoke('set_isosurface_sign_mode', { mode: 'both' }).catch(console.warn);
            }
        }).then((f: any) => unlisten = f).catch(console.warn);
        
        return () => {
            unlisten();
        };
    }, [setOpenAccordion]);

    return (
        <div className="space-y-3">
            <button onClick={async () => {
                try {
                    const file = await safeDialogOpen({ title: 'Open Volumetric File' });
                    if (file && typeof file === 'string') {
                        const info = await safeInvoke<any>('load_volumetric_file', { path: file });
                        if (info) {
                            setVolumetricInfo(info);
                            let dMin = info.data_min;
                            let dMax = info.data_max;
                            setVolumetricRange({ min: dMin, max: dMax });
                            setIsovalue((dMax - dMin) * 0.1 + dMin);
                        }
                        if (onStructureUpdate) onStructureUpdate();
                    }
                } catch (e: any) {
                    alert(String(e));
                }
            }} className="flex-1 w-full py-1.5 bg-emerald-50 dark:bg-emerald-500/10 hover:bg-emerald-100 dark:hover:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 rounded-md text-xs font-medium transition-colors border border-emerald-200/50 dark:border-emerald-800/50 active:scale-[0.98] pointer-events-auto">
                Load Volumetric Data...
            </button>

            {volumetricInfo && (
                <div className="bg-slate-100 dark:bg-slate-800/50 rounded flex flex-col p-2 space-y-1 text-[10px] text-slate-500 dark:text-slate-400 font-mono">
                    <div className="flex justify-between items-center text-xs">
                        <span className="font-semibold text-slate-700 dark:text-slate-300">Data Info</span>
                        <span className="bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 px-1.5 py-0.5 rounded uppercase">{volumetricInfo.format}</span>
                    </div>
                    <div className="flex justify-between">
                        <span>Grid Size:</span>
                        <span>{volumetricInfo.grid_dims[0]}×{volumetricInfo.grid_dims[1]}×{volumetricInfo.grid_dims[2]}</span>
                    </div>
                    <div className="flex justify-between">
                        <span>Min Den:</span>
                        <span>{volumetricInfo.data_min.toExponential(2)}</span>
                    </div>
                    <div className="flex justify-between">
                        <span>Max Den:</span>
                        <span>{volumetricInfo.data_max.toExponential(2)}</span>
                    </div>
                </div>
            )}
            
            <div className="space-y-1">
                <label className="text-[11px] text-slate-500 dark:text-slate-400">Render Mode</label>
                <select
                    className="w-full bg-slate-100 dark:bg-slate-800/60 rounded px-2 py-1.5 outline-none border border-slate-200 dark:border-slate-700 text-xs text-slate-700 dark:text-slate-300 pointer-events-auto"
                    onChange={(e) => safeInvoke('set_volume_render_mode', { mode: e.target.value }).catch((e: any) => alert(String(e)))}
                    defaultValue="both"
                >
                    <option value="both">Both (Isosurface + Volume)</option>
                    <option value="isosurface">Isosurface Only</option>
                    <option value="volume">Volume Only</option>
                </select>
            </div>

            <div className="space-y-1">
                <div className="flex justify-between items-center text-[11px] text-slate-500 dark:text-slate-400">
                    <span>Isovalue</span>
                    <span>{isovalue.toExponential(2)}</span>
                </div>
                <input
                    type="range" min={0} max={Math.max(Math.abs(volumetricRange.min), Math.abs(volumetricRange.max))} step={Math.max(Math.abs(volumetricRange.min), Math.abs(volumetricRange.max)) / 1000.0}
                    value={isovalue}
                    onChange={(e) => {
                        let v = parseFloat(e.target.value);
                        setIsovalue(v);
                        safeInvoke('set_isovalue', { value: v }).catch((e: any) => alert(String(e)));
                    }}
                    className="w-full h-1 accent-emerald-500 cursor-pointer pointer-events-auto"
                />
            </div>

            <div className="space-y-1">
                <div className="flex justify-between items-center text-[11px] text-slate-500 dark:text-slate-400">
                    <span>Surface Opacity</span>
                </div>
                <input
                    type="range" min={0.0} max={1.0} step={0.05}
                    defaultValue={0.5}
                    onChange={(e) => safeInvoke('set_isosurface_opacity', { opacity: parseFloat(e.target.value) }).catch((e: any) => alert(String(e)))}
                    className="w-full h-1 accent-emerald-500 cursor-pointer pointer-events-auto"
                />
            </div>

            <div className="space-y-1">
                <label className="text-[11px] text-slate-500 dark:text-slate-400">Sign Mode (Charge Diff)</label>
                <select
                    className="w-full bg-slate-100 dark:bg-slate-800/60 rounded px-2 py-1.5 outline-none border border-slate-200 dark:border-slate-700 text-xs text-slate-700 dark:text-slate-300 pointer-events-auto"
                    onChange={(e) => {
                        safeInvoke('set_isosurface_sign_mode', { mode: e.target.value }).catch((err: any) => alert(String(err)));
                    }}
                    defaultValue="both"
                >
                    <option value="both">Both (±)</option>
                    <option value="positive">Positive Only</option>
                    <option value="negative">Negative Only</option>
                </select>
            </div>

            <div className="space-y-1">
                <label className="text-[11px] text-slate-500 dark:text-slate-400">Volume Colormap</label>
                <select
                    className="w-full bg-slate-100 dark:bg-slate-800/60 rounded px-2 py-1.5 outline-none border border-slate-200 dark:border-slate-700 text-xs text-slate-700 dark:text-slate-300 pointer-events-auto"
                    onChange={(e) => safeInvoke('set_volume_colormap', { mode: e.target.value }).catch((err: any) => alert(String(err)))}
                    defaultValue="viridis"
                >
                    <option value="viridis">Viridis</option>
                    <option value="inferno">Inferno</option>
                    <option value="plasma">Plasma</option>
                    <option value="magma">Magma</option>
                    <option value="cividis">Cividis</option>
                    <option value="turbo">Turbo (Rainbow)</option>
                    <option value="hot">Hot</option>
                    <option value="coolwarm">Coolwarm (± diverging)</option>
                    <option value="rdylbu">RdYlBu (± diverging)</option>
                    <option value="grayscale">Grayscale</option>
                </select>
            </div>

            <div className="space-y-1">
                <div className="flex justify-between items-center text-[11px] text-slate-500 dark:text-slate-400">
                    <span>Volume Density Cutoff</span>
                </div>
                <input
                    type="range" min={0} max={Math.max(Math.abs(volumetricRange.min), Math.abs(volumetricRange.max))} step={Math.max(Math.abs(volumetricRange.min), Math.abs(volumetricRange.max)) / 500.0}
                    defaultValue={0}
                    onChange={(e) => safeInvoke('set_volume_density_cutoff', { cutoff: parseFloat(e.target.value) }).catch((err: any) => alert(String(err)))}
                    className="w-full h-1 accent-emerald-500 cursor-pointer pointer-events-auto"
                />
            </div>

            <div className="space-y-1">
                <div className="flex justify-between items-center text-[11px] text-slate-500 dark:text-slate-400">
                    <span>Volume Opacity Scale</span>
                </div>
                <input
                    type="range" min={0.1} max={5.0} step={0.1}
                    defaultValue={1.0}
                    onChange={(e) => safeInvoke('set_volume_opacity_range', { min: volumetricRange.min, max: volumetricRange.max, opacityScale: parseFloat(e.target.value) }).catch((err: any) => alert(String(err)))}
                    className="w-full h-1 accent-emerald-500 cursor-pointer pointer-events-auto"
                />
            </div>
        </div>
    );
}
