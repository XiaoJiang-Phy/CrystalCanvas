import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const source = await readFile(new URL('../src/App.tsx', import.meta.url), 'utf8');

function between(start, end) {
    const start_index = source.indexOf(start);
    assert.notEqual(start_index, -1, `missing ${start}`);
    const end_index = source.indexOf(end, start_index);
    assert.notEqual(end_index, -1, `missing ${end}`);
    return source.slice(start_index, end_index);
}

test('SYNC-1A rejects duplicate and stale state_changed versions before fetching', () => {
    const listener = between("safeListen('state_changed'", ").then((unlisten) =>");
    const stale_guard = listener.indexOf('version <= lastAppliedVersionRef.current');
    const pending_guard = listener.indexOf('pendingStateVersionsRef.current.has(version)');
    const reserve = listener.indexOf('pendingStateVersionsRef.current.add(version)');
    const fetch = listener.indexOf('void fetch_crystal_state(version)');

    assert.match(listener, /if\s*\(\s*version\s*<=\s*lastAppliedVersionRef\.current\s*\|\|\s*pendingStateVersionsRef\.current\.has\(version\)\s*\)\s*return\s*;/s);
    assert.ok(stale_guard >= 0 && stale_guard < reserve);
    assert.ok(pending_guard >= 0 && pending_guard < reserve);
    assert.ok(reserve < fetch);
});

test('SYNC-1A applies only a newer snapshot and releases failed requests for retry', () => {
    const fetch = between('const fetch_crystal_state', 'useEffect(() =>');
    const snapshot = fetch.indexOf("await safeInvoke('get_crystal_state')");
    const monotonic_guard = fetch.indexOf('state.version > lastAppliedVersionRef.current');
    const advance = fetch.indexOf('lastAppliedVersionRef.current = state.version');
    const apply = fetch.indexOf('setCrystalState(state)');
    const release = fetch.indexOf('pendingStateVersionsRef.current.delete(requestedVersion)');

    assert.match(fetch, /if\s*\(\s*state\s*&&\s*state\.version\s*>\s*lastAppliedVersionRef\.current\s*\)\s*\{\s*lastAppliedVersionRef\.current\s*=\s*state\.version\s*;\s*setCrystalState\(state\)\s*;\s*\}/s);
    assert.match(fetch, /finally\s*\{\s*if\s*\(\s*requestedVersion\s*!==\s*null\s*\)\s*\{\s*pendingStateVersionsRef\.current\.delete\(requestedVersion\)\s*;\s*\}\s*\}/s);
    assert.ok(snapshot >= 0 && snapshot < monotonic_guard);
    assert.ok(monotonic_guard < advance && advance < apply);
    assert.ok(release > apply);
});

test('SYNC-1A starts exactly one initial load and cleans up its listener', () => {
    const effect = between('useEffect(() => {', '// Menu and File drop event listener');
    const listener_ready = effect.indexOf('unlistenStateChanged = unlisten');
    const initial_guard = effect.indexOf('if (!initialStateLoadStartedRef.current)');
    const initial_start = effect.indexOf('initialStateLoadStartedRef.current = true');
    const initial_fetch = effect.indexOf('void fetch_crystal_state()');
    const dispose = effect.indexOf('disposed = true');
    const unlisten = effect.indexOf('unlistenStateChanged();');

    assert.ok(listener_ready >= 0 && listener_ready < initial_guard);
    assert.ok(initial_guard < initial_start && initial_start < initial_fetch);
    assert.ok(dispose >= 0 && dispose < unlisten);
    assert.match(effect, /catch\s*\(\([^)]*\)\s*=>\s*\{[\s\S]*?if \(!disposed && !initialStateLoadStartedRef\.current\)/);
});
