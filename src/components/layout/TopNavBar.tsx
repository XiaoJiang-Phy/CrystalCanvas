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
    isPerspective: boolean;
    onTogglePerspective: () => void;
    showLabels: boolean;
    onToggleLabels: () => void;
    interactionMode: 'select' | 'move' | 'rotate' | 'measure';
    setInteractionMode: (mode: 'select' | 'move' | 'rotate' | 'measure') => void;
}

export const TopNavBar: React.FC<TopNavBarProps> = ({
    showAssistant, onToggleAssistant,
    isPerspective, onTogglePerspective,
    showLabels, onToggleLabels,
    interactionMode, setInteractionMode
}) => {
    const { theme, toggleTheme } = useTheme();

    return (
        <div className={cn(
            "w-full h-12 flex items-center justify-between px-4 shrink-0",
            "bg-white/80 dark:bg-slate-900/80 backdrop-blur-xl",
            "border-b border-slate-200/80 dark:border-slate-700/50",
            "shadow-sm z-50 pointer-events-auto transition-colors duration-300"
        )}>

            {/* Left: Brand + Basic Tools */}
            <div className="flex items-center gap-4">
                <div className="flex items-center gap-2 font-semibold text-base tracking-tight text-emerald-600 dark:text-emerald-400">
                    <Sparkles className="w-4 h-4" />
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

            {/* Center: View Perspectives */}
            <div className="flex items-center gap-1">
                <button
                    onClick={onTogglePerspective}
                    className={cn(
                        "px-2.5 py-1 text-xs font-medium rounded-md transition-colors",
                        isPerspective
                            ? "bg-emerald-100 dark:bg-emerald-900/40 text-emerald-700 dark:text-emerald-300"
                            : "hover:bg-slate-200/70 dark:hover:bg-slate-800/70 border border-transparent hover:border-slate-300 dark:hover:border-slate-600"
                    )}>
                    Perspective
                </button>
                <button
                    onClick={onTogglePerspective}
                    className={cn(
                        "px-2.5 py-1 text-xs font-medium rounded-md transition-colors",
                        !isPerspective
                            ? "bg-emerald-100 dark:bg-emerald-900/40 text-emerald-700 dark:text-emerald-300"
                            : "hover:bg-slate-200/70 dark:hover:bg-slate-800/70 border border-transparent hover:border-slate-300 dark:hover:border-slate-600"
                    )}>
                    Ortho
                </button>
                <div className="w-2" />
                <ViewButton label="[a]" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'a' })} />
                <ViewButton label="[b]" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'b' })} />
                <ViewButton label="[c]" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'c' })} />
                <ViewButton label="[a*]" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'a_star' })} />
                <ViewButton label="[b*]" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'b_star' })} />
                <ViewButton label="[c*]" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'c_star' })} />
            </div>

            {/* Right: Toggles & Actions */}
            <div className="flex items-center gap-2">
                <NavButton label="Reset View" onClick={() => safeInvoke('set_camera_view_axis', { axis: 'reset' })} />
                <NavButton label="Symmetry" />

                <div className="flex items-center gap-1.5 cursor-pointer" onClick={onToggleLabels}>
                    <span className="text-xs font-medium select-none">Labels</span>
                    <ToggleSwitch checked={showLabels} />
                </div>

                <div className="h-4 w-px bg-slate-300 dark:bg-slate-700" />

                {/* LLM Assistant Toggle */}
                <button
                    onClick={onToggleAssistant}
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
                    className="p-1.5 rounded-lg hover:bg-slate-200 dark:hover:bg-slate-800 transition-colors"
                    title="Toggle Theme"
                >
                    {theme === 'dark' ? <Sun className="w-3.5 h-3.5" /> : <Moon className="w-3.5 h-3.5" />}
                </button>

                <button className="p-1.5 rounded-lg hover:bg-slate-200 dark:hover:bg-slate-800 transition-colors">
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
    <button onClick={onClick} className="px-1.5 py-1 text-xs font-medium rounded-md hover:bg-slate-200/70 dark:hover:bg-slate-800/70 transition-colors border border-transparent hover:border-slate-300 dark:hover:border-slate-600">
        {label}
    </button>
);

const NavButton = ({ label, onClick }: { label: string, onClick?: () => void }) => (
    <button onClick={onClick} className="text-xs font-medium hover:text-emerald-500 transition-colors px-2 py-1 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800">
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
