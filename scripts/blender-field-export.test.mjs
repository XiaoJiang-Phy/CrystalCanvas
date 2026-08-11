// [Overview: RED contract tests for DELIVERY-2 portable Blender field export.]
// Tests the CPU-to-core-glTF field scene boundary without requiring Blender locally.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const read = (relative) => readFile(new URL(relative, import.meta.url), 'utf8');
const [scene, blender, recipe, fileIo] = await Promise.all([
    read('../src-tauri/src/scene_export.rs'),
    read('../src-tauri/src/blender_export.rs'),
    read('../src-tauri/src/export_recipe.rs'),
    read('../src-tauri/src/commands/file_io.rs'),
]);

function commandBody(source, command) {
    const start = source.indexOf(`pub fn ${command}(`);
    assert.notEqual(start, -1, `missing Rust command ${command}`);
    const next = source.indexOf('\n#[tauri::command]', start);
    return source.slice(start, next === -1 ? source.length : next);
}

test('DELIVERY-2 emits one deterministic field-aware GLB/sidecar semantic inventory', () => {
    for (const required of [
        'PublicationFieldSceneSnapshot',
        'PublicationFieldPrimitive',
        'field_primitives',
        'source_layer_revision',
        'source_artifact_sha256',
        'normalized_layer_sha256',
        'semantic_inventory',
        'BlenderFieldScene',
        'export_id',
    ]) {
        assert.match(`${scene}\n${blender}\n${recipe}`, new RegExp(required),
            `field export lacks deterministic semantic inventory field ${required}`);
    }
    assert.match(recipe, /write_publication_glb_pair/);
    assert.match(recipe, /validate_glb_export_identity/);
});

test('DELIVERY-2 only serializes portable realized field geometry', () => {
    const text = `${scene}\n${blender}`;
    for (const required of [
        'isosurface', 'slice', 'contour', 'COLOR_0', 'clip_planes', 'raycast-only',
    ]) {
        assert.match(text, new RegExp(required), `portable field representation missing ${required}`);
    }
    for (const forbidden of ['OpenVDB', 'NanoVDB', 'EXT_mesh_gpu_instancing', 'read_buffer', 'map_async']) {
        assert.doesNotMatch(text, new RegExp(forbidden), `nonportable field export mechanism ${forbidden}`);
    }
});

test('field GLB export is built after the locks release and cannot export a raycast-only field', () => {
    const body = commandBody(fileIo, 'export_blender_scene');
    const rendererDrop = body.indexOf('drop(renderer)');
    const build = body.indexOf('build_blender_glb');
    assert.ok(rendererDrop >= 0 && build > rendererDrop,
        'the CPU field realization must not hold renderer state while constructing GLB bytes');
    assert.match(`${body}\n${scene}`, /raycast-only|portable/i,
        'the export path must provide the explicit raycast-only portability rejection');
});
