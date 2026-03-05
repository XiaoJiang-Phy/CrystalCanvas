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
                "w-full h-12 flex items-center justify-between px-4 pl-[80px] shrink-0 relative",
                "bg-white/80 dark:bg-slate-900/80 backdrop-blur-xl",
                "border-b border-slate-200/80 dark:border-slate-700/50",
                "shadow-sm z-50 pointer-events-auto transition-colors duration-300"
            )}>

            {/* Drag region layer: sits behind all buttons so clicking buttons works normally */}
            <div
                data-tauri-drag-region
                className="absolute inset-0 z-0"
            />

            {/* Left: Brand + Basic Tools */}
            <div className="flex items-center gap-4 relative z-10">
                <div className="flex items-center gap-2 font-semibold text-base tracking-tight text-emerald-600 dark:text-emerald-400">
                    <img src={logoUrl} className="w-5 h-5 object-contain" alt="Logo" />
                    <span>CrystalCanvas</span>
                </div>

                <div className="h-5 w-px bg-slate-300 dark:bg-slate-700" />

                {/* Tool Group */}
                <div className="flex items-center gap-0.5 bg-slate-100/80 dark:bg-slate-800/80 p-0.5 rounded-lg">
                    <ToolButton icon={<MousePointerClick className="w-3.5 h-3.5" />} active={interactionMode === 'select'} onClick={() => setInteractionMode('select')} tooltip="Select" />
                    <ToolButton icon={<Move className="w-3.5 h-3.5" />} active={interactionMode === 'move'} onClick={() => setInteractionMode('move')} tooltip="Move" />
                    <ToolButton icon={<Rotate3D className="w-3.5 h-3.5" />} active={interactionMode === 'rotate'} onClick={() => setInteractionMode('rotate')} tooltip="Rotate" />
                    <ToolButton icon={<BoxSelection className="w-3.5 h-3.5" />} active={interactionMode === 'measure'} onClick={() => setInteractionMode('measure')} tooltip="Measure/Select" />
                </div>
            </div>

            {/* Center: Axis View Buttons */}
            <div className="flex items-center gap-1 relative z-10">
                <ViewButton label="a" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'a' })} />
                <ViewButton label="b" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'b' })} />
                <ViewButton label="c" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'c' })} />
                <ViewButton label="a*" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'a_star' })} />
                <ViewButton label="b*" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'b_star' })} />
                <ViewButton label="c*" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'c_star' })} />
            </div>

            {/* Right: Toggles & Actions */}
            <div className="flex items-center gap-2 relative z-10">
                <NavButton label="Reset View" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'reset' })} />
                <NavButton label="Symmetry" />

                <div className="flex items-center gap-1.5 cursor-pointer" onClick={onToggleLabels} data-tauri-drag-region="false">
                    <span className="text-xs font-medium select-none" data-tauri-drag-region="false">Labels</span>
                    <ToggleSwitch checked={showLabels} />
                </div>

                <div className="h-4 w-px bg-slate-300 dark:bg-slate-700" />

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
                    title="Toggle LLM Assistant"
                >
                    <Bot className="w-3.5 h-3.5" />
                </button>

                <button
                    onClick={toggleTheme}
                    data-tauri-drag-region="false"
                    className="p-1.5 rounded-lg hover:bg-slate-200 dark:hover:bg-slate-800 transition-colors"
                    title="Toggle Theme"
                >
                    {theme === 'dark' ? <Sun className="w-3.5 h-3.5" /> : <Moon className="w-3.5 h-3.5" />}
                </button>

                <button
                    onClick={onOpenSettings}
                    data-tauri-drag-region="false"
                    className="p-1.5 rounded-lg hover:bg-slate-200 dark:hover:bg-slate-800 transition-colors"
                >
                    <Settings className="w-3.5 h-3.5" />
                </button>
            </div>

        </div>
    );
};

// --- Subcomponents ---

const ToolButton = ({ icon, active = false, tooltip, onClick }: { icon: React.ReactNode, active?: boolean, tooltip?: string, onClick?: () => void }) => (
    <button
        onClick={onClick}
        data-tauri-drag-region="false"
        className={cn(
            "p-1.5 rounded-md transition-all duration-200",
            active
                ? "bg-white dark:bg-slate-700 shadow-sm text-emerald-600 dark:text-emerald-400"
                : "text-slate-500 hover:text-slate-900 dark:hover:text-slate-100 hover:bg-slate-200/50 dark:hover:bg-slate-700/50"
        )}
        title={tooltip}
    >
        {icon}
    </button>
);

const ViewButton = ({ label, onClick }: { label: string, onClick?: () => void }) => (
    <button onClick={onClick} data-tauri-drag-region="false" className="w-7 h-6 flex items-center justify-center text-xs font-mono font-medium rounded-md bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-emerald-50 hover:text-emerald-600 dark:hover:bg-emerald-900/40 dark:hover:text-emerald-400 border border-slate-200 dark:border-slate-700 transition-colors shadow-sm active:scale-[0.96] ml-0.5">
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
