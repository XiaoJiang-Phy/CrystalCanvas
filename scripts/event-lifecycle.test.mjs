import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const file_drop_source = await readFile(
    new URL('../src/hooks/useFileDrop.ts', import.meta.url),
    'utf8',
);
const contract_source = await readFile(
    new URL('../src/ipc/contracts.ts', import.meta.url),
    'utf8',
);
const event_classification = JSON.parse(await readFile(
    new URL('../ipc/event-classification.json', import.meta.url),
    'utf8',
));

const canonical_events = [
    'tauri://drag-enter',
    'tauri://drag-drop',
    'tauri://drag-leave',
];

function settle() {
    return Promise.resolve().then(() => Promise.resolve()).then(() => Promise.resolve());
}

function deferred() {
    let resolve;
    let reject;
    const promise = new Promise((next_resolve, next_reject) => {
        resolve = next_resolve;
        reject = next_reject;
    });
    return { promise, resolve, reject };
}

class EffectRuntime {
    cleanup = null;

    use_effect(effect) {
        this.cleanup = effect();
    }

    unmount() {
        this.cleanup?.();
    }
}

class ListenerHarness {
    registrations = [];
    invokes = [];

    listen = (event, handler) => {
        const registration = {
            event,
            handler,
            deferred: deferred(),
            unlisten_count: 0,
        };
        this.registrations.push(registration);
        return registration.deferred.promise;
    };

    invoke = (command, args) => {
        this.invokes.push({ command, args });
        return Promise.resolve(null);
    };

    resolve_all() {
        for (const registration of this.registrations) {
            registration.deferred.resolve(() => {
                registration.unlisten_count += 1;
            });
        }
    }

    emit(event, payload) {
        for (const registration of this.registrations) {
            if (registration.event === event && registration.unlisten_count === 0) {
                registration.handler({ payload });
            }
        }
    }

    unlisten_count() {
        return this.registrations.reduce((total, registration) => total + registration.unlisten_count, 0);
    }
}

const react_bridge = {
    useEffect: (...args) => globalThis.__file_drop_runtime.use_effect(...args),
};

globalThis.__file_drop_react = react_bridge;
globalThis.__file_drop_tauri = {
    safeInvoke: (...args) => globalThis.__file_drop_listeners.invoke(...args),
    safeListen: (...args) => globalThis.__file_drop_listeners.listen(...args),
};

function executable_hook_source(source) {
    return source
        .replace(
            "import { useEffect } from 'react';",
            'const { useEffect } = globalThis.__file_drop_react;',
        )
        .replace(
            "import { safeInvoke, safeListen } from '../utils/tauri-mock';",
            'const { safeInvoke, safeListen } = globalThis.__file_drop_tauri;',
        );
}

const transpiled_hook = ts.transpileModule(executable_hook_source(file_drop_source), {
    compilerOptions: {
        module: ts.ModuleKind.ESNext,
        target: ts.ScriptTarget.ES2022,
    },
}).outputText;
const { useFileDrop } = await import(
    `data:text/javascript;base64,${Buffer.from(transpiled_hook).toString('base64')}`,
);

function mount_file_drop(dragging_states = []) {
    const runtime = new EffectRuntime();
    const listeners = new ListenerHarness();
    globalThis.__file_drop_runtime = runtime;
    globalThis.__file_drop_listeners = listeners;
    useFileDrop({ setIsDragging: (value) => dragging_states.push(value) });
    return { runtime, listeners, dragging_states };
}

test('IPC-3A keeps file-drop on one canonical Tauri v2 event lifecycle', () => {
    const registered_events = [...file_drop_source.matchAll(/safeListen\('([^']+)'/g)]
        .map((match) => match[1])
        .sort();

    assert.deepEqual(registered_events, [...canonical_events].sort(),
        'one mount must register exactly the canonical Tauri v2 drag lifecycle');
    for (const legacy_event of [
        'tauri://file-drop',
        'tauri://file-drop-hover',
        'tauri://file-drop-cancelled',
    ]) {
        assert.doesNotMatch(file_drop_source, new RegExp(legacy_event.replace('://', '://')),
            `${legacy_event} must not retain a second native file-drop owner`);
        assert.doesNotMatch(contract_source, new RegExp(legacy_event.replace('://', '://')),
            `${legacy_event} must not remain in the frontend event contract`);
        assert.equal(event_classification.events[legacy_event], undefined,
            `${legacy_event} must not remain classified after its listener is removed`);
    }
});

test('IPC-3A releases every resolved listener exactly once, including repeated cleanup', async () => {
    const harness = mount_file_drop();
    harness.listeners.resolve_all();
    await settle();

    harness.runtime.unmount();
    harness.runtime.unmount();

    assert.equal(harness.listeners.unlisten_count(), harness.listeners.registrations.length,
        'every acquired unlisten handle must be released exactly once');
    assert.ok(harness.listeners.registrations.every((registration) => registration.unlisten_count === 1),
        'cleanup must be idempotent for each individual listener handle');
});

test('IPC-3A self-cleans listener registrations that resolve after unmount', async () => {
    const harness = mount_file_drop();
    harness.runtime.unmount();
    harness.listeners.resolve_all();
    await settle();

    assert.equal(harness.listeners.unlisten_count(), harness.listeners.registrations.length,
        'a late listener registration must release itself after its owner has unmounted');
});

test('IPC-3A remount does not retain an old drag-enter listener', async () => {
    const dragging_states = [];
    const first = mount_file_drop(dragging_states);
    first.listeners.resolve_all();
    await settle();
    first.runtime.unmount();

    const second = mount_file_drop(dragging_states);
    second.listeners.resolve_all();
    await settle();
    dragging_states.length = 0;

    first.listeners.emit('tauri://drag-enter', { paths: ['/tmp/first.cif'], position: { x: 1, y: 1 } });
    second.listeners.emit('tauri://drag-enter', { paths: ['/tmp/second.cif'], position: { x: 2, y: 2 } });

    assert.deepEqual(dragging_states, [true],
        'a remount must own one drag-enter listener rather than accumulating prior mounts');
});

test('IPC-3A maps one native drop to one load mutation and deterministic overlay transitions', async () => {
    const harness = mount_file_drop();
    harness.listeners.resolve_all();
    await settle();

    const payload = { paths: ['/tmp/Si.cif'], position: { x: 120, y: 80 } };
    harness.listeners.emit('tauri://drag-enter', payload);
    harness.listeners.emit('tauri://file-drop', payload);
    harness.listeners.emit('tauri://drag-drop', payload);
    await settle();

    const loads = harness.listeners.invokes.filter((call) => call.command === 'load_cif_file');
    assert.equal(loads.length, 1, 'one native drop must issue exactly one file-load mutation');
    assert.deepEqual(loads[0].args, { path: '/tmp/Si.cif' });
    assert.deepEqual(harness.dragging_states, [true, false],
        'drag-enter then drop must deterministically clear the overlay');

    harness.listeners.emit('tauri://drag-enter', payload);
    harness.listeners.emit('tauri://drag-leave', null);
    assert.deepEqual(harness.dragging_states, [true, false, true, false],
        'drag-leave must clear the overlay without loading a file');
});
