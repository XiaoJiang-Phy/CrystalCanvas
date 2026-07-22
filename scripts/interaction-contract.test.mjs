import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import node_test from 'node:test';
import ts from 'typescript';

const hook_path = 'src/hooks/useCameraInteraction.ts';
const hook_source = await readFile(new URL(`../${hook_path}`, import.meta.url), 'utf8');
const app_source = await readFile(new URL('../src/App.tsx', import.meta.url), 'utf8');

const contract_cases = [];

function test(name, callback) {
    contract_cases.push({ name, callback });
}

function strip_comments(source) {
    return source
        .replace(/\/\*[\s\S]*?\*\//g, '')
        .replace(/^\s*\/\/.*$/gm, '');
}

function replace_named_import(source, module_name, global_name) {
    const escaped_module = module_name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const pattern = new RegExp(
        `import\\s*\\{([\\s\\S]*?)\\}\\s*from\\s*['"]${escaped_module}['"];?\\s*`,
    );
    assert.match(source, pattern, `missing named import from ${module_name}`);
    return source.replace(pattern, (_match, bindings) =>
        `const {${bindings}} = globalThis.${global_name};\n`,
    );
}

function deps_changed(previous, next) {
    return previous === undefined
        || previous.length !== next.length
        || previous.some((value, index) => !Object.is(value, next[index]));
}

class HookRuntime {
    refs = [];
    effects = [];
    ref_cursor = 0;
    effect_cursor = 0;

    render(callback) {
        this.ref_cursor = 0;
        this.effect_cursor = 0;
        callback();
    }

    useRef(initial_value) {
        const index = this.ref_cursor++;
        if (!this.refs[index]) this.refs[index] = { current: initial_value };
        return this.refs[index];
    }

    useEffect(callback, deps) {
        const index = this.effect_cursor++;
        const previous = this.effects[index];
        if (previous && !deps_changed(previous.deps, deps)) return;
        previous?.cleanup?.();
        this.effects[index] = { deps, cleanup: callback() };
    }

    unmount() {
        for (const effect of this.effects) effect?.cleanup?.();
        this.effects = [];
    }
}

class FakeEventTarget {
    listeners = new Map();
    captures = new Set();
    capture_log = [];
    release_log = [];

    addEventListener(type, listener) {
        const listeners = this.listeners.get(type) ?? new Set();
        listeners.add(listener);
        this.listeners.set(type, listeners);
    }

    removeEventListener(type, listener) {
        this.listeners.get(type)?.delete(listener);
    }

    dispatch(type, init = {}) {
        const event = {
            button: 0,
            buttons: 0,
            pointerId: 7,
            clientX: 0,
            clientY: 0,
            shiftKey: false,
            key: '',
            deltaY: 0,
            defaultPrevented: false,
            preventDefault() {
                this.defaultPrevented = true;
            },
            ...init,
        };
        for (const listener of this.listeners.get(type) ?? []) listener(event);
        return event;
    }

    setPointerCapture(pointer_id) {
        this.captures.add(pointer_id);
        this.capture_log.push(pointer_id);
    }

    releasePointerCapture(pointer_id) {
        this.captures.delete(pointer_id);
        this.release_log.push(pointer_id);
    }

    getBoundingClientRect() {
        return { left: 10, top: 20, width: 400, height: 300 };
    }
}

class FakeAnimationFrames {
    next_id = 1;
    callbacks = new Map();

    request = (callback) => {
        const id = this.next_id++;
        this.callbacks.set(id, callback);
        return id;
    };

    cancel = (id) => {
        this.callbacks.delete(id);
    };

    flush() {
        const callbacks = [...this.callbacks.values()];
        this.callbacks.clear();
        for (const callback of callbacks) callback(0);
    }
}

class FakeIpc {
    calls = [];
    handlers = new Map();

    invoke = (command, args) => {
        this.calls.push({ command, args });
        return this.handlers.get(command)?.(args) ?? Promise.resolve(null);
    };

    count(command) {
        return this.calls.filter((call) => call.command === command).length;
    }

    calls_for(command) {
        return this.calls.filter((call) => call.command === command);
    }
}

const react_bridge = {
    useEffect: (...args) => globalThis.__interaction_runtime.useEffect(...args),
    useRef: (...args) => globalThis.__interaction_runtime.useRef(...args),
};

globalThis.__interaction_react = react_bridge;
globalThis.__interaction_tauri = {
    safeInvoke: (...args) => globalThis.__interaction_ipc.invoke(...args),
};

const executable_hook_source = replace_named_import(
    replace_named_import(hook_source, 'react', '__interaction_react'),
    '../utils/tauri-mock',
    '__interaction_tauri',
);
const transpiled_hook = ts.transpileModule(executable_hook_source, {
    compilerOptions: {
        module: ts.ModuleKind.ESNext,
        target: ts.ScriptTarget.ES2022,
    },
}).outputText;
const { useCameraInteraction } = await import(
    `data:text/javascript;base64,${Buffer.from(transpiled_hook).toString('base64')}`,
);

async function settle() {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
}

async function settle_many(rounds = 128) {
    for (let index = 0; index < rounds; index += 1) await Promise.resolve();
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

function create_harness(overrides = {}) {
    const viewport = new FakeEventTarget();
    const window_target = new FakeEventTarget();
    const frames = new FakeAnimationFrames();
    const ipc = new FakeIpc();
    const runtime = new HookRuntime();
    const selected_updates = [];
    const context_updates = [];
    let props = {
        viewportRef: { current: viewport },
        interactionMode: 'move',
        selectedAtoms: [2, 5],
        updateSelection: (update) => {
            selected_updates.push(update);
        },
        setContextMenu: (position) => context_updates.push(position),
        ...overrides,
    };

    ipc.handlers.set('begin_atom_drag', () => Promise.resolve('drag-session-7'));
    globalThis.__interaction_runtime = runtime;
    globalThis.__interaction_ipc = ipc;
    globalThis.window = window_target;
    window_target.devicePixelRatio = 2;
    globalThis.requestAnimationFrame = frames.request;
    globalThis.cancelAnimationFrame = frames.cancel;

    const render = (next = {}) => {
        props = { ...props, ...next };
        runtime.render(() => useCameraInteraction(props));
    };
    render();

    return {
        viewport,
        window_target,
        frames,
        ipc,
        runtime,
        selected_updates,
        context_updates,
        render,
    };
}

async function begin_drag(harness) {
    harness.viewport.dispatch('pointerdown', {
        button: 0,
        buttons: 1,
        pointerId: 11,
        clientX: 100,
        clientY: 100,
    });
    await settle();
    assert.equal(harness.ipc.count('begin_atom_drag'), 1, 'selected move starts one drag session');
    assert.equal(harness.viewport.capture_log.length, 1, 'capture begins only after begin resolves');
}

function assert_no_drag_commit(harness, detail) {
    assert.equal(harness.ipc.count('commit_atom_drag'), 0, detail);
    assert.equal(harness.ipc.count('cancel_atom_drag'), 1, detail);
}

const clean_hook_source = strip_comments(hook_source);

test('INTERACT-1B reserves the session protocol for selected move and preserves snapshot ownership', () => {
    assert.doesNotMatch(
        clean_hook_source,
        /translate_atoms_screen/,
        'viewport move must not retain the per-event committed mutation path',
    );
    for (const command of [
        'begin_atom_drag',
        'update_atom_drag',
        'commit_atom_drag',
        'cancel_atom_drag',
    ]) {
        assert.match(clean_hook_source, new RegExp(`safeInvoke\\('${command}'`));
    }
    assert.match(clean_hook_source, /requestAnimationFrame/);
    assert.match(clean_hook_source, /cancelAnimationFrame/);
    assert.doesNotMatch(clean_hook_source, /safeInvoke\('get_crystal_state'/);
    assert.equal(
        (app_source.match(/safeInvoke\('get_crystal_state'/g) ?? []).length,
        1,
        'App remains the sole complete snapshot owner',
    );
});

test('INTERACT-1B starts after begin, coalesces extreme motion to one frame update, then commits once', async () => {
    const harness = create_harness();
    await begin_drag(harness);

    harness.viewport.dispatch('pointermove', { buttons: 1, pointerId: 11, clientX: 104, clientY: 103 });
    harness.viewport.dispatch('pointermove', { buttons: 1, pointerId: 11, clientX: 151, clientY: 127 });
    harness.viewport.dispatch('pointermove', { buttons: 1, pointerId: 11, clientX: 199, clientY: 166 });
    assert.equal(harness.ipc.count('update_atom_drag'), 0, 'moves wait for the frame boundary');
    harness.frames.flush();
    await settle();

    assert.deepEqual(harness.ipc.calls_for('update_atom_drag'), [{
        command: 'update_atom_drag',
        args: { sessionId: 'drag-session-7', dx: 99, dy: 66 },
    }]);

    harness.viewport.dispatch('pointermove', { buttons: 1, pointerId: 11, clientX: 210, clientY: 180 });
    harness.viewport.dispatch('pointerup', { button: 0, buttons: 0, pointerId: 11, clientX: 210, clientY: 180 });
    await settle();

    assert.equal(harness.ipc.count('update_atom_drag'), 2, 'pointer-up flushes the final pending delta');
    assert.equal(harness.ipc.count('commit_atom_drag'), 1, 'pointer-up commits once');
    assert.equal(harness.ipc.count('cancel_atom_drag'), 0, 'successful pointer-up does not cancel');
    const update_index = harness.ipc.calls.findIndex((call) => call.command === 'update_atom_drag');
    const commit_index = harness.ipc.calls.findIndex((call) => call.command === 'commit_atom_drag');
    assert.ok(update_index >= 0 && update_index < commit_index, 'commit follows preview delivery');
    harness.viewport.dispatch('pointerup', { button: 0, buttons: 0, pointerId: 11 });
    await settle();
    assert.equal(harness.ipc.count('commit_atom_drag'), 1, 'duplicate pointer-up cannot commit again');
});

test('INTERACT-1B keeps an atom session isolated from a foreign camera pointer', async () => {
    const harness = create_harness();
    try {
        await begin_drag(harness);

        harness.viewport.dispatch('pointerdown', {
            button: 1,
            buttons: 4,
            pointerId: 22,
            clientX: 900,
            clientY: 700,
        });
        harness.viewport.dispatch('pointermove', {
            buttons: 4,
            pointerId: 22,
            clientX: 920,
            clientY: 730,
        });
        harness.viewport.dispatch('pointermove', {
            buttons: 1,
            pointerId: 11,
            clientX: 105,
            clientY: 103,
        });
        harness.frames.flush();
        await settle();

        assert.deepEqual(harness.viewport.capture_log, [11], 'a foreign pointer cannot acquire viewport capture');
        assert.equal(harness.ipc.count('pan_camera'), 0, 'a foreign pointer cannot start camera work');
        assert.equal(harness.ipc.count('rotate_camera'), 0, 'a foreign pointer cannot start rotation work');
        assert.deepEqual(harness.ipc.calls_for('update_atom_drag'), [{
            command: 'update_atom_drag',
            args: { sessionId: 'drag-session-7', dx: 5, dy: 3 },
        }], 'foreign coordinates cannot alter the active atom session delta');
    }
    finally {
        harness.runtime.unmount();
        await settle_many();
    }
});

test('INTERACT-1B keeps a camera session isolated from a foreign atom pointer', async () => {
    const harness = create_harness();
    try {
        harness.viewport.dispatch('pointerdown', {
            button: 1,
            buttons: 4,
            pointerId: 31,
            clientX: 10,
            clientY: 20,
        });
        harness.viewport.dispatch('pointerdown', {
            button: 0,
            buttons: 1,
            pointerId: 32,
            clientX: 900,
            clientY: 700,
        });
        await settle();
        harness.viewport.dispatch('pointermove', {
            buttons: 4,
            pointerId: 31,
            clientX: 20,
            clientY: 30,
        });
        await settle();

        assert.deepEqual(harness.viewport.capture_log, [31], 'a foreign atom pointer cannot acquire capture');
        assert.equal(harness.ipc.count('begin_atom_drag'), 0, 'an active camera owner rejects a second atom session');
        assert.deepEqual(harness.ipc.calls_for('pan_camera'), [{
            command: 'pan_camera',
            args: { dx: 10, dy: 10 },
        }], 'foreign atom coordinates cannot alter the active camera delta');
    }
    finally {
        harness.runtime.unmount();
        await settle_many();
    }
});

test('INTERACT-1B rejects foreign primary completion while a camera owns select or measure', async (t) => {
    for (const interaction_mode of ['select', 'measure']) {
        await t.test(interaction_mode, async () => {
            let selection = [];
            const harness = create_harness({
                interactionMode: interaction_mode,
                selectedAtoms: [],
                updateSelection: (update) => {
                    selection = update(selection);
                },
            });
            try {
                harness.viewport.dispatch('pointerdown', {
                    button: 1,
                    buttons: 4,
                    pointerId: 31,
                    clientX: 100,
                    clientY: 200,
                });
                harness.viewport.dispatch('pointerdown', {
                    button: 0,
                    buttons: 1,
                    pointerId: 32,
                    clientX: 102,
                    clientY: 203,
                });
                harness.viewport.dispatch('pointerup', {
                    button: 0,
                    buttons: 0,
                    pointerId: 32,
                    clientX: 103,
                    clientY: 204,
                });
                await settle();

                assert.equal(harness.ipc.count('pick_atom'), 0, 'foreign primary completion cannot pick');
                assert.equal(harness.ipc.count('update_selection'), 0, 'foreign primary completion cannot mutate selection');
                assert.deepEqual(harness.viewport.capture_log, [31], 'foreign primary cannot acquire a second owner');
                assert.deepEqual(harness.viewport.release_log, [], 'foreign primary cannot release the camera owner');

                harness.viewport.dispatch('pointermove', {
                    buttons: 4,
                    pointerId: 31,
                    clientX: 120,
                    clientY: 230,
                });
                await settle();
                assert.deepEqual(harness.ipc.calls_for('pan_camera'), [{
                    command: 'pan_camera',
                    args: { dx: 20, dy: 30 },
                }], 'the original camera owner keeps its delta baseline');
                assert.deepEqual(harness.viewport.capture_log, [31], 'the original owner remains unique');
                assert.deepEqual(harness.viewport.release_log, [], 'the original owner remains captured');
            }
            finally {
                harness.runtime.unmount();
                await settle_many();
            }
        });
    }
});

test('INTERACT-1B recovers camera ownership after capture acquisition throws', async () => {
    const harness = create_harness({ interactionMode: 'rotate' });
    const set_pointer_capture = harness.viewport.setPointerCapture.bind(harness.viewport);
    let reject_next_capture = true;
    harness.viewport.setPointerCapture = (pointer_id) => {
        if (reject_next_capture) {
            reject_next_capture = false;
            throw new Error('inactive pointer cannot be captured');
        }
        set_pointer_capture(pointer_id);
    };
    try {
        assert.doesNotThrow(() => harness.viewport.dispatch('pointerdown', {
            button: 0,
            buttons: 1,
            pointerId: 41,
            clientX: 10,
            clientY: 20,
        }), 'a failed capture must not escape the viewport handler');

        harness.viewport.dispatch('pointerdown', {
            button: 0,
            buttons: 1,
            pointerId: 42,
            clientX: 20,
            clientY: 30,
        });
        harness.viewport.dispatch('pointermove', {
            buttons: 1,
            pointerId: 42,
            clientX: 25,
            clientY: 35,
        });
        await settle();

        assert.deepEqual(harness.viewport.capture_log, [42], 'a failed capture cannot retain the camera owner');
        assert.deepEqual(harness.ipc.calls_for('rotate_camera'), [{
            command: 'rotate_camera',
            args: { dx: 5, dy: 5 },
        }], 'the next pointer can become the camera owner with its own delta baseline');
    }
    finally {
        harness.runtime.unmount();
        await settle_many();
    }
});

test('INTERACT-1B recovers camera ownership after lost pointer capture', async () => {
    const harness = create_harness({ interactionMode: 'rotate' });
    try {
        harness.viewport.dispatch('pointerdown', {
            button: 0,
            buttons: 1,
            pointerId: 51,
            clientX: 10,
            clientY: 20,
        });
        harness.viewport.dispatch('lostpointercapture', { pointerId: 51 });
        harness.viewport.dispatch('pointerdown', {
            button: 0,
            buttons: 1,
            pointerId: 52,
            clientX: 30,
            clientY: 40,
        });
        harness.viewport.dispatch('pointermove', {
            buttons: 1,
            pointerId: 52,
            clientX: 35,
            clientY: 45,
        });
        await settle();

        assert.deepEqual(harness.viewport.capture_log, [51, 52], 'lost capture releases the old owner for a new pointer');
        assert.deepEqual(harness.viewport.release_log, [51], 'lost capture cleanup releases the stale local capture once');
        assert.deepEqual(harness.ipc.calls_for('rotate_camera'), [{
            command: 'rotate_camera',
            args: { dx: 5, dy: 5 },
        }], 'the replacement owner keeps an independent rotation baseline');
    }
    finally {
        harness.runtime.unmount();
        await settle_many();
    }
});

test('INTERACT-1B bounds stalled preview work to one in-flight update plus one coalesced delta', async () => {
    const harness = create_harness();
    const first_update = deferred();
    let update_calls = 0;
    harness.ipc.handlers.set('update_atom_drag', () => {
        update_calls += 1;
        return update_calls === 1 ? first_update.promise : Promise.resolve(null);
    });
    try {
        await begin_drag(harness);

        harness.viewport.dispatch('pointermove', { buttons: 1, pointerId: 11, clientX: 101, clientY: 100 });
        harness.frames.flush();
        await settle();
        assert.equal(harness.ipc.count('update_atom_drag'), 1, 'the first preview is in flight');

        for (let client_x = 102; client_x <= 165; client_x += 1) {
            harness.viewport.dispatch('pointermove', { buttons: 1, pointerId: 11, clientX: client_x, clientY: 100 });
            harness.frames.flush();
        }
        await settle();
        assert.equal(harness.ipc.count('update_atom_drag'), 1, 'stalled IPC must not start concurrent previews');

        first_update.resolve(null);
        await settle_many();
        assert.deepEqual(
            harness.ipc.calls_for('update_atom_drag'),
            [
                { command: 'update_atom_drag', args: { sessionId: 'drag-session-7', dx: 1, dy: 0 } },
                { command: 'update_atom_drag', args: { sessionId: 'drag-session-7', dx: 64, dy: 0 } },
            ],
            'recovery sends one aggregate preview instead of replaying unbounded frame history',
        );
    }
    finally {
        first_update.resolve(null);
        harness.runtime.unmount();
        await settle_many();
    }
});

test('INTERACT-1B issues cancellation even when a preview IPC never settles', async () => {
    const harness = create_harness();
    const pending_update = deferred();
    harness.ipc.handlers.set('update_atom_drag', () => pending_update.promise);
    try {
        await begin_drag(harness);

        harness.viewport.dispatch('pointermove', { buttons: 1, pointerId: 11, clientX: 140, clientY: 120 });
        harness.frames.flush();
        await settle();
        assert.equal(harness.ipc.count('update_atom_drag'), 1, 'the preview is now stalled');

        harness.window_target.dispatch('blur');
        await settle();
        assert.equal(harness.ipc.count('cancel_atom_drag'), 1, 'blur must send cancellation without waiting for a stalled preview');
        assert.equal(harness.ipc.count('commit_atom_drag'), 0, 'a cancelled session must not commit');

        pending_update.resolve(null);
        await settle();
        assert.equal(harness.ipc.count('cancel_atom_drag'), 1, 'late preview completion cannot duplicate cancellation');
    }
    finally {
        pending_update.resolve(null);
        harness.runtime.unmount();
        await settle_many();
    }
});

test('INTERACT-1B cancels a pending commit when its final preview is stalled', async (t) => {
    const endings = [
        ['window blur', (harness) => harness.window_target.dispatch('blur')],
        ['unmount', (harness) => harness.runtime.unmount()],
    ];

    for (const [name, end] of endings) {
        await t.test(name, async () => {
            const harness = create_harness();
            const pending_update = deferred();
            harness.ipc.handlers.set('update_atom_drag', () => pending_update.promise);
            try {
                await begin_drag(harness);
                harness.viewport.dispatch('pointermove', {
                    buttons: 1,
                    pointerId: 11,
                    clientX: 140,
                    clientY: 120,
                });
                harness.frames.flush();
                await settle();

                harness.viewport.dispatch('pointerup', {
                    button: 0,
                    buttons: 0,
                    pointerId: 11,
                    clientX: 140,
                    clientY: 120,
                });
                await settle();
                assert.equal(harness.ipc.count('commit_atom_drag'), 0, 'stalled preview defers commit');

                end(harness);
                await settle();
                assert.equal(harness.ipc.count('cancel_atom_drag'), 1, `${name} cancels an unissued commit`);
                assert.equal(harness.ipc.count('commit_atom_drag'), 0, `${name} prevents a late commit`);

                pending_update.resolve(null);
                await settle_many();
                assert.equal(harness.ipc.count('cancel_atom_drag'), 1, `${name} cannot duplicate cancellation`);
                assert.equal(harness.ipc.count('commit_atom_drag'), 0, `${name} cannot commit after preview recovery`);
            }
            finally {
                pending_update.resolve(null);
                harness.runtime.unmount();
                await settle_many();
            }
        });
    }
});

test('INTERACT-1B ignores the lost-capture event caused by its own pending commit release', async () => {
    const harness = create_harness();
    const pending_update = deferred();
    harness.ipc.handlers.set('update_atom_drag', () => pending_update.promise);
    try {
        await begin_drag(harness);
        harness.viewport.dispatch('pointermove', {
            buttons: 1,
            pointerId: 11,
            clientX: 140,
            clientY: 120,
        });
        harness.frames.flush();
        await settle();

        harness.viewport.dispatch('pointerup', {
            button: 0,
            buttons: 0,
            pointerId: 11,
            clientX: 140,
            clientY: 120,
        });
        harness.viewport.dispatch('lostpointercapture', { pointerId: 11 });
        await settle();
        assert.equal(harness.ipc.count('cancel_atom_drag'), 0, 'own capture release cannot cancel a pending commit');

        pending_update.resolve(null);
        await settle_many();
        assert.equal(harness.ipc.count('commit_atom_drag'), 1, 'normal preview recovery still commits once');
        assert.equal(harness.ipc.count('cancel_atom_drag'), 0, 'normal preview recovery remains non-cancelling');
    }
    finally {
        pending_update.resolve(null);
        harness.runtime.unmount();
        await settle_many();
    }
});

test('INTERACT-1B resolves a fast release after a delayed begin without late capture or duplicate terminal IPC', async () => {
    const harness = create_harness();
    const pending_begin = deferred();
    harness.ipc.handlers.set('begin_atom_drag', () => pending_begin.promise);
    harness.viewport.dispatch('pointerdown', {
        button: 0,
        buttons: 1,
        pointerId: 12,
        clientX: 100,
        clientY: 100,
    });
    harness.viewport.dispatch('pointerup', {
        button: 0,
        buttons: 0,
        pointerId: 12,
        clientX: 100,
        clientY: 100,
    });
    assert.equal(harness.viewport.capture_log.length, 0, 'unresolved begin cannot capture');
    assert.equal(harness.ipc.count('commit_atom_drag'), 0, 'terminal IPC waits for session identity');

    pending_begin.resolve('drag-session-7');
    await settle();
    assert.equal(harness.viewport.capture_log.length, 0, 'released pointer cannot capture after delayed begin');
    assert.equal(harness.ipc.count('commit_atom_drag'), 1, 'fast release still owns one terminal commit');
    assert.equal(harness.ipc.count('cancel_atom_drag'), 0, 'pointer-up is not reclassified as cancel');
    harness.viewport.dispatch('pointerup', { button: 0, buttons: 0, pointerId: 12 });
    await settle();
    assert.equal(harness.ipc.count('commit_atom_drag'), 1, 'late duplicate pointer-up cannot commit again');
});

test('INTERACT-1B cancels an active session for every abnormal termination and clears queued work', async (t) => {
    const endings = [
        ['Escape', (harness) => harness.window_target.dispatch('keydown', { key: 'Escape' })],
        ['pointer cancel', (harness) => harness.viewport.dispatch('pointercancel', { pointerId: 11 })],
        ['lost capture', (harness) => harness.viewport.dispatch('lostpointercapture', { pointerId: 11 })],
        ['window blur', (harness) => harness.window_target.dispatch('blur')],
        ['unmount', (harness) => harness.runtime.unmount()],
        ['tool change', (harness) => harness.render({ interactionMode: 'rotate' })],
    ];

    for (const [name, end] of endings) {
        await t.test(name, async () => {
            const harness = create_harness();
            await begin_drag(harness);
            harness.viewport.dispatch('pointermove', { buttons: 1, pointerId: 11, clientX: 160, clientY: 150 });
            end(harness);
            harness.frames.flush();
            await settle();
            assert_no_drag_commit(harness, `${name} must cancel exactly once without commit`);
            assert.equal(harness.ipc.count('update_atom_drag'), 0, `${name} drops queued preview work`);
        });
    }
});

test('INTERACT-1B restores local drag ownership after begin or preview IPC failure', async () => {
    const begin_failure = create_harness();
    begin_failure.ipc.handlers.set('begin_atom_drag', () => Promise.reject(new Error('begin failed')));
    begin_failure.viewport.dispatch('pointerdown', {
        button: 0,
        buttons: 1,
        pointerId: 11,
        clientX: 100,
        clientY: 100,
    });
    await settle();
    assert.equal(begin_failure.viewport.capture_log.length, 0, 'failed begin cannot capture');
    assert.equal(begin_failure.ipc.count('update_atom_drag'), 0);
    assert.equal(begin_failure.ipc.count('commit_atom_drag'), 0);
    assert.equal(begin_failure.ipc.count('cancel_atom_drag'), 0);

    const update_failure = create_harness();
    update_failure.ipc.handlers.set('update_atom_drag', () => Promise.reject(new Error('preview failed')));
    await begin_drag(update_failure);
    update_failure.viewport.dispatch('pointermove', { buttons: 1, pointerId: 11, clientX: 140, clientY: 120 });
    update_failure.frames.flush();
    await settle();
    assert.equal(update_failure.ipc.count('cancel_atom_drag'), 1, 'failed preview cancels the backend session');
    update_failure.viewport.dispatch('pointerup', { button: 0, buttons: 0, pointerId: 11 });
    await settle();
    assert.equal(update_failure.ipc.count('commit_atom_drag'), 0, 'failed preview cannot later commit');
    assert.doesNotMatch(clean_hook_source, /get_crystal_state/, 'failure recovery cannot create a second snapshot owner');
});

test('INTERACT-1B preserves non-drag viewport semantics and keeps pointer listeners off global chrome', async () => {
    const rotate = create_harness({ interactionMode: 'rotate', selectedAtoms: [] });
    rotate.viewport.dispatch('pointerdown', { button: 0, buttons: 1, pointerId: 3, clientX: 10, clientY: 20 });
    rotate.viewport.dispatch('pointermove', { buttons: 1, pointerId: 3, clientX: 30, clientY: 45 });
    assert.equal(rotate.ipc.count('rotate_camera'), 1);
    assert.equal(rotate.ipc.count('begin_atom_drag'), 0);

    const pan = create_harness({ interactionMode: 'move', selectedAtoms: [] });
    pan.viewport.dispatch('pointerdown', { button: 0, buttons: 1, pointerId: 4, clientX: 10, clientY: 20 });
    pan.viewport.dispatch('pointermove', { buttons: 1, pointerId: 4, clientX: 30, clientY: 45 });
    assert.equal(pan.ipc.count('pan_camera'), 1, 'empty selection remains a camera pan');
    assert.equal(pan.ipc.count('begin_atom_drag'), 0);

    const select = create_harness({ interactionMode: 'select', selectedAtoms: [] });
    select.ipc.handlers.set('pick_atom', () => Promise.resolve(4));
    select.viewport.dispatch('pointerdown', { button: 0, buttons: 1, pointerId: 5, clientX: 40, clientY: 50 });
    select.viewport.dispatch('pointerup', { button: 0, buttons: 0, pointerId: 5, clientX: 41, clientY: 51 });
    await settle();
    assert.equal(select.ipc.count('pick_atom'), 1, 'select click keeps renderer picking');
    assert.equal(select.ipc.count('begin_atom_drag'), 0);

    const wheel = select.viewport.dispatch('wheel', { deltaY: 8 });
    assert.equal(wheel.defaultPrevented, true, 'viewport wheel remains owned by camera zoom');
    assert.equal(select.ipc.count('zoom_camera'), 1);

    for (const target of [select.window_target, select.viewport]) {
        for (const type of ['pointerdown', 'pointermove', 'pointerup', 'pointercancel']) {
            const listeners = target.listeners.get(type)?.size ?? 0;
            if (target === select.window_target) assert.equal(listeners, 0, 'window must not own 3D pointer events');
        }
    }
});

node_test('INTERACT-1B contract gates', async (context) => {
    for (const { name, callback } of contract_cases) await context.test(name, callback);
});
