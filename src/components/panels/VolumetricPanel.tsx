import React, { useState, useEffect, useRef } from 'react';
import { safeInvoke, safeListen, safeDialogOpen } from '../../utils/tauri-mock';
import {
    IpcException,
    is_isosurface_sign_mode,
    is_volume_colormap,
    is_volume_render_mode,
    type IpcError,
    type FieldSceneInfo,
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

const colormap_from_mode = (mode: number): VolumeColormap => [
    'viridis', 'grayscale', 'inferno', 'plasma', 'coolwarm',
    'hot', 'magma', 'cividis', 'turbo', 'rdylbu',
][mode] as VolumeColormap ?? 'viridis';

export default function VolumetricPanel({ setOpenAccordion }: PanelProps) {
    const [fieldScene, setFieldScene] = useState<FieldSceneInfo>({ revision: 0, active_layer_id: null, layers: [] });
    const [volumetricInfo, setVolumetricInfo] = useState<VolumetricInfo | null>(null);
    const [isovalue, setIsovalue] = useState(0);
    const [surfaceOpacity, setSurfaceOpacity] = useState(0.5);
    const [densityCutoff, setDensityCutoff] = useState(0);
    const [opacityScale, setOpacityScale] = useState(1);
    const [volumeRenderMode, setVolumeRenderMode] = useState<VolumeRenderMode>('both');
    const [signMode, setSignMode] = useState<IsosurfaceSignMode>('both');
    const [volumeColormap, setVolumeColormap] = useState<VolumeColormap>('viridis');
    const [combinationLayerIds, setCombinationLayerIds] = useState<[number | null, number | null]>([null, null]);
    const [combinationCoefficients, setCombinationCoefficients] = useState<[number, number]>([1, -1]);
    const [combinationLabel, setCombinationLabel] = useState('linear-combination');
    const [renameLabel, setRenameLabel] = useState('');
    const [fieldImportUnit, setFieldImportUnit] = useState<'electron_per_cubic_angstrom' | 'electron_per_bohr_cubed' | ''>('');
    const [isLoading, setIsLoading] = useState(false);
    const [pendingControl, setPendingControl] = useState<string | null>(null);
    const [error, setError] = useState<IpcError | null>(null);
    const fieldRefreshSequence = useRef(0);
    const committedIsovalue = useRef(0);
    const isovalueTimer = useRef<number | null>(null);
    const isovalueRequestSequence = useRef(0);
    const isovalueInputSequence = useRef(0);
    const isovalueQueue = useRef<Promise<void>>(Promise.resolve());
    const activeFieldTarget = useRef<{ layerId: number | null; revision: number }>({ layerId: null, revision: 0 });

    const invalidateIsovalueRequests = () => {
        if (isovalueTimer.current !== null) {
            window.clearTimeout(isovalueTimer.current);
            isovalueTimer.current = null;
        }
        isovalueRequestSequence.current += 1;
    };

    const isCurrentIsovalueRequest = (sequence: number, layerId: number, revision: number) => (
        sequence === isovalueRequestSequence.current
        && activeFieldTarget.current.layerId === layerId
        && activeFieldTarget.current.revision === revision
    );

    const setPanelError = (cause: unknown, fallback: string) => {
        if (cause instanceof IpcException) {
            setError({ code: cause.code, message: cause.message, recoverable: cause.recoverable });
            return;
        }
        setError({ code: 'internal_error', message: fallback, recoverable: false });
    };

    const applyVolumetricInfo = (info: VolumetricInfo) => {
        isovalueInputSequence.current += 1;
        invalidateIsovalueRequests();
        activeFieldTarget.current = { layerId: null, revision: 0 };
        setVolumetricInfo(info);
        const value = initial_isovalue(info);
        committedIsovalue.current = value ?? 0;
        setIsovalue(committedIsovalue.current);
        setSurfaceOpacity(0.5);
        setDensityCutoff(0);
        setOpacityScale(1);
        setVolumeRenderMode('both');
        setSignMode('positive');
        setVolumeColormap(info.data_min < -0.01 * Math.abs(info.data_max) ? 'coolwarm' : 'viridis');
        return value;
    };

    const applyFieldScene = (scene: FieldSceneInfo) => {
        if (
            activeFieldTarget.current.layerId !== scene.active_layer_id
            || activeFieldTarget.current.revision !== scene.revision
        ) {
            invalidateIsovalueRequests();
        }
        activeFieldTarget.current = { layerId: scene.active_layer_id, revision: scene.revision };
        setFieldScene(scene);
        const active = scene.layers.find((layer) => layer.id === scene.active_layer_id);
        if (active) {
            setVolumetricInfo({
                grid_dims: active.grid_dims,
                data_min: active.data_min,
                data_max: active.data_max,
                format: 'field',
            });
            committedIsovalue.current = active.isovalue;
            setIsovalue(active.isovalue);
            setSurfaceOpacity(active.opacity);
            setSignMode(active.sign_mode);
            setVolumeRenderMode(active.render_mode);
            setVolumeColormap(colormap_from_mode(active.colormap_mode));
        } else {
            setVolumetricInfo(null);
        }
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
        return () => {
            invalidateIsovalueRequests();
        };
    }, []);

    useEffect(() => {
        let disposed = false;
        let unlisten = () => { disposed = true; };
        safeListen('volumetric_loaded', (event) => {
            const info = event.payload;
            if (info) {
                const defaultIsovalue = applyVolumetricInfo(info);
                
                if (setOpenAccordion) {
                    setOpenAccordion('Volumetric');
                }
                
                if (defaultIsovalue !== null) {
                    const inputSequence = isovalueInputSequence.current;
                    void safeInvoke('get_field_scene_info').then((scene) => {
                        if (
                            inputSequence !== isovalueInputSequence.current
                            || scene.revision < activeFieldTarget.current.revision
                        ) return;
                        applyFieldScene(scene);
                        const layerId = scene.active_layer_id;
                        if (layerId === null) return;
                        const revision = scene.revision;
                        const sequence = ++isovalueRequestSequence.current;
                        const request = isovalueQueue.current
                            .catch(() => undefined)
                            .then(() => {
                                if (!isCurrentIsovalueRequest(sequence, layerId, revision)) return;
                                return safeInvoke('set_isovalue', {
                                    value: defaultIsovalue,
                                    layerId,
                                    expectedRevision: revision,
                                }).then(() => {
                                    if (!isCurrentIsovalueRequest(sequence, layerId, revision)) return;
                                    committedIsovalue.current = defaultIsovalue;
                                    setDensityCutoff(defaultIsovalue);
                                });
                            })
                            .catch((cause) => {
                                if (isCurrentIsovalueRequest(sequence, layerId, revision)) {
                                    setPanelError(cause, 'Unable to initialize the isovalue.');
                                }
                            });
                        isovalueQueue.current = request;
                    }).catch((cause) => setPanelError(cause, 'Unable to initialize the isovalue.'));
                    safeInvoke('set_volume_render_mode', { mode: 'both' })
                        .then(() => setVolumeRenderMode('both'))
                        .catch((cause) => setPanelError(cause, 'Unable to initialize the volume renderer.'));
                    safeInvoke('set_isosurface_sign_mode', { mode: 'both' })
                        .then(() => setSignMode('both'))
                        .catch((cause) => setPanelError(cause, 'Unable to initialize the isosurface sign mode.'));
                }
            }
        }).then((listener) => {
            if (disposed) listener();
            else unlisten = listener;
        }).catch(console.warn);
        
        return () => {
            unlisten();
        };
    }, [setOpenAccordion]);

    useEffect(() => {
        let disposed = false;
        let unlisten = () => { disposed = true; };
        const refreshFieldScene = () => {
            const requestSequence = ++fieldRefreshSequence.current;
            return safeInvoke('get_field_scene_info')
            .then((scene) => {
                if (!disposed && requestSequence === fieldRefreshSequence.current) applyFieldScene(scene);
            })
            .catch((cause) => setPanelError(cause, 'Unable to refresh the field scene.'));
        };
        void refreshFieldScene();
        safeListen('field_scene_changed', () => {
            void refreshFieldScene();
        }).then((listener) => {
            if (disposed) listener();
            else unlisten = listener;
        }).catch(console.warn);
        return () => {
            unlisten();
        };
    }, []);

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

    useEffect(() => {
        setCombinationLayerIds((previous) => {
            const valid = previous.filter((id): id is number => id !== null && fieldScene.layers.some((layer) => layer.id === id));
            if (valid.length === 2 && valid[0] !== valid[1]) return [valid[0], valid[1]];
            const available = fieldScene.layers.filter((layer) => !valid.includes(layer.id));
            return [valid[0] ?? available[0]?.id ?? null, valid[1] ?? available[1]?.id ?? null];
        });
    }, [fieldScene.layers]);

    useEffect(() => {
        setRenameLabel(fieldScene.layers.find((layer) => layer.id === fieldScene.active_layer_id)?.label ?? '');
    }, [fieldScene.active_layer_id, fieldScene.layers]);

    const selectFieldLayer = async (layerId: number) => {
        if (isPanelBusy || layerId === fieldScene.active_layer_id) return;
        setPendingControl('field-layer');
        try {
            const scene = await safeInvoke('select_active_field_layer', {
                layerId,
                expectedRevision: fieldScene.revision,
            });
            applyFieldScene(scene);
        } catch (cause) {
            setPanelError(cause, 'Unable to select the field layer.');
        } finally {
            setPendingControl(null);
        }
    };

    const removeActiveFieldLayer = async () => {
        const layerId = fieldScene.active_layer_id;
        if (isPanelBusy || layerId === null) return;
        setPendingControl('field-remove');
        try {
            const scene = await safeInvoke('remove_field_layer', {
                layerId,
                expectedRevision: fieldScene.revision,
            });
            applyFieldScene(scene);
        } catch (cause) {
            setPanelError(cause, 'Unable to remove the field layer.');
        } finally {
            setPendingControl(null);
        }
    };

    const setActiveFieldVisibility = async (visible: boolean) => {
        const layerId = fieldScene.active_layer_id;
        if (isPanelBusy || layerId === null) return;
        setPendingControl('field-visibility');
        try {
            const scene = await safeInvoke('set_field_layer_visibility', {
                layerId,
                visible,
                expectedRevision: fieldScene.revision,
            });
            applyFieldScene(scene);
        } catch (cause) {
            setPanelError(cause, 'Unable to update field visibility.');
        } finally {
            setPendingControl(null);
        }
    };

    const renameActiveFieldLayer = async () => {
        const layerId = fieldScene.active_layer_id;
        const label = renameLabel.trim();
        if (isPanelBusy || layerId === null || !label) return;
        setPendingControl('field-rename');
        try {
            const scene = await safeInvoke('rename_field_layer', {
                layerId,
                label,
                expectedRevision: fieldScene.revision,
            });
            applyFieldScene(scene);
        } catch (cause) {
            setPanelError(cause, 'Unable to rename the field layer.');
        } finally {
            setPendingControl(null);
        }
    };

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

            <ActionButton label="Add Field Layer..." busyLabel="Adding field layer…" onClick={async () => {
                if (isPanelBusy) return;
                setError(null);
                setPendingControl('field-add');
                try {
                    const file = await safeDialogOpen({ title: 'Add Field Layer' });
                    if (file && typeof file === 'string') {
                        const scene = await safeInvoke('add_field_layer', {
                            path: file,
                            scalarUnit: fieldImportUnit || null,
                            expectedRevision: fieldScene.revision,
                        });
                        applyFieldScene(scene);
                    }
                } catch (cause) {
                    setPanelError(cause, 'Unable to add the field layer.');
                } finally {
                    setPendingControl(null);
                }
            }} disabled={isPanelBusy} busy={pendingControl === 'field-add'} />

            <SelectInput
                label="Imported scalar unit (when adapter does not declare one)"
                value={fieldImportUnit}
                onChange={(value) => {
                    if (value === '' || value === 'electron_per_cubic_angstrom' || value === 'electron_per_bohr_cubed') setFieldImportUnit(value);
                }}
                disabled={isPanelBusy}
            >
                <option value="">Undeclared — display only</option>
                <option value="electron_per_cubic_angstrom">e/Å³</option>
                <option value="electron_per_bohr_cubed">e/a₀³</option>
            </SelectInput>

            {fieldScene.layers.length > 0 && <SelectInput
                label="Active Field Layer"
                value={String(fieldScene.active_layer_id ?? '')}
                onChange={(value) => void selectFieldLayer(Number(value))}
                disabled={isPanelBusy}
                busy={pendingControl === 'field-layer'}
            >
                {fieldScene.layers.map((layer) => <option key={layer.id} value={layer.id}>{layer.label}</option>)}
            </SelectInput>}

            {fieldScene.active_layer_id !== null && <ActionButton
                label="Remove Active Field Layer"
                busyLabel="Removing field layer…"
                onClick={() => void removeActiveFieldLayer()}
                disabled={isPanelBusy}
                busy={pendingControl === 'field-remove'}
            />}

            {fieldScene.active_layer_id !== null && <label className="block text-xs text-[var(--cc-muted)]">
                Active layer label
                <div className="mt-1 flex gap-2">
                    <input
                        className="min-w-0 flex-1 rounded border border-[var(--cc-border)] bg-[var(--cc-panel)] px-2 py-1 text-[var(--cc-text)]"
                        value={renameLabel}
                        maxLength={256}
                        onChange={(event) => setRenameLabel(event.target.value)}
                        disabled={isPanelBusy}
                    />
                    <ActionButton
                        label="Rename"
                        busyLabel="Renaming…"
                        onClick={() => void renameActiveFieldLayer()}
                        disabled={isPanelBusy || !renameLabel.trim()}
                        busy={pendingControl === 'field-rename'}
                    />
                </div>
            </label>}

            {fieldScene.active_layer_id !== null && (() => {
                const active = fieldScene.layers.find((layer) => layer.id === fieldScene.active_layer_id);
                if (!active) return null;
                return <ActionButton
                    label={active.visible ? 'Hide Active Field Layer' : 'Show Active Field Layer'}
                    busyLabel="Updating field visibility…"
                    onClick={() => void setActiveFieldVisibility(!active.visible)}
                    disabled={isPanelBusy}
                    busy={pendingControl === 'field-visibility'}
                />;
            })()}

            {fieldScene.layers.length >= 2 && <>
                <SelectInput
                    label="Linear Combination A"
                    value={String(combinationLayerIds[0] ?? '')}
                    onChange={(value) => setCombinationLayerIds(([_, second]) => [Number(value), second])}
                    disabled={isPanelBusy}
                >
                    {fieldScene.layers.map((layer) => <option key={layer.id} value={layer.id}>{layer.label}</option>)}
                </SelectInput>
                <label className="block text-xs text-[var(--cc-muted)]">
                    Coefficient A
                    <input
                        className="mt-1 w-full rounded border border-[var(--cc-border)] bg-[var(--cc-panel)] px-2 py-1 text-[var(--cc-text)]"
                        type="number"
                        value={combinationCoefficients[0]}
                        step="any"
                        onChange={(event) => setCombinationCoefficients(([, second]) => [Number(event.target.value), second])}
                        disabled={isPanelBusy}
                    />
                </label>
                <SelectInput
                    label="Linear Combination B"
                    value={String(combinationLayerIds[1] ?? '')}
                    onChange={(value) => setCombinationLayerIds(([first]) => [first, Number(value)])}
                    disabled={isPanelBusy}
                >
                    {fieldScene.layers.map((layer) => <option key={layer.id} value={layer.id}>{layer.label}</option>)}
                </SelectInput>
                <label className="block text-xs text-[var(--cc-muted)]">
                    Coefficient B
                    <input
                        className="mt-1 w-full rounded border border-[var(--cc-border)] bg-[var(--cc-panel)] px-2 py-1 text-[var(--cc-text)]"
                        type="number"
                        value={combinationCoefficients[1]}
                        step="any"
                        onChange={(event) => setCombinationCoefficients(([first]) => [first, Number(event.target.value)])}
                        disabled={isPanelBusy}
                    />
                </label>
                <label className="block text-xs text-[var(--cc-muted)]">
                    Output label
                    <input
                        className="mt-1 w-full rounded border border-[var(--cc-border)] bg-[var(--cc-panel)] px-2 py-1 text-[var(--cc-text)]"
                        value={combinationLabel}
                        maxLength={256}
                        onChange={(event) => setCombinationLabel(event.target.value)}
                        disabled={isPanelBusy}
                    />
                </label>
            <ActionButton label="Combine Selected Fields" busyLabel="Combining fields…" onClick={async () => {
                if (isPanelBusy) return;
                const [firstId, secondId] = combinationLayerIds;
                const [firstCoefficient, secondCoefficient] = combinationCoefficients;
                if (firstId === null || secondId === null || firstId === secondId || !Number.isFinite(firstCoefficient) || !Number.isFinite(secondCoefficient) || !combinationLabel.trim()) {
                    setPanelError(new IpcException({ code: 'invalid_argument', message: 'Choose two different layers, finite coefficients, and an output label.', recoverable: true }), 'Choose two different layers, finite coefficients, and an output label.');
                    return;
                }
                setPendingControl('field-combine');
                try {
                    const scene = await safeInvoke('combine_field_layers', {
                        terms: [{ layerId: firstId, coefficient: firstCoefficient }, { layerId: secondId, coefficient: secondCoefficient }],
                        outputLabel: combinationLabel.trim(),
                        expectedRevision: fieldScene.revision,
                    });
                    applyFieldScene(scene);
                } catch (cause) {
                    setPanelError(cause, 'Unable to combine field layers.');
                } finally {
                    setPendingControl(null);
                }
            }} disabled={isPanelBusy} busy={pendingControl === 'field-combine'} />
            </>}

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
                setError(null);
                setIsovalue(value);
                isovalueInputSequence.current += 1;
                if (isovalueTimer.current !== null) window.clearTimeout(isovalueTimer.current);
                const layerId = activeFieldTarget.current.layerId;
                const revision = activeFieldTarget.current.revision;
                const sequence = ++isovalueRequestSequence.current;
                if (layerId === null) {
                    setIsovalue(committedIsovalue.current);
                    return;
                }
                isovalueTimer.current = window.setTimeout(() => {
                    isovalueTimer.current = null;
                    const request = isovalueQueue.current
                        .catch(() => undefined)
                        .then(() => {
                            if (!isCurrentIsovalueRequest(sequence, layerId, revision)) return;
                            return safeInvoke('set_isovalue', {
                                value,
                                layerId,
                                expectedRevision: revision,
                            }).then(() => {
                                if (!isCurrentIsovalueRequest(sequence, layerId, revision)) return;
                                committedIsovalue.current = value;
                                if (volumeRenderMode === 'both') setDensityCutoff(value);
                                else setDensityCutoff(0);
                            });
                        })
                        .catch((cause) => {
                            if (isCurrentIsovalueRequest(sequence, layerId, revision)) {
                                setIsovalue((current) => current === value ? committedIsovalue.current : current);
                                setPanelError(cause, 'Unable to change the isovalue.');
                            }
                        });
                    isovalueQueue.current = request;
                }, 100);
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
