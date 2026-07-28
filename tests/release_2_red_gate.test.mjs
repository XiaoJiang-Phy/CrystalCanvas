import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

// RELEASE-2 is a closure gate, not a second implementation track.  Its
// evidence must be independently checkable from the release candidate.
const ROOT = new URL('../', import.meta.url);
const RELEASE_VERSION = '0.7.0';
const EVIDENCE_PATH = 'release/v0.7.0/release-evidence.json';
const REQUIRED_STANDARD_GATES = [
    'cargo_check',
    'cargo_test',
    'cpp_ctest',
    'pnpm_install_frozen',
    'ipc_inventory',
    'ipc_contract',
    'ipc_tests',
    'typescript',
    'frontend_build',
    'diff_check',
];
const OPTIONAL_PLATFORM_GATES = [
    'macos_apple_silicon_metal',
    'ubuntu_vulkan',
    'windows_contract_validation',
];

async function source(relativePath) {
    return readFile(new URL(relativePath, ROOT), 'utf8');
}

async function readJson(relativePath) {
    return JSON.parse(await source(relativePath));
}

function sha256(bytes) {
    return createHash('sha256').update(bytes).digest('hex');
}

function nonEmptyString(value, label) {
    assert.equal(typeof value, 'string', `${label} must be a string`);
    assert.notEqual(value.trim(), '', `${label} must not be empty`);
}

function sha256String(value, label) {
    assert.match(value, /^[0-9a-f]{64}$/i, `${label} must be a SHA-256 digest`);
}

function repositoryPath(relativePath, label) {
    nonEmptyString(relativePath, label);
    assert.equal(path.isAbsolute(relativePath), false, `${label} must be repository-relative`);
    assert.equal(relativePath.split(path.sep).includes('..'), false, `${label} must not escape the repository`);
    return relativePath;
}

function requirePass(entry, label) {
    assert.equal(entry?.status, 'PASS', `${label} must be PASS for v${RELEASE_VERSION}`);
    nonEmptyString(entry?.evidence, `${label}.evidence`);
}

function requireHonestOptionalStatus(entry, label) {
    assert.ok(entry, `${label} is required even when unavailable`);
    assert.ok(['PASS', 'NOT_RUN', 'NOT_AVAILABLE'].includes(entry.status),
        `${label} must be PASS, NOT_RUN, or NOT_AVAILABLE`);
    nonEmptyString(entry.evidence, `${label}.evidence`);
    if (entry.status !== 'PASS') {
        nonEmptyString(entry.reason, `${label}.reason`);
    }
}

async function validateArtifactPair(artifact, seenPaths, seenIdentities) {
    assert.ok(artifact && typeof artifact === 'object', 'each release artifact must be an object');
    assert.ok(['raster', 'glb'].includes(artifact.kind), 'artifact.kind must be raster or glb');
    nonEmptyString(artifact.identity, 'artifact.identity');
    assert.equal(seenIdentities.has(artifact.identity), false,
        `duplicate artifact identity ${artifact.identity}`);
    seenIdentities.add(artifact.identity);

    const artifactPath = repositoryPath(artifact.path, 'artifact.path');
    const sidecarPath = repositoryPath(artifact.sidecar_path, 'artifact.sidecar_path');
    assert.equal(path.dirname(artifactPath), path.dirname(sidecarPath),
        'an artifact and its sidecar must be siblings');
    assert.equal(path.extname(sidecarPath), '.json', 'sidecar must be JSON');
    assert.equal(seenPaths.has(artifactPath), false, `duplicate artifact path ${artifactPath}`);
    assert.equal(seenPaths.has(sidecarPath), false, `duplicate sidecar path ${sidecarPath}`);
    seenPaths.add(artifactPath);
    seenPaths.add(sidecarPath);
    sha256String(artifact.sha256, 'artifact.sha256');
    sha256String(artifact.sidecar_sha256, 'artifact.sidecar_sha256');

    const artifactBytes = await readFile(new URL(artifactPath, ROOT));
    const sidecarBytes = await readFile(new URL(sidecarPath, ROOT));
    assert.ok((await stat(new URL(artifactPath, ROOT))).size > 0, `${artifactPath} must not be empty`);
    assert.ok((await stat(new URL(sidecarPath, ROOT))).size > 0, `${sidecarPath} must not be empty`);
    assert.equal(sha256(artifactBytes), artifact.sha256, `${artifactPath} hash does not match evidence`);
    assert.equal(sha256(sidecarBytes), artifact.sidecar_sha256,
        `${sidecarPath} hash does not match evidence`);

    const sidecar = JSON.parse(sidecarBytes);
    assert.equal(sidecar.success, true, `${sidecarPath} must record a successful export`);
    assert.equal(sidecar.artifact?.file_name, path.basename(artifactPath),
        `${sidecarPath} must name its sibling artifact`);
    assert.equal(sidecar.artifact?.sha256, artifact.sha256,
        `${sidecarPath} artifact hash must match release evidence`);
    if (artifact.kind === 'glb') {
        assert.equal(sidecar.export_id, artifact.identity,
            `${sidecarPath} GLB export_id must match release evidence identity`);
    }
}

function validateEvidenceShape(evidence) {
    assert.equal(evidence.schema_version, 1, 'release evidence schema must be explicitly versioned');
    assert.equal(evidence.release_version, RELEASE_VERSION);
    assert.equal(evidence.status, 'RELEASE_READY', 'v0.7.0 must not ship as a known-exception release');
    assert.deepEqual(evidence.blockers, [], 'release blockers must be empty');
    assert.equal('known_exception' in evidence, false,
        'v0.7.0 must not silently inherit the v0.6.2 known-exception path');
    assert.match(evidence.release_candidate_commit, /^[0-9a-f]{40}$/i,
        'release evidence must bind to a full candidate commit SHA');

    for (const node of ['RENDER-2', 'LOOK-2', 'DELIVERY-2']) {
        assert.equal(evidence.nodes?.[node]?.status, 'APPROVED', `${node} requires Auditor approval`);
        nonEmptyString(evidence.nodes[node].implementation_commit, `${node}.implementation_commit`);
    }
    for (const gate of REQUIRED_STANDARD_GATES) {
        requirePass(evidence.standard_gates?.[gate], `standard_gates.${gate}`);
    }

    requirePass(evidence.desktop_matrix?.macos_intel_metal, 'desktop_matrix.macos_intel_metal');
    requirePass(evidence.desktop_matrix?.browser_not_in_tauri,
        'desktop_matrix.browser_not_in_tauri');
    for (const gate of OPTIONAL_PLATFORM_GATES) {
        requireHonestOptionalStatus(evidence.desktop_matrix?.[gate], `desktop_matrix.${gate}`);
    }

    const manual = evidence.manual_validation;
    assert.ok(manual && typeof manual === 'object', 'manual validation is required');
    assert.ok(['NATIVE_PRIMARY', 'NATIVE_LIMITED_BLENDER_FALLBACK'].includes(manual.native_fidelity_verdict),
        'manual native fidelity verdict must be explicit');
    requirePass(manual.blender_headless_import, 'manual_validation.blender_headless_import');
    requirePass(manual.blender_gui_import, 'manual_validation.blender_gui_import');
    nonEmptyString(manual.blender_version, 'manual_validation.blender_version');
    nonEmptyString(manual.reviewed_by, 'manual_validation.reviewed_by');
    nonEmptyString(manual.reviewed_at, 'manual_validation.reviewed_at');
}

test('RELEASE-2 rejects empty, forged, or incomplete evidence shapes', () => {
    assert.throws(() => validateEvidenceShape({}), /schema|release_version|RELEASE_READY/i);
    assert.throws(() => validateEvidenceShape({
        schema_version: 1,
        release_version: RELEASE_VERSION,
        status: 'RELEASE_READY',
        blockers: [],
        release_candidate_commit: '0'.repeat(40),
        nodes: {},
        standard_gates: {},
        desktop_matrix: {},
        manual_validation: {},
    }), /RENDER-2|Auditor approval/i);
});

test('RELEASE-2 requires one hash-bound sibling sidecar for every declared artifact', async () => {
    const evidence = await readJson(EVIDENCE_PATH);
    validateEvidenceShape(evidence);
    assert.ok(Array.isArray(evidence.artifacts) && evidence.artifacts.length > 0,
        'release evidence must not declare an empty artifact set');

    const seenPaths = new Set();
    const seenIdentities = new Set();
    for (const artifact of evidence.artifacts) {
        await validateArtifactPair(artifact, seenPaths, seenIdentities);
    }

    assert.ok(evidence.artifacts.some((artifact) => artifact.kind === 'raster'),
        'release evidence needs at least one native raster artifact');
    assert.ok(evidence.artifacts.some((artifact) => artifact.kind === 'glb'),
        'release evidence needs at least one one-way Blender GLB artifact');
});

test('RELEASE-2 synchronizes public v0.7.0 metadata and runs closure before tag publication', async () => {
    const [packageJson, cargoToml, tauriConfig, cargoLock, citation, changelog, readme, roadmap, workflow] = await Promise.all([
        readJson('package.json'),
        source('src-tauri/Cargo.toml'),
        readJson('src-tauri/tauri.conf.json'),
        source('src-tauri/Cargo.lock'),
        source('CITATION.cff'),
        source('CHANGELOG.md'),
        source('README.md'),
        source('ROADMAP.md'),
        source('.github/workflows/release.yml'),
    ]);

    assert.equal(packageJson.version, RELEASE_VERSION);
    assert.match(cargoToml, /^version = "0\.7\.0"$/m);
    assert.equal(tauriConfig.version, RELEASE_VERSION);
    assert.match(cargoLock, /\[\[package\]\]\nname = "crystal-canvas"\nversion = "0\.7\.0"/);
    assert.match(citation, /^version: "0\.7\.0"$/m);
    assert.match(citation, /^date-released: "\d{4}-\d{2}-\d{2}"$/m);
    assert.match(changelog, /^## \[0\.7\.0\] - \d{4}-\d{2}-\d{2}$/m);
    assert.match(readme, /^> \*\*Latest release\*\*: `v0\.7\.0`$/m);
    assert.match(roadmap, /Latest release: `v0\.7\.0`/);

    const closureGate = workflow.indexOf('node --test tests/release_2_red_gate.test.mjs');
    const releaseAction = workflow.indexOf('tauri-apps/tauri-action@v0');
    assert.ok(closureGate >= 0, 'tag workflow must run the v0.7.0 closure gate');
    assert.ok(releaseAction >= 0 && closureGate < releaseAction,
        'tag workflow must complete RELEASE-2 closure before publishing a release');

    for (const requiredPreflight of [
        'cargo check --manifest-path src-tauri/Cargo.toml',
        'cargo test --all-targets --no-fail-fast --manifest-path src-tauri/Cargo.toml',
        'cmake --build cpp/tests/build',
        'ctest --test-dir cpp/tests/build --output-on-failure',
        'npm run ipc:inventory',
        'npm run check:ipc',
        'npm run test:ipc',
        './node_modules/.bin/tsc --noEmit',
        'pnpm run build',
        'git diff --check',
    ]) {
        const gate = workflow.indexOf(requiredPreflight);
        assert.ok(gate >= 0, `tag workflow is missing release preflight ${requiredPreflight}`);
        assert.ok(gate < releaseAction,
            `release preflight ${requiredPreflight} must run before publication`);
    }
});
