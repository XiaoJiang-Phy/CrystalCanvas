// [Overview: Top navigation bar for main actions and view toggles.]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React from 'react';
import { useTheme } from './Shell';
import { cn } from '../../utils/cn';
import { MousePointerClick, Move, Rotate3D, BoxSelection, Sun, Moon, Sparkles, Settings, Bot } from '../../utils/Icons';

import { safeInvoke } from '../../utils/tauri-mock';

interface TopNavBarProps {
    showAssistant: boolean;
    onToggleAssistant: () => void;
    showLabels: boolean;
    onToggleLabels: () => void;
    interactionMode: 'select' | 'move' | 'rotate' | 'measure';
    setInteractionMode: (mode: 'select' | 'move' | 'rotate' | 'measure') => void;
    onOpenSettings: () => void;
}

import logoUrl from '../../assets/logo.svg';

export const TopNavBar: React.FC<TopNavBarProps> = ({
    showAssistant, onToggleAssistant,
    showLabels, onToggleLabels,
    interactionMode, setInteractionMode,
    onOpenSettings
}) => {
    const { theme, toggleTheme } = useTheme();

    return (
        <div
            className={cn(
                "cc-chrome w-full h-12 grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center px-4 pl-[80px] shrink-0 relative",
                "border-b z-50 pointer-events-auto"
            )}>

            {/* Drag region layer: sits behind all buttons so clicking buttons works normally */}
            <div
                data-tauri-drag-region
                className="absolute inset-0 z-0"
            />

            {/* Left: Brand + Basic Tools */}
            <div className="flex min-w-0 items-center gap-3 relative z-10">
                <div data-command-group="brand-global" className="flex shrink-0 items-center gap-2 font-semibold text-base tracking-tight text-emerald-600 dark:text-emerald-400">
                    <img src={logoUrl} className="w-5 h-5 object-contain" alt="Logo" />
                    <div className="flex items-baseline gap-1.5">
                        <span>CrystalCanvas</span>
                        <span className="px-1.5 py-0.5 rounded text-[9px] font-bold uppercase tracking-wider bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-400 border border-amber-200 dark:border-amber-800/50 select-none">Alpha</span>
                    </div>
                </div>

                <div className="h-6 w-px bg-[var(--cc-border)]" />

                {/* Tool Group */}
                <div data-command-group="interaction" className="flex shrink-0 items-center gap-0.5">
                    <ToolButton icon={<MousePointerClick className="w-3.5 h-3.5" />} active={interactionMode === 'select'} onClick={() => setInteractionMode('select')} tooltip="Select" />
                    <ToolButton icon={<Move className="w-3.5 h-3.5" />} active={interactionMode === 'move'} onClick={() => setInteractionMode('move')} tooltip="Move" />
                    <ToolButton icon={<Rotate3D className="w-3.5 h-3.5" />} active={interactionMode === 'rotate'} onClick={() => setInteractionMode('rotate')} tooltip="Rotate" />
                    <ToolButton icon={<BoxSelection className="w-3.5 h-3.5" />} active={interactionMode === 'measure'} onClick={() => setInteractionMode('measure')} tooltip="Measure/Select" />
                </div>
            </div>

            {/* Center: Axis View Buttons */}
            <div data-command-group="view" className="flex shrink-0 items-center gap-0.5 relative z-10">
                <ViewButton label="a" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'a' })} />
                <ViewButton label="b" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'b' })} />
                <ViewButton label="c" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'c' })} />
                <ViewButton label="a*" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'a_star' })} />
                <ViewButton label="b*" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'b_star' })} />
                <ViewButton label="c*" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'c_star' })} />
            </div>

            {/* Right: Toggles & Actions */}
            <div data-command-group="application" className="flex min-w-0 items-center justify-end gap-1 relative z-10">
                <NavButton label="Reset View" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'reset' })} />
                <NavButton label="Symmetry" />

                <button
                    type="button"
                    className="flex items-center gap-1.5 cursor-pointer"
                    onClick={onToggleLabels}
                    data-tauri-drag-region="false"
                    aria-pressed={showLabels}
                >
                    <span className="text-xs font-medium select-none">Labels</span>
                    <ToggleSwitch checked={showLabels} />
                </button>

                <div className="mx-1 h-4 w-px bg-[var(--cc-border)]" />

                {/* LLM Assistant Toggle */}
                <button
                    onClick={onToggleAssistant}
                    data-tauri-drag-region="false"
                    className={cn(
                        "p-1.5 rounded-lg transition-colors",
                        showAssistant
                            ? "bg-emerald-100 dark:bg-emerald-900/40 text-emerald-600 dark:text-emerald-400"
                            : "hover:bg-slate-200 dark:hover:bg-slate-800 text-slate-500"
                    )}
                    aria-label="Toggle LLM Assistant"
                    aria-pressed={showAssistant}
                    title="Toggle LLM Assistant"
                >
                    <Bot className="w-3.5 h-3.5" />
                </button>

                <button
                    onClick={toggleTheme}
                    data-tauri-drag-region="false"
                    className="p-1.5 rounded-lg hover:bg-slate-200 dark:hover:bg-slate-800 transition-colors"
                    aria-label="Toggle Theme"
                    title="Toggle Theme"
                >
                    {theme === 'dark' ? <Sun className="w-3.5 h-3.5" /> : <Moon className="w-3.5 h-3.5" />}
                </button>

                <button
                    onClick={onOpenSettings}
                    data-tauri-drag-region="false"
                    className="p-1.5 rounded-lg hover:bg-slate-200 dark:hover:bg-slate-800 transition-colors"
                    aria-label="Open Settings"
                    title="Open Settings"
                >
                    <Settings className="w-3.5 h-3.5" />
                </button>
            </div>

        </div>
    );
};

// --- Subcomponents ---

const ToolButton = ({ icon, active = false, tooltip, onClick }: { icon: React.ReactNode, active?: boolean, tooltip: string, onClick?: () => void }) => (
    <button
        onClick={onClick}
        data-tauri-drag-region="false"
        className={cn(
            "p-1.5 rounded-md transition-colors duration-150",
            active
                ? "bg-slate-200/80 dark:bg-slate-700/80 text-emerald-600 dark:text-emerald-400"
                : "text-slate-500 hover:text-slate-900 dark:hover:text-slate-100 hover:bg-slate-200/50 dark:hover:bg-slate-700/50"
        )}
        aria-label={tooltip}
        aria-pressed={active}
        title={tooltip}
    >
        {icon}
    </button>
);

const ViewButton = ({ label, onClick }: { label: string, onClick?: () => void }) => (
    <button onClick={onClick} data-tauri-drag-region="false" className="w-7 h-7 flex items-center justify-center text-xs font-mono font-medium rounded-md text-slate-600 dark:text-slate-300 hover:bg-slate-200/70 hover:text-emerald-600 dark:hover:bg-slate-700/70 dark:hover:text-emerald-400 transition-colors active:bg-slate-300/70 dark:active:bg-slate-600/70">
        {label}
    </button>
);

const NavButton = ({ label, onClick }: { label: string, onClick?: () => void }) => (
    <button onClick={onClick} data-tauri-drag-region="false" className="text-xs font-medium hover:text-emerald-500 transition-colors px-2 py-1 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800">
        {label}
    </button>
);

const ToggleSwitch = ({ checked }: { checked: boolean }) => (
    <div className={cn(
        "w-7 h-3.5 rounded-full p-0.5 cursor-pointer flex items-center transition-colors",
        checked ? "bg-emerald-500" : "bg-slate-300 dark:bg-slate-600"
    )}>
        <div className={cn(
            "w-2.5 h-2.5 bg-white rounded-full shadow-sm transition-transform",
            checked ? "translate-x-3.5" : "translate-x-0"
        )} />
    </div>
);
