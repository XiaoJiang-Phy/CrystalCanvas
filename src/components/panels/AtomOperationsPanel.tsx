import React, { useState } from 'react';
import { IpcException, type IpcError } from '../../ipc/contracts';
import { safeInvoke } from '../../utils/tauri-mock';
import { PromptModal } from '../layout/PromptModal';
import { ActionButton, PanelError } from './shared';
import { PanelProps } from './index';

export default function AtomOperationsPanel({ crystalState, selectedAtoms = [], onSelectionChange }: PanelProps) {
    const [promptConfig, setPromptConfig] = useState<{
        isOpen: boolean;
        title: string;
        description?: string;
        placeholder?: string;
        initialValue?: string;
        onSubmit: (value: string) => void;
    }>({ isOpen: false, title: '', onSubmit: () => {} });
    const [error, setError] = useState<IpcError | null>(null);
    const [activeOperation, setActiveOperation] = useState<'delete' | 'replace' | null>(null);

    const setMutationError = (cause: unknown, fallback: string) => {
        if (cause instanceof IpcException) {
            setError({ code: cause.code, message: cause.message, recoverable: cause.recoverable });
            return;
        }
        setError({ code: 'internal_error', message: fallback, recoverable: false });
    };

    const handleDeleteAtom = async () => {
        if (selectedAtoms.length === 0 || activeOperation) return;
        setError(null);
        setActiveOperation('delete');
        try {
            await safeInvoke('delete_atoms', { indices: selectedAtoms });
            onSelectionChange?.([]);
        } catch (cause) {
            setMutationError(cause, 'Unable to delete the selected atoms.');
        } finally {
            setActiveOperation(null);
        }
    };

    const handleReplacementSubmit = async (newElem: string) => {
        if (!newElem || newElem.trim().length === 0 || activeOperation) return;
        setError(null);
        setActiveOperation('replace');
        try {
            await safeInvoke('substitute_atoms', {
                indices: selectedAtoms,
                newElementSymbol: newElem.trim(),
                newAtomicNumber: 0
            });
        } catch (cause) {
            setMutationError(cause, 'Unable to replace the selected atoms.');
            throw cause;
        } finally {
            setActiveOperation(null);
        }
    };

    const handleReplaceAtom = () => {
        if (selectedAtoms.length === 0 || activeOperation) return;
        setPromptConfig({
            isOpen: true,
            title: 'Replace Atom(s)',
            description: 'Enter new element symbol (e.g., Fe, O, C):',
            placeholder: 'Element symbol',
            onSubmit: handleReplacementSubmit,
        });
    };

    const hasSelection = selectedAtoms.length > 0;
    const isBusy = activeOperation !== null;

    return (
        <div className="space-y-3">
            <dl className="grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 text-xs">
                <dt className="text-[var(--cc-muted)]">Selected</dt>
                <dd className="font-medium text-[var(--cc-text)]">
                    {hasSelection ? (selectedAtoms.length === 1 ? `Atom #${selectedAtoms[0]}` : `${selectedAtoms.length} atoms`) : 'None'}
                </dd>
                <dt className="text-[var(--cc-muted)]">Element</dt>
                <dd className="font-medium text-[var(--cc-text)]">
                    {selectedAtoms.length === 1 && crystalState ? crystalState.elements?.[selectedAtoms[0]] : (selectedAtoms.length > 1 ? 'Mixed' : '—')}
                </dd>
            </dl>

            {error && <PanelError error={error} message={error.message} />}

            <div className="space-y-2">
                <ActionButton label="Replace Atom(s)" onClick={handleReplaceAtom} disabled={!hasSelection || isBusy} busy={activeOperation === 'replace'} />
                <ActionButton label="Delete Atom(s)" onClick={handleDeleteAtom} disabled={!hasSelection || isBusy} busy={activeOperation === 'delete'} tone="danger" />
                <ActionButton label="Add Sub-Atom" disabled tone="secondary" />
            </div>

            <PromptModal
                {...promptConfig}
                onClose={() => setPromptConfig((current) => ({ ...current, isOpen: false }))}
            />
        </div>
    );
}
