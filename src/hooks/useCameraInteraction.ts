import { useEffect, useRef } from 'react';
import { safeInvoke } from '../utils/tauri-mock';

interface UseCameraInteractionProps {
    viewportRef: React.RefObject<HTMLDivElement | null>;
    interactionMode: 'select' | 'move' | 'rotate' | 'measure';
    selectedAtoms: number[];
    updateSelection: (sel: number[] | ((prev: number[]) => number[])) => void;
    setContextMenu: (pos: { x: number, y: number } | null) => void;
    onStateChange: () => void;
}

export function useCameraInteraction({
    viewportRef,
    interactionMode,
    selectedAtoms,
    updateSelection,
    setContextMenu,
    onStateChange
}: UseCameraInteractionProps) {
    const isDraggingCamera = useRef(false);
    const lastMousePos = useRef({ x: 0, y: 0 });
    const pointerDownPos = useRef({ x: 0, y: 0 });

    useEffect(() => {
        const el = viewportRef.current;
        if (!el) return;

        console.warn(`[CameraInteraction] Effect registered. mode=${interactionMode}`);

        const onPointerDown = (e: PointerEvent) => {
            if (e.button !== 0 && e.button !== 1 && e.button !== 2) return;
            if (e.button === 2) {
                setContextMenu({ x: e.clientX, y: e.clientY });
                return;
            }

            console.warn(`[CameraInteraction] pointerDown btn=${e.button} mode=${interactionMode}`);
            pointerDownPos.current = { x: e.clientX, y: e.clientY };

            if (interactionMode === 'rotate' || interactionMode === 'move' || e.button === 1) {
                isDraggingCamera.current = true;
                lastMousePos.current = { x: e.clientX, y: e.clientY };
                el.setPointerCapture(e.pointerId);
            }
        };

        const onPointerMove = (e: PointerEvent) => {
            if (!isDraggingCamera.current) return;
            const dx = e.clientX - lastMousePos.current.x;
            const dy = e.clientY - lastMousePos.current.y;
            lastMousePos.current = { x: e.clientX, y: e.clientY };

            if (e.buttons === 1 && interactionMode === 'move' && selectedAtoms.length > 0) {
                safeInvoke('translate_atoms_screen', { indices: selectedAtoms, dx, dy })
                    .then(onStateChange)
                    .catch(console.error);
            } else if (e.buttons === 4 || (e.buttons === 1 && interactionMode === 'move')) {
                safeInvoke('pan_camera', { dx, dy }).catch(console.error);
            } else if (e.buttons === 1 && interactionMode === 'rotate') {
                safeInvoke('rotate_camera', { dx: dx * 1.0, dy: dy * 1.0 }).catch(console.error);
            }
        };

        const onPointerUp = (e: PointerEvent) => {
            console.warn(`[CameraInteraction] pointerUp btn=${e.button} mode=${interactionMode} isDrag=${isDraggingCamera.current}`);
            if (isDraggingCamera.current) {
                isDraggingCamera.current = false;
                el.releasePointerCapture(e.pointerId);
            }
            const willPick = e.button === 0 && (interactionMode === 'select' || interactionMode === 'measure');
            console.warn(`[CameraInteraction] willPick=${willPick}`);
            if (willPick) {
                const ddx = Math.abs(e.clientX - pointerDownPos.current.x);
                const ddy = Math.abs(e.clientY - pointerDownPos.current.y);
                console.warn(`[CameraInteraction] drag delta: ddx=${ddx} ddy=${ddy}`);
                if (ddx < 5 && ddy < 5) {
                    const rect = el.getBoundingClientRect();
                    const dpr = window.devicePixelRatio || 1;
                    const x = (e.clientX - rect.left) * dpr;
                    const y = (e.clientY - rect.top) * dpr;
                    console.warn(`[CameraInteraction] calling pick_atom x=${x} y=${y} W=${rect.width * dpr} H=${rect.height * dpr}`);
                    safeInvoke<number | null>('pick_atom', {
                        x, y,
                        screenW: rect.width * dpr,
                        screenH: rect.height * dpr
                    }).then((idx) => {
                        console.warn("[CameraInteraction] pick_atom returned:", idx);
                        updateSelection(prev => {
                            let newSel = [...prev];
                            if (idx !== null && idx !== undefined) {
                                if (e.shiftKey || interactionMode === 'measure') {
                                    if (newSel.includes(idx)) {
                                        newSel = newSel.filter(i => i !== idx);
                                    } else {
                                        newSel.push(idx);
                                        if (interactionMode === 'measure' && newSel.length > 4) {
                                            newSel.shift();
                                        }
                                    }
                                } else {
                                    newSel = [idx];
                                }
                            } else {
                                if (!e.shiftKey && interactionMode !== 'measure') {
                                    newSel = [];
                                }
                            }
                            console.warn("[CameraInteraction] newSel:", newSel);
                            safeInvoke('update_selection', { indices: newSel }).catch(console.error);
                            return newSel;
                        });
                    }).catch((err) => {
                        console.error("[CameraInteraction] pick_atom ERROR:", err);
                    });
                }
            }
        };

        const onWheel = (e: WheelEvent) => {
            e.preventDefault();
            safeInvoke('zoom_camera', { delta: Math.sign(e.deltaY) }).catch(console.error);
        };

        el.addEventListener('pointerdown', onPointerDown);
        el.addEventListener('pointermove', onPointerMove);
        el.addEventListener('pointerup', onPointerUp);
        el.addEventListener('pointercancel', onPointerUp);
        el.addEventListener('wheel', onWheel, { passive: false });

        return () => {
            el.removeEventListener('pointerdown', onPointerDown);
            el.removeEventListener('pointermove', onPointerMove);
            el.removeEventListener('pointerup', onPointerUp);
            el.removeEventListener('pointercancel', onPointerUp);
            el.removeEventListener('wheel', onWheel);
        };
    }, [interactionMode, selectedAtoms, updateSelection, setContextMenu, onStateChange, viewportRef]);
}
