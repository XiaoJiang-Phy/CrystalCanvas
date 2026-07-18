import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const files = Object.fromEntries(await Promise.all([
    'src/App.tsx',
    'src/hooks/useTauriMenu.ts',
    'src/hooks/useCameraInteraction.ts',
    'src/hooks/useFileDrop.ts',
    'src/components/panels/index.ts',
    'src/components/layout/RightSidebar.tsx',
    'src/components/panels/AtomOperationsPanel.tsx',
    'src/components/panels/MeasurementPanel.tsx',
    'src/components/panels/SupercellPanel.tsx',
    'src/components/panels/SlabPanel.tsx',
    'src/components/panels/VolumetricPanel.tsx',
].map(async (path) => [path, await readFile(new URL(`../${path}`, import.meta.url), 'utf8')])));

test('SYNC-1B reserves get_crystal_state for the App snapshot owner', () => {
    assert.equal((files['src/App.tsx'].match(/safeInvoke\('get_crystal_state'/g) ?? []).length, 1);
    for (const [path, source] of Object.entries(files)) {
        if (path === 'src/App.tsx') continue;
        assert.doesNotMatch(source, /safeInvoke\('get_crystal_state'/, path);
    }
});

test('SYNC-1B does not pass App snapshot refresh into committed mutation callsites', () => {
    const app = files['src/App.tsx'];
    assert.doesNotMatch(app, /useFileDrop\(\{[\s\S]*?onFileLoaded\s*:/);
    assert.doesNotMatch(app, /useTauriMenu\(\{[\s\S]*?onStateChange\s*:/);
    assert.doesNotMatch(app, /useCameraInteraction\(\{[\s\S]*?onStateChange\s*:/);
    assert.doesNotMatch(app, /onStructureUpdate=\{fetch_crystal_state\}/);
    assert.doesNotMatch(app, /\.then\(fetch_crystal_state\)/);
});

test('SYNC-1B removes refresh-only callback APIs from structural mutation hooks and panels', () => {
    for (const path of [
        'src/hooks/useTauriMenu.ts',
        'src/hooks/useCameraInteraction.ts',
        'src/components/panels/index.ts',
        'src/components/layout/RightSidebar.tsx',
        'src/components/panels/AtomOperationsPanel.tsx',
        'src/components/panels/MeasurementPanel.tsx',
        'src/components/panels/SupercellPanel.tsx',
        'src/components/panels/SlabPanel.tsx',
        'src/components/panels/VolumetricPanel.tsx',
    ]) {
        assert.doesNotMatch(files[path], /onStateChange|onStructureUpdate/, path);
    }
    assert.doesNotMatch(files['src/hooks/useFileDrop.ts'], /onFileLoaded/);
});

test('SYNC-1B preserves volumetric renderer events without a full-state refresh callback', () => {
    const volumetric = files['src/components/panels/VolumetricPanel.tsx'];
    assert.match(volumetric, /safeListen\('volumetric_loaded'/);
    assert.doesNotMatch(volumetric, /onStructureUpdate/);
});
