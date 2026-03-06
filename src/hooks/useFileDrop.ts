import { useEffect } from 'react';
import { safeInvoke, safeListen } from '../utils/tauri-mock';

interface UseFileDropProps {
    setIsDragging: (isDragging: boolean) => void;
    onFileLoaded: () => void;
}

export function useFileDrop({ setIsDragging, onFileLoaded }: UseFileDropProps) {
    useEffect(() => {
        let unlistenDrop = () => { };
        let unlistenHover = () => { };
        let unlistenCancel = () => { };
        let unlistenDragDrop = () => { };

        const handleDrop = (path: string | undefined) => {
            if (path) {
                console.log('Got drop path:', path);
                safeInvoke('load_cif_file', { path })
                    .then(onFileLoaded)
                    .catch(e => alert(`Failed to load structure:\n${e}`));
            }
        };

        // Tauri v1 / fallback file drop event
        safeListen<{ paths: string[] }>('tauri://file-drop', (event) => {
            setIsDragging(false);
            handleDrop(event.payload.paths?.[0]);
        }).then(f => unlistenDrop = f).catch(console.warn);

        // Tauri v2 drag-drop event
        safeListen<{ paths: string[] }>('tauri://drag-drop', (event) => {
            setIsDragging(false);
            handleDrop(event.payload.paths?.[0]);
        }).then(f => unlistenDragDrop = f).catch(console.warn);

        safeListen('tauri://file-drop-hover', () => setIsDragging(true)).then(f => unlistenHover = f).catch(console.warn);
        safeListen('tauri://drag-enter', () => setIsDragging(true)).catch(console.warn);

        safeListen('tauri://file-drop-cancelled', () => setIsDragging(false)).then(f => unlistenCancel = f).catch(console.warn);
        safeListen('tauri://drag-leave', () => setIsDragging(false)).catch(console.warn);

        return () => {
            unlistenDrop();
            unlistenHover();
            unlistenCancel();
            unlistenDragDrop();
        };
    }, [setIsDragging, onFileLoaded]);
}
