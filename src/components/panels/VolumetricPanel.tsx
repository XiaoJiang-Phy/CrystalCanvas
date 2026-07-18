import React, { useState, useEffect } from 'react';
import { safeInvoke, safeListen, safeDialogOpen } from '../../utils/tauri-mock';
import {
    IpcException,
    is_isosurface_sign_mode,
    is_volume_colormap,
    is_volume_render_mode,
    type IpcError,
    type IsosurfaceSignMode,
    type VolumeColormap,
    type VolumetricInfo,
    type VolumeRenderMode,
} from '../../ipc/contracts';
import { PanelProps } from './index';
import { ActionButton, PanelError, RangeInput, SelectInput } from './shared';

const initial_isovalue = (info: VolumetricInfo): number | null => {
    const bound = Math.max(Math.abs(info.data_min), Math.abs(info.data_max));
    if (!Number.isFinite(info.data_min) || !Number.isFinite(info.data_max) || bound <= 0) return null;
    if (info.data_min < 0) return bound * 0.1;
    const value = info.data_max * 0.1;
    return value < info.data_min ? info.data_min + (info.data_max - info.data_min) * 0.1 : value;
};

export default function VolumetricPanel({ setOpenAccordion }: PanelProps) {
    const [volumetricInfo, setVolumetricInfo] = useState<VolumetricInfo | null>(null);
    const [isovalue, setIsovalue] = useState(0);
    const [surfaceOpacity, setSurfaceOpacity] = useState(0.5);
    const [densityCutoff, setDensityCutoff] = useState(0);
    const [opacityScale, setOpacityScale] = useState(1);
    const [volumeRenderMode, setVolumeRenderMode] = useState<VolumeRenderMode>('both');
    const [signMode, setSignMode] = useState<IsosurfaceSignMode>('both');
    const [volumeColormap, setVolumeColormap] = useState<VolumeColormap>('viridis');
    const [isLoading, setIsLoading] = useState(false);
    const [pendingControl, setPendingControl] = useState<string | null>(null);
    const [error, setError] = useState<IpcError | null>(null);

    const setPanelError = (cause: unknown, fallback: string) => {
        if (cause instanceof IpcException) {
            setError({ code: cause.code, message: cause.message, recoverable: cause.recoverable });
            return;
        }
        setError({ code: 'internal_error', message: fallback, recoverable: false });
    };

    const applyVolumetricInfo = (info: VolumetricInfo) => {
        setVolumetricInfo(info);
        const value = initial_isovalue(info);
        setIsovalue(value ?? 0);
        setSurfaceOpacity(0.5);
        setDensityCutoff(0);
        setOpacityScale(1);
        setVolumeRenderMode('both');
        setSignMode('positive');
        setVolumeColormap(info.data_min < -0.01 * Math.abs(info.data_max) ? 'coolwarm' : 'viridis');
        return value;
    };

    const hasLoadedVolumetricData = volumetricInfo !== null;
    const volumetricBound = hasLoadedVolumetricData ? Math.max(Math.abs(volumetricInfo.data_min), Math.abs(volumetricInfo.data_max)) : 0;
    const isovalueStep = volumetricBound / 1000;
    const densityCutoffStep = volumetricBound / 500;
    const hasUsableVolumetricRange = Number.isFinite(volumetricInfo?.data_min ?? Number.NaN)
        && Number.isFinite(volumetricInfo?.data_max ?? Number.NaN)
        && volumetricBound > 0
        && Number.isFinite(isovalueStep)
        && isovalueStep > 0
        && Number.isFinite(densityCutoffStep)
        && densityCutoffStep > 0;

    useEffect(() => {
        let unlisten = () => {};
        safeListen('volumetric_loaded', (event) => {
            const info = event.payload;
            if (info) {
                const defaultIsovalue = applyVolumetricInfo(info);
                
                if (setOpenAccordion) {
                    setOpenAccordion('Volumetric');
                }
                
                if (defaultIsovalue !== null) {
                    safeInvoke('set_isovalue', { value: defaultIsovalue })
                        .then(() => setDensityCutoff(defaultIsovalue))
                        .catch((cause) => setPanelError(cause, 'Unable to initialize the isovalue.'));
                    safeInvoke('set_volume_render_mode', { mode: 'both' })
                        .then(() => setVolumeRenderMode('both'))
                        .catch((cause) => setPanelError(cause, 'Unable to initialize the volume renderer.'));
                    safeInvoke('set_isosurface_sign_mode', { mode: 'both' })
                        .then(() => setSignMode('both'))
                        .catch((cause) => setPanelError(cause, 'Unable to initialize the isosurface sign mode.'));
                }
            }
        }).then((f) => unlisten = f).catch(console.warn);
        
        return () => {
            unlisten();
        };
    }, [setOpenAccordion]);

    const handleRenderMode = async (value: string) => {
        if (isLoading || pendingControl || !is_volume_render_mode(value)) return;
        const mode = value;
        setError(null);
        setPendingControl('render-mode');
        try {
            await safeInvoke('set_volume_render_mode', { mode });
            setVolumeRenderMode(mode);
            setDensityCutoff(mode === 'both' ? isovalue : 0);
        } catch (cause) {
            setPanelError(cause, 'Unable to change the volume render mode.');
        } finally {
            setPendingControl(null);
        }
    };

    const handleSignMode = async (value: string) => {
        if (isLoading || pendingControl || !is_isosurface_sign_mode(value)) return;
        const mode = value;
        setError(null);
        setPendingControl('sign-mode');
        try {
            await safeInvoke('set_isosurface_sign_mode', { mode });
            setSignMode(mode);
        } catch (cause) {
            setPanelError(cause, 'Unable to change the isosurface sign mode.');
        } finally {
            setPendingControl(null);
        }
    };

    const handleVolumeColormap = async (value: string) => {
        if (isLoading || pendingControl || !is_volume_colormap(value)) return;
        const mode = value;
        setError(null);
        setPendingControl('colormap');
        try {
            await safeInvoke('set_volume_colormap', { mode });
            setVolumeColormap(mode);
        } catch (cause) {
            setPanelError(cause, 'Unable to change the volume colormap.');
        } finally {
            setPendingControl(null);
        }
    };

    const isPanelBusy = isLoading || pendingControl !== null;

    return (
        <div className="space-y-3" aria-busy={isPanelBusy}>
            <ActionButton label="Load Volumetric Data..." busyLabel="Loading volumetric data…" onClick={async () => {
                if (isPanelBusy) return;
                setError(null);
                setIsLoading(true);
                try {
                    const file = await safeDialogOpen({ title: 'Open Volumetric File' });
                    if (file && typeof file === 'string') {
                        const info = await safeInvoke('load_volumetric_file', { path: file });
                        if (info) {
                            applyVolumetricInfo(info);
                        }
                    }
                } catch (cause) {
                    setPanelError(cause, 'Unable to load volumetric data.');
                } finally {
                    setIsLoading(false);
                }
            }} disabled={isPanelBusy} busy={isLoading} />

            {error && <PanelError error={error} message={error.message} />}
            {!hasLoadedVolumetricData && !isLoading && !error && <div role="status" className="text-xs text-[var(--cc-muted)]">No volumetric data is loaded.</div>}

            {hasLoadedVolumetricData && (
                <>
                <div className="rounded border border-[var(--cc-border)] bg-[var(--cc-panel)] p-2 text-[10px] text-[var(--cc-muted)] font-mono space-y-1">
                    <div className="flex justify-between items-center text-xs">
                        <span className="font-semibold text-[var(--cc-text)]">Data Info</span>
                        <span className="rounded border border-[var(--cc-border)] px-1.5 py-0.5 uppercase text-[var(--cc-text)]">{volumetricInfo.format}</span>
                    </div>
                    <div className="flex justify-between">
                        <span>Grid Size:</span>
                        <span>{volumetricInfo.grid_dims[0]}×{volumetricInfo.grid_dims[1]}×{volumetricInfo.grid_dims[2]}</span>
                    </div>
                    <div className="flex justify-between">
                        <span>Min Den:</span>
                        <span>{Number.isFinite(volumetricInfo.data_min) ? volumetricInfo.data_min.toExponential(2) : 'Unavailable'}</span>
                    </div>
                    <div className="flex justify-between">
                        <span>Max Den:</span>
                        <span>{Number.isFinite(volumetricInfo.data_max) ? volumetricInfo.data_max.toExponential(2) : 'Unavailable'}</span>
                    </div>
                </div>
                {hasUsableVolumetricRange ? (
                <>
            <SelectInput
                label="Render Mode"
                value={volumeRenderMode}
                onChange={(value) => void handleRenderMode(value)}
                disabled={isPanelBusy}
                busy={pendingControl === 'render-mode'}
            >
                    <option value="both">Both (Isosurface + Volume)</option>
                    <option value="isosurface">Isosurface Only</option>
                    <option value="volume">Volume Only</option>
            </SelectInput>

            <RangeInput label="Isovalue" value={isovalue} displayValue={isovalue.toExponential(2)} min={0} max={volumetricBound} step={isovalueStep} disabled={isPanelBusy} onChange={(value) => {
                const previous = isovalue;
                setError(null);
                setIsovalue(value);
                safeInvoke('set_isovalue', { value })
                    .then(() => {
                        if (volumeRenderMode === 'both') {
                            setDensityCutoff(value);
                        } else {
                            setDensityCutoff(0);
                        }
                    })
                    .catch((cause) => {
                        setIsovalue((current) => current === value ? previous : current);
                        setPanelError(cause, 'Unable to change the isovalue.');
                    });
            }} />

            <RangeInput label="Surface Opacity" value={surfaceOpacity} displayValue={surfaceOpacity.toFixed(2)} min={0} max={1} step={0.05} disabled={isPanelBusy} onChange={(value) => {
                const previous = surfaceOpacity;
                setError(null);
                setSurfaceOpacity(value);
                safeInvoke('set_isosurface_opacity', { opacity: value }).catch((cause) => {
                    setSurfaceOpacity((current) => current === value ? previous : current);
                    setPanelError(cause, 'Unable to change the surface opacity.');
                });
            }} />

            <SelectInput
                label="Sign Mode (Charge Diff)"
                value={signMode}
                onChange={(value) => void handleSignMode(value)}
                disabled={isPanelBusy}
                busy={pendingControl === 'sign-mode'}
            >
                    <option value="both">Both (±)</option>
                    <option value="positive">Positive Only</option>
                    <option value="negative">Negative Only</option>
            </SelectInput>

            <SelectInput
                label="Volume Colormap"
                value={volumeColormap}
                onChange={(value) => void handleVolumeColormap(value)}
                disabled={isPanelBusy}
                busy={pendingControl === 'colormap'}
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
            </SelectInput>

            <RangeInput label="Volume Density Cutoff" value={densityCutoff} displayValue={densityCutoff.toExponential(2)} min={0} max={volumetricBound} step={densityCutoffStep} disabled={isPanelBusy} onChange={(value) => {
                const previous = densityCutoff;
                setError(null);
                setDensityCutoff(value);
                safeInvoke('set_volume_density_cutoff', { cutoff: value }).catch((cause) => {
                    setDensityCutoff((current) => current === value ? previous : current);
                    setPanelError(cause, 'Unable to change the volume density cutoff.');
                });
            }} />

            <RangeInput label="Volume Opacity Scale" value={opacityScale} displayValue={opacityScale.toFixed(1)} min={0.1} max={5} step={0.1} disabled={isPanelBusy} onChange={(value) => {
                const previous = opacityScale;
                setError(null);
                setOpacityScale(value);
                safeInvoke('set_volume_opacity_range', { min: volumetricInfo.data_min, max: volumetricInfo.data_max, opacityScale: value }).catch((cause) => {
                    setOpacityScale((current) => current === value ? previous : current);
                    setPanelError(cause, 'Unable to change the volume opacity scale.');
                });
            }} />
                </>
                ) : (
                    <div role="status" className="text-xs text-[var(--cc-muted)]">Volumetric controls are unavailable because the data range is not finite and positive.</div>
                )}
                </>
            )}
        </div>
    );
}
