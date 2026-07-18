import assert from 'node:assert/strict';
import test from 'node:test';
import { analyze_contract, assert_contract, create_inventory } from './ipc-inventory.mjs';

test('IPC inventory classifies every registered command and event', async () => {
    const inventory = await create_inventory();
    assert.doesNotThrow(() => assert_contract(inventory));
    assert.deepEqual(inventory.backend_command_args.export_image, ['bgMode', 'height', 'path', 'width']);
    assert.deepEqual(inventory.backend_command_args.pick_atom, ['screenH', 'screenW', 'x', 'y']);
});

test('IPC inventory rejects an unregistered classified command', () => {
    const inventory = analyze_contract({
        backend_commands: ['write_text_file'],
        frontend_commands: {},
        classification: { write_text_file: 'external_io', save_text_file: 'external_io' },
    });
    assert.deepEqual(inventory.unregistered_classified_commands, ['save_text_file']);
});

test('IPC inventory rejects a dynamic frontend command', () => {
    const inventory = analyze_contract({
        backend_commands: ['write_text_file'],
        frontend_commands: {},
        classification: { write_text_file: 'external_io' },
        dynamic_frontend_commands: [{ file: 'src/example.ts', expression: 'command' }],
    });
    assert.throws(() => assert_contract(inventory), /dynamic frontend command/);
});

test('IPC inventory rejects non-literal command arguments and explicit type arguments', () => {
    const invalid_args = analyze_contract({
        backend_commands: ['write_text_file'],
        frontend_commands: {},
        classification: { write_text_file: 'external_io' },
        invalid_frontend_command_args: [{ file: 'src/example.ts', expression: 'args' }],
    });
    const type_argument = analyze_contract({
        backend_commands: ['write_text_file'],
        frontend_commands: {},
        classification: { write_text_file: 'external_io' },
        unsafe_frontend_type_arguments: [{ file: 'src/example.ts', expression: 'safeInvoke<any>(...)' }],
    });
    assert.throws(() => assert_contract(invalid_args), /non-literal frontend command args/);
    assert.throws(() => assert_contract(type_argument), /forbidden IPC type argument/);
});

test('IPC inventory rejects adapter re-exports', () => {
    const inventory = analyze_contract({
        backend_commands: ['write_text_file'], frontend_commands: {},
        classification: { write_text_file: 'external_io' },
        forbidden_ipc_reexports: [{ file: 'src/ipc-proxy.ts', module: './utils/tauri-mock' }],
    });
    assert.throws(() => assert_contract(inventory), /forbidden IPC re-export/);
});

test('IPC inventory rejects frontend argument names that differ from the Rust command', () => {
    const inventory = analyze_contract({
        backend_commands: ['pick_atom'],
        backend_command_args: { pick_atom: ['screenH', 'screenW', 'x', 'y'] },
        frontend_command_calls: [{
            command: 'pick_atom', file: 'src/example.ts', args: ['screenHeight', 'screenW', 'x', 'y'],
        }],
        frontend_commands: { pick_atom: ['src/example.ts'] },
        classification: { pick_atom: 'read' },
    });
    assert.throws(() => assert_contract(inventory), /argument mismatch.*pick_atom/);
});
