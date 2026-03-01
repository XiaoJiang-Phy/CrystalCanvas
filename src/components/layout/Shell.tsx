import React, { createContext, useContext, useEffect, useState } from 'react';
import { cn } from '../../utils/cn';

type Theme = 'light' | 'dark';

interface ThemeContextType {
    theme: Theme;
    toggleTheme: () => void;
}

const ThemeContext = createContext<ThemeContextType>({
    theme: 'dark',
    toggleTheme: () => { },
});

export const useTheme = () => useContext(ThemeContext);

interface ShellProps {
    children: React.ReactNode;
    viewportRef: React.RefObject<HTMLDivElement | null>;
}

export const Shell: React.FC<ShellProps> = ({ children, viewportRef }) => {
    const [theme, setTheme] = useState<Theme>('dark');

    // Toggle dark class on html root for Tailwind
    useEffect(() => {
        const root = window.document.documentElement;
        if (theme === 'dark') {
            root.classList.add('dark');
        } else {
            root.classList.remove('dark');
        }
    }, [theme]);

    const toggleTheme = () => {
        setTheme(prev => (prev === 'light' ? 'dark' : 'light'));
    };

    return (
        <ThemeContext.Provider value={{ theme, toggleTheme }}>
            {/* 
                Main Application Wrapper 
                Transparent so html background gradient is visible.
            */}
            <div className={cn(
                "relative w-screen h-screen overflow-hidden",
                "text-slate-800 dark:text-slate-200",
                "bg-transparent flex flex-col selection:bg-emerald-500/30"
            )}>

                {/*
                    Z-Index 0: The Transparent WGPU Viewport Layer
                    In Tauri, the actual 3D rendering happens on the OS Metal/Vulkan layer behind the window.
                    This div captures pointer events to pass to Rust for camera controls.
                */}
                <div
                    ref={viewportRef}
                    className="absolute inset-0 z-0 bg-transparent"
                />

                {/* 
                    Z-Index 10: The Floating React UI Layers 
                    pointer-events-none lets clicks pass through to the 3D viewport.
                    Individual UI panels restore pointer-events-auto.
                */}
                <div className="absolute inset-0 z-10 flex flex-col pointer-events-none">
                    {children}
                </div>

            </div>
        </ThemeContext.Provider>
    );
}
