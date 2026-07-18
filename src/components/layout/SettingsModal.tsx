// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React, { useEffect, useId, useRef, useState } from 'react';
import { Palette, Settings as SettingsIcon, Sliders, X, Zap } from 'lucide-react';
import { IpcException, type AppSettingsDto, type IpcError } from '../../ipc/contracts';
import { cn } from '../../utils/cn';
import { safeInvoke } from '../../utils/tauri-mock';

interface SettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
    elements: string[];
}

export const SettingsModal: React.FC<SettingsModalProps> = ({ isOpen, onClose, elements }) => {
    const titleId = useId();
    const dialogRef = useRef<HTMLDivElement>(null);
    const previousFocusRef = useRef<HTMLElement | null>(null);
    const onCloseRef = useRef(onClose);
    const busyRef = useRef(false);
    const [settings, setSettings] = useState<AppSettingsDto | null>(null);
    const [activeTab, setActiveTab] = useState<'general' | 'bonds' | 'elements'>('general');
    const [isLoading, setIsLoading] = useState(false);
    const [activeOperation, setActiveOperation] = useState<'apply' | 'ok' | null>(null);
    const [error, setError] = useState<IpcError | null>(null);

    const isSaving = activeOperation !== null;
    const isBusy = isLoading || isSaving;
    onCloseRef.current = onClose;
    busyRef.current = isSaving;

    const setModalError = (cause: unknown, fallback: string) => {
        if (cause instanceof IpcException) {
            setError({ code: cause.code, message: cause.message, recoverable: cause.recoverable });
            return;
        }
        setError({ code: 'internal_error', message: fallback, recoverable: false });
    };

    useEffect(() => {
        if (!isOpen) return;
        previousFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
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

    useEffect(() => {
        if (!isOpen) return;
        setSettings(null);
        setActiveTab('general');
        setError(null);
        setIsLoading(true);
        let active = true;
        safeInvoke('get_settings')
            .then((nextSettings) => {
                if (active) setSettings(nextSettings ?? null);
            })
            .catch((cause) => {
                if (active) setModalError(cause, 'Unable to load application settings.');
            })
            .finally(() => {
                if (active) setIsLoading(false);
            });
        return () => {
            active = false;
        };
    }, [isOpen]);

    if (!isOpen) return null;

    const handleApply = async () => {
        if (!settings || isBusy) return;
        setError(null);
        setActiveOperation('apply');
        try {
            await safeInvoke('update_settings', { newSettings: settings });
        } catch (cause) {
            setModalError(cause, 'Unable to apply application settings.');
        } finally {
            setActiveOperation(null);
        }
    };

    const handleOk = async () => {
        if (!settings || isBusy) return;
        setError(null);
        setActiveOperation('ok');
        try {
            await safeInvoke('update_settings', { newSettings: settings });
            onClose();
        } catch (cause) {
            setModalError(cause, 'Unable to update application settings.');
        } finally {
            setActiveOperation(null);
        }
    };

    const updateColor = (element: string, hex: string) => {
        if (!settings) return;
        const r = parseInt(hex.slice(1, 3), 16) / 255;
        const g = parseInt(hex.slice(3, 5), 16) / 255;
        const b = parseInt(hex.slice(5, 7), 16) / 255;
        setSettings({
            ...settings,
            custom_atom_colors: {
                ...settings.custom_atom_colors,
                [element]: [r, g, b, 1],
            },
        });
    };

    const rgbToHex = (rgba: [number, number, number, number]) => {
        const r = Math.round(rgba[0] * 255).toString(16).padStart(2, '0');
        const g = Math.round(rgba[1] * 255).toString(16).padStart(2, '0');
        const b = Math.round(rgba[2] * 255).toString(16).padStart(2, '0');
        return `#${r}${g}${b}`;
    };

    return (
        <div className="pointer-events-auto fixed inset-0 z-[200] flex items-center justify-center bg-black/50 p-4">
            <div
                ref={dialogRef}
                role="dialog"
                aria-modal="true"
                aria-labelledby={titleId}
                aria-busy={isBusy}
                tabIndex={-1}
                className="flex max-h-[80vh] w-full max-w-2xl flex-col overflow-hidden rounded border border-[var(--cc-border)] bg-[var(--cc-chrome)] text-[var(--cc-text)] shadow-sm outline-none"
            >
                <header className="flex items-center justify-between border-b border-[var(--cc-border)] px-4 py-3">
                    <div className="flex items-center gap-2">
                        <SettingsIcon aria-hidden="true" className="h-4 w-4 text-[var(--cc-accent)]" />
                        <h2 id={titleId} className="text-sm font-semibold">CrystalCanvas Settings</h2>
                    </div>
                    <button
                        type="button"
                        onClick={onClose}
                        disabled={isSaving}
                        aria-label="Close settings"
                        className="rounded border border-transparent p-1 text-[var(--cc-muted)] transition-colors duration-150 hover:border-[var(--cc-border)] hover:text-[var(--cc-text)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60"
                    >
                        <X aria-hidden="true" className="h-4 w-4" />
                    </button>
                </header>

                <div className="flex min-h-0 flex-1">
                    <nav aria-label="Settings categories" className="w-40 shrink-0 space-y-1 border-r border-[var(--cc-border)] bg-[var(--cc-panel)] p-2">
                        <TabButton active={activeTab === 'general'} onClick={() => setActiveTab('general')} icon={<Sliders className="h-4 w-4" />} label="General" />
                        <TabButton active={activeTab === 'bonds'} onClick={() => setActiveTab('bonds')} icon={<Zap className="h-4 w-4" />} label="Bonds" />
                        <TabButton active={activeTab === 'elements'} onClick={() => setActiveTab('elements')} icon={<Palette className="h-4 w-4" />} label="Elements" />
                    </nav>

                    <div className="min-w-0 flex-1 overflow-y-auto p-4">
                        {error && (
                            <div role="alert" data-error-code={error.code} className="mb-3 rounded border border-[var(--cc-danger)] bg-[var(--cc-panel)] px-2 py-1.5 text-xs">
                                {error.message}
                            </div>
                        )}
                        {isLoading && <div role="status" className="text-xs text-[var(--cc-muted)]">Loading settings…</div>}
                        {!isLoading && !settings && !error && <div role="status" className="text-xs text-[var(--cc-muted)]">Settings are unavailable.</div>}

                        {settings && activeTab === 'general' && (
                            <section aria-labelledby="settings-general" className="space-y-3">
                                <h3 id="settings-general" className="text-xs font-semibold uppercase tracking-wide text-[var(--cc-muted)]">Atom display</h3>
                                <label className="block space-y-1 text-xs">
                                    <span className="flex justify-between"><span>Global atom scale</span><span className="tabular-nums text-[var(--cc-muted)]">{settings.atom_scale.toFixed(2)}×</span></span>
                                    <input type="range" min="0.1" max="2" step="0.05" value={settings.atom_scale} disabled={isBusy} onChange={(event) => setSettings({ ...settings, atom_scale: parseFloat(event.target.value) })} className="w-full accent-[var(--cc-accent)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--cc-accent)] disabled:opacity-60" />
                                </label>
                            </section>
                        )}

                        {settings && activeTab === 'bonds' && (
                            <section aria-labelledby="settings-bonds" className="space-y-4">
                                <h3 id="settings-bonds" className="text-xs font-semibold uppercase tracking-wide text-[var(--cc-muted)]">Bonding parameters</h3>
                                <label className="block space-y-1 text-xs">
                                    <span className="flex justify-between"><span>Search tolerance (Å)</span><span className="tabular-nums text-[var(--cc-muted)]">+{settings.bond_tolerance.toFixed(2)}</span></span>
                                    <input type="range" min="0.1" max="1.5" step="0.05" value={settings.bond_tolerance} disabled={isBusy} onChange={(event) => setSettings({ ...settings, bond_tolerance: parseFloat(event.target.value) })} className="w-full accent-[var(--cc-accent)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--cc-accent)] disabled:opacity-60" />
                                </label>
                                <label className="block space-y-1 text-xs">
                                    <span className="flex justify-between"><span>Bond visual radius (Å)</span><span className="tabular-nums text-[var(--cc-muted)]">{settings.bond_radius.toFixed(2)}</span></span>
                                    <input type="range" min="0.01" max="0.3" step="0.01" value={settings.bond_radius} disabled={isBusy} onChange={(event) => setSettings({ ...settings, bond_radius: parseFloat(event.target.value) })} className="w-full accent-[var(--cc-accent)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--cc-accent)] disabled:opacity-60" />
                                </label>
                                <label className="flex items-center justify-between text-xs">
                                    <span>Bond color</span>
                                    <input
                                        type="color"
                                        value={rgbToHex(settings.bond_color)}
                                        disabled={isBusy}
                                        onChange={(event) => {
                                            const hex = event.target.value;
                                            const r = parseInt(hex.slice(1, 3), 16) / 255;
                                            const g = parseInt(hex.slice(3, 5), 16) / 255;
                                            const b = parseInt(hex.slice(5, 7), 16) / 255;
                                            setSettings({ ...settings, bond_color: [r, g, b, 1] });
                                        }}
                                        className="h-7 w-9 cursor-pointer rounded border border-[var(--cc-border)] bg-[var(--cc-field)] disabled:cursor-not-allowed disabled:opacity-60"
                                    />
                                </label>
                            </section>
                        )}

                        {settings && activeTab === 'elements' && (
                            <section aria-labelledby="settings-elements" className="space-y-3">
                                <h3 id="settings-elements" className="text-xs font-semibold uppercase tracking-wide text-[var(--cc-muted)]">Element customization</h3>
                                <div className="grid grid-cols-2 gap-2">
                                    {elements.map((element) => {
                                        const currentColor: [number, number, number, number] = settings.custom_atom_colors[element] || [0.5, 0.5, 0.5, 1];
                                        return (
                                            <label key={element} className="flex items-center justify-between rounded border border-[var(--cc-border)] bg-[var(--cc-panel)] p-2 text-xs">
                                                <span className="font-mono">{element}</span>
                                                <input type="color" value={rgbToHex(currentColor)} disabled={isBusy} onChange={(event) => updateColor(element, event.target.value)} className="h-7 w-9 cursor-pointer rounded border border-[var(--cc-border)] bg-[var(--cc-field)] disabled:cursor-not-allowed disabled:opacity-60" />
                                            </label>
                                        );
                                    })}
                                    {elements.length === 0 && <p className="col-span-2 text-xs text-[var(--cc-muted)]">No structure elements are available.</p>}
                                </div>
                            </section>
                        )}
                    </div>
                </div>

                <footer className="flex justify-end gap-2 border-t border-[var(--cc-border)] bg-[var(--cc-panel)] px-4 py-3">
                    <button type="button" onClick={onClose} disabled={isSaving} className="rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-3 py-1.5 text-xs font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60">Cancel</button>
                    <button type="button" onClick={() => void handleApply()} disabled={!settings || isBusy} aria-busy={activeOperation === 'apply'} className="rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-3 py-1.5 text-xs font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60">{activeOperation === 'apply' ? 'Applying…' : 'Apply'}</button>
                    <button type="button" onClick={() => void handleOk()} disabled={!settings || isBusy} aria-busy={activeOperation === 'ok'} className="rounded border border-[var(--cc-accent)] bg-[var(--cc-accent)] px-3 py-1.5 text-xs font-medium text-white transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60">{activeOperation === 'ok' ? 'Saving…' : 'OK'}</button>
                </footer>
            </div>
        </div>
    );
};

const TabButton = ({ active, onClick, icon, label }: { active: boolean; onClick: () => void; icon: React.ReactNode; label: string }) => (
    <button
        type="button"
        onClick={onClick}
        aria-pressed={active}
        className={cn(
            'flex w-full items-center gap-2 rounded border px-3 py-2 text-xs font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)]',
            active
                ? 'border-[var(--cc-border)] bg-[var(--cc-field)] text-[var(--cc-text)]'
                : 'border-transparent text-[var(--cc-muted)] hover:border-[var(--cc-border)] hover:text-[var(--cc-text)]',
        )}
    >
        <span aria-hidden="true">{icon}</span>
        {label}
    </button>
);
