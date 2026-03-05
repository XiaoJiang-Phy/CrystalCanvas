// [Overview: Modal dialog for exporting high-resolution publication-quality images.]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React, { useState } from 'react';
import { X, Image as ImageIcon } from 'lucide-react';
import { safeInvoke, safeDialogSave } from '../../utils/tauri-mock';

interface ExportImageModalProps {
    isOpen: boolean;
    onClose: () => void;
    viewportWidth: number;
    viewportHeight: number;
}

type BgMode = 'transparent' | 'white' | 'black' | 'default';

const SCALE_OPTIONS = [
    { label: '1x (Screen)', value: 1 },
    { label: '2x (Hi-DPI)', value: 2 },
    { label: '4x (Print)', value: 4 },
    { label: '8x (Ultra)', value: 8 },
];

const BG_OPTIONS: { label: string; value: BgMode; preview: string }[] = [
    { label: 'Transparent', value: 'transparent', preview: 'bg-[conic-gradient(#e2e8f0_25%,#f1f5f9_25%_50%,#e2e8f0_50%_75%,#f1f5f9_75%)]' },
    { label: 'White', value: 'white', preview: 'bg-white border border-slate-200' },
    { label: 'Black', value: 'black', preview: 'bg-black' },
    { label: 'Current Theme', value: 'default', preview: 'bg-[#0f172a]' },
];

export const ExportImageModal: React.FC<ExportImageModalProps> = ({
    isOpen,
    onClose,
    viewportWidth,
    viewportHeight,
}) => {
    const [scale, setScale] = useState(2);
    const [bgMode, setBgMode] = useState<BgMode>('transparent');
    const [format, setFormat] = useState<'png' | 'jpeg'>('png');
    const [customWidth, setCustomWidth] = useState<number | null>(null);
    const [customHeight, setCustomHeight] = useState<number | null>(null);
    const [useCustomSize, setUseCustomSize] = useState(false);
    const [isExporting, setIsExporting] = useState(false);

    if (!isOpen) return null;

    const outputW = useCustomSize && customWidth ? customWidth : viewportWidth * scale;
    const outputH = useCustomSize && customHeight ? customHeight : viewportHeight * scale;

    const handleExport = async () => {
        const ext = format === 'jpeg' ? 'jpg' : 'png';
        let path = await safeDialogSave({
            title: 'Export Image',
            filters: [
                { name: format === 'png' ? 'PNG Image' : 'JPEG Image', extensions: format === 'jpeg' ? ['jpg', 'jpeg'] : ['png'] },
            ],
            defaultPath: `crystal_export.${ext}`,
        });

        if (!path) return;

        // Ensure the path has the correct extension
        const pathLower = path.toLowerCase();
        if (format === 'png' && !pathLower.endsWith('.png')) {
            path = path + '.png';
        } else if (format === 'jpeg' && !pathLower.endsWith('.jpg') && !pathLower.endsWith('.jpeg')) {
            path = path + '.jpg';
        }

        setIsExporting(true);
        try {
            await safeInvoke('export_image', {
                path,
                width: outputW,
                height: outputH,
                bgMode,
            });
            onClose();
        } catch (e) {
            alert(`Export failed:\n${e}`);
        } finally {
            setIsExporting(false);
        }
    };

    return (
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-slate-900/40 backdrop-blur-sm animate-in fade-in duration-300">
            <div className="bg-white dark:bg-slate-900 w-full max-w-lg rounded-2xl shadow-2xl border border-slate-200 dark:border-slate-800 overflow-hidden flex flex-col animate-in zoom-in-95 duration-300">
                {/* Header */}
                <div className="px-6 py-4 border-b border-slate-100 dark:border-slate-800 flex items-center justify-between bg-slate-50/50 dark:bg-slate-800/50">
                    <div className="flex items-center gap-3">
                        <div className="p-2 bg-blue-500/10 rounded-lg text-blue-600 dark:text-blue-400">
                            <ImageIcon className="w-5 h-5" />
                        </div>
                        <h2 className="text-lg font-semibold text-slate-900 dark:text-white">
                            Export High-Res Image
                        </h2>
                    </div>
                    <button
                        onClick={onClose}
                        className="p-2 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors"
                    >
                        <X className="w-5 h-5 text-slate-500" />
                    </button>
                </div>

                {/* Content */}
                <div className="p-6 space-y-6">
                    {/* Resolution */}
                    <section className="space-y-3">
                        <h3 className="text-sm font-medium text-slate-500 uppercase tracking-wider">
                            Resolution
                        </h3>

                        <div className="flex items-center gap-2 mb-3">
                            <label className="flex items-center gap-2 cursor-pointer">
                                <input
                                    type="checkbox"
                                    checked={useCustomSize}
                                    onChange={(e) => setUseCustomSize(e.target.checked)}
                                    className="accent-blue-500 w-4 h-4"
                                />
                                <span className="text-sm text-slate-600 dark:text-slate-300">Custom resolution</span>
                            </label>
                        </div>

                        {!useCustomSize ? (
                            <div className="grid grid-cols-4 gap-2">
                                {SCALE_OPTIONS.map((opt) => (
                                    <button
                                        key={opt.value}
                                        onClick={() => setScale(opt.value)}
                                        className={`px-3 py-2 rounded-xl text-xs font-medium transition-all ${scale === opt.value
                                            ? 'bg-blue-500 text-white shadow-lg shadow-blue-500/20'
                                            : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-700'
                                            }`}
                                    >
                                        {opt.label}
                                    </button>
                                ))}
                            </div>
                        ) : (
                            <div className="flex items-center gap-3">
                                <input
                                    type="number"
                                    min={1}
                                    max={16384}
                                    value={customWidth ?? outputW}
                                    onChange={(e) => setCustomWidth(Math.max(1, parseInt(e.target.value) || 1))}
                                    className="w-28 px-3 py-2 bg-slate-100 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl text-sm text-center"
                                    placeholder="Width"
                                />
                                <span className="text-slate-400 text-sm">×</span>
                                <input
                                    type="number"
                                    min={1}
                                    max={16384}
                                    value={customHeight ?? outputH}
                                    onChange={(e) => setCustomHeight(Math.max(1, parseInt(e.target.value) || 1))}
                                    className="w-28 px-3 py-2 bg-slate-100 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl text-sm text-center"
                                    placeholder="Height"
                                />
                                <span className="text-xs text-slate-400">px</span>
                            </div>
                        )}

                        <p className="text-xs text-slate-400 mt-1">
                            Output: <span className="font-mono text-blue-500">{outputW} × {outputH}</span> px
                            {' '}({(outputW * outputH / 1e6).toFixed(1)} MP)
                        </p>
                    </section>

                    {/* Background */}
                    <section className="space-y-3">
                        <h3 className="text-sm font-medium text-slate-500 uppercase tracking-wider">
                            Background
                        </h3>
                        <div className="grid grid-cols-4 gap-3">
                            {BG_OPTIONS.map((opt) => (
                                <button
                                    key={opt.value}
                                    onClick={() => setBgMode(opt.value)}
                                    className={`flex flex-col items-center gap-2 p-3 rounded-xl transition-all ${bgMode === opt.value
                                        ? 'ring-2 ring-blue-500 ring-offset-2 dark:ring-offset-slate-900'
                                        : 'hover:bg-slate-50 dark:hover:bg-slate-800'
                                        }`}
                                >
                                    <div
                                        className={`w-10 h-10 rounded-lg ${opt.preview}`}
                                        style={opt.value === 'transparent' ? { backgroundSize: '12px 12px' } : undefined}
                                    />
                                    <span className="text-[11px] font-medium text-slate-600 dark:text-slate-400">
                                        {opt.label}
                                    </span>
                                </button>
                            ))}
                        </div>
                        {bgMode === 'transparent' && (
                            <p className="text-[11px] text-amber-500">
                                ⚠ Transparency requires PNG format. JPEG will composite onto white.
                            </p>
                        )}
                    </section>

                    {/* Format */}
                    <section className="space-y-3">
                        <h3 className="text-sm font-medium text-slate-500 uppercase tracking-wider">
                            Format
                        </h3>
                        <div className="flex gap-3">
                            <button
                                onClick={() => setFormat('png')}
                                className={`flex-1 px-4 py-2.5 rounded-xl text-sm font-medium transition-all ${format === 'png'
                                    ? 'bg-blue-500 text-white shadow-lg shadow-blue-500/20'
                                    : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-700'
                                    }`}
                            >
                                PNG (Lossless)
                            </button>
                            <button
                                onClick={() => setFormat('jpeg')}
                                className={`flex-1 px-4 py-2.5 rounded-xl text-sm font-medium transition-all ${format === 'jpeg'
                                    ? 'bg-blue-500 text-white shadow-lg shadow-blue-500/20'
                                    : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-700'
                                    }`}
                            >
                                JPEG (Smaller)
                            </button>
                        </div>
                        {format === 'jpeg' && bgMode === 'transparent' && (
                            <p className="text-[11px] text-amber-500">
                                ⚠ JPEG does not support transparency. Background will be composited onto white.
                            </p>
                        )}
                    </section>
                </div>

                {/* Footer */}
                <div className="flex justify-end gap-3 px-6 py-4 border-t border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800/50">
                    <button
                        onClick={onClose}
                        className="px-4 py-2 text-sm font-medium text-slate-600 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-xl transition-colors"
                    >
                        Cancel
                    </button>
                    <button
                        onClick={handleExport}
                        disabled={isExporting}
                        className="px-6 py-2 text-sm font-medium text-white bg-blue-500 hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed rounded-xl shadow-lg shadow-blue-500/20 active:scale-95 transition-all flex items-center gap-2"
                    >
                        {isExporting ? (
                            <>
                                <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                                Rendering...
                            </>
                        ) : (
                            <>
                                <ImageIcon className="w-4 h-4" />
                                Export
                            </>
                        )}
                    </button>
                </div>
            </div>
        </div>
    );
};
