import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const phonon_panel = await readFile(
    new URL('../src/components/panels/PhononPanel.tsx', import.meta.url),
    'utf8',
);
const command_classification = JSON.parse(await readFile(
    new URL('../ipc/command-classification.json', import.meta.url),
    'utf8',
));

function effect_bodies(source) {
    const bodies = [];
    const effect_start = /useEffect\s*\(\s*\(\)\s*=>\s*\{/g;
    let match;

    while ((match = effect_start.exec(source))) {
        const body_start = source.indexOf('{', match.index);
        let depth = 0;
        let body_end = -1;

        for (let index = body_start; index < source.length; index += 1) {
            if (source[index] === '{') depth += 1;
            if (source[index] === '}') depth -= 1;
            if (depth === 0) {
                body_end = index + 1;
                break;
            }
        }

        assert.ok(body_end > body_start, 'each PhononPanel effect must have a balanced body');
        bodies.push(source.slice(body_start, body_end));
        effect_start.lastIndex = body_end;
    }

    return bodies;
}

function assert_no_match(source, expression, message) {
    assert.equal(expression.test(source), false, message);
}

test('INTERACT-2B prohibits React frame loops from issuing phonon-phase IPC', () => {
    assert_no_match(
        phonon_panel,
        /\b(?:requestAnimationFrame|cancelAnimationFrame)\s*\(/,
        'phonon animation frames are renderer-owned and PhononPanel must not create a RAF lifecycle',
    );
    assert_no_match(
        phonon_panel,
        /\b(?:setInterval|clearInterval|setTimeout|clearTimeout)\s*\(/,
        'PhononPanel must not replace the prohibited RAF lifecycle with a frontend timer loop',
    );

    for (const effect of effect_bodies(phonon_panel)) {
        assert_no_match(
            effect,
            /safeInvoke\(\s*['"]set_phonon_phase['"]/,
            'a React effect must not drive per-frame set_phonon_phase IPC',
        );
    }
});

class IpcException extends Error {
    constructor(error) {
        super(error.message);
        this.code = error.code;
        this.recoverable = error.recoverable;
    }
}

class PanelRuntime {
    states = [];
    cursor = 0;

    useState(initial_value) {
        const index = this.cursor++;
        if (!(index in this.states)) this.states[index] = initial_value;
        const set_state = (next_value) => {
            this.states[index] = typeof next_value === 'function'
                ? next_value(this.states[index])
                : next_value;
        };
        return [this.states[index], set_state];
    }

    render(component, props) {
        this.cursor = 0;
        return component(props);
    }
}

class FakeIpc {
    calls = [];
    handlers = new Map();

    invoke = (command, args) => {
        this.calls.push({ command, args });
        return this.handlers.get(command)?.(args) ?? Promise.resolve(null);
    };

    calls_for(command) {
        return this.calls.filter((call) => call.command === command);
    }
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

async function settle() {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
}

const widgets = {
    ActionButton: () => null,
    PanelError: () => null,
    RangeInput: () => null,
    SelectInput: () => null,
    PhononImportModal: () => null,
};

const react_bridge = {
    Fragment: Symbol('Fragment'),
    createElement(type, props, ...children) {
        return {
            type,
            props: {
                ...props,
                children: children.length === 1 ? children[0] : children,
            },
        };
    },
    useState: (...args) => globalThis.__phonon_panel_runtime.useState(...args),
};

globalThis.__phonon_react = react_bridge;
globalThis.__phonon_widgets = widgets;
globalThis.__phonon_contracts = { IpcException };
globalThis.__phonon_tauri = {
    safeInvoke: (...args) => globalThis.__phonon_panel_ipc.invoke(...args),
};

function executable_panel_source(source) {
    return source
        .replace(
            "import React, { useState } from 'react';",
            'const React = globalThis.__phonon_react;\nconst { useState } = React;',
        )
        .replace(
            "import { safeInvoke } from '../../utils/tauri-mock';",
            'const { safeInvoke } = globalThis.__phonon_tauri;',
        )
        .replace(
            "import { IpcException, type IpcError } from '../../ipc/contracts';",
            'const { IpcException } = globalThis.__phonon_contracts;',
        )
        .replace(
            "import { PhononImportModal } from '../layout/PhononImportModal';",
            'const { PhononImportModal } = globalThis.__phonon_widgets;',
        )
        .replace(
            "import { ActionButton, PanelError, RangeInput, SelectInput } from './shared';",
            'const { ActionButton, PanelError, RangeInput, SelectInput } = globalThis.__phonon_widgets;',
        )
        .replace(/^import .*;\n/gm, '');
}

const transpiled_panel = ts.transpileModule(executable_panel_source(phonon_panel), {
    compilerOptions: {
        jsx: ts.JsxEmit.React,
        module: ts.ModuleKind.ESNext,
        target: ts.ScriptTarget.ES2022,
    },
}).outputText;
const { default: PhononPanel } = await import(
    `data:text/javascript;base64,${Buffer.from(transpiled_panel).toString('base64')}`,
);

function descendants(tree) {
    const result = [];
    const visit = (node) => {
        if (Array.isArray(node)) {
            for (const child of node) visit(child);
            return;
        }
        if (!node || typeof node !== 'object' || !('type' in node)) return;
        result.push(node);
        visit(node.props?.children);
    };
    visit(tree);
    return result;
}

function control(tree, type, label) {
    const found = descendants(tree).find((node) => node.type === type && node.props.label === label);
    assert.ok(found, `PhononPanel must expose ${label}`);
    return found;
}

function reset_control(tree) {
    const found = descendants(tree).find((node) =>
        node.type === widgets.ActionButton && /reset/i.test(node.props.label ?? ''),
    );
    assert.ok(found, 'PhononPanel must expose an explicit reset action');
    return found;
}

function create_harness() {
    const runtime = new PanelRuntime();
    const ipc = new FakeIpc();
    const active_mode_updates = [];

    const render = () => {
        globalThis.__phonon_panel_runtime = runtime;
        globalThis.__phonon_panel_ipc = ipc;
        return runtime.render(PhononPanel, {
            onActivePhononModeUpdate: (mode) => active_mode_updates.push(mode),
        });
    };

    return { runtime, ipc, active_mode_updates, render };
}

const phonon_modes = [{
    index: 0,
    q_point: [0, 0, 0],
    frequency_cm1: 1,
    is_imaginary: false,
}];

async function load_and_select_mode(harness) {
    harness.ipc.handlers.set('load_axsf_phonon', () => Promise.resolve(phonon_modes));
    let tree = harness.render();
    const modal = descendants(tree).find((node) => node.type === widgets.PhononImportModal);
    assert.ok(modal, 'PhononPanel must retain its import modal');
    await modal.props.onSubmit({ axsf: '/tmp/mode.axsf', scfIn: '', scfOut: '', modes: '' });
    await settle();

    tree = harness.render();
    await control(tree, widgets.SelectInput, 'Select Mode').props.onChange('0');
    await settle();
    return harness.render();
}

function renderer_control_calls(calls) {
    return calls.filter(({ command }) => [
        'set_phonon_phase',
        'set_phonon_display_scale',
        'reset_phonon_animation',
    ].includes(command));
}

test('INTERACT-2B sends one discrete start and stop request and retains typed browser rejection', async () => {
    const harness = create_harness();
    let tree = await load_and_select_mode(harness);

    await control(tree, widgets.ActionButton, 'Play Animation').props.onClick();
    await settle();
    assert.deepEqual(harness.ipc.calls_for('set_phonon_playing'), [{
        command: 'set_phonon_playing',
        args: { playing: true },
    }]);

    tree = harness.render();
    await control(tree, widgets.ActionButton, 'Pause Animation').props.onClick();
    await settle();
    assert.deepEqual(harness.ipc.calls_for('set_phonon_playing'), [
        { command: 'set_phonon_playing', args: { playing: true } },
        { command: 'set_phonon_playing', args: { playing: false } },
    ]);

    const browser = create_harness();
    tree = await load_and_select_mode(browser);
    browser.ipc.handlers.set('set_phonon_playing', () => Promise.reject(new IpcException({
        code: 'not_in_tauri',
        message: 'native renderer mutation is unavailable in browser mode',
        recoverable: false,
    })));
    await control(tree, widgets.ActionButton, 'Play Animation').props.onClick();
    await settle();
    tree = browser.render();
    assert.ok(
        descendants(tree).some((node) => node.type === widgets.PanelError && node.props.error.code === 'not_in_tauri'),
        'browser mutation failure must remain typed and visible',
    );
    control(tree, widgets.ActionButton, 'Play Animation');
});

test('INTERACT-2B sends exactly one discrete display-scale mutation', async () => {
    const harness = create_harness();
    let tree = await load_and_select_mode(harness);

    const scale_before = harness.ipc.calls.length;
    await control(tree, widgets.RangeInput, 'Amplitude').props.onChange(2.5);
    await settle();
    const scale_calls = renderer_control_calls(harness.ipc.calls.slice(scale_before));
    assert.equal(scale_calls.length, 1, 'one amplitude gesture must issue one renderer display-scale mutation');
    assert.equal(
        scale_calls[0].args.amplitude ?? scale_calls[0].args.displayScale,
        2.5,
        'the renderer mutation must carry the requested display scale',
    );
});

test('INTERACT-2B sends exactly one discrete reset mutation', async () => {
    const harness = create_harness();
    let tree = await load_and_select_mode(harness);

    tree = harness.render();
    const reset_before = harness.ipc.calls.length;
    await reset_control(tree).props.onClick();
    await settle();
    const reset_calls = renderer_control_calls(harness.ipc.calls.slice(reset_before));
    assert.equal(reset_calls.length, 1, 'one reset gesture must issue one renderer reset mutation');
    assert.equal(
        reset_calls[0].args.phase ?? 0,
        0,
        'reset must restore the display phase to zero',
    );
});

test('INTERACT-2B rejects mode and display-scale work while playback owns a pending request', async () => {
    const harness = create_harness();
    let tree = await load_and_select_mode(harness);
    const playing = deferred();
    harness.ipc.handlers.set('set_phonon_playing', () => playing.promise);

    const play_request = control(tree, widgets.ActionButton, 'Play Animation').props.onClick();
    await settle();
    tree = harness.render();

    const mode = control(tree, widgets.SelectInput, 'Select Mode');
    const scale = control(tree, widgets.RangeInput, 'Amplitude');
    assert.equal(mode.props.disabled, true, 'mode selection must be disabled while playback owns the renderer');
    assert.equal(scale.props.disabled, true, 'display scale must be disabled while playback owns the renderer');

    const mode_count = harness.ipc.calls_for('set_phonon_mode').length;
    await mode.props.onChange('0');
    await settle();
    assert.equal(
        harness.ipc.calls_for('set_phonon_mode').length,
        mode_count,
        'a foreign mode request must not cross an active playback request',
    );

    playing.resolve(null);
    await play_request;
});

test('INTERACT-2B classifies every frontend phonon control as a discrete renderer mutation', () => {
    for (const command of [
        'set_phonon_mode',
        'set_phonon_phase',
        'set_phonon_display_scale',
        'set_phonon_playing',
    ]) {
        assert.equal(
            command_classification.commands[command],
            'renderer_only',
            `${command} must remain a discrete renderer-only command after INTERACT-2B`,
        );
    }
});
