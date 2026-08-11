import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

// RELEASE-2 validates the active release candidate. Historical release evidence
// remains immutable under release/v0.7.0 and is intentionally not reused here.
const ROOT = new URL('../', import.meta.url);
const packageJson = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
const RELEASE_VERSION = packageJson.version;
const EVIDENCE_PATH = `release/v${RELEASE_VERSION}/release-evidence.json`;
const PUBLISH_MODE = process.env.CRYSTALCANVAS_RELEASE_PUBLISH === '1';
const REQUIRED_NODES = ['FIELD-1', 'FIGURE-2', 'DELIVERY-2'];
const MANUAL_GATE_NAMES = [
    'macos_intel_metal',
    'macos_apple_silicon_metal',
    'blender_headless_import',
    'blender_gui_import',
];
const APPLE_SILICON_LIMITATION = 'Apple Silicon compatibility is not established and must not be claimed.';
const REQUIRED_STANDARD_GATES = [
    'cargo_fmt',
    'cargo_check',
    'cargo_test_all_targets',
    'cpp_ctest',
    'pnpm_install_frozen',
    'ipc_inventory',
    'ipc_contract',
    'ipc_tests',
    'typescript',
    'frontend_build',
    'diff_check',
    'clean_worktree',
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

function commitSha(value, label) {
    assert.match(value, /^(?!0{40}$)[0-9a-f]{40}$/i, `${label} must be a non-zero commit SHA`);
}

function repositoryPath(relativePath, label) {
    nonEmptyString(relativePath, label);
    assert.equal(path.isAbsolute(relativePath), false, `${label} must be repository-relative`);
    assert.equal(relativePath.split(path.sep).includes('..'), false, `${label} must not escape the repository`);
    return relativePath;
}

function assertReachableCommit(value, label) {
    commitSha(value, label);
    execFileSync('git', ['cat-file', '-e', `${value}^{commit}`], { cwd: fileUrlToPath(ROOT) });
    execFileSync('git', ['merge-base', '--is-ancestor', value, 'HEAD'], { cwd: fileUrlToPath(ROOT) });
}

function fileUrlToPath(url) {
    return new URL('.', url).pathname;
}

function requireCandidateEntry(entry, label, releaseReady) {
    assert.ok(entry && typeof entry === 'object', `${label} is required`);
    assert.ok(['PENDING', 'PASS', 'NOT_AVAILABLE'].includes(entry.status),
        `${label}.status must be PENDING, PASS, or NOT_AVAILABLE`);
    nonEmptyString(entry.evidence, `${label}.evidence`);
    if (entry.status === 'PASS') {
        assert.equal(releaseReady, true, `${label} cannot be PASS in candidate evidence`);
        assert.equal('reason' in entry, false, `${label}.reason must be removed after a PASS`);
    } else {
        nonEmptyString(entry.reason, `${label}.reason`);
    }
}

function isoUtcTimestamp(value, label) {
    nonEmptyString(value, label);
    assert.match(value, /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/, `${label} must be an ISO-8601 UTC timestamp`);
    assert.equal(new Date(value).toISOString().replace('.000Z', 'Z'), value,
        `${label} must be a valid UTC timestamp`);
}

function validateManualAttestation(manual, featureBaselineCommit, releaseReady) {
    if (!releaseReady) return;
    nonEmptyString(manual.reviewed_by, 'manual_validation.reviewed_by');
    isoUtcTimestamp(manual.reviewed_at, 'manual_validation.reviewed_at');
    assertReachableCommit(manual.validated_feature_baseline_commit,
        'manual_validation.validated_feature_baseline_commit');
    assert.equal(manual.validated_feature_baseline_commit, featureBaselineCommit,
        'manual validation must bind to the declared feature baseline');
    assert.match(manual.blender_version, /^Blender 4\.4(?:\.\d+)?\b/,
        'manual validation must identify Blender 4.4');

}

function assertMetadataVersions({ cargoToml, tauriConfig, cargoLock, citation, changelog, readme, roadmap, docs }) {
    const escaped = RELEASE_VERSION.replaceAll('.', String.raw`\.`);
    assert.match(RELEASE_VERSION, /^\d+\.\d+\.\d+$/, 'package version must be semantic versioning');
    assert.match(cargoToml, new RegExp(`^version = "${escaped}"$`, 'm'));
    assert.equal(tauriConfig.version, RELEASE_VERSION);
    assert.match(cargoLock, new RegExp(`\\[\\[package\\]\\]\\nname = "crystal-canvas"\\nversion = "${escaped}"`));
    assert.match(citation, new RegExp(`^version: "${escaped}"$`, 'm'));
    assert.match(citation, /^date-released: "\d{4}-\d{2}-\d{2}"$/m);
    assert.match(changelog, new RegExp(`^## \\[${escaped}\\] - \\d{4}-\\d{2}-\\d{2}$`, 'm'));
    assert.match(readme, new RegExp('^> \\*\\*Latest release\\*\\*: `v' + escaped + '`$', 'm'));
    assert.match(roadmap, new RegExp('Latest release: `v' + escaped + '`'));
    for (const [name, text] of Object.entries(docs)) {
        assert.match(text, new RegExp('^> Baseline: `v' + escaped + '`', 'm'), `${name} baseline is stale`);
    }
}

async function validateArtifactPair(artifact, seenPaths, seenIdentities) {
    assert.ok(artifact && typeof artifact === 'object', 'each release artifact must be an object');
    assert.ok(['raster', 'glb'].includes(artifact.kind), 'artifact.kind must be raster or glb');
    assert.ok(['software_contract', 'native_desktop', 'blender_4_4'].includes(artifact.validation_scope),
        'artifact.validation_scope must declare software, native desktop, or Blender 4.4 evidence');
    nonEmptyString(artifact.identity, 'artifact.identity');
    assert.equal(seenIdentities.has(artifact.identity), false,
        `duplicate artifact identity ${artifact.identity}`);
    seenIdentities.add(artifact.identity);

    const artifactPath = repositoryPath(artifact.path, 'artifact.path');
    const sidecarPath = repositoryPath(artifact.sidecar_path, 'artifact.sidecar_path');
    const commitPath = repositoryPath(artifact.commit_path, 'artifact.commit_path');
    const directory = path.dirname(artifactPath);
    assert.equal(path.dirname(sidecarPath), directory, 'an artifact and sidecar must be siblings');
    assert.equal(path.dirname(commitPath), directory, 'an artifact and commit marker must be siblings');
    assert.equal(path.extname(sidecarPath), '.json', 'sidecar must be JSON');
    assert.equal(path.extname(commitPath), '.json', 'commit marker must be JSON');
    for (const entry of [artifactPath, sidecarPath, commitPath]) {
        assert.equal(seenPaths.has(entry), false, `duplicate release path ${entry}`);
        seenPaths.add(entry);
    }
    sha256String(artifact.sha256, 'artifact.sha256');
    sha256String(artifact.sidecar_sha256, 'artifact.sidecar_sha256');
    sha256String(artifact.commit_sha256, 'artifact.commit_sha256');

    const [artifactBytes, sidecarBytes, commitBytes] = await Promise.all([
        readFile(new URL(artifactPath, ROOT)),
        readFile(new URL(sidecarPath, ROOT)),
        readFile(new URL(commitPath, ROOT)),
    ]);
    for (const [entry, filePath] of [[artifactBytes, artifactPath], [sidecarBytes, sidecarPath], [commitBytes, commitPath]]) {
        assert.ok((await stat(new URL(filePath, ROOT))).size > 0, `${filePath} must not be empty`);
        assert.ok(entry.length > 0, `${filePath} must contain bytes`);
    }
    assert.equal(sha256(artifactBytes), artifact.sha256, `${artifactPath} hash does not match evidence`);
    assert.equal(sha256(sidecarBytes), artifact.sidecar_sha256, `${sidecarPath} hash does not match evidence`);
    assert.equal(sha256(commitBytes), artifact.commit_sha256, `${commitPath} hash does not match evidence`);

    const sidecar = JSON.parse(sidecarBytes);
    const marker = JSON.parse(commitBytes);
    assert.equal(sidecar.success, true, `${sidecarPath} must record a successful export`);
    assert.equal(sidecar.application_version, RELEASE_VERSION,
        `${sidecarPath} must be generated by v${RELEASE_VERSION}`);
    assert.equal(sidecar.artifact?.file_name, path.basename(artifactPath),
        `${sidecarPath} must name its sibling artifact`);
    assert.equal(sidecar.artifact?.sha256, artifact.sha256,
        `${sidecarPath} artifact hash must match release evidence`);
    assert.equal(marker.schema, 'crystalcanvas.publication-pair-commit');
    assert.equal(marker.schema_version, 1);
    assert.equal(marker.primary_file_name, path.basename(artifactPath));
    assert.equal(marker.primary_sha256, artifact.sha256);
    assert.equal(marker.sidecar_file_name, path.basename(sidecarPath));
    assert.equal(marker.sidecar_sha256, artifact.sidecar_sha256);
    if (artifact.kind === 'glb') {
        assert.equal(sidecar.kind, 'blender_field_scene', `${sidecarPath} must record a field-aware GLB`);
        assert.equal(sidecar.export_id, artifact.identity,
            `${sidecarPath} GLB export_id must match release evidence identity`);
        assert.ok(sidecar.field_scene, `${sidecarPath} must retain field provenance`);
    } else {
        assert.equal(sidecar.kind, 'publication_raster', `${sidecarPath} must record a raster export`);
    }
    return { sidecar };
}

function validateEvidenceShape(evidence) {
    assert.equal(evidence.schema_version, 2, 'release evidence schema must be explicitly versioned');
    assert.equal(evidence.release_version, RELEASE_VERSION);
    assert.ok(['RELEASE_CANDIDATE', 'RELEASE_READY'].includes(evidence.status),
        'release evidence must declare candidate or ready state');
    assert.equal('known_exception' in evidence, false,
        'RELEASE-2 must not silently publish through a known-exception path');
    assertReachableCommit(evidence.feature_baseline_commit, 'feature_baseline_commit');

    for (const node of REQUIRED_NODES) {
        const entry = evidence.nodes?.[node];
        assert.equal(entry?.status, 'APPROVED', `${node} requires approval`);
        assert.ok(Array.isArray(entry.implementation_commits) && entry.implementation_commits.length > 0,
            `${node}.implementation_commits must be non-empty`);
        for (const commit of entry.implementation_commits) {
            assertReachableCommit(commit, `${node}.implementation_commits`);
        }
    }
    assert.deepEqual(evidence.required_standard_gates, REQUIRED_STANDARD_GATES,
        'release evidence must declare every required automated gate in canonical order');

    const manual = evidence.manual_validation;
    assert.ok(manual && typeof manual === 'object', 'manual validation is required');
    assert.ok(['PENDING', 'NATIVE_PRIMARY', 'NATIVE_LIMITED_BLENDER_FALLBACK'].includes(manual.native_fidelity_verdict),
        'manual native fidelity verdict must be explicit');
    const releaseReady = evidence.status === 'RELEASE_READY' || PUBLISH_MODE;
    requireCandidateEntry(manual.macos_intel_metal, 'manual_validation.macos_intel_metal', releaseReady);
    requireCandidateEntry(manual.macos_apple_silicon_metal, 'manual_validation.macos_apple_silicon_metal', releaseReady);
    requireCandidateEntry(manual.blender_headless_import, 'manual_validation.blender_headless_import', releaseReady);
    requireCandidateEntry(manual.blender_gui_import, 'manual_validation.blender_gui_import', releaseReady);
    nonEmptyString(manual.software_scope, 'manual_validation.software_scope');
    if (manual.macos_apple_silicon_metal.status === 'NOT_AVAILABLE') {
        assert.equal(manual.macos_apple_silicon_metal.reason, APPLE_SILICON_LIMITATION,
            'Apple Silicon unavailability must preserve the authorized compatibility limitation');
    }

    if (releaseReady) {
        assert.equal(evidence.status, 'RELEASE_READY', 'publish mode requires RELEASE_READY evidence');
        assert.deepEqual(evidence.blockers, [], 'release blockers must be empty');
        if (manual.native_fidelity_verdict === 'NATIVE_PRIMARY') {
            for (const name of MANUAL_GATE_NAMES) {
                assert.equal(manual[name].status, 'PASS', `manual_validation.${name} must pass before publication`);
            }
        } else {
            assert.equal(manual.native_fidelity_verdict, 'NATIVE_LIMITED_BLENDER_FALLBACK',
                'publication requires primary native validation or the authorized limited fallback');
            assert.equal(manual.macos_intel_metal.status, 'PASS',
                'Intel/Metal validation remains mandatory under the limited fallback');
            assert.equal(manual.macos_apple_silicon_metal.status, 'NOT_AVAILABLE',
                'Apple Silicon must remain explicitly unavailable under the limited fallback');
            assert.equal(manual.macos_apple_silicon_metal.reason, APPLE_SILICON_LIMITATION,
                'the limited fallback must not imply Apple Silicon compatibility');
            for (const name of ['blender_headless_import', 'blender_gui_import']) {
                assert.equal(manual[name].status, 'PASS',
                    `manual_validation.${name} must pass under the limited fallback`);
            }
        }
    } else {
        assert.ok(Array.isArray(evidence.blockers) && evidence.blockers.length > 0,
            'a candidate must state its remaining blockers');
    }
    return { manual, featureBaselineCommit: evidence.feature_baseline_commit, releaseReady };
}

test('RELEASE-2 rejects forged candidate evidence and stale node lineage', () => {
    assert.throws(() => validateEvidenceShape({}), /schema|release_version|feature_baseline_commit/i);
    assert.throws(() => validateEvidenceShape({
        schema_version: 2,
        release_version: RELEASE_VERSION,
        status: 'RELEASE_CANDIDATE',
        blockers: ['manual evidence pending'],
        feature_baseline_commit: '0'.repeat(40),
        nodes: {},
        required_standard_gates: [],
        manual_validation: {},
    }), /commit|FIELD-1/i);
});

test('RELEASE-2 validates the active v0.8 candidate metadata and evidence state', async () => {
    const evidence = await readJson(EVIDENCE_PATH);
    const { manual, featureBaselineCommit, releaseReady } = validateEvidenceShape(evidence);
    validateManualAttestation(manual, featureBaselineCommit, releaseReady);
});

test('RELEASE-2 requires hash-bound artifact triplets before publication', async () => {
    const evidence = await readJson(EVIDENCE_PATH);
    const { manual, featureBaselineCommit, releaseReady } = validateEvidenceShape(evidence);
    validateManualAttestation(manual, featureBaselineCommit, releaseReady);
    assert.ok(Array.isArray(evidence.artifacts) && evidence.artifacts.length > 0,
        'release evidence must not declare an empty artifact set');
    const seenPaths = new Set();
    const seenIdentities = new Set();
    const validatedArtifacts = [];
    for (const artifact of evidence.artifacts) {
        validatedArtifacts.push({ artifact, ...(await validateArtifactPair(artifact, seenPaths, seenIdentities)) });
    }
    assert.ok(evidence.artifacts.some((artifact) => artifact.kind === 'raster'),
        'release evidence needs at least one raster artifact');
    assert.ok(evidence.artifacts.some((artifact) => artifact.kind === 'glb'),
        'release evidence needs at least one field-aware Blender GLB artifact');
    assert.ok(validatedArtifacts.some(({ artifact }) => artifact.kind === 'raster'),
        'release evidence needs a raster software-contract artifact');
    assert.ok(validatedArtifacts.some(({ artifact }) => artifact.kind === 'glb'),
        'release evidence needs a field-aware GLB software-contract artifact');
});

test('RELEASE-2 synchronizes active metadata and seals publication behind the closure gate', async () => {
    const [cargoToml, tauriConfig, cargoLock, citation, changelog, readme, roadmap, workflow, ...docs] = await Promise.all([
        source('src-tauri/Cargo.toml'),
        readJson('src-tauri/tauri.conf.json'),
        source('src-tauri/Cargo.lock'),
        source('CITATION.cff'),
        source('CHANGELOG.md'),
        source('README.md'),
        source('ROADMAP.md'),
        source('.github/workflows/release.yml'),
        source('docs/UserManual.md'),
        source('docs/DeveloperGuide.md'),
        source('docs/TestingGuide.md'),
        source('docs/FAQ.md'),
        source('docs/IPC_Commands.md'),
        source('docs/Algorithms.md'),
        source('docs/Shader_Reference.md'),
    ]);
    assertMetadataVersions({
        cargoToml,
        tauriConfig,
        cargoLock,
        citation,
        changelog,
        readme,
        roadmap,
        docs: Object.fromEntries(['UserManual', 'DeveloperGuide', 'TestingGuide', 'FAQ', 'IPC_Commands', 'Algorithms', 'Shader_Reference']
            .map((name, index) => [name, docs[index]])),
    });
    const closureGate = workflow.indexOf('CRYSTALCANVAS_RELEASE_PUBLISH=1 node --test tests/release_2_red_gate.test.mjs');
    const tagGate = workflow.indexOf('node scripts/validate-release-tag.mjs');
    const releaseNotesGate = workflow.indexOf('node scripts/release-notes.mjs');
    const draftGate = workflow.indexOf('gh release create');
    const bundleGate = workflow.indexOf('tauri-apps/tauri-action@v0');
    const evidenceUploadGate = workflow.indexOf('gh release upload');
    const publishGate = workflow.indexOf('gh release edit');
    assert.match(workflow, /push:\s*\n\s*tags:\s*\n\s*- "v\*"/,
        'release requires an explicit v-prefixed tag push');
    assert.doesNotMatch(workflow, /workflow_dispatch:/,
        'release must not require a separate manual dispatch after a tag is pushed');
    assert.ok(tagGate >= 0 && closureGate > tagGate,
        'tag identity must be bound before release closure validation');
    assert.ok(closureGate >= 0 && draftGate > closureGate && bundleGate > draftGate,
        'release closure must pass before any bundle is created');
    assert.ok(releaseNotesGate > closureGate && releaseNotesGate < draftGate,
        'the validated changelog section must become the draft release body');
    assert.ok(evidenceUploadGate > bundleGate && publishGate > evidenceUploadGate,
        'the release must attach evidence and publish only after all draft bundles complete');
    assert.match(workflow, /releaseDraft:\s*true/);
    assert.match(workflow, /needs:\s*\[preflight, bundle\]/);
    assert.match(workflow, /fetch-depth:\s*0/);
    assert.match(workflow, /validate-release-tag\.mjs/);
    assert.match(workflow, /release-notes\.mjs/);
    assert.match(workflow, /--notes-file release-notes\.md/);
    assert.doesNotMatch(workflow, /Validated RELEASE-2 evidence is attached/);
    assert.match(workflow, /gh release create[^\n]+--target/);
    assert.match(workflow, /gh release upload[^\n]+release-evidence\.json/);
    const plannedReleases = roadmap.slice(roadmap.indexOf('## Planned Releases'));
    assert.doesNotMatch(plannedReleases, /^### `v0\.8\.0`/m,
        'v0.8.0 must not remain in planned releases after its release scope is recorded');
    const evolution = await source('EVOLUTION.log');
    assert.match(evolution, /v0\.8\.0 rapid Intel\/Metal-only release validation/);
    assert.match(evolution, /Apple Silicon compatibility is not established and must not be claimed/);
});
