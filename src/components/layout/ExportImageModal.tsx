// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React, { useEffect, useId, useRef, useState } from 'react';
import { Image as ImageIcon, X } from 'lucide-react';
import { IpcException, type ExportImageBackground, type IpcError } from '../../ipc/contracts';
import { safeDialogSave, safeInvoke } from '../../utils/tauri-mock';

interface ExportImageModalProps {
    isOpen: boolean;
    onClose: () => void;
    viewportWidth: number;
    viewportHeight: number;
}

const SCALE_OPTIONS = [
    { label: '1× Screen', value: 1 },
    { label: '2× Hi-DPI', value: 2 },
    { label: '4× Print', value: 4 },
    { label: '8× Ultra', value: 8 },
];

const BG_OPTIONS: { label: string; value: ExportImageBackground; preview: string }[] = [
    { label: 'Transparent', value: 'transparent', preview: 'bg-[conic-gradient(#cbd5e1_25%,#f8fafc_25%_50%,#cbd5e1_50%_75%,#f8fafc_75%)]' },
    { label: 'White', value: 'white', preview: 'border border-[var(--cc-border)] bg-white' },
    { label: 'Black', value: 'black', preview: 'bg-black' },
    { label: 'Current', value: 'default', preview: 'bg-[var(--cc-canvas)]' },
];

export const ExportImageModal: React.FC<ExportImageModalProps> = ({
    isOpen,
    onClose,
    viewportWidth,
    viewportHeight,
}) => {
    const titleId = useId();
    const dialogRef = useRef<HTMLDivElement>(null);
    const previousFocusRef = useRef<HTMLElement | null>(null);
    const onCloseRef = useRef(onClose);
    const busyRef = useRef(false);
    const [scale, setScale] = useState(2);
    const [bgMode, setBgMode] = useState<ExportImageBackground>('transparent');
    const [format, setFormat] = useState<'png' | 'jpeg'>('png');
    const [customWidth, setCustomWidth] = useState<number | null>(null);
    const [customHeight, setCustomHeight] = useState<number | null>(null);
    const [useCustomSize, setUseCustomSize] = useState(false);
    const [isExporting, setIsExporting] = useState(false);
    const [error, setError] = useState<IpcError | null>(null);

    onCloseRef.current = onClose;
    busyRef.current = isExporting;

    useEffect(() => {
        if (!isOpen) return;
        previousFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        setError(null);
        dialogRef.current?.focus();

        const handleKeydown = (event: KeyboardEvent) => {
            if (event.key === 'Escape' && !busyRef.current) onCloseRef.current();
            if (event.key === 'Tab') {
                const focusable = dialogRef.current?.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])');
                if (!focusable || focusable.length === 0) {
                    event.preventDefault();
                    return;
                }
                const first = focusable[0];
                const last = focusable[focusable.length - 1];
                if (event.shiftKey && document.activeElement === first) {
                    event.preventDefault();
                    last.focus();
                } else if (!event.shiftKey && document.activeElement === last) {
                    event.preventDefault();
                    first.focus();
                }
            }
        };
        document.addEventListener('keydown', handleKeydown);
        return () => {
            document.removeEventListener('keydown', handleKeydown);
            previousFocusRef.current?.focus();
        };
    }, [isOpen]);

    if (!isOpen) return null;

    const outputW = useCustomSize && customWidth ? customWidth : viewportWidth * scale;
    const outputH = useCustomSize && customHeight ? customHeight : viewportHeight * scale;
    const hasValidOutputSize = Number.isFinite(outputW) && Number.isFinite(outputH)
        && outputW >= 1 && outputH >= 1 && outputW <= 16384 && outputH <= 16384;

    const setExportError = (cause: unknown) => {
        if (cause instanceof IpcException) {
            setError({ code: cause.code, message: cause.message, recoverable: cause.recoverable });
            return;
        }
        setError({ code: 'internal_error', message: 'Unable to export the image.', recoverable: false });
    };

    const handleExport = async () => {
        if (isExporting || !hasValidOutputSize) return;
        const ext = format === 'jpeg' ? 'jpg' : 'png';
        setError(null);
        setIsExporting(true);
        try {
            let path = await safeDialogSave({
                title: 'Export Image',
                filters: [
                    { name: format === 'png' ? 'PNG Image' : 'JPEG Image', extensions: format === 'jpeg' ? ['jpg', 'jpeg'] : ['png'] },
                ],
                defaultPath: `crystal_export.${ext}`,
            });

            if (!path) return;

            const pathLower = path.toLowerCase();
            if (format === 'png' && !pathLower.endsWith('.png')) {
                path = `${path}.png`;
            } else if (format === 'jpeg' && !pathLower.endsWith('.jpg') && !pathLower.endsWith('.jpeg')) {
                path = `${path}.jpg`;
            }

            await safeInvoke('export_image', {
                path,
                width: outputW,
                height: outputH,
                bgMode,
            });
            onClose();
        } catch (cause) {
            setExportError(cause);
        } finally {
            setIsExporting(false);
        }
    };

    return (
        <div className="pointer-events-auto fixed inset-0 z-[200] flex items-center justify-center bg-black/50 p-4">
            <div
                ref={dialogRef}
                role="dialog"
                aria-modal="true"
                aria-labelledby={titleId}
                aria-busy={isExporting}
                tabIndex={-1}
                className="flex max-h-[80vh] w-full max-w-lg flex-col overflow-hidden rounded border border-[var(--cc-border)] bg-[var(--cc-chrome)] text-[var(--cc-text)] shadow-sm outline-none"
            >
                <header className="flex items-center justify-between border-b border-[var(--cc-border)] px-4 py-3">
                    <div className="flex items-center gap-2">
                        <ImageIcon aria-hidden="true" className="h-4 w-4 text-[var(--cc-accent)]" />
                        <h2 id={titleId} className="text-sm font-semibold">Export High-Resolution Image</h2>
                    </div>
                    <button type="button" onClick={onClose} disabled={isExporting} aria-label="Close export dialog" className="rounded border border-transparent p-1 text-[var(--cc-muted)] transition-colors duration-150 hover:border-[var(--cc-border)] hover:text-[var(--cc-text)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60">
                        <X aria-hidden="true" className="h-4 w-4" />
                    </button>
                </header>

                <div className="space-y-5 overflow-y-auto p-4">
                    <section aria-labelledby="export-resolution" className="space-y-3">
                        <h3 id="export-resolution" className="text-xs font-semibold uppercase tracking-wide text-[var(--cc-muted)]">Resolution</h3>
                        <label className="flex items-center gap-2 text-xs">
                            <input type="checkbox" checked={useCustomSize} disabled={isExporting} onChange={(event) => setUseCustomSize(event.target.checked)} className="accent-[var(--cc-accent)]" />
                            Custom resolution
                        </label>

                        {!useCustomSize ? (
                            <div className="grid grid-cols-4 gap-2">
                                {SCALE_OPTIONS.map((option) => (
                                    <button key={option.value} type="button" onClick={() => setScale(option.value)} disabled={isExporting} aria-pressed={scale === option.value} className="rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-2 py-1.5 text-[11px] font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:opacity-60 aria-pressed:border-[var(--cc-accent)]">
                                        {option.label}
                                    </button>
                                ))}
                            </div>
                        ) : (
                            <div className="flex items-center gap-2">
                                <input aria-label="Output width" type="number" min={1} max={16384} value={customWidth ?? Math.max(1, viewportWidth * scale)} disabled={isExporting} onChange={(event) => setCustomWidth(Math.min(16384, Math.max(1, parseInt(event.target.value, 10) || 1)))} className="w-28 rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-2 py-1.5 text-center text-xs tabular-nums outline-none focus-visible:ring-1 focus-visible:ring-[var(--cc-accent)] disabled:opacity-60" />
                                <span className="text-xs text-[var(--cc-muted)]">×</span>
                                <input aria-label="Output height" type="number" min={1} max={16384} value={customHeight ?? Math.max(1, viewportHeight * scale)} disabled={isExporting} onChange={(event) => setCustomHeight(Math.min(16384, Math.max(1, parseInt(event.target.value, 10) || 1)))} className="w-28 rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-2 py-1.5 text-center text-xs tabular-nums outline-none focus-visible:ring-1 focus-visible:ring-[var(--cc-accent)] disabled:opacity-60" />
                                <span className="text-xs text-[var(--cc-muted)]">px</span>
                            </div>
                        )}
                        <p className="text-xs text-[var(--cc-muted)]">Output: <span className="tabular-nums text-[var(--cc-text)]">{outputW} × {outputH} px ({(outputW * outputH / 1e6).toFixed(1)} MP)</span></p>
                    </section>

                    <section aria-labelledby="export-background" className="space-y-3">
                        <h3 id="export-background" className="text-xs font-semibold uppercase tracking-wide text-[var(--cc-muted)]">Background</h3>
                        <div className="grid grid-cols-4 gap-2">
                            {BG_OPTIONS.map((option) => (
                                <button key={option.value} type="button" onClick={() => setBgMode(option.value)} disabled={isExporting} aria-pressed={bgMode === option.value} className="flex flex-col items-center gap-1 rounded border border-[var(--cc-border)] bg-[var(--cc-field)] p-2 text-[10px] transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:opacity-60 aria-pressed:border-[var(--cc-accent)]">
                                    <span aria-hidden="true" className={`h-8 w-8 rounded ${option.preview}`} style={option.value === 'transparent' ? { backgroundSize: '10px 10px' } : undefined} />
                                    {option.label}
                                </button>
                            ))}
                        </div>
                    </section>

                    <section aria-labelledby="export-format" className="space-y-3">
                        <h3 id="export-format" className="text-xs font-semibold uppercase tracking-wide text-[var(--cc-muted)]">Format</h3>
                        <div className="grid grid-cols-2 gap-2">
                            <button type="button" onClick={() => setFormat('png')} disabled={isExporting} aria-pressed={format === 'png'} className="rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-3 py-1.5 text-xs transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:opacity-60 aria-pressed:border-[var(--cc-accent)]">PNG (lossless)</button>
                            <button type="button" onClick={() => setFormat('jpeg')} disabled={isExporting} aria-pressed={format === 'jpeg'} className="rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-3 py-1.5 text-xs transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:opacity-60 aria-pressed:border-[var(--cc-accent)]">JPEG (smaller)</button>
                        </div>
                        {format === 'jpeg' && bgMode === 'transparent' && <p role="status" className="text-[11px] text-[var(--cc-muted)]">JPEG composites transparent pixels onto white.</p>}
                    </section>

                    {!hasValidOutputSize && <div role="alert" className="rounded border border-[var(--cc-danger)] bg-[var(--cc-panel)] px-2 py-1.5 text-xs">Output dimensions must be between 1 and 16384 px.</div>}
                    {error && <div role="alert" data-error-code={error.code} className="rounded border border-[var(--cc-danger)] bg-[var(--cc-panel)] px-2 py-1.5 text-xs">{error.message}</div>}
                </div>

                <footer className="flex justify-end gap-2 border-t border-[var(--cc-border)] bg-[var(--cc-panel)] px-4 py-3">
                    <button type="button" onClick={onClose} disabled={isExporting} className="rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-3 py-1.5 text-xs font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60">Cancel</button>
                    <button type="button" onClick={() => void handleExport()} disabled={isExporting || !hasValidOutputSize} aria-busy={isExporting} className="rounded border border-[var(--cc-accent)] bg-[var(--cc-accent)] px-3 py-1.5 text-xs font-medium text-white transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60">{isExporting ? 'Rendering…' : 'Export'}</button>
                </footer>
            </div>
        </div>
    );
};
