// [Overview: Modal dialog for configuring global rendering and visual settings.]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React, { useState, useEffect } from 'react';
import { X, Settings as SettingsIcon, Sliders, Palette, Zap } from 'lucide-react';
import { cn } from '../../utils/cn';
import { safeInvoke } from '../../utils/tauri-mock';

interface AppSettings {
    atom_scale: number;
    bond_tolerance: number;
    bond_radius: number;
    bond_color: [number, number, number, number];
    custom_atom_colors: Record<string, [number, number, number, number]>;
}

interface SettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
    elements: string[];
}

export const SettingsModal: React.FC<SettingsModalProps> = ({ isOpen, onClose, elements }) => {
    const [settings, setSettings] = useState<AppSettings | null>(null);
    const [activeTab, setActiveTab] = useState<'general' | 'bonds' | 'elements'>('general');

    useEffect(() => {
        if (isOpen) {
            safeInvoke<AppSettings>('get_settings')
                .then(s => s && setSettings(s))
                .catch(console.error);
        }
    }, [isOpen]);

    if (!isOpen || !settings) return null;

    const handleApply = () => {
        safeInvoke('update_settings', { newSettings: settings })
            .catch(err => alert("Failed to apply settings: " + err));
    };

    const handleOk = () => {
        safeInvoke('update_settings', { newSettings: settings })
            .then(onClose)
            .catch(err => alert("Failed to update settings: " + err));
    };

    const updateColor = (elem: string, hex: string) => {
        const r = parseInt(hex.slice(1, 3), 16) / 255;
        const g = parseInt(hex.slice(3, 5), 16) / 255;
        const b = parseInt(hex.slice(5, 7), 16) / 255;

        setSettings({
            ...settings,
            custom_atom_colors: {
                ...settings.custom_atom_colors,
                [elem]: [r, g, b, 1.0]
            }
        });
    };

    const rgbToHex = (rgba: [number, number, number, number]) => {
        const r = Math.round(rgba[0] * 255).toString(16).padStart(2, '0');
        const g = Math.round(rgba[1] * 255).toString(16).padStart(2, '0');
        const b = Math.round(rgba[2] * 255).toString(16).padStart(2, '0');
        return `#${r}${g}${b}`;
    };

    return (
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-slate-900/40 backdrop-blur-sm animate-in fade-in duration-300">
            <div className="bg-white dark:bg-slate-900 w-full max-w-2xl rounded-2xl shadow-2xl border border-slate-200 dark:border-slate-800 overflow-hidden flex flex-col max-h-[80vh] animate-in zoom-in-95 duration-300">
                {/* Header */}
                <div className="px-6 py-4 border-b border-slate-100 dark:border-slate-800 flex items-center justify-between bg-slate-50/50 dark:bg-slate-800/50">
                    <div className="flex items-center gap-3">
                        <div className="p-2 bg-emerald-500/10 rounded-lg text-emerald-600 dark:text-emerald-400">
                            <SettingsIcon className="w-5 h-5" />
                        </div>
                        <h2 className="text-lg font-semibold text-slate-900 dark:text-white">CrystalCanvas Settings</h2>
                    </div>
                    <button onClick={onClose} className="p-2 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors">
                        <X className="w-5 h-5 text-slate-500" />
                    </button>
                </div>

                <div className="flex flex-1 overflow-hidden">
                    {/* Sidebar */}
                    <div className="w-48 border-r border-slate-100 dark:border-slate-800 p-2 bg-slate-50/30 dark:bg-slate-900/30">
                        <TabButton
                            active={activeTab === 'general'}
                            onClick={() => setActiveTab('general')}
                            icon={<Sliders className="w-4 h-4" />}
                            label="General"
                        />
                        <TabButton
                            active={activeTab === 'bonds'}
                            onClick={() => setActiveTab('bonds')}
                            icon={<Zap className="w-4 h-4" />}
                            label="Bonds"
                        />
                        <TabButton
                            active={activeTab === 'elements'}
                            onClick={() => setActiveTab('elements')}
                            icon={<Palette className="w-4 h-4" />}
                            label="Elements"
                        />
                    </div>

                    {/* Content */}
                    <div className="flex-1 overflow-y-auto p-6 space-y-6">
                        {activeTab === 'general' && (
                            <div className="space-y-4">
                                <section className="space-y-3">
                                    <h3 className="text-sm font-medium text-slate-500 uppercase tracking-wider">Atom Display</h3>
                                    <div className="space-y-2">
                                        <label className="text-sm font-medium text-slate-700 dark:text-slate-300 flex justify-between">
                                            <span>Global Atom Scale</span>
                                            <span className="text-emerald-500 font-mono">{settings.atom_scale.toFixed(2)}x</span>
                                        </label>
                                        <input
                                            type="range" min="0.1" max="2.0" step="0.05"
                                            value={settings.atom_scale}
                                            onChange={(e) => setSettings({ ...settings, atom_scale: parseFloat(e.target.value) })}
                                            className="w-full h-1.5 bg-slate-200 dark:bg-slate-700 rounded-lg appearance-none cursor-pointer accent-emerald-500"
                                        />
                                    </div>
                                </section>
                            </div>
                        )}

                        {activeTab === 'bonds' && (
                            <div className="space-y-6">
                                <section className="space-y-4">
                                    <h3 className="text-sm font-medium text-slate-500 uppercase tracking-wider">Bonding Parameters</h3>

                                    <div className="space-y-2">
                                        <label className="text-sm font-medium text-slate-700 dark:text-slate-300 flex justify-between">
                                            <span>Search Tolerance (Å)</span>
                                            <span className="text-emerald-500 font-mono">+{settings.bond_tolerance.toFixed(2)}</span>
                                        </label>
                                        <input
                                            type="range" min="0.1" max="1.5" step="0.05"
                                            value={settings.bond_tolerance}
                                            onChange={(e) => setSettings({ ...settings, bond_tolerance: parseFloat(e.target.value) })}
                                            className="w-full h-1.5 bg-slate-200 dark:bg-slate-700 rounded-lg appearance-none cursor-pointer accent-emerald-500"
                                        />
                                        <p className="text-[10px] text-slate-400">Increases the search radius beyond the sum of covalent radii.</p>
                                    </div>

                                    <div className="space-y-2">
                                        <label className="text-sm font-medium text-slate-700 dark:text-slate-300 flex justify-between">
                                            <span>Bond Visual Radius (Å)</span>
                                            <span className="text-emerald-500 font-mono">{settings.bond_radius.toFixed(2)}</span>
                                        </label>
                                        <input
                                            type="range" min="0.01" max="0.3" step="0.01"
                                            value={settings.bond_radius}
                                            onChange={(e) => setSettings({ ...settings, bond_radius: parseFloat(e.target.value) })}
                                            className="w-full h-1.5 bg-slate-200 dark:bg-slate-700 rounded-lg appearance-none cursor-pointer accent-emerald-500"
                                        />
                                    </div>

                                    <div className="flex items-center justify-between pt-2">
                                        <label className="text-sm font-medium text-slate-700 dark:text-slate-300">Bond Color</label>
                                        <div className="flex items-center gap-3">
                                            <div
                                                className="w-6 h-6 rounded-full border border-slate-200 dark:border-slate-700 shadow-sm"
                                                style={{ backgroundColor: rgbToHex(settings.bond_color) }}
                                            />
                                            <input
                                                type="color"
                                                value={rgbToHex(settings.bond_color)}
                                                onChange={(e) => {
                                                    const hex = e.target.value;
                                                    const r = parseInt(hex.slice(1, 3), 16) / 255;
                                                    const g = parseInt(hex.slice(3, 5), 16) / 255;
                                                    const b = parseInt(hex.slice(5, 7), 16) / 255;
                                                    setSettings({ ...settings, bond_color: [r, g, b, 1.0] });
                                                }}
                                                className="opacity-0 absolute w-6 h-6 cursor-pointer"
                                            />
                                        </div>
                                    </div>
                                </section>
                            </div>
                        )}

                        {activeTab === 'elements' && (
                            <div className="space-y-4">
                                <h3 className="text-sm font-medium text-slate-500 uppercase tracking-wider">Element Customization</h3>
                                <div className="grid grid-cols-2 gap-4">
                                    {elements.map(elem => {
                                        const currentColor = settings.custom_atom_colors[elem] || [0.5, 0.5, 0.5, 1.0]; // Fallback
                                        return (
                                            <div key={elem} className="p-3 bg-slate-50 dark:bg-slate-800/50 rounded-xl border border-slate-100 dark:border-slate-800 flex items-center justify-between group hover:border-emerald-200 dark:hover:border-emerald-900/50 transition-colors">
                                                <div className="flex items-center gap-3">
                                                    <div
                                                        className="w-8 h-8 rounded-lg flex items-center justify-center font-bold text-xs shadow-sm bg-white dark:bg-slate-800"
                                                        style={{ color: rgbToHex(currentColor as [number, number, number, number]) }}
                                                    >
                                                        {elem}
                                                    </div>
                                                    <span className="text-sm font-medium text-slate-700 dark:text-slate-300 font-mono tracking-tight">{elem} Detail</span>
                                                </div>
                                                <input
                                                    type="color"
                                                    value={rgbToHex(currentColor as [number, number, number, number])}
                                                    onChange={(e) => updateColor(elem, e.target.value)}
                                                    className="w-6 h-6 rounded-md cursor-pointer bg-transparent border-none appearance-none"
                                                />
                                            </div>
                                        );
                                    })}
                                    {elements.length === 0 && (
                                        <p className="col-span-2 text-center py-8 text-slate-400 text-sm italic">No structure loaded to list elements.</p>
                                    )}
                                </div>
                            </div>
                        )}
                    </div>
                </div>

                {/* Footer Actions */}
                <div className="flex justify-end gap-3 px-6 py-4 border-t border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800/50">
                    <button
                        onClick={onClose}
                        className="px-4 py-2 text-sm font-medium text-slate-600 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-xl transition-colors"
                    >
                        Cancel
                    </button>
                    <button
                        onClick={handleApply}
                        className="px-4 py-2 text-sm font-medium text-emerald-600 dark:text-emerald-400 bg-emerald-50 dark:bg-emerald-500/10 hover:bg-emerald-100 dark:hover:bg-emerald-500/20 rounded-xl transition-colors"
                    >
                        Apply
                    </button>
                    <button
                        onClick={handleOk}
                        className="px-6 py-2 text-sm font-medium text-white bg-emerald-500 hover:bg-emerald-600 rounded-xl shadow-lg shadow-emerald-500/20 active:scale-95 transition-all"
                    >
                        OK
                    </button>
                </div>
            </div>
        </div>
    );
};

const TabButton = ({ active, onClick, icon, label }: { active: boolean, onClick: () => void, icon: React.ReactNode, label: string }) => (
    <button
        onClick={onClick}
        className={cn(
            "w-full flex items-center gap-3 px-4 py-3 rounded-xl text-sm font-medium transition-all duration-200 mb-1",
            active
                ? "bg-white dark:bg-slate-800 text-emerald-600 dark:text-emerald-400 shadow-sm border border-slate-200 dark:border-slate-700"
                : "text-slate-500 hover:bg-white/50 dark:hover:bg-slate-800/50 hover:text-slate-800 dark:hover:text-slate-200"
        )}
    >
        {icon}
        {label}
    </button>
);
