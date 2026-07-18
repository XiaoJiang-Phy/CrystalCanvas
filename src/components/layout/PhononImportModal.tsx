import React, { useEffect, useId, useRef, useState } from 'react';
import { safeDialogOpen } from '../../utils/tauri-mock';

interface PhononImportModalProps {
    isOpen: boolean;
    onClose: () => void;
    onSubmit: (paths: { scfIn: string; scfOut: string; modes: string; axsf: string }) => void | Promise<void>;
}

export const PhononImportModal: React.FC<PhononImportModalProps> = ({ isOpen, onClose, onSubmit }) => {
    const titleId = useId();
    const dialogRef = useRef<HTMLDivElement>(null);
    const previousFocusRef = useRef<HTMLElement | null>(null);
    const onCloseRef = useRef(onClose);
    const busyRef = useRef(false);
    const [scfIn, setScfIn] = useState('');
    const [scfOut, setScfOut] = useState('');
    const [modes, setModes] = useState('');
    const [axsf, setAxsf] = useState('');
    const [selectingField, setSelectingField] = useState<string | null>(null);
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    onCloseRef.current = onClose;
    busyRef.current = selectingField !== null || isSubmitting;

    useEffect(() => {
        if (!isOpen) return;
        previousFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        setScfIn('');
        setScfOut('');
        setModes('');
        setAxsf('');
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

    const handleSelectFile = async (field: string, setter: (value: string) => void, title: string, extensions: string[]) => {
        if (busyRef.current) return;
        setError(null);
        setSelectingField(field);
        try {
            const path = await safeDialogOpen({
                title,
                filters: [{ name: title, extensions }],
                multiple: false,
                directory: false,
            });
            if (path && typeof path === 'string') {
                setter(path);
            }
        } catch (cause) {
            setError(cause instanceof Error ? cause.message : `Unable to select ${title}.`);
        } finally {
            setSelectingField(null);
        }
    };

    const canSubmit = Boolean(axsf || (scfIn && scfOut && modes));
    const isBusy = selectingField !== null || isSubmitting;

    const handleSubmit = async () => {
        if (!canSubmit || isBusy) return;
        setError(null);
        setIsSubmitting(true);
        try {
            await onSubmit({ scfIn, scfOut, modes, axsf });
        } catch (cause) {
            setError(cause instanceof Error ? cause.message : 'Unable to load the selected phonon data.');
        } finally {
            setIsSubmitting(false);
        }
    };

    const fileRow = (
        id: string,
        label: string,
        value: string,
        setter: (value: string) => void,
        title: string,
        extensions: string[],
    ) => (
        <div className="space-y-1">
            <label htmlFor={id} className="text-xs font-medium text-[var(--cc-text)]">{label}</label>
            <div className="flex gap-2">
                <input
                    id={id}
                    type="text"
                    readOnly
                    value={value}
                    placeholder={`Select ${label}…`}
                    className="min-w-0 flex-1 rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-2 py-1.5 text-xs text-[var(--cc-text)] outline-none"
                />
                <button
                    type="button"
                    onClick={() => void handleSelectFile(id, setter, title, extensions)}
                    disabled={isBusy}
                    aria-busy={selectingField === id}
                    className="rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-3 py-1.5 text-xs font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60"
                >
                    {selectingField === id ? 'Selecting…' : 'Browse'}
                </button>
            </div>
        </div>
    );

    return (
        <div className="pointer-events-auto fixed inset-0 z-[200] flex items-center justify-center bg-black/50 p-4">
            <div
                ref={dialogRef}
                role="dialog"
                aria-modal="true"
                aria-labelledby={titleId}
                aria-busy={isBusy}
                tabIndex={-1}
                className="w-full max-w-lg overflow-hidden rounded border border-[var(--cc-border)] bg-[var(--cc-chrome)] text-[var(--cc-text)] shadow-sm outline-none"
            >
                <header className="border-b border-[var(--cc-border)] px-4 py-3">
                    <h2 id={titleId} className="text-sm font-semibold">Load Phonon Data</h2>
                    <p className="mt-1 text-xs text-[var(--cc-muted)]">Select a Quantum ESPRESSO file set or one AXSF animation file.</p>
                </header>

                <div className="max-h-[65vh] space-y-4 overflow-y-auto p-4">
                    {fileRow('phonon-scf-in', 'scf.in', scfIn, setScfIn, 'Select scf.in', ['in', 'txt', ''])}
                    {fileRow('phonon-scf-out', 'scf.out', scfOut, setScfOut, 'Select scf.out', ['out', 'txt', ''])}
                    {fileRow('phonon-modes', 'matdyn.modes or dynmat.dat', modes, setModes, 'Select modes file', ['modes', 'dat', 'mold', ''])}

                    <div className="flex items-center gap-3" aria-hidden="true">
                        <div className="h-px flex-1 bg-[var(--cc-border)]" />
                        <span className="text-[10px] font-medium uppercase text-[var(--cc-muted)]">or</span>
                        <div className="h-px flex-1 bg-[var(--cc-border)]" />
                    </div>

                    {fileRow('phonon-axsf', 'AXSF animation file', axsf, setAxsf, 'Select AXSF file', ['axsf', ''])}
                    <p className="text-[11px] text-[var(--cc-muted)]">When AXSF is selected, the Quantum ESPRESSO inputs are ignored.</p>
                    {error && <div role="alert" className="rounded border border-[var(--cc-danger)] bg-[var(--cc-panel)] px-2 py-1.5 text-xs">{error}</div>}
                </div>

                <footer className="flex justify-end gap-2 border-t border-[var(--cc-border)] bg-[var(--cc-panel)] px-4 py-3">
                    <button
                        type="button"
                        onClick={onClose}
                        disabled={isBusy}
                        className="rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-3 py-1.5 text-xs font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60"
                    >
                        Cancel
                    </button>
                    <button
                        type="button"
                        onClick={() => void handleSubmit()}
                        disabled={!canSubmit || isBusy}
                        className="rounded border border-[var(--cc-accent)] bg-[var(--cc-accent)] px-3 py-1.5 text-xs font-medium text-white transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60"
                    >
                        {isSubmitting ? 'Loading…' : 'Load Data'}
                    </button>
                </footer>
            </div>
        </div>
    );
};
