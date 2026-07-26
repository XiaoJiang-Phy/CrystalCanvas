import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const RELEASE_VERSION = '0.6.2';
const NEXT_VERSION = '0.7.0';
const PUBLIC_DOCS = [
    'docs/UserManual.md',
    'docs/DeveloperGuide.md',
    'docs/TestingGuide.md',
    'docs/FAQ.md',
    'docs/IPC_Commands.md',
    'docs/Algorithms.md',
    'docs/Shader_Reference.md',
];

async function source(path) {
    return readFile(new URL(`../${path}`, import.meta.url), 'utf8');
}

function sha256(source_text) {
    return createHash('sha256').update(source_text).digest('hex');
}

const [
    package_source,
    cargo_source,
    tauri_source,
    lock_source,
    menu_source,
    readme_source,
    roadmap_source,
    internal_roadmap_source,
    tdd_source,
    changelog_source,
    citation_source,
    workflow_source,
    evolution_source,
    evidence_source,
    performance_report_source,
    performance_context_source,
    ...public_doc_sources
] = await Promise.all([
    source('package.json'),
    source('src-tauri/Cargo.toml'),
    source('src-tauri/tauri.conf.json'),
    source('src-tauri/Cargo.lock'),
    source('src/hooks/useTauriMenu.ts'),
    source('README.md'),
    source('ROADMAP.md'),
    source('doc/roadmap.md'),
    source('doc/TDD_CrystalCanvas_v1.md'),
    source('CHANGELOG.md'),
    source('CITATION.cff'),
    source('.github/workflows/release.yml'),
    source('EVOLUTION.log'),
    source('doc/v0.6.2_release_evidence.json'),
    source('doc/v0.6.2_visualization_perf_report.json'),
    source('doc/v0.6.2_visualization_perf_run_context.json'),
    ...PUBLIC_DOCS.map(source),
]);

test('v0.6.2 release versions are synchronized', () => {
    assert.equal(JSON.parse(package_source).version, RELEASE_VERSION);
    assert.match(cargo_source, /^version = "0\.6\.2"$/m);
    assert.equal(JSON.parse(tauri_source).version, RELEASE_VERSION);
    assert.match(lock_source, /\[\[package\]\]\nname = "crystal-canvas"\nversion = "0\.6\.2"/);
    assert.ok(menu_source.includes(String.raw`CrystalCanvas\nVersion 0.6.2`));
});

test('v0.6.2 release documentation is promoted consistently', () => {
    assert.match(readme_source, /^> \*\*Latest release\*\*: `v0\.6\.2`$/m);
    assert.match(readme_source, /^> \*\*Current development line\*\*: `v0\.7\.0` publication-rendering core$/m);
    assert.match(readme_source, /Download_v0\.6\.2/);
    assert.match(roadmap_source, /Latest release: `v0\.6\.2`/);
    assert.match(roadmap_source, /^\| `v0\.6\.2` \| 2026-07-26 \|/m);
    assert.match(internal_roadmap_source, /^> \*\*Latest release\*\*: `v0\.6\.2`$/m);
    assert.match(internal_roadmap_source, /^> \*\*Current development version\*\*: `v0\.7\.0`$/m);
    assert.match(tdd_source, /^> \*\*Latest release baseline\*\*: `v0\.6\.2`$/m);
    assert.match(tdd_source, /^> \*\*Current development baseline\*\*: `v0\.7\.0` publication-rendering core$/m);
    assert.match(changelog_source, /^## \[0\.6\.2\] - 2026-07-26$/m);
    assert.doesNotMatch(changelog_source, /^## \[Unreleased\]$/m);
    assert.match(citation_source, /^version: "0\.6\.2"$/m);
    assert.match(citation_source, /^date-released: "2026-07-26"$/m);
    for (let index = 0; index < PUBLIC_DOCS.length; index += 1) {
        assert.match(public_doc_sources[index], /^> Baseline: `v0\.6\.2`/m,
            `${PUBLIC_DOCS[index]} must use the v0.6.2 baseline`);
    }
});

test('release workflow freezes dependencies and validates metadata before publishing', () => {
    assert.match(workflow_source, /pnpm install --frozen-lockfile/);
    assert.match(workflow_source, /node --test scripts\/release-closure\.test\.mjs/);
    assert.match(workflow_source, /args: "--target aarch64-apple-darwin"/);
    assert.match(workflow_source, /args: "--target x86_64-apple-darwin"/);
    assert.match(workflow_source, /^\s*prerelease:\s*false\s*$/m);
});

test('known release exception is explicit without relabeling unrun checks', () => {
    const evidence = JSON.parse(evidence_source);
    assert.equal(evidence.schema_version, 3);
    assert.equal(evidence.release_version, RELEASE_VERSION);
    assert.equal(evidence.status, 'RELEASED_WITH_KNOWN_EXCEPTION');
    assert.deepEqual(evidence.blockers, []);
    assert.equal(evidence.known_exception?.authorized, true);
    assert.equal(evidence.known_exception?.authorized_by, 'user');
    assert.match(evolution_source, /^## 2026-07-26 — v0\.6\.2 direct release$/m);
    assert.equal(evidence.standard_gates?.cargo_test?.status, 'NOT_RUN');
    assert.equal(evidence.standard_gates?.cpp_ctest?.status, 'NOT_RUN');
    assert.equal(evidence.desktop_matrix?.macos_intel_metal?.status, 'NOT_RUN');
    assert.equal(evidence.desktop_matrix?.macos_apple_silicon_metal?.status, 'NOT_RUN');
    assert.equal(evidence.desktop_matrix?.ubuntu_vulkan?.status, 'NOT_RUN');
    assert.equal(evidence.performance?.raw_report_sha256, sha256(performance_report_source));
    assert.equal(evidence.performance?.run_context_sha256, sha256(performance_context_source));
    assert.equal(NEXT_VERSION, '0.7.0');
});
