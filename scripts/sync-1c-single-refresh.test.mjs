import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';
import { assert_contract, create_inventory } from './ipc-inventory.mjs';

const app_source = await readFile(new URL('../src/App.tsx', import.meta.url), 'utf8');
const contracts_source = await readFile(new URL('../src/ipc/contracts.ts', import.meta.url), 'utf8');
const refresh_only_sources = Object.fromEntries(await Promise.all([
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
const contracts = await import(`data:text/javascript;base64,${Buffer.from(ts.transpileModule(contracts_source, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText).toString('base64')}`);

function between(source, start, end) {
    const start_index = source.indexOf(start);
    assert.notEqual(start_index, -1, `missing ${start}`);
    const end_index = source.indexOf(end, start_index);
    assert.notEqual(end_index, -1, `missing ${end}`);
    return source.slice(start_index, end_index);
}

test('SYNC-1C accepts only a nonnegative integer state_changed version', () => {
    assert.deepEqual(contracts.validate_ipc_event('state_changed', { version: 0 }), { version: 0 });
    assert.deepEqual(contracts.validate_ipc_event('state_changed', { version: 4_294_967_295 }), { version: 4_294_967_295 });
    for (const payload of [
        null,
        {},
        { version: -1 },
        { version: 1.5 },
        { version: Number.NaN },
        { version: Number.POSITIVE_INFINITY },
        { version: '7' },
    ]) assert.throws(() => contracts.validate_ipc_event('state_changed', payload));
});

test('SYNC-1C maps each accepted version to one snapshot request', () => {
    const listener = between(app_source, "safeListen('state_changed'", ').then((unlisten) =>');
    const fetch = between(app_source, 'const fetch_crystal_state', 'useEffect(() =>');
    const effect = between(app_source, 'useEffect(() => {', '// Menu and File drop event listener');

    assert.equal((app_source.match(/safeInvoke\('get_crystal_state'/g) ?? []).length, 1);
    assert.equal((listener.match(/void fetch_crystal_state\(version\)/g) ?? []).length, 1);
    assert.match(listener, /version\s*<=\s*lastAppliedVersionRef\.current[\s\S]*pendingStateVersionsRef\.current\.has\(version\)[\s\S]*return\s*;/);
    assert.match(fetch, /await safeInvoke\('get_crystal_state'\)/);
    assert.match(fetch, /state\.version\s*>\s*lastAppliedVersionRef\.current/);
    assert.match(effect, /unlistenStateChanged\s*=\s*unlisten[\s\S]*?if\s*\(!initialStateLoadStartedRef\.current\)[\s\S]*?initialStateLoadStartedRef\.current\s*=\s*true[\s\S]*?void fetch_crystal_state\(\)/);
    assert.match(effect, /catch\s*\([^)]*\)\s*=>\s*\{[\s\S]*?if\s*\(!disposed\s*&&\s*!initialStateLoadStartedRef\.current\)[\s\S]*?initialStateLoadStartedRef\.current\s*=\s*true[\s\S]*?void fetch_crystal_state\(\)/);
});

test('SYNC-1C rejects refresh callbacks outside the state_changed owner', () => {
    assert.doesNotMatch(app_source, /\.then\(fetch_crystal_state\)/);
    assert.doesNotMatch(app_source, /useFileDrop\(\{[\s\S]*?onFileLoaded\s*:/);
    assert.doesNotMatch(app_source, /useTauriMenu\(\{[\s\S]*?onStateChange\s*:/);
    assert.doesNotMatch(app_source, /useCameraInteraction\(\{[\s\S]*?onStateChange\s*:/);
    assert.doesNotMatch(app_source, /onStructureUpdate=\{fetch_crystal_state\}/);
    for (const [path, source] of Object.entries(refresh_only_sources)) {
        assert.doesNotMatch(source, /onStateChange|onStructureUpdate/, path);
    }
    assert.doesNotMatch(refresh_only_sources['src/hooks/useFileDrop.ts'], /onFileLoaded/);
});

test('SYNC-1C inventory exposes one state snapshot owner and one state event subscriber', async () => {
    const inventory = await create_inventory();
    assert.doesNotThrow(() => assert_contract(inventory));
    assert.deepEqual(inventory.frontend_commands.get_crystal_state, ['src/App.tsx']);
    assert.deepEqual(inventory.frontend_events.state_changed, ['src/App.tsx']);
    assert.equal(inventory.event_classification.state_changed, 'state_sync');
});
