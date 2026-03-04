import React, { useState, useEffect } from 'react';

interface PromptModalProps {
    isOpen: boolean;
    title: string;
    description?: string;
    placeholder?: string;
    initialValue?: string;
    onClose: () => void;
    onSubmit: (value: string) => void;
}

export const PromptModal: React.FC<PromptModalProps> = ({
    isOpen,
    title,
    description,
    placeholder,
    initialValue = "",
    onClose,
    onSubmit
}) => {
    const [value, setValue] = useState(initialValue);

    useEffect(() => {
        if (isOpen) {
            setValue(initialValue);
        }
    }, [isOpen, initialValue]);

    if (!isOpen) return null;

    const handleSubmit = (e: React.FormEvent) => {
        e.preventDefault();
        onSubmit(value);
        onClose();
    };

    return (
        <div className="fixed inset-0 z-[200] flex items-center justify-center bg-black/40 backdrop-blur-sm pointer-events-auto">
            <div className="bg-white dark:bg-slate-900 rounded-xl shadow-2xl p-6 w-96 max-w-full border border-slate-200 dark:border-slate-800 transform animate-in zoom-in-95 duration-200">
                <h2 className="text-xl font-semibold text-slate-900 dark:text-slate-100 mb-2">{title}</h2>
                {description && <p className="text-sm text-slate-500 dark:text-slate-400 mb-4">{description}</p>}

                <form onSubmit={handleSubmit}>
                    <input
                        autoFocus
                        type="text"
                        value={value}
                        onChange={(e) => setValue(e.target.value)}
                        placeholder={placeholder}
                        className="w-full px-4 py-2 bg-slate-50 dark:bg-slate-800 border-2 border-slate-200 dark:border-slate-700 rounded-lg text-slate-900 dark:text-slate-100 focus:outline-none focus:border-emerald-500 focus:bg-white dark:focus:bg-slate-900 transition-colors mb-6"
                    />

                    <div className="flex justify-end gap-3">
                        <button
                            type="button"
                            onClick={onClose}
                            className="px-4 py-2 rounded-lg text-sm font-medium text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
                        >
                            Cancel
                        </button>
                        <button
                            type="submit"
                            className="px-4 py-2 rounded-lg text-sm font-medium text-white bg-emerald-500 hover:bg-emerald-600 transition-colors shadow-sm"
                        >
                            Confirm
                        </button>
                    </div>
                </form>
            </div>
        </div>
    );
};
