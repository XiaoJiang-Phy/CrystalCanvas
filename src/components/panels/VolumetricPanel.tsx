import React, { useState, useEffect, useRef } from 'react';
import { safeInvoke, safeListen, safeDialogOpen } from '../../utils/tauri-mock';
import {
    IpcException,
    is_isosurface_sign_mode,
    is_volume_colormap,
    is_volume_render_mode,
    type IpcError,
    type FieldLayerInfo,
    type FieldSceneChangedPayload,
    type FieldSceneInfo,
    type FieldPresentationSettings,
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

const rgba_to_hex = (color: [number, number, number, number]): string => `#${color
    .slice(0, 3)
    .map((component) => Math.round(Math.min(1, Math.max(0, component)) * 255).toString(16).padStart(2, '0'))
    .join('')}`;

const hex_to_rgba = (hex: string, alpha: number): [number, number, number, number] => [
    Number.parseInt(hex.slice(1, 3), 16) / 255,
    Number.parseInt(hex.slice(3, 5), 16) / 255,
    Number.parseInt(hex.slice(5, 7), 16) / 255,
    alpha,
];

const default_field_presentation = (): FieldPresentationSettings => ({
    clip_planes: [],
    slices: [],
    transfer_function: {
        color_space: 'LinearRgb',
        negative_control_points: [{ position: 0, color_linear_rgba: [0.13, 0.20, 0.70, 0] }, { position: 1, color_linear_rgba: [0.18, 0.42, 1, 0.65] }],
        positive_control_points: [{ position: 0, color_linear_rgba: [0.95, 0.45, 0.04, 0] }, { position: 1, color_linear_rgba: [1, 0.73, 0.18, 0.65] }],
    },
    use_explicit_transfer_function: false,
    display_range: null,
    opacity_scale: 3,
    density_cutoff: 0,
    transparency_method: 'premultiplied_alpha_fallback',
    field_material_mode: 'lit',
});

const field_compatibility_reason = (first: FieldLayerInfo | undefined, second: FieldLayerInfo | undefined): string => {
    if (!first || !second) return 'Select two field layers.';
    if (!first.metadata_declared || !second.metadata_declared) return 'Scalar metadata is undeclared.';
    if (first.grid_dims.some((dimension, index) => dimension !== second.grid_dims[index])) return 'Grid dimensions differ.';
    if (first.lattice_angstrom.some((value, index) => Math.abs(value - second.lattice_angstrom[index]) > 1e-5)) return 'Lattice mappings differ.';
    if (first.origin_angstrom.some((value, index) => Math.abs(value - second.origin_angstrom[index]) > 1e-5)) return 'Origins differ.';
    if (first.ordering !== second.ordering) return 'Grid ordering differs.';
    if (first.periodic_axes.some((value, index) => value !== second.periodic_axes[index])) return 'Periodic axes differ.';
    if (first.attachment !== second.attachment) return 'Grid attachment differs.';
    if (first.scalar_unit !== second.scalar_unit) return 'Scalar dimensions differ.';
    if (Math.abs(first.scalar_unit_scale - second.scalar_unit_scale) > Number.EPSILON) return 'Scalar unit scales differ.';
    if (first.normalization !== second.normalization) return 'Normalization differs.';
    return 'Compatible for backend linear combination.';
};

type RenderControlKey = 'surfaceOpacity' | 'densityCutoff' | 'opacityScale';

export default function VolumetricPanel({ setOpenAccordion }: PanelProps) {
    const [fieldScene, setFieldScene] = useState<FieldSceneInfo>({ revision: 0, active_layer_id: null, layers: [] });
    const [volumetricInfo, setVolumetricInfo] = useState<VolumetricInfo | null>(null);
    const [isovalue, setIsovalue] = useState(0);
    const [surfaceOpacity, setSurfaceOpacity] = useState(0.5);
    const [surfaceOpacityDraft, setSurfaceOpacityDraft] = useState(0.5);
    const [positiveColor, setPositiveColor] = useState('#b40426');
    const [negativeColor, setNegativeColor] = useState('#3b4cc0');
    const [densityCutoff, setDensityCutoff] = useState(0);
    const [densityCutoffDraft, setDensityCutoffDraft] = useState(0);
    const [opacityScale, setOpacityScale] = useState(3);
    const [opacityScaleDraft, setOpacityScaleDraft] = useState(3);
    const [volumeRenderMode, setVolumeRenderMode] = useState<VolumeRenderMode>('isosurface');
    const [signMode, setSignMode] = useState<IsosurfaceSignMode>('positive');
    const [volumeColormap, setVolumeColormap] = useState<VolumeColormap>('viridis');
    const [fieldPresentation, setFieldPresentation] = useState<FieldPresentationSettings>(default_field_presentation);
    const [clipNormal, setClipNormal] = useState<[number, number, number]>([0, 0, 1]);
    const [clipOffset, setClipOffset] = useState(0);
    const [combinationLayerIds, setCombinationLayerIds] = useState<[number | null, number | null]>([null, null]);
    const [combinationCoefficients, setCombinationCoefficients] = useState<[number, number]>([1, -1]);
    const [combinationLabel, setCombinationLabel] = useState('linear-combination');
    const [renameLabel, setRenameLabel] = useState('');
    const [fieldImportUnit, setFieldImportUnit] = useState<'electron_per_cubic_angstrom' | 'electron_per_bohr_cubed' | ''>('');
    const [isLoading, setIsLoading] = useState(false);
    const [pendingControl, setPendingControl] = useState<string | null>(null);
    const [isPresentationPending, setIsPresentationPending] = useState(false);
    const [error, setError] = useState<IpcError | null>(null);
    const fieldSceneCommitSequence = useRef(0);
    const committedIsovalue = useRef(0);
    const isovalueTimer = useRef<number | null>(null);
    const isovalueRequestSequence = useRef(0);
    const isovalueInputSequence = useRef(0);
    const presentationRequestSequence = useRef(0);
    const renderControlRequestSequence = useRef<Record<RenderControlKey, number>>({ surfaceOpacity: 0, densityCutoff: 0, opacityScale: 0 });
    const renderControlQueue = useRef<Promise<void>>(Promise.resolve());
    const pendingRenderControls = useRef(new Set<RenderControlKey>());
    const isovaluePending = useRef(false);
    const activeFieldTarget = useRef<{ layerId: number | null; revision: number; dataMin: number; dataMax: number }>({ layerId: null, revision: 0, dataMin: 0, dataMax: 0 });

    const invalidateIsovalueRequests = () => {
        if (isovalueTimer.current !== null) {
            window.clearTimeout(isovalueTimer.current);
            isovalueTimer.current = null;
        }
        isovalueRequestSequence.current += 1;
    };

    const invalidateFieldSceneCommits = () => {
        fieldSceneCommitSequence.current += 1;
    };

    const invalidateRenderControlRequests = () => {
        for (const key of Object.keys(renderControlRequestSequence.current) as RenderControlKey[]) {
            renderControlRequestSequence.current[key] += 1;
        }
        pendingRenderControls.current.clear();
    };

    const beginFieldSceneCommit = () => ++fieldSceneCommitSequence.current;

    const isCurrentIsovalueRequest = (sequence: number, layerId: number) => (
        sequence === isovalueRequestSequence.current
        && activeFieldTarget.current.layerId === layerId
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
        invalidateFieldSceneCommits();
        invalidateRenderControlRequests();
        isovaluePending.current = false;
        activeFieldTarget.current = { layerId: null, revision: 0, dataMin: 0, dataMax: 0 };
        setVolumetricInfo(info);
        const value = initial_isovalue(info);
        committedIsovalue.current = value ?? 0;
        setIsovalue(committedIsovalue.current);
        setSurfaceOpacity(0.5);
        setSurfaceOpacityDraft(0.5);
        setDensityCutoff(0);
        setDensityCutoffDraft(0);
        setOpacityScale(3);
        setOpacityScaleDraft(3);
        setVolumeRenderMode('isosurface');
        setSignMode(info.data_min < -0.01 * Math.abs(info.data_max) ? 'both' : 'positive');
        setVolumeColormap(info.data_min < -0.01 * Math.abs(info.data_max) ? 'coolwarm' : 'viridis');
        return value;
    };

    const applyFieldScene = (scene: FieldSceneInfo) => {
        if (scene.revision < activeFieldTarget.current.revision) return;
        if (activeFieldTarget.current.layerId !== scene.active_layer_id) {
            invalidateIsovalueRequests();
            invalidateRenderControlRequests();
            isovaluePending.current = false;
        }
        setFieldScene(scene);
        const active = scene.layers.find((layer) => layer.id === scene.active_layer_id);
        activeFieldTarget.current = {
            layerId: scene.active_layer_id,
            revision: scene.revision,
            dataMin: active?.data_min ?? 0,
            dataMax: active?.data_max ?? 0,
        };
        if (active) {
            setVolumetricInfo({
                grid_dims: active.grid_dims,
                data_min: active.data_min,
                data_max: active.data_max,
                format: 'field',
            });
            if (!isovaluePending.current) {
                committedIsovalue.current = active.isovalue;
                setIsovalue(active.isovalue);
            }
            if (!pendingRenderControls.current.has('surfaceOpacity')) {
                setSurfaceOpacity(active.opacity);
                setSurfaceOpacityDraft(active.opacity);
            }
            setPositiveColor(rgba_to_hex(active.color));
            setNegativeColor(rgba_to_hex(active.color_negative));
            if (!pendingRenderControls.current.has('opacityScale')) {
                setOpacityScale(active.opacity_scale);
                setOpacityScaleDraft(active.opacity_scale);
            }
            setSignMode(active.sign_mode);
            setVolumeRenderMode(active.render_mode);
            setVolumeColormap(colormap_from_mode(active.colormap_mode));
            setFieldPresentation(active.presentation_settings);
            if (!pendingRenderControls.current.has('densityCutoff')) {
                setDensityCutoff(active.presentation_settings.density_cutoff);
                setDensityCutoffDraft(active.presentation_settings.density_cutoff);
            }
        } else {
            setVolumetricInfo(null);
        }
    };

    const applyLatestFieldScene = (sequence: number, scene: FieldSceneInfo) => {
        if (sequence !== fieldSceneCommitSequence.current
            || scene.revision < activeFieldTarget.current.revision) return false;
        applyFieldScene(scene);
        return true;
    };

    const queueFieldRenderControl = (
        key: RenderControlKey,
        invoke: (layerId: number, expectedRevision: number) => Promise<FieldSceneChangedPayload>,
        onSuccess: () => void,
        onFailure: () => void,
        fallback: string,
    ) => {
        const layerId = activeFieldTarget.current.layerId;
        if (layerId === null) return;
        const sequence = ++renderControlRequestSequence.current[key];
        pendingRenderControls.current.add(key);
        const request = renderControlQueue.current.catch(() => undefined).then(async () => {
            if (sequence !== renderControlRequestSequence.current[key] || activeFieldTarget.current.layerId !== layerId) return;
            const updated = await invoke(layerId, activeFieldTarget.current.revision);
            if (updated.active_layer_id === layerId) {
                activeFieldTarget.current.revision = updated.revision;
            }
            if (sequence === renderControlRequestSequence.current[key] && updated.active_layer_id === layerId) {
                pendingRenderControls.current.delete(key);
                onSuccess();
            }
        });
        const completed = request.catch((cause) => {
            if (sequence === renderControlRequestSequence.current[key] && activeFieldTarget.current.layerId === layerId) {
                pendingRenderControls.current.delete(key);
                onFailure();
                setPanelError(cause, fallback);
            }
        });
        renderControlQueue.current = completed.catch(() => undefined);
    };

    const queueFieldMutation = (work: () => Promise<void>) => {
        const request = renderControlQueue.current.catch(() => undefined).then(work);
        renderControlQueue.current = request.catch(() => undefined);
        return request;
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

    const updateFieldPresentation = (mutate: (current: FieldPresentationSettings) => FieldPresentationSettings) => {
        const targetLayerId = activeFieldTarget.current.layerId;
        if (targetLayerId === null) return Promise.resolve();
        const requestSequence = ++presentationRequestSequence.current;
        setIsPresentationPending(true);
        const request = renderControlQueue.current.catch(() => undefined).then(async () => {
            const scene = await safeInvoke('get_field_scene_info');
            if (scene.active_layer_id !== targetLayerId) return;
            const active = scene.layers.find((layer) => layer.id === targetLayerId);
            if (!active) return;
            const presentationSettings = mutate(active.presentation_settings);
            const commitSequence = beginFieldSceneCommit();
            const updated = await safeInvoke('set_field_layer_presentation', {
                layerId: active.id,
                presentationSettings,
                expectedRevision: scene.revision,
            });
            if (
                requestSequence === presentationRequestSequence.current
                && updated.active_layer_id === targetLayerId
            ) {
                applyLatestFieldScene(commitSequence, updated);
            }
        });
        const completed = request.finally(() => {
            if (requestSequence === presentationRequestSequence.current) {
                setIsPresentationPending(false);
            }
        });
        renderControlQueue.current = completed.catch(() => undefined);
        return completed;
    };

    useEffect(() => {
        return () => {
            invalidateIsovalueRequests();
            invalidateFieldSceneCommits();
            invalidateRenderControlRequests();
            presentationRequestSequence.current += 1;
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
                    const commitSequence = beginFieldSceneCommit();
                    void safeInvoke('get_field_scene_info').then((scene) => {
                        if (inputSequence !== isovalueInputSequence.current) return;
                        applyLatestFieldScene(commitSequence, scene);
                    }).catch((cause) => setPanelError(cause, 'Unable to initialize the isovalue.'));
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
            const commitSequence = beginFieldSceneCommit();
            return safeInvoke('get_field_scene_info')
            .then((scene) => {
                if (!disposed) applyLatestFieldScene(commitSequence, scene);
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
        const layerId = activeFieldTarget.current.layerId;
        if (isLoading || pendingControl || layerId === null || !is_volume_render_mode(value)) return;
        const mode = value;
        setError(null);
        setPendingControl('render-mode');
        try {
            await queueFieldMutation(async () => {
                if (activeFieldTarget.current.layerId !== layerId) return;
                const updated = await safeInvoke('set_volume_render_mode', {
                    mode,
                    layerId,
                    expectedRevision: activeFieldTarget.current.revision,
                });
                if (updated.active_layer_id !== layerId) return;
                activeFieldTarget.current.revision = updated.revision;
                setVolumeRenderMode(mode);
                const nextDensityCutoff = mode === 'both' ? isovalue : 0;
                setDensityCutoff(nextDensityCutoff);
                setDensityCutoffDraft(nextDensityCutoff);
            });
        } catch (cause) {
            setPanelError(cause, 'Unable to change the volume render mode.');
        } finally {
            setPendingControl(null);
        }
    };

    const handleSignMode = async (value: string) => {
        const layerId = activeFieldTarget.current.layerId;
        if (isLoading || pendingControl || layerId === null || !is_isosurface_sign_mode(value)) return;
        const mode = value;
        setError(null);
        setPendingControl('sign-mode');
        try {
            await queueFieldMutation(async () => {
                if (activeFieldTarget.current.layerId !== layerId) return;
                const updated = await safeInvoke('set_isosurface_sign_mode', {
                    mode,
                    layerId,
                    expectedRevision: activeFieldTarget.current.revision,
                });
                if (updated.active_layer_id !== layerId) return;
                activeFieldTarget.current.revision = updated.revision;
                setSignMode(mode);
            });
        } catch (cause) {
            setPanelError(cause, 'Unable to change the isosurface sign mode.');
        } finally {
            setPendingControl(null);
        }
    };

    const handleVolumeColormap = async (value: string) => {
        const layerId = activeFieldTarget.current.layerId;
        if (isLoading || pendingControl || layerId === null || !is_volume_colormap(value)) return;
        const mode = value;
        setError(null);
        setPendingControl('colormap');
        try {
            await queueFieldMutation(async () => {
                if (activeFieldTarget.current.layerId !== layerId) return;
                const updated = await safeInvoke('set_volume_colormap', {
                    mode,
                    layerId,
                    expectedRevision: activeFieldTarget.current.revision,
                });
                if (updated.active_layer_id !== layerId) return;
                activeFieldTarget.current.revision = updated.revision;
                setVolumeColormap(mode);
            });
        } catch (cause) {
            setPanelError(cause, 'Unable to change the volume colormap.');
        } finally {
            setPendingControl(null);
        }
    };

    const handleSurfaceColor = async (branch: 'positive' | 'negative', value: string) => {
        const layerId = activeFieldTarget.current.layerId;
        if (isPanelBusy || layerId === null) return;
        const nextPositive = branch === 'positive' ? value : positiveColor;
        const nextNegative = branch === 'negative' ? value : negativeColor;
        setError(null);
        setPendingControl(`${branch}-color`);
        try {
            await queueFieldMutation(async () => {
                if (activeFieldTarget.current.layerId !== layerId) return;
                const updated = await safeInvoke('set_isosurface_colors', {
                    positiveColor: hex_to_rgba(nextPositive, surfaceOpacity),
                    negativeColor: hex_to_rgba(nextNegative, surfaceOpacity),
                    layerId,
                    expectedRevision: activeFieldTarget.current.revision,
                });
                if (updated.active_layer_id !== layerId) return;
                activeFieldTarget.current.revision = updated.revision;
                setPositiveColor(nextPositive);
                setNegativeColor(nextNegative);
            });
        } catch (cause) {
            setPanelError(cause, 'Unable to change the isosurface colors.');
        } finally {
            setPendingControl(null);
        }
    };

    const isPanelBusy = isLoading || pendingControl !== null || isPresentationPending;

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
            await queueFieldMutation(async () => {
                const commitSequence = beginFieldSceneCommit();
                const scene = await safeInvoke('select_active_field_layer', {
                    layerId,
                    expectedRevision: activeFieldTarget.current.revision,
                });
                applyLatestFieldScene(commitSequence, scene);
            });
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
            await queueFieldMutation(async () => {
                const commitSequence = beginFieldSceneCommit();
                const scene = await safeInvoke('remove_field_layer', {
                    layerId,
                    expectedRevision: activeFieldTarget.current.revision,
                });
                applyLatestFieldScene(commitSequence, scene);
            });
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
            await queueFieldMutation(async () => {
                const commitSequence = beginFieldSceneCommit();
                const scene = await safeInvoke('set_field_layer_visibility', {
                    layerId,
                    visible,
                    expectedRevision: activeFieldTarget.current.revision,
                });
                applyLatestFieldScene(commitSequence, scene);
            });
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
            await queueFieldMutation(async () => {
                const commitSequence = beginFieldSceneCommit();
                const scene = await safeInvoke('rename_field_layer', {
                    layerId,
                    label,
                    expectedRevision: activeFieldTarget.current.revision,
                });
                applyLatestFieldScene(commitSequence, scene);
            });
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
                        await queueFieldMutation(async () => {
                            const commitSequence = beginFieldSceneCommit();
                            const scene = await safeInvoke('add_field_layer', {
                                path: file,
                                scalarUnit: fieldImportUnit || null,
                                expectedRevision: activeFieldTarget.current.revision,
                            });
                            applyLatestFieldScene(commitSequence, scene);
                        });
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
                {(() => {
                    const firstLayer = fieldScene.layers.find((layer) => layer.id === combinationLayerIds[0]);
                    const secondLayer = fieldScene.layers.find((layer) => layer.id === combinationLayerIds[1]);
                    const compatibility = field_compatibility_reason(firstLayer, secondLayer);
                    const provenance = firstLayer && secondLayer
                        ? `${combinationCoefficients[0]} × ${firstLayer.normalized_sha256.slice(0, 12)} + ${combinationCoefficients[1]} × ${secondLayer.normalized_sha256.slice(0, 12)}`
                        : 'Select two field layers.';
                    const receipts = [
                        ...(firstLayer?.lineage?.map((term) => term.compatibility_receipt_sha256) ?? []),
                        ...(secondLayer?.lineage?.map((term) => term.compatibility_receipt_sha256) ?? []),
                    ];
                    return <div role="status" className="space-y-1 rounded border border-[var(--cc-border)] bg-[var(--cc-panel)] p-2 text-[10px] text-[var(--cc-muted)] font-mono">
                        <div><span className="text-[var(--cc-text)]">Compatibility:</span> {compatibility}</div>
                        {firstLayer && secondLayer && <div><span className="text-[var(--cc-text)]">Units:</span> {firstLayer.scalar_unit} × {firstLayer.scalar_unit_scale} / {secondLayer.scalar_unit} × {secondLayer.scalar_unit_scale}; <span className="text-[var(--cc-text)]">Normalization:</span> {firstLayer.normalization} / {secondLayer.normalization}</div>}
                        <div><span className="text-[var(--cc-text)]">Output preview:</span> {provenance}</div>
                        {receipts.length > 0 && <div><span className="text-[var(--cc-text)]">Compatibility receipts:</span> {receipts.map((receipt) => receipt.slice(0, 12)).join(', ')}</div>}
                    </div>;
                })()}
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
                    await queueFieldMutation(async () => {
                        const commitSequence = beginFieldSceneCommit();
                        const scene = await safeInvoke('combine_field_layers', {
                            terms: [{ layerId: firstId, coefficient: firstCoefficient }, { layerId: secondId, coefficient: secondCoefficient }],
                            outputLabel: combinationLabel.trim(),
                            expectedRevision: activeFieldTarget.current.revision,
                        });
                        applyLatestFieldScene(commitSequence, scene);
                    });
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
                label="Render Mode (Representation)"
                value={volumeRenderMode}
                onChange={(value) => void handleRenderMode(value)}
                disabled={isPanelBusy}
                busy={pendingControl === 'render-mode'}
            >
                    <option value="both">Both (Isosurface + Volume)</option>
                    <option value="isosurface">Isosurface Only</option>
                    <option value="volume">Volume Only</option>
            </SelectInput>
            <p className="text-xs text-[var(--cc-muted)]">Clipping, slice, and contour presentation settings remain layer-owned and are preserved for portable Blender export.</p>
            <div className="space-y-2 rounded border border-[var(--cc-border)] p-2">
                <div className="text-xs font-medium text-[var(--cc-text)]">Portable field geometry</div>
                <div className="grid grid-cols-4 gap-1">
                    {clipNormal.map((component, index) => (
                        <input key={index} aria-label={`Clip normal ${index}`} type="number" step="0.1" value={component} onChange={(event) => {
                            const next = [...clipNormal] as [number, number, number];
                            next[index] = Number(event.target.value);
                            setClipNormal(next);
                        }} disabled={isPanelBusy} className="w-full rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-1 py-1 text-xs" />
                    ))}
                    <input aria-label="Clip plane offset" type="number" step="0.1" value={clipOffset} onChange={(event) => setClipOffset(Number(event.target.value))} disabled={isPanelBusy} className="w-full rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-1 py-1 text-xs" />
                </div>
                <div className="grid grid-cols-2 gap-2">
                    <ActionButton label="Add Clip Plane" tone="secondary" disabled={isPanelBusy} onClick={() => void updateFieldPresentation((current) => ({ ...current, clip_planes: [...current.clip_planes, { normal: clipNormal, signed_offset_angstrom: clipOffset, keep_positive: true }] })).catch((cause) => setPanelError(cause, 'Unable to update clipping.'))} />
                    <ActionButton label="Clear Clip Planes" tone="secondary" disabled={isPanelBusy} onClick={() => void updateFieldPresentation((current) => ({ ...current, clip_planes: [] })).catch((cause) => setPanelError(cause, 'Unable to update clipping.'))} />
                    <ActionButton label={fieldPresentation.slices.length ? 'Remove Slices' : 'Add Slice From Plane'} tone="secondary" disabled={isPanelBusy} onClick={() => void updateFieldPresentation((current) => ({ ...current, slices: current.slices.length ? [] : [{ plane: { normal: clipNormal, signed_offset_angstrom: clipOffset, interpolation: 'trilinear' }, dimensions: [128, 128], contour_levels: [] }] })).catch((cause) => setPanelError(cause, 'Unable to update slices.'))} />
                    <ActionButton label={fieldPresentation.slices.some((slice) => slice.contour_levels.length > 0) ? 'Remove Contours' : 'Add Three Contours'} tone="secondary" disabled={isPanelBusy || fieldPresentation.slices.length === 0} onClick={() => void updateFieldPresentation((current) => ({ ...current, slices: current.slices.map((slice) => ({ ...slice, contour_levels: slice.contour_levels.length ? [] : [-volumetricBound * 0.1, 0, volumetricBound * 0.1] })) })).catch((cause) => setPanelError(cause, 'Unable to update contours.'))} />
                </div>
                <div className="grid grid-cols-2 gap-2">
                    <SelectInput label="Field Material" value={fieldPresentation.field_material_mode} onChange={(value) => void updateFieldPresentation((current) => ({ ...current, field_material_mode: value === 'unlit' ? 'unlit' : 'lit' })).catch((cause) => setPanelError(cause, 'Unable to update field material.'))} disabled={isPanelBusy}>
                        <option value="lit">Lit</option><option value="unlit">Unlit</option>
                    </SelectInput>
                    <label className="flex items-end gap-2 pb-1 text-xs text-[var(--cc-muted)]"><input type="checkbox" checked={fieldPresentation.use_explicit_transfer_function} disabled={isPanelBusy} onChange={(event) => void updateFieldPresentation((current) => ({ ...current, use_explicit_transfer_function: event.target.checked })).catch((cause) => setPanelError(cause, 'Unable to update transfer function.'))} />Explicit transfer</label>
                </div>
            </div>

            <RangeInput label="Isovalue" value={isovalue} displayValue={isovalue.toExponential(2)} min={0} max={volumetricBound} step={isovalueStep} disabled={isPanelBusy} onChange={(value) => {
                setError(null);
                setIsovalue(value);
                isovalueInputSequence.current += 1;
                if (isovalueTimer.current !== null) window.clearTimeout(isovalueTimer.current);
                const layerId = activeFieldTarget.current.layerId;
                const sequence = ++isovalueRequestSequence.current;
                if (layerId === null) {
                    setIsovalue(committedIsovalue.current);
                    return;
                }
                isovaluePending.current = true;
                isovalueTimer.current = window.setTimeout(() => {
                    isovalueTimer.current = null;
                    const request = renderControlQueue.current
                        .catch(() => undefined)
                        .then(async () => {
                            if (!isCurrentIsovalueRequest(sequence, layerId)) return;
                            const updated = await safeInvoke('set_isovalue', {
                                value,
                                layerId,
                                expectedRevision: activeFieldTarget.current.revision,
                            });
                            if (updated.active_layer_id === layerId) {
                                activeFieldTarget.current.revision = updated.revision;
                            }
                            if (isCurrentIsovalueRequest(sequence, layerId) && updated.active_layer_id === layerId) {
                                isovaluePending.current = false;
                                committedIsovalue.current = value;
                                setIsovalue(value);
                            }
                        })
                        .catch((cause) => {
                            if (isCurrentIsovalueRequest(sequence, layerId)) {
                                isovaluePending.current = false;
                                setIsovalue((current) => current === value ? committedIsovalue.current : current);
                                setPanelError(cause, 'Unable to change the isovalue.');
                            }
                        });
                    renderControlQueue.current = request.catch(() => undefined);
                }, 100);
            }} />

            <RangeInput label="Surface Opacity" value={surfaceOpacityDraft} displayValue={surfaceOpacityDraft.toFixed(2)} min={0} max={1} step={0.05} disabled={isPanelBusy} onChange={(value) => {
                setError(null);
                setSurfaceOpacityDraft(value);
                queueFieldRenderControl(
                    'surfaceOpacity',
                    (layerId, expectedRevision) => safeInvoke('set_isosurface_opacity', {
                        opacity: value,
                        layerId,
                        expectedRevision,
                    }),
                    () => {
                        setSurfaceOpacity(value);
                        setSurfaceOpacityDraft(value);
                    },
                    () => setSurfaceOpacityDraft(surfaceOpacity),
                    'Unable to change the surface opacity.',
                );
            }} />

            <div className="grid grid-cols-2 gap-2">
                <label className="text-xs text-[var(--cc-muted)]">
                    <span className="mb-1 block">Positive Color</span>
                    <input aria-label="Positive isosurface color" type="color" value={positiveColor} disabled={isPanelBusy} onChange={(event) => void handleSurfaceColor('positive', event.target.value)} className="h-8 w-full cursor-pointer rounded border border-[var(--cc-border)] bg-[var(--cc-panel)] disabled:cursor-not-allowed disabled:opacity-50" />
                </label>
                <label className="text-xs text-[var(--cc-muted)]">
                    <span className="mb-1 block">Negative Color</span>
                    <input aria-label="Negative isosurface color" type="color" value={negativeColor} disabled={isPanelBusy} onChange={(event) => void handleSurfaceColor('negative', event.target.value)} className="h-8 w-full cursor-pointer rounded border border-[var(--cc-border)] bg-[var(--cc-panel)] disabled:cursor-not-allowed disabled:opacity-50" />
                </label>
            </div>

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

            <RangeInput label="Volume Density Cutoff" value={densityCutoffDraft} displayValue={densityCutoffDraft.toExponential(2)} min={0} max={volumetricBound} step={densityCutoffStep} disabled={isPanelBusy} onChange={(value) => {
                setError(null);
                setDensityCutoffDraft(value);
                queueFieldRenderControl(
                    'densityCutoff',
                    (layerId, expectedRevision) => safeInvoke('set_volume_density_cutoff', {
                        cutoff: value,
                        layerId,
                        expectedRevision,
                    }),
                    () => {
                        setDensityCutoff(value);
                        setDensityCutoffDraft(value);
                    },
                    () => setDensityCutoffDraft(densityCutoff),
                    'Unable to change the volume density cutoff.',
                );
            }} />

            <RangeInput label="Volume Opacity Scale" value={opacityScaleDraft} displayValue={opacityScaleDraft.toFixed(1)} min={0.1} max={5} step={0.1} disabled={isPanelBusy} onChange={(value) => {
                setError(null);
                setOpacityScaleDraft(value);
                queueFieldRenderControl(
                    'opacityScale',
                    (layerId, expectedRevision) => safeInvoke('set_volume_opacity_range', {
                        min: activeFieldTarget.current.dataMin,
                        max: activeFieldTarget.current.dataMax,
                        opacityScale: value,
                        layerId,
                        expectedRevision,
                    }),
                    () => {
                        setOpacityScale(value);
                        setOpacityScaleDraft(value);
                    },
                    () => setOpacityScaleDraft(opacityScale),
                    'Unable to change the volume opacity scale.',
                );
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
