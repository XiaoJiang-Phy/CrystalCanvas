import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const ROOT = new URL('../', import.meta.url);
const RELEASE_VERSION = '0.6.1';

export const FULL_RELEASE_GATES = [
    'cargo check --manifest-path src-tauri/Cargo.toml',
    'cargo test --no-fail-fast --manifest-path src-tauri/Cargo.toml',
    'cmake --build cpp/tests/build',
    'ctest --test-dir cpp/tests/build --output-on-failure',
    'npm run ipc:inventory',
    'npm run check:ipc',
    'npm run test:ipc',
    './node_modules/.bin/tsc --noEmit',
    'npm run build',
    'git diff --check',
];

async function source(path) {
    return readFile(new URL(path, ROOT), 'utf8');
}

function section(markdown, heading) {
    const start = markdown.indexOf(heading);
    assert.notEqual(start, -1, `missing ${heading}`);
    const end = markdown.indexOf('\n## ', start + heading.length);
    return markdown.slice(start, end === -1 ? undefined : end);
}

test('REL-1 keeps every tracked public release identifier at v0.6.1', async () => {
    const [package_json, cargo_toml, tauri_config, citation, readme, tauri_menu] = await Promise.all([
        source('package.json'),
        source('src-tauri/Cargo.toml'),
        source('src-tauri/tauri.conf.json'),
        source('CITATION.cff'),
        source('README.md'),
        source('src/hooks/useTauriMenu.ts'),
    ]);

    assert.equal(JSON.parse(package_json).version, RELEASE_VERSION);
    assert.match(cargo_toml, /^version = "0\.6\.1"$/m);
    assert.equal(JSON.parse(tauri_config).version, RELEASE_VERSION);
    assert.match(citation, /^version: "0\.6\.1"$/m);
    assert.match(citation, /^date-released: "2026-07-18"$/m);
    assert.match(readme, /> \*\*Current Release\*\*: `v0\.6\.1`/);
    assert.match(readme, /Download_v0\.6\.1/);
    assert.match(readme, /\| \*\*v0\.6\.1\*\* \|/);
    assert.match(readme, /\| \*\*v0\.6\.2\*\* \| Interaction & Geometry \|/);
    assert.match(tauri_menu, /CrystalCanvas\\nVersion 0\.6\.1\\n/);
});

test('REL-1 release notes and public roadmap close v0.6.1 without reopening v0.6.2 work', async () => {
    const [changelog, public_roadmap] = await Promise.all([
        source('CHANGELOG.md'),
        source('ROADMAP.md'),
    ]);

    assert.match(changelog, /^## \[0\.6\.1\] - \d{4}-\d{2}-\d{2}$/m);
    const release_notes = section(changelog, '## [0.6.1]');
    assert.match(release_notes, /IPC|contract/i);
    assert.match(release_notes, /atomic|transaction|undo/i);
    assert.match(release_notes, /state_changed|state.sync|refresh/i);

    assert.match(public_roadmap, /Current Release: v0\.6\.1/);
    assert.match(public_roadmap, /v0\.6\.2 — Interaction & Geometry Hardening/);
});

test('REL-1 keeps its complete release command set explicit and non-duplicated', () => {
    assert.deepEqual(FULL_RELEASE_GATES, [
        'cargo check --manifest-path src-tauri/Cargo.toml',
        'cargo test --no-fail-fast --manifest-path src-tauri/Cargo.toml',
        'cmake --build cpp/tests/build',
        'ctest --test-dir cpp/tests/build --output-on-failure',
        'npm run ipc:inventory',
        'npm run check:ipc',
        'npm run test:ipc',
        './node_modules/.bin/tsc --noEmit',
        'npm run build',
        'git diff --check',
    ]);
});
