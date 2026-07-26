import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const CANDIDATE_VERSION = '0.6.2';
const LATEST_RELEASE_VERSION = '0.6.1';
const REPOSITORY_ROOT = new URL('../', import.meta.url);
const EVIDENCE_PATH = 'doc/v0.6.2_release_evidence.json';
const EVIDENCE_FOLLOW_UP_PATHS = new Set([
    EVIDENCE_PATH,
    'doc/v0.6.2_release_audit.md',
]);
const NODE_NAMES = [
    'UI-2F',
    'UI-3',
    'INTERACT-1A',
    'INTERACT-1B',
    'INTERACT-2A',
    'INTERACT-2B',
    'IPC-3A',
    'IPC-3B',
    'PERF-1A',
    'PERF-1B',
];
const STANDARD_GATES = [
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
const DESKTOP_MATRIX = [
    'macos_intel_metal',
    'macos_apple_silicon_metal',
    'ubuntu_vulkan',
    'browser_mutation_rejection',
];
const DOCS = [
    ['README.md', [
        /^> \*\*Latest release\*\*: \`v0\.6\.1\`$/m,
        /^> \*\*Current development line\*\*: \`v0\.6\.2\` release candidate \(REL-2 closure pending\)$/m,
    ]],
    ['ROADMAP.md', [
        /Latest release: \`v0\.6\.1\`/,
        /^### \`v0\.6\.2\` — Scientific Workbench Hardening$/m,
        /release remains blocked until the REL-2 evidence and platform gates close\./,
    ]],
    ['doc/roadmap.md', [
        /^> \*\*Latest release\*\*: \`v0\.6\.1\`$/m,
        /^> \*\*Current development version\*\*: \`v0\.6\.2\` release candidate$/m,
    ]],
    ['doc/TDD_CrystalCanvas_v1.md', [
        /^> \*\*Latest release baseline\*\*: \`v0\.6\.1\`$/m,
        /^> \*\*Current development baseline\*\*: \`v0\.6\.2\` release candidate$/m,
    ]],
    ['docs/UserManual.md', [/^> Baseline: \`v0\.6\.2\`/m]],
    ['docs/DeveloperGuide.md', [/^> Baseline: \`v0\.6\.2\`/m]],
    ['docs/TestingGuide.md', [/^> Baseline: \`v0\.6\.2\`/m]],
    ['docs/FAQ.md', [/^> Baseline: \`v0\.6\.2\`/m]],
    ['docs/IPC_Commands.md', [/^> Baseline: \`v0\.6\.2\`/m]],
    ['docs/Algorithms.md', [/^> Baseline: \`v0\.6\.2\`/m]],
    ['docs/Shader_Reference.md', [/^> Baseline: \`v0\.6\.2\`/m]],
];
const WARNING_ALLOWLIST = new Set([
    'crystal_state.rs:new_occupancies unused mut',
    'renderer/isosurface.rs:max_vertices unread',
]);

async function source(path) {
    return readFile(new URL(`../${path}`, import.meta.url), 'utf8');
}

async function optional_source(path) {
    try {
        return await source(path);
    } catch (error) {
        if (error && typeof error === 'object' && error.code === 'ENOENT') return null;
        throw error;
    }
}

function exact_version(source_text, path) {
    assert.match(source_text,
        new RegExp(`^version\\s*=\\s*"${CANDIDATE_VERSION.replaceAll('.', '\\.') }"$`, 'm'),
        `${path} must declare candidate version ${CANDIDATE_VERSION}`);
}

function pass_record(record, label) {
    assert.equal(record?.status, 'PASS', `${label} must be PASS; NOT_RUN and SKIPPED do not close REL-2`);
    assert.match(record?.evidence ?? '', /\S/, `${label} requires concrete evidence`);
}

function sha256(source_text) {
    return createHash('sha256').update(source_text).digest('hex');
}

function git(args) {
    return execFileSync('git', args, {
        cwd: REPOSITORY_ROOT,
        encoding: 'utf8',
    }).trim();
}

function git_succeeds(args) {
    return spawnSync('git', args, {
        cwd: REPOSITORY_ROOT,
        encoding: 'utf8',
    }).status === 0;
}

function assert_known_ancestor(ancestor, descendant, label) {
    assert.ok(git_succeeds(['rev-parse', '--verify', `${ancestor}^{commit}`]),
        `${label} must name an existing commit`);
    assert.ok(git_succeeds(['merge-base', '--is-ancestor', ancestor, descendant]),
        `${label} must be an ancestor of the committed evidence`);
}

function is_evidence_follow_up_path(path) {
    return EVIDENCE_FOLLOW_UP_PATHS.has(path)
        || path.startsWith('doc/v0.6.2_release_runs/');
}

const [
    package_source,
    cargo_source,
    tauri_source,
    lock_source,
    changelog_source,
    workflow_source,
    evidence_source,
    citation_source,
    performance_report_source,
    performance_context_source,
    ...doc_sources
] = await Promise.all([
    source('package.json'),
    source('src-tauri/Cargo.toml'),
    source('src-tauri/tauri.conf.json'),
    source('src-tauri/Cargo.lock'),
    source('CHANGELOG.md'),
    source('.github/workflows/release.yml'),
    optional_source(EVIDENCE_PATH),
    source('CITATION.cff'),
    source('doc/v0.6.2_visualization_perf_report.json'),
    source('doc/v0.6.2_visualization_perf_run_context.json'),
    ...DOCS.map(([path]) => source(path)),
]);

test('REL-2 candidate refuses split versions and a premature release changelog', () => {
    assert.equal(JSON.parse(package_source).version, CANDIDATE_VERSION);
    exact_version(cargo_source, 'src-tauri/Cargo.toml');
    assert.equal(JSON.parse(tauri_source).version, CANDIDATE_VERSION);
    assert.match(lock_source,
        new RegExp(`\\[\\[package\\]\\]\\nname = "crystal-canvas"\\nversion = "${CANDIDATE_VERSION.replaceAll('.', '\\.') }"`),
        'src-tauri/Cargo.lock must lock the application package at the candidate version');
    assert.match(changelog_source, /^## \[Unreleased\]$/m,
        'a blocked candidate must remain under an Unreleased changelog section');
    assert.match(changelog_source, /v0\.6\.2 release candidate/i,
        'the Unreleased changelog section must identify the candidate version');
    assert.doesNotMatch(changelog_source,
        new RegExp(`^## \\[${CANDIDATE_VERSION.replaceAll('.', '\\.') }\\] - \\d{4}-\\d{2}-\\d{2}$`, 'm'),
        'a blocked candidate must not have a dated release entry');
});

test('REL-2 candidate distinguishes the last release from candidate documentation', () => {
    for (let index = 0; index < DOCS.length; index += 1) {
        const [path, expectations] = DOCS[index];
        for (const expected of expectations) {
            assert.match(doc_sources[index], expected, `${path} must identify candidate and release states accurately`);
        }
    }
    assert.match(citation_source,
        new RegExp(`^version: "${LATEST_RELEASE_VERSION.replaceAll('.', '\\.') }"$`, 'm'),
        'CITATION.cff must cite the last published release until REL-2 closes');
    assert.match(package_source, /"version": "0\.6\.2"/,
        'package.json must remain the frontend version authority');
});

test('REL-2 refuses a mutable dependency install or accidental prerelease workflow', () => {
    assert.match(workflow_source, /pnpm install --frozen-lockfile/,
        'release workflow must use the committed pnpm lockfile');
    assert.match(workflow_source, /args: "--target aarch64-apple-darwin"/);
    assert.match(workflow_source, /args: "--target x86_64-apple-darwin"/);
    assert.match(workflow_source, /^\s*prerelease:\s*false\s*$/m,
        'REL-2 is a stable release; prerelease must be explicit false');
});

test('REL-2 refuses unsupported closure claims, unbound commits, or absent platform evidence', () => {
    assert.ok(evidence_source, `REL-2 requires ${EVIDENCE_PATH}`);
    const evidence = JSON.parse(evidence_source);
    assert.ok(git_succeeds(['cat-file', '-e', `HEAD:${EVIDENCE_PATH}`]),
        'release evidence must be committed; an untracked or dirty-worktree report cannot close REL-2');
    const evidence_at_head = git(['show', `HEAD:${EVIDENCE_PATH}`]);
    const evidence_commit = git(['rev-parse', 'HEAD']);

    assert.equal(evidence_at_head, evidence_source.trimEnd(),
        'release evidence must match the committed evidence artifact exactly');
    assert.equal(evidence.schema_version, 2);
    assert.equal(evidence.release_version, CANDIDATE_VERSION);
    assert.equal(evidence.status, 'PASS', 'REL-2 evidence must explicitly be PASS');
    assert.deepEqual(evidence.blockers, [], 'REL-2 evidence must not retain blockers');
    assert.match(evidence.release_candidate_commit ?? '', /^[0-9a-f]{40}$/,
        'release evidence must identify the exact candidate commit');
    assert.notEqual(evidence.release_candidate_commit, evidence_commit,
        'candidate and evidence must be separate commits to avoid a self-referential commit hash');
    assert_known_ancestor(evidence.release_candidate_commit, evidence_commit, 'release_candidate_commit');

    const candidate_audit = evidence.candidate_audit;
    pass_record(candidate_audit, 'candidate_audit');
    assert.equal(candidate_audit?.auditor, 'LGTM', 'candidate_audit requires an Auditor LGTM');
    assert.equal(candidate_audit?.candidate_commit, evidence.release_candidate_commit,
        'candidate_audit must bind to release_candidate_commit');

    const post_candidate_paths = git(['diff', '--name-only', `${evidence.release_candidate_commit}..${evidence_commit}`])
        .split('\n')
        .filter(Boolean);
    assert.ok(post_candidate_paths.every(is_evidence_follow_up_path),
        `only evidence artifacts may change after the candidate commit: ${post_candidate_paths.join(', ')}`);

    for (const node of NODE_NAMES) {
        const record = evidence.nodes?.[node];
        pass_record(record, node);
        assert.equal(record.auditor, 'LGTM', `${node} requires an Auditor LGTM`);
        assert.match(record.implementation_commit ?? '', /^[0-9a-f]{40}$/,
            `${node} requires the implementation commit`);
        assert_known_ancestor(record.implementation_commit, evidence.release_candidate_commit,
            `${node} implementation_commit`);
    }
    for (const gate of STANDARD_GATES) pass_record(evidence.standard_gates?.[gate], gate);
    for (const platform of DESKTOP_MATRIX) pass_record(evidence.desktop_matrix?.[platform], platform);

    assert.deepEqual(
        new Set(evidence.baseline_warnings?.map((warning) => `${warning.path}:${warning.message}`)),
        WARNING_ALLOWLIST,
        'REL-2 allows exactly the two documented baseline warnings and no new warning');
    assert.equal(evidence.performance?.raw_report_sha256, sha256(performance_report_source),
        'performance report digest must bind to the retained report');
    assert.equal(evidence.performance?.run_context_sha256, sha256(performance_context_source),
        'performance run-context digest must bind to the retained context');
});
