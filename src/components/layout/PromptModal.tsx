import React, { useEffect, useId, useRef, useState } from 'react';

interface PromptModalProps {
    isOpen: boolean;
    title: string;
    description?: string;
    placeholder?: string;
    initialValue?: string;
    onClose: () => void;
    onSubmit: (value: string) => void | Promise<void>;
}

export const PromptModal: React.FC<PromptModalProps> = ({
    isOpen,
    title,
    description,
    placeholder,
    initialValue = '',
    onClose,
    onSubmit,
}) => {
    const titleId = useId();
    const descriptionId = useId();
    const dialogRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLInputElement>(null);
    const previousFocusRef = useRef<HTMLElement | null>(null);
    const onCloseRef = useRef(onClose);
    const busyRef = useRef(false);
    const [value, setValue] = useState(initialValue);
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    onCloseRef.current = onClose;
    busyRef.current = isSubmitting;

    useEffect(() => {
        if (!isOpen) return;
        previousFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        setValue(initialValue);
        setError(null);
        inputRef.current?.focus();

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
    }, [initialValue, isOpen]);

    if (!isOpen) return null;

    const handleSubmit = async (event: React.FormEvent) => {
        event.preventDefault();
        if (isSubmitting) return;
        setError(null);
        setIsSubmitting(true);
        try {
            await onSubmit(value);
            onClose();
        } catch (cause) {
            setError(cause instanceof Error ? cause.message : 'Unable to submit this value.');
        } finally {
            setIsSubmitting(false);
        }
    };

    return (
        <div className="pointer-events-auto fixed inset-0 z-[200] flex items-center justify-center bg-black/50 p-4">
            <div
                ref={dialogRef}
                role="dialog"
                aria-modal="true"
                aria-labelledby={titleId}
                aria-describedby={description ? descriptionId : undefined}
                aria-busy={isSubmitting}
                tabIndex={-1}
                className="w-full max-w-sm rounded border border-[var(--cc-border)] bg-[var(--cc-chrome)] p-4 text-[var(--cc-text)] shadow-sm outline-none"
            >
                <h2 id={titleId} className="text-sm font-semibold">{title}</h2>
                {description && <p id={descriptionId} className="mt-1 text-xs text-[var(--cc-muted)]">{description}</p>}

                <form onSubmit={handleSubmit} className="mt-4 space-y-4">
                    <input
                        ref={inputRef}
                        autoFocus
                        type="text"
                        value={value}
                        onChange={(event) => setValue(event.target.value)}
                        placeholder={placeholder}
                        disabled={isSubmitting}
                        className="w-full rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-3 py-2 text-sm text-[var(--cc-text)] outline-none transition-colors duration-150 focus-visible:border-[var(--cc-accent)] focus-visible:ring-1 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60"
                    />

                    {error && <div role="alert" className="rounded border border-[var(--cc-danger)] bg-[var(--cc-panel)] px-2 py-1.5 text-xs">{error}</div>}

                    <div className="flex justify-end gap-2">
                        <button
                            type="button"
                            onClick={onClose}
                            disabled={isSubmitting}
                            className="rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-3 py-1.5 text-xs font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60"
                        >
                            Cancel
                        </button>
                        <button
                            type="submit"
                            disabled={isSubmitting}
                            className="rounded border border-[var(--cc-accent)] bg-[var(--cc-accent)] px-3 py-1.5 text-xs font-medium text-white transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60"
                        >
                            {isSubmitting ? 'Confirming…' : 'Confirm'}
                        </button>
                    </div>
                </form>
            </div>
        </div>
    );
};
