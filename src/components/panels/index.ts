import { CrystalState, PhononModeSummary } from '../../types/crystal';

export interface PanelProps {
    crystalState: CrystalState | null;
    selectedAtoms?: number[];
    onSelectionChange?: (indices: number[]) => void;
    onStructureUpdate?: () => void;
    onBondCountUpdate?: (count: number) => void;
    onActivePhononModeUpdate?: (mode: PhononModeSummary | null) => void;
    interactionMode?: 'select' | 'move' | 'rotate' | 'measure';
    setInteractionMode?: (mode: 'select' | 'move' | 'rotate' | 'measure') => void;
    setOpenAccordion?: (id: string | null) => void;
}

export const lazyConfig = {
    BondAnalysisPanel: () => import('./BondAnalysisPanel'),
    VolumetricPanel: () => import('./VolumetricPanel'),
    PhononPanel: () => import('./PhononPanel'),
    BrillouinZonePanel: () => import('./BrillouinZonePanel'),
    WannierPanel: () => import('./WannierPanel'),
    SupercellPanel: () => import('./SupercellPanel'),
    SlabPanel: () => import('./SlabPanel'),
    AtomOperationsPanel: () => import('./AtomOperationsPanel'),
    MeasurementPanel: () => import('./MeasurementPanel'),
};
