import { useEffect } from 'react';
import { safeInvoke, safeListen } from '../utils/tauri-mock';

interface UseFileDropProps {
    setIsDragging: (isDragging: boolean) => void;
}

export function useFileDrop({ setIsDragging }: UseFileDropProps) {
    useEffect(() => {
        let disposed = false;
        const unlistenHandlers: Array<() => void> = [];

        const retainUnlisten = (registration: Promise<() => void>) => {
            void registration.then((unlisten) => {
                let released = false;
                const release = () => {
                    if (released) return;
                    released = true;
                    unlisten();
                };
                if (disposed) {
                    release();
                } else {
                    unlistenHandlers.push(release);
                }
            }).catch((error) => {
                console.warn('[file-drop] listener registration failed:', error);
            });
        };

        const handleDrop = (path: string | undefined) => {
            if (path) {
                safeInvoke('load_cif_file', { path })
                    .catch(e => alert(`Failed to load structure:\n${e}`));
            }
        };

        retainUnlisten(safeListen('tauri://drag-drop', (event) => {
            setIsDragging(false);
            handleDrop(event.payload.paths?.[0]);
        }));

        retainUnlisten(safeListen('tauri://drag-enter', () => setIsDragging(true)));
        retainUnlisten(safeListen('tauri://drag-leave', () => setIsDragging(false)));

        return () => {
            if (disposed) return;
            disposed = true;
            for (const unlisten of unlistenHandlers) unlisten();
            unlistenHandlers.length = 0;
        };
    }, [setIsDragging]);
}
