import React from 'react';
import { cn } from '../../utils/cn';
import { Bot, Sparkles } from '../../utils/Icons';

interface LlmAssistantProps {
    isOpen: boolean;
    onClose: () => void;
}

export const LlmAssistant: React.FC<LlmAssistantProps> = ({ isOpen, onClose }) => {
    if (!isOpen) {
        return null;
    }

    return (
        <div className={cn(
            "absolute bottom-10 right-[295px] w-72 z-30",
            "bg-white/90 dark:bg-slate-900/90 backdrop-blur-xl",
            "border border-white/30 dark:border-slate-700/50",
            "rounded-xl shadow-2xl shadow-black/10 dark:shadow-black/30",
            "flex flex-col overflow-hidden pointer-events-auto",
            "animate-in slide-in-from-bottom-2"
        )}>

            {/* Header */}
            <div className="px-3 py-2.5 flex justify-between items-center bg-gradient-to-r from-emerald-500/10 to-transparent border-b border-slate-100 dark:border-slate-800">
                <div className="flex items-center gap-2">
                    <Bot className="w-3.5 h-3.5 text-emerald-600 dark:text-emerald-400" />
                    <h3 className="font-semibold text-xs text-slate-800 dark:text-slate-200">LLM Assistant</h3>
                </div>
                <div className="flex items-center gap-2">
                    <span className="text-[9px] font-mono text-slate-400 bg-slate-100 dark:bg-slate-800 px-1.5 py-0.5 rounded">
                        M9
                    </span>
                    <button
                        onClick={onClose}
                        className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 transition-colors"
                        title="Hide Assistant"
                    >
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} className="w-3.5 h-3.5">
                            <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                    </button>
                </div>
            </div>

            {/* Body / Chat Area */}
            <div className="p-3 flex flex-col gap-2.5">
                <textarea
                    className={cn(
                        "w-full h-20 bg-slate-50 dark:bg-slate-800/50 rounded-md",
                        "border border-slate-200 dark:border-slate-700",
                        "p-2 text-xs text-slate-700 dark:text-slate-300",
                        "outline-none resize-none",
                        "focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500/30",
                        "transition-all placeholder:text-slate-400"
                    )}
                    placeholder="Ask me to build a 2x2x2 supercell or replace Na with K..."
                />

                <div className="flex gap-2">
                    <button className="flex-1 py-1.5 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 rounded-md text-xs font-medium transition-colors border border-slate-200 dark:border-slate-700">
                        Send Command
                    </button>
                    <button className="flex-1 py-1.5 bg-emerald-500 hover:bg-emerald-600 text-white rounded-md text-xs font-medium transition-colors shadow-sm flex items-center justify-center gap-1 active:scale-[0.98]">
                        <Sparkles className="w-2.5 h-2.5" />
                        Ask Assistant
                    </button>
                </div>
            </div>

        </div>
    );
};
