import { useEffect, useRef } from 'react';
import { safeInvoke } from '../utils/tauri-mock';

interface UseCameraInteractionProps {
    viewportRef: React.RefObject<HTMLDivElement | null>;
    interactionMode: 'select' | 'move' | 'rotate' | 'measure';
    selectedAtoms: number[];
    updateSelection: (sel: number[] | ((prev: number[]) => number[])) => void;
    setContextMenu: (pos: { x: number, y: number } | null) => void;
}

type DragTerminal = 'commit' | 'cancel';

interface AtomDragSession {
    pointerId: number;
    lastClientX: number;
    lastClientY: number;
    sessionId: string | null;
    pendingDx: number;
    pendingDy: number;
    rafId: number | null;
    previewInFlight: boolean;
    terminal: DragTerminal | null;
    terminalQueued: boolean;
    captureOwned: boolean;
}

export function useCameraInteraction({
    viewportRef,
    interactionMode,
    selectedAtoms,
    updateSelection,
    setContextMenu
}: UseCameraInteractionProps) {
    const isDraggingCamera = useRef(false);
    const cameraPointerId = useRef<number | null>(null);
    const lastMousePos = useRef({ x: 0, y: 0 });
    const pointerDownPos = useRef({ x: 0, y: 0 });
    const atomDrag = useRef<AtomDragSession | null>(null);

    useEffect(() => {
        const el = viewportRef.current;
        if (!el) return;

        const releasePointerCapture = (pointerId: number) => {
            try {
                el.releasePointerCapture(pointerId);
            } catch {
                // Pointer capture can already be released by the browser.
            }
        };

        const releaseSessionCapture = (session: AtomDragSession) => {
            if (!session.captureOwned) return;
            session.captureOwned = false;
            releasePointerCapture(session.pointerId);
        };

        const clearSession = (session: AtomDragSession) => {
            if (atomDrag.current !== session) return;
            if (session.rafId !== null) {
                cancelAnimationFrame(session.rafId);
                session.rafId = null;
            }
            releaseSessionCapture(session);
            atomDrag.current = null;
        };

        const queueTerminal = (session: AtomDragSession) => {
            if (
                atomDrag.current !== session
                || !session.sessionId
                || !session.terminal
                || session.terminalQueued
            ) return;

            if (session.terminal === 'commit') {
                if (
                    session.previewInFlight
                    || session.rafId !== null
                    || session.pendingDx !== 0
                    || session.pendingDy !== 0
                ) return;
            }

            session.terminalQueued = true;
            const terminalInvoke = session.terminal === 'commit'
                ? safeInvoke('commit_atom_drag', { sessionId: session.sessionId })
                : safeInvoke('cancel_atom_drag', { sessionId: session.sessionId });
            void terminalInvoke
                .catch(console.error)
                .finally(() => clearSession(session));
        };

        const requestTerminal = (session: AtomDragSession, terminal: DragTerminal, forceCancel = false) => {
            if (atomDrag.current !== session || session.terminal === 'cancel') return;
            if (
                session.terminal === 'commit'
                && (terminal === 'commit' || session.terminalQueued || !forceCancel)
            ) return;

            session.terminal = terminal;
            if (session.rafId !== null) {
                cancelAnimationFrame(session.rafId);
                session.rafId = null;
            }
            if (terminal === 'cancel') {
                session.pendingDx = 0;
                session.pendingDy = 0;
            }
            releaseSessionCapture(session);
            if (terminal === 'cancel') queueTerminal(session);
        };

        const flushPreview = (session: AtomDragSession) => {
            if (session.rafId !== null) {
                cancelAnimationFrame(session.rafId);
                session.rafId = null;
            }
            if (
                atomDrag.current !== session
                || !session.sessionId
                || session.terminal === 'cancel'
                || session.previewInFlight
            ) return;

            const { pendingDx, pendingDy } = session;
            if (pendingDx === 0 && pendingDy === 0) {
                queueTerminal(session);
                return;
            }
            session.pendingDx = 0;
            session.pendingDy = 0;
            session.previewInFlight = true;
            void safeInvoke('update_atom_drag', {
                sessionId: session.sessionId,
                dx: pendingDx,
                dy: pendingDy,
            }).then(() => {
                session.previewInFlight = false;
                if (atomDrag.current !== session || session.terminal === 'cancel') return;
                if (session.pendingDx !== 0 || session.pendingDy !== 0) {
                    flushPreview(session);
                } else {
                    queueTerminal(session);
                }
            }).catch((error) => {
                session.previewInFlight = false;
                console.error(error);
                requestTerminal(session, 'cancel', true);
            });
        };

        const schedulePreview = (session: AtomDragSession) => {
            if (!session.sessionId || session.terminal || session.rafId !== null) return;
            session.rafId = requestAnimationFrame(() => {
                session.rafId = null;
                flushPreview(session);
            });
        };

        const addPointerDelta = (session: AtomDragSession, e: PointerEvent) => {
            const dx = e.clientX - session.lastClientX;
            const dy = e.clientY - session.lastClientY;
            session.lastClientX = e.clientX;
            session.lastClientY = e.clientY;
            if (session.terminal) return;
            session.pendingDx += dx;
            session.pendingDy += dy;
            schedulePreview(session);
        };

        const onPointerDown = (e: PointerEvent) => {
            if (e.button !== 0 && e.button !== 1 && e.button !== 2) return;
            if (e.button === 2) {
                setContextMenu({ x: e.clientX, y: e.clientY });
                return;
            }
            if (atomDrag.current || isDraggingCamera.current) return;

            pointerDownPos.current = { x: e.clientX, y: e.clientY };

            if (e.button === 0 && interactionMode === 'move' && selectedAtoms.length > 0) {
                const session: AtomDragSession = {
                    pointerId: e.pointerId,
                    lastClientX: e.clientX,
                    lastClientY: e.clientY,
                    sessionId: null,
                    pendingDx: 0,
                    pendingDy: 0,
                    rafId: null,
                    previewInFlight: false,
                    terminal: null,
                    terminalQueued: false,
                    captureOwned: false,
                };
                atomDrag.current = session;
                void safeInvoke('begin_atom_drag', { indices: selectedAtoms }).then((sessionId) => {
                    if (atomDrag.current !== session) return;
                    session.sessionId = sessionId;
                    if (session.terminal) {
                        if (session.terminal === 'commit') flushPreview(session);
                        queueTerminal(session);
                        return;
                    }
                    try {
                        el.setPointerCapture(session.pointerId);
                        session.captureOwned = true;
                    } catch {
                        requestTerminal(session, 'cancel');
                        return;
                    }
                    schedulePreview(session);
                }).catch((error) => {
                    console.error(error);
                    clearSession(session);
                });
                return;
            }

            if (interactionMode === 'rotate' || interactionMode === 'move' || e.button === 1) {
                try {
                    el.setPointerCapture(e.pointerId);
                } catch {
                    return;
                }
                isDraggingCamera.current = true;
                cameraPointerId.current = e.pointerId;
                lastMousePos.current = { x: e.clientX, y: e.clientY };
            }
        };

        const onPointerMove = (e: PointerEvent) => {
            const session = atomDrag.current;
            if (session?.pointerId === e.pointerId) {
                if (e.buttons === 1) addPointerDelta(session, e);
                return;
            }
            if (!isDraggingCamera.current || cameraPointerId.current !== e.pointerId) return;

            const dx = e.clientX - lastMousePos.current.x;
            const dy = e.clientY - lastMousePos.current.y;
            lastMousePos.current = { x: e.clientX, y: e.clientY };

            if (e.buttons === 4 || (e.buttons === 1 && interactionMode === 'move')) {
                safeInvoke('pan_camera', { dx, dy }).catch(console.error);
            } else if (e.buttons === 1 && interactionMode === 'rotate') {
                safeInvoke('rotate_camera', { dx, dy }).catch(console.error);
            }
        };

        const finishCameraDrag = (e: PointerEvent) => {
            if (!isDraggingCamera.current || cameraPointerId.current !== e.pointerId) return;
            isDraggingCamera.current = false;
            cameraPointerId.current = null;
            releasePointerCapture(e.pointerId);
        };

        const onPointerUp = (e: PointerEvent) => {
            const session = atomDrag.current;
            if (session?.pointerId === e.pointerId) {
                addPointerDelta(session, e);
                requestTerminal(session, 'commit');
                if (session.terminal === 'commit') {
                    flushPreview(session);
                    queueTerminal(session);
                }
                return;
            }
            if (isDraggingCamera.current && cameraPointerId.current !== e.pointerId) return;
            finishCameraDrag(e);

            const willPick = e.button === 0 && (interactionMode === 'select' || interactionMode === 'measure');
            if (willPick) {
                const ddx = Math.abs(e.clientX - pointerDownPos.current.x);
                const ddy = Math.abs(e.clientY - pointerDownPos.current.y);
                if (ddx < 5 && ddy < 5) {
                    const rect = el.getBoundingClientRect();
                    const dpr = window.devicePixelRatio || 1;
                    const x = (e.clientX - rect.left) * dpr;
                    const y = (e.clientY - rect.top) * dpr;
                    safeInvoke('pick_atom', {
                        x,
                        y,
                        screenW: rect.width * dpr,
                        screenH: rect.height * dpr
                    }).then((idx) => {
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
                            } else if (!e.shiftKey && interactionMode !== 'measure') {
                                newSel = [];
                            }
                            safeInvoke('update_selection', { indices: newSel }).catch(console.error);
                            return newSel;
                        });
                    }).catch(console.error);
                }
            }
        };

        const onPointerCancel = (e: PointerEvent) => {
            const session = atomDrag.current;
            if (session?.pointerId === e.pointerId) {
                requestTerminal(session, 'cancel', true);
                return;
            }
            finishCameraDrag(e);
        };

        const onLostPointerCapture = (e: PointerEvent) => {
            const session = atomDrag.current;
            if (session?.pointerId === e.pointerId && session.captureOwned) {
                requestTerminal(session, 'cancel', true);
                return;
            }
            finishCameraDrag(e);
        };

        const onKeyDown = (e: KeyboardEvent) => {
            if (e.key === 'Escape') {
                const session = atomDrag.current;
                if (session) requestTerminal(session, 'cancel', true);
            }
        };

        const onWindowBlur = () => {
            isDraggingCamera.current = false;
            cameraPointerId.current = null;
            const session = atomDrag.current;
            if (session) requestTerminal(session, 'cancel', true);
        };

        const onWheel = (e: WheelEvent) => {
            e.preventDefault();
            safeInvoke('zoom_camera', { delta: Math.sign(e.deltaY) }).catch(console.error);
        };

        el.addEventListener('pointerdown', onPointerDown);
        el.addEventListener('pointermove', onPointerMove);
        el.addEventListener('pointerup', onPointerUp);
        el.addEventListener('pointercancel', onPointerCancel);
        el.addEventListener('lostpointercapture', onLostPointerCapture);
        el.addEventListener('wheel', onWheel, { passive: false });
        window.addEventListener('keydown', onKeyDown);
        window.addEventListener('blur', onWindowBlur);

        return () => {
            const session = atomDrag.current;
            if (session) requestTerminal(session, 'cancel', true);
            isDraggingCamera.current = false;
            cameraPointerId.current = null;
            el.removeEventListener('pointerdown', onPointerDown);
            el.removeEventListener('pointermove', onPointerMove);
            el.removeEventListener('pointerup', onPointerUp);
            el.removeEventListener('pointercancel', onPointerCancel);
            el.removeEventListener('lostpointercapture', onLostPointerCapture);
            el.removeEventListener('wheel', onWheel);
            window.removeEventListener('keydown', onKeyDown);
            window.removeEventListener('blur', onWindowBlur);
        };
    }, [interactionMode, selectedAtoms, updateSelection, setContextMenu, viewportRef]);
}
