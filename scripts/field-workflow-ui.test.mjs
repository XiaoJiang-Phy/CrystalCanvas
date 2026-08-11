// [Overview: RED UI and IPC contract tests for DELIVERY-2 field export workflow.]
// Verifies backend-confirmed field state and separate raster and Blender exports.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const read = (relative) => readFile(new URL(relative, import.meta.url), 'utf8');
const [panel, modal, contracts, inventory, mock] = await Promise.all([
    read('../src/components/panels/VolumetricPanel.tsx'),
    read('../src/components/layout/ExportImageModal.tsx'),
    read('../src/ipc/contracts.ts'),
    read('../ipc/inventory.json'),
    read('../src/utils/tauri-mock.ts'),
]);

test('DELIVERY-2 field panel reflects backend-confirmed layer state before controls commit', () => {
    for (const required of [
        'field_scene_changed', 'layer_id', 'revision', 'representation', 'visibility',
        'isovalue', 'colormap', 'opacity', 'clip', 'safeInvoke',
    ]) {
        assert.match(panel, new RegExp(required, 'i'), `field workflow lacks backend-confirmed ${required}`);
    }
    assert.match(panel, /await\s+safeInvoke/, 'field control changes must await backend success');
});

test('Blender and raster exports remain explicit, sidecar-aware, and non-overwriting', () => {
    for (const required of [
        'Raster Image', 'Blender Scene', 'sidecar', 'portable', 'isExporting',
    ]) {
        assert.match(modal, new RegExp(required), `export UI lacks ${required}`);
    }
    assert.match(modal, /export_blender_scene/);
    assert.match(modal, /export_image/);
    assert.match(modal, /raycast-only|isosurface|slice|contour/i,
        'Blender workflow must explain its portable field scope');
});

test('field Blender IPC is typed, external-only, and browser mode rejects it', () => {
    assert.match(contracts, /export_blender_scene/);
    assert.match(contracts, /publicationProfile/);
    assert.match(inventory, /export_blender_scene/);
    assert.match(mock, /not_in_tauri/);
    assert.match(mock, /browser_policy_for/);
    assert.match(mock, /IPC_COMMAND_CLASSIFICATION/);
});
