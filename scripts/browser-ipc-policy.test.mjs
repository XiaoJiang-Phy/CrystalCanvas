import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const contracts_source = await readFile(
    new URL('../src/ipc/contracts.ts', import.meta.url),
    'utf8',
);
const tauri_mock_source = await readFile(
    new URL('../src/utils/tauri-mock.ts', import.meta.url),
    'utf8',
);
const command_classification = JSON.parse(await readFile(
    new URL('../ipc/command-classification.json', import.meta.url),
    'utf8',
)).commands;

const transpiled_contracts = ts.transpileModule(contracts_source, {
    compilerOptions: {
        module: ts.ModuleKind.ESNext,
        target: ts.ScriptTarget.ES2022,
    },
}).outputText;
const contracts = await import(
    `data:text/javascript;base64,${Buffer.from(transpiled_contracts).toString('base64')}`,
);

globalThis.__browser_ipc_contracts = contracts;
globalThis.__browser_ipc_classification = command_classification;
globalThis.window = {};

function executable_tauri_mock(source) {
    return source
        .replace(
            "import { IpcException, normalize_ipc_error, validate_ipc_event, validate_ipc_result } from '../ipc/contracts';",
            'const { IpcException, normalize_ipc_error, validate_ipc_event, validate_ipc_result } = globalThis.__browser_ipc_contracts;',
        )
        .replace(
            "import { IPC_COMMAND_CLASSIFICATION } from '../ipc/commands.generated';",
            'const IPC_COMMAND_CLASSIFICATION = globalThis.__browser_ipc_classification;',
        )
        .replace(/^import type .*;\n/gm, '');
}

const transpiled_tauri_mock = ts.transpileModule(executable_tauri_mock(tauri_mock_source), {
    compilerOptions: {
        module: ts.ModuleKind.ESNext,
        target: ts.ScriptTarget.ES2022,
    },
}).outputText;
const { safeInvoke, safeListen } = await import(
    `data:text/javascript;base64,${Buffer.from(transpiled_tauri_mock).toString('base64')}`,
);

const read_commands = Object.entries(command_classification)
    .filter(([, classification]) => classification === 'read')
    .map(([command]) => command)
    .sort();
const mutation_commands = Object.entries(command_classification)
    .filter(([, classification]) => classification !== 'read')
    .map(([command]) => command)
    .sort();

test('IPC-3B gives every browser read command a validator-approved neutral fixture', async () => {
    assert.ok(read_commands.length > 0, 'inventory must expose read commands to exercise browser policy');

    for (const command of read_commands) {
        const value = await safeInvoke(command, {});
        assert.notEqual(value, undefined,
            `${command} must not silently resolve undefined in browser mode`);
        assert.doesNotThrow(
            () => contracts.validate_ipc_result(command, value),
            `${command} browser fixture must satisfy its runtime DTO validator`,
        );
    }
});

test('IPC-3B read fixtures cannot become mutable browser-side CrystalState', async () => {
    const first = await safeInvoke('get_crystal_state');
    assert.ok(first && typeof first === 'object',
        'browser mode must provide an explicit neutral CrystalState fixture');
    assert.doesNotThrow(() => contracts.validate_ipc_result('get_crystal_state', first));

    first.name = 'mutated in browser';
    first.labels.push('X1');
    const second = await safeInvoke('get_crystal_state');

    assert.notStrictEqual(second, first,
        'each browser read must return an independent fixture rather than shared mutable state');
    assert.equal(second.name, '',
        'a caller mutation must not become browser-side canonical structure state');
    assert.deepEqual(second.labels, [],
        'a caller mutation must not persist atoms in a browser fixture');
});

test('IPC-3B rejects every browser mutation with typed not_in_tauri and no false success', async () => {
    assert.ok(mutation_commands.length > 0, 'inventory must expose mutation commands to exercise browser policy');

    for (const command of mutation_commands) {
        await assert.rejects(
            () => safeInvoke(command, {}),
            (error) => error instanceof contracts.IpcException
                && error.code === 'not_in_tauri'
                && error.recoverable === false,
            `${command} must reject as typed not_in_tauri rather than resolving a false success`,
        );
    }
});

test('IPC-3B browser listener cleanup is idempotent and cannot emit mock state', async () => {
    let handler_calls = 0;
    const unlisten = await safeListen('state_changed', () => {
        handler_calls += 1;
    });

    assert.doesNotThrow(() => unlisten());
    assert.doesNotThrow(() => unlisten());
    assert.equal(handler_calls, 0,
        'browser listener policy must not fabricate state_changed or invoke the business handler');
});

test('IPC-3B keeps browser policy free of native timers, listeners, and synthetic state ownership', () => {
    assert.doesNotMatch(tauri_mock_source, /\b(?:setTimeout|setInterval|requestAnimationFrame)\s*\(/,
        'browser IPC policy must not create background work');
    assert.doesNotMatch(tauri_mock_source, /\.emit\(\s*['"]state_changed['"]/, 
        'browser IPC policy must not fabricate canonical state-change events');
    assert.match(tauri_mock_source, /\[tauri-mock\] listen\('\$\{event\}'\) failed:/,
        'native listener registration failure must remain visible during development');
});
