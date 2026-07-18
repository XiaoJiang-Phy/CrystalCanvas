import React, { useId } from 'react';
import type { IpcError } from '../../ipc/contracts';

type NumberInputProps = {
    label: string;
    value: number;
    onChange: (value: number) => void;
    disabled?: boolean;
    invalid?: boolean;
    busy?: boolean;
};

export const NumberInput = ({ label, value, onChange, disabled = false, invalid = false, busy = false }: NumberInputProps) => {
    const id = useId();

    return (
        <div className="flex-1 space-y-1" data-ui-control="field" aria-busy={busy}>
            <label htmlFor={id} className="text-[11px] font-medium text-[var(--cc-muted)]">{label}</label>
            <input
                id={id}
                type="number"
                value={value}
                onChange={(event) => onChange(parseInt(event.target.value, 10) || 0)}
                min={label.startsWith('N') ? 1 : undefined}
                disabled={disabled || busy}
                aria-invalid={invalid}
                className="w-full rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-2 py-1 text-xs text-[var(--cc-text)] outline-none transition-colors duration-150 focus-visible:border-[var(--cc-accent)] focus-visible:ring-1 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60 tabular-nums"
            />
        </div>
    );
};

type RangeInputProps = {
    label: string;
    value: number;
    displayValue: string;
    min: number;
    max: number;
    step: number;
    onChange: (value: number) => void;
    disabled?: boolean;
    invalid?: boolean;
    busy?: boolean;
};

export const RangeInput = ({ label, value, displayValue, min, max, step, onChange, disabled = false, invalid = false, busy = false }: RangeInputProps) => {
    const id = useId();

    return (
        <div className="space-y-1" data-ui-control="range" aria-busy={busy}>
            <label htmlFor={id} className="flex justify-between text-[11px] font-medium text-[var(--cc-muted)]">
                <span>{label}</span>
                <span className="tabular-nums">{displayValue}</span>
            </label>
            <input
                id={id}
                type="range"
                min={min}
                max={max}
                step={step}
                value={value}
                onChange={(event) => onChange(parseFloat(event.target.value))}
                disabled={disabled || busy}
                aria-invalid={invalid}
                className="w-full accent-[var(--cc-accent)] outline-none focus-visible:ring-1 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60"
            />
        </div>
    );
};

type SelectInputProps = {
    label: string;
    value: string;
    onChange: (value: string) => void;
    children: React.ReactNode;
    disabled?: boolean;
    invalid?: boolean;
    busy?: boolean;
};

export const SelectInput = ({ label, value, onChange, children, disabled = false, invalid = false, busy = false }: SelectInputProps) => {
    const id = useId();

    return (
        <div className="space-y-1" data-ui-control="select" aria-busy={busy}>
            <label htmlFor={id} className="text-[11px] font-medium text-[var(--cc-muted)]">{label}</label>
            <select
                id={id}
                value={value}
                onChange={(event) => onChange(event.target.value)}
                disabled={disabled || busy}
                aria-invalid={invalid}
                className="w-full rounded border border-[var(--cc-border)] bg-[var(--cc-field)] px-2 py-1.5 text-xs text-[var(--cc-text)] outline-none transition-colors duration-150 focus-visible:border-[var(--cc-accent)] focus-visible:ring-1 focus-visible:ring-[var(--cc-accent)] disabled:cursor-not-allowed disabled:opacity-60"
            >
                {children}
            </select>
        </div>
    );
};

type ActionButtonProps = {
    label: string;
    busyLabel?: string;
    onClick?: () => void | Promise<void>;
    disabled?: boolean;
    busy?: boolean;
    tone?: 'primary' | 'secondary' | 'danger';
};

const ACTION_TONES = {
    primary: 'border-[var(--cc-accent)] bg-[var(--cc-accent)] text-white hover:opacity-90',
    secondary: 'border-[var(--cc-border)] bg-[var(--cc-field)] text-[var(--cc-text)] hover:border-[var(--cc-muted)]',
    danger: 'border-[var(--cc-danger)] bg-[var(--cc-danger)] text-white hover:opacity-90',
};

export const ActionButton = ({ label, busyLabel, onClick, disabled = false, busy = false, tone = 'primary' }: ActionButtonProps) => (
    <button
        type="button"
        data-ui-control="action"
        onClick={onClick}
        disabled={disabled || busy}
        aria-busy={busy}
        className={`w-full rounded border px-3 py-1.5 text-xs font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--cc-accent)] focus-visible:ring-offset-1 disabled:cursor-not-allowed disabled:opacity-60 ${ACTION_TONES[tone]}`}
    >
        {busy ? (busyLabel ?? `${label}…`) : label}
    </button>
);

export const PanelError = ({ error, message = error.message }: { error: IpcError; message?: string }) => (
    <div role="alert" data-error-code={error.code} className="rounded border border-[var(--cc-danger)] bg-[var(--cc-panel)] px-2 py-1.5 text-[11px] text-[var(--cc-text)]">
        <span className="mr-1 font-medium uppercase text-[var(--cc-danger)]">{error.code.replace('_', ' ')}</span>
        <span>{message}</span>
    </div>
);
