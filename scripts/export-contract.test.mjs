import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const inventory = JSON.parse(await readFile(
    new URL('../ipc/inventory.json', import.meta.url),
    'utf8',
));
const classifications = JSON.parse(await readFile(
    new URL('../ipc/command-classification.json', import.meta.url),
    'utf8',
)).commands;
const file_io = await readFile(
    new URL('../src-tauri/src/commands/file_io.rs', import.meta.url),
    'utf8',
);
const recipe = await readFile(
    new URL('../src-tauri/src/export_recipe.rs', import.meta.url),
    'utf8',
);
const renderer = await readFile(
    new URL('../src-tauri/src/renderer/renderer.rs', import.meta.url),
    'utf8',
);

function command_body(source, command) {
    const start = source.indexOf(`pub fn ${command}(`);
    assert.notEqual(start, -1, `missing Rust command ${command}`);
    const next = source.indexOf('\n#[tauri::command]', start);
    return source.slice(start, next === -1 ? source.length : next);
}

test('EXPORT-1A keeps the existing browser wire contract and native-only policy', () => {
    assert.deepEqual(
        inventory.backend_command_args.export_image,
        ['bgMode', 'height', 'path', 'width'],
    );
    assert.equal(classifications.export_image, 'external_io');
});

test('EXPORT-1A snapshots state in global lock order and writes the pair after unlock', () => {
    const body = command_body(file_io, 'export_image');
    const crystal_lock = body.indexOf('let crystal = crystal_state');
    const settings_lock = body.indexOf('let settings = settings_state');
    const renderer_lock = body.indexOf('let renderer = renderer_state');
    const renderer_drop = body.indexOf('drop(renderer)');
    const pair_write = body.indexOf('write_publication_raster_pair');

    assert.ok(crystal_lock < settings_lock && settings_lock < renderer_lock);
    assert.ok(renderer_lock < renderer_drop && renderer_drop < pair_write);
});

test('EXPORT-1A declares the v6 recipe envelope, admission receipt, and paired artifact hash', () => {
    assert.match(recipe, /"crystalcanvas\.export-recipe"/);
    assert.match(recipe, /EXPORT_RECIPE_SCHEMA_VERSION:\s*u32\s*=\s*6/);
    assert.match(recipe, /PublicationRaster/);
    assert.match(recipe, /BlenderStructureScene/);
    assert.match(recipe, /sha256/);
    assert.match(recipe, /publication_sidecar_path/);
    assert.match(recipe, /publication_admission/);
    assert.match(renderer, /PublicationExportBudgets/);
});
