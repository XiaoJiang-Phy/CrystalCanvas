import React from 'react';

export const BottomStatusBar: React.FC = () => {
    return (
        <div className="w-full h-7 shrink-0 bg-slate-100/80 dark:bg-slate-900/80 backdrop-blur-md border-t border-slate-200/80 dark:border-slate-700/50 flex items-center justify-between px-4 text-[11px] z-40 pointer-events-auto transition-colors duration-300">
            <div className="flex items-center gap-4 text-slate-500 dark:text-slate-400 tabular-nums">
                <span>Coordinates: x: 1.23, y: 0.00, z: -0.54</span>
            </div>
            <div className="flex items-center gap-6 text-slate-500 dark:text-slate-400">
                <span>Selected Atoms: <span className="font-medium text-slate-700 dark:text-slate-300">1</span></span>
                <span>View: <span className="font-medium text-emerald-600 dark:text-emerald-400">[a]</span></span>
            </div>
        </div>
    );
};
