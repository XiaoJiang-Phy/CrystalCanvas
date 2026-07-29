// [Overview: Node IPC contract gate for FIELD-1 field-scene UI ownership.]
// Implementation: validates generated IPC bindings and frontend mutation flow.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const ROOT = new URL('../', import.meta.url);

async function source(relativePath) {
    return readFile(new URL(relativePath, ROOT), 'utf8');
}

test('FIELD-1 exposes one typed field-scene IPC contract without a frontend scalar store', async () => {
    const [contracts, generated, inventory, commands, panel] = await Promise.all([
        source('src/ipc/contracts.ts'),
        source('src/ipc/commands.generated.ts'),
        source('ipc/inventory.json'),
        source('src-tauri/src/commands/volumetric.rs'),
        source('src/components/panels/VolumetricPanel.tsx'),
    ]);

    for (const required of [
        'FieldSceneInfo',
        'FieldLayerInfo',
        'FieldSceneChangedPayload',
        'get_field_scene_info',
        'add_field_layer',
        'remove_field_layer',
        'reorder_field_layer',
        'select_active_field_layer',
        'combine_field_layers',
        'field_scene_changed',
    ]) {
        assert.match(contracts, new RegExp(required), `typed IPC contract lacks ${required}`);
        assert.match(generated, new RegExp(required), `generated IPC registry lacks ${required}`);
        assert.match(inventory, new RegExp(required), `reviewed IPC inventory lacks ${required}`);
        if (required !== 'FieldSceneInfo' && required !== 'FieldLayerInfo' && required !== 'FieldSceneChangedPayload') {
            assert.match(commands, new RegExp(required), `Rust command/event source lacks ${required}`);
        }
    }

    assert.match(panel, /safeListen\('field_scene_changed'/,
        'the field panel must refresh from the dedicated field-scene event');
    assert.doesNotMatch(panel, /get_crystal_state/,
        'the field panel must not become a second structural snapshot owner');
    assert.doesNotMatch(panel, /Float32Array|new\s+Array\([^)]*grid|data:\s*\[/,
        'the frontend must not own scalar grid data');
});

test('FIELD-1 preserves camelCase/snake_case and rejects browser-native field mutation', async () => {
    const [contracts, commands, mock] = await Promise.all([
        source('src/ipc/contracts.ts'),
        source('src-tauri/src/commands/volumetric.rs'),
        source('src/utils/tauri-mock.ts'),
    ]);

    for (const [camelCase, snakeCase] of [
        ['layerId', 'layer_id'],
        ['expectedRevision', 'expected_revision'],
        ['outputLabel', 'output_label'],
    ]) {
        assert.match(contracts, new RegExp(camelCase), `TypeScript contract lacks ${camelCase}`);
        assert.match(commands, new RegExp(snakeCase), `Rust command contract lacks ${snakeCase}`);
    }
    assert.match(mock, /not_in_tauri/,
        'browser fallback must retain explicit rejection for field mutations');
});

test('FIELD-1 UI serializes field operations and commits local controls only after backend success', async () => {
    const panel = await source('src/components/panels/VolumetricPanel.tsx');

    for (const required of ['pendingControl', 'add_field_layer', 'combine_field_layers', 'select_active_field_layer']) {
        assert.match(panel, new RegExp(required), `field UI lacks ${required}`);
    }
    assert.match(panel, /catch\s*\([^)]*\)\s*=>\s*setPanelError/,
        'field UI must surface typed backend failures rather than retain a false success state');
    assert.match(panel, /return\s*\(\)\s*=>\s*\{\s*unlisten\(\);\s*\}/,
        'field-scene listener must be cleaned up on unmount');
});
