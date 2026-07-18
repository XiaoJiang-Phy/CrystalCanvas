import React, { useState, useCallback, useEffect, useRef, Suspense, lazy } from 'react';
import { cn } from '../../utils/cn';
import { CrystalState, PhononModeSummary } from '../../types/crystal';
import { lazyConfig } from '../panels';

const BondAnalysisPanel = lazy(lazyConfig.BondAnalysisPanel);
const VolumetricPanel = lazy(lazyConfig.VolumetricPanel);
const PhononPanel = lazy(lazyConfig.PhononPanel);
const BrillouinZonePanel = lazy(lazyConfig.BrillouinZonePanel);
const WannierPanel = lazy(lazyConfig.WannierPanel);
const SupercellPanel = lazy(lazyConfig.SupercellPanel);
const SlabPanel = lazy(lazyConfig.SlabPanel);
const AtomOperationsPanel = lazy(lazyConfig.AtomOperationsPanel);
const MeasurementPanel = lazy(lazyConfig.MeasurementPanel);

const TOOL_SECTIONS = [
    { key: 'Structural Analysis', label: 'Bonds & Polyhedra', icon: <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round"><circle cx="7" cy="12" r="3.5" /><circle cx="17" cy="12" r="3.5" /><line x1="10.5" y1="12" x2="13.5" y2="12" /><line x1="12" y1="9" x2="12" y2="10.5" /></svg> },
    { key: 'Volumetric', label: 'Isosurface / Volume', icon: <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round"><path d="M6 19a4 4 0 01-.78-7.93A7 7 0 0118.5 10.5a4.5 4.5 0 01-.36 8.5H6z" /></svg> },
    { key: 'Phonon Animation', label: 'Phonon Modes', icon: <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round"><path d="M2 12c2-4 4-4 6 0s4 4 6 0 4-4 6 0" /><circle cx="5" cy="12" r="1.2" fill="currentColor" stroke="none" /><circle cx="11" cy="12" r="1.2" fill="currentColor" stroke="none" /><circle cx="17" cy="12" r="1.2" fill="currentColor" stroke="none" /></svg> },
    { key: 'Reciprocal Space', label: 'Brillouin Zone', icon: <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round"><polygon points="12,3 19.5,7.5 19.5,16.5 12,21 4.5,16.5 4.5,7.5" /><circle cx="12" cy="12" r="1.5" fill="currentColor" stroke="none" /><line x1="12" y1="12" x2="19.5" y2="7.5" strokeDasharray="2 2" /></svg> },
    { key: 'Tight-Binding', label: 'Wannier / Hopping', icon: <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round"><circle cx="6" cy="16" r="3" /><circle cx="18" cy="8" r="3" /><path d="M9 14l3.5-3.5M12.5 10.5L10.5 10M12.5 10.5L13 12.5" /></svg> },
    { key: 'Supercell', label: 'Supercell', icon: <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round"><rect x="3" y="3" width="8" height="8" rx="1" /><rect x="13" y="3" width="8" height="8" rx="1" strokeDasharray="3 2" /><rect x="3" y="13" width="8" height="8" rx="1" strokeDasharray="3 2" /><rect x="13" y="13" width="8" height="8" rx="1" strokeDasharray="3 2" /></svg> },
    { key: 'Cutting Plane', label: 'Slab (hkl)', icon: <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round"><rect x="4" y="4" width="16" height="4" rx="1" /><rect x="4" y="10" width="16" height="4" rx="1" /><rect x="4" y="16" width="16" height="4" rx="1" /><line x1="2" y1="9" x2="22" y2="9" strokeWidth={2} stroke="#ef4444" strokeDasharray="4 2" /></svg> },
    { key: 'Atom Operations', label: 'Add / Delete Atoms', icon: <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round"><circle cx="11" cy="13" r="5" /><circle cx="11" cy="13" r="1.8" fill="currentColor" stroke="none" /><circle cx="17.5" cy="6.5" r="4" fill="white" stroke="currentColor" strokeWidth={1.5} /><line x1="17.5" y1="4.5" x2="17.5" y2="8.5" strokeWidth={1.5} /><line x1="15.5" y1="6.5" x2="19.5" y2="6.5" strokeWidth={1.5} /></svg> },
    { key: 'Measurements', label: 'Measurements Tool', icon: <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round"><path d="M21.17 3.25l-2.42-2.42a1 1 0 00-1.42 0L2.17 16a1 1 0 000 1.42l2.42 2.42a1 1 0 001.42 0l15.16-15.16a1 1 0 000-1.43zM6.5 17L5 15.5M10.5 13L9 11.5M14.5 9L13 7.5M18.5 5L17 3.5" /></svg> },
];

export const RightSidebar: React.FC<{
    crystalState: CrystalState | null,
    selectedAtoms?: number[],
    onSelectionChange?: (indices: number[]) => void,
    onBondCountUpdate?: (count: number) => void,
    onActivePhononModeUpdate?: (mode: PhononModeSummary | null) => void,
    interactionMode?: 'select' | 'move' | 'rotate' | 'measure',
    setInteractionMode?: (mode: 'select' | 'move' | 'rotate' | 'measure') => void
}> = (props) => {
    const [openAccordion, setOpenAccordionRaw] = useState<string | null>(null);
    const previousModeRef = useRef<'select' | 'move' | 'rotate' | 'measure'>('rotate');

    const setOpenAccordion = useCallback((key: string | null) => {
        setOpenAccordionRaw(prev => {
            if (key === 'Measurements' && prev !== 'Measurements' && props.setInteractionMode) {
                previousModeRef.current = props.interactionMode || 'rotate';
                props.setInteractionMode('measure');
            } else if (prev === 'Measurements' && key !== 'Measurements' && props.setInteractionMode) {
                props.setInteractionMode(previousModeRef.current);
            }
            return key;
        });
    }, [props.setInteractionMode, props.interactionMode]);

    const activeTool = openAccordion;
    const fallbackSpinner = (
        <div className="flex justify-center py-6">
            <div className="w-6 h-6 border-2 border-emerald-500 border-t-transparent rounded-full animate-spin"></div>
        </div>
    );

    return (
        <div className="shrink-0 h-full flex flex-row pointer-events-none">
            <div className={cn(
                "h-full overflow-hidden transition-[width,opacity] duration-150 ease-in-out",
                activeTool ? "w-[260px] opacity-100" : "w-0 opacity-0",
            )}>
                <div className="w-[260px] h-full p-3 pointer-events-none">
                    <PersistentInspector title="Bonds & Polyhedra" active={activeTool === 'Structural Analysis'}>
                        <Suspense fallback={fallbackSpinner}><BondAnalysisPanel {...props} /></Suspense>
                    </PersistentInspector>
                    <PersistentInspector title="Isosurface / Volume" active={activeTool === 'Volumetric'}>
                        <Suspense fallback={fallbackSpinner}><VolumetricPanel {...props} setOpenAccordion={setOpenAccordion} /></Suspense>
                    </PersistentInspector>
                    <PersistentInspector title="Phonon Modes" active={activeTool === 'Phonon Animation'}>
                        <Suspense fallback={fallbackSpinner}><PhononPanel {...props} /></Suspense>
                    </PersistentInspector>
                    <PersistentInspector title="Brillouin Zone" active={activeTool === 'Reciprocal Space'}>
                        <Suspense fallback={fallbackSpinner}><BrillouinZonePanel {...props} /></Suspense>
                    </PersistentInspector>
                    <PersistentInspector title="Wannier / Hopping" active={activeTool === 'Tight-Binding'}>
                        <Suspense fallback={fallbackSpinner}><WannierPanel {...props} /></Suspense>
                    </PersistentInspector>
                    <PersistentInspector title="Supercell" active={activeTool === 'Supercell'}>
                        <Suspense fallback={fallbackSpinner}><SupercellPanel /></Suspense>
                    </PersistentInspector>
                    <PersistentInspector title="Slab (hkl)" active={activeTool === 'Cutting Plane'}>
                        <Suspense fallback={fallbackSpinner}><SlabPanel /></Suspense>
                    </PersistentInspector>
                    <PersistentInspector title="Add / Delete Atoms" active={activeTool === 'Atom Operations'}>
                        <Suspense fallback={fallbackSpinner}><AtomOperationsPanel {...props} /></Suspense>
                    </PersistentInspector>
                    <PersistentInspector title="Measurements Tool" active={activeTool === 'Measurements'}>
                        <Suspense fallback={fallbackSpinner}><MeasurementPanel {...props} /></Suspense>
                    </PersistentInspector>
                </div>
            </div>

            <div
                className="w-[44px] shrink-0 h-full min-h-0 flex flex-col items-center gap-1 overflow-y-auto py-2 pointer-events-auto custom-scrollbar"
                data-tool-rail="scientific-tools"
                role="toolbar"
                aria-label="Scientific tools"
            >
                {TOOL_SECTIONS.map((section) => (
                    <button
                        type="button"
                        key={section.key}
                        title={section.label}
                        aria-label={section.label}
                        aria-pressed={openAccordion === section.key}
                        onClick={() => setOpenAccordion(openAccordion === section.key ? null : section.key)}
                        className={cn(
                            "w-9 h-9 flex items-center justify-center rounded-lg transition-colors duration-150",
                            openAccordion === section.key
                                ? "bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 shadow-sm ring-1 ring-emerald-500/30"
                                : "text-slate-500 dark:text-slate-400 hover:bg-white/60 dark:hover:bg-slate-800/60 hover:text-slate-700 dark:hover:text-slate-200"
                        )}
                    >
                        {section.icon}
                    </button>
                ))}
            </div>
        </div>
    );
};

const PersistentInspector: React.FC<{ title: string; active: boolean; children: React.ReactNode }> = ({ title, active, children }) => {
    const [hasOpened, setHasOpened] = useState(active);

    useEffect(() => {
        if (active) setHasOpened(true);
    }, [active]);

    if (!hasOpened) return null;

    return (
    <section className={cn("cc-panel h-full flex flex-col overflow-hidden pointer-events-auto", !active && "hidden")} aria-label={title} aria-hidden={!active}>
        <header className="shrink-0 border-b border-slate-100 px-3 py-2 dark:border-slate-800">
            <h2 className="text-sm font-medium text-slate-800 dark:text-slate-200">{title}</h2>
        </header>
        <div className="min-h-0 flex-1 overflow-y-auto px-3 py-3 custom-scrollbar">
            {children}
        </div>
    </section>
    );
};
