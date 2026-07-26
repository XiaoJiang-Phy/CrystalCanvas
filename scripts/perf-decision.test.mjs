import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const REPORT_PATH = 'doc/v0.6.2_visualization_perf_report.json';
const DECISION_PATH = 'doc/v0.6.2_performance_report.md';
const DECISION_LOG_PATH = 'doc/DECISION_LOG.md';
const MANIFEST_PATH = 'src-tauri/benches/fixtures/visualization-perf-manifest.json';
const EXPECTED_DATASETS = [
    ['visualization-500', 500],
    ['visualization-1000', 1_000],
    ['visualization-5000', 5_000],
    ['visualization-10000', 10_000],
];
const METRICS = [
    'command_parse_and_structural_validation',
    'snapshot_serialization',
    'scene_build',
    'gpu_upload',
    'cpu_picking',
    'atom_drag_preview_commit',
    'phonon_pacing_uniform_update',
    'ui_idle_cpu_compositor',
    'listener_count_event_latency',
];

async function optional_source(path) {
    try {
        return await readFile(new URL(`../${path}`, import.meta.url), 'utf8');
    } catch (error) {
        if (error && typeof error === 'object' && error.code === 'ENOENT') return null;
        throw error;
    }
}

function required_source(source, path) {
    assert.ok(source, `PERF-1B requires ${path}`);
    return source;
}

function required_json(source, path) {
    return JSON.parse(required_source(source, path));
}

function assert_metric(metric, metric_name, dataset_id) {
    assert.ok(metric && typeof metric === 'object', `${dataset_id} is missing ${metric_name}`);
    assert.ok(['MEASURED', 'NOT_AVAILABLE'].includes(metric.status),
        `${dataset_id}/${metric_name} must be MEASURED or NOT_AVAILABLE`);
    if (metric.status === 'MEASURED') {
        for (const statistic of ['median', 'p95', 'max']) {
            assert.ok(Number.isFinite(metric[statistic]) && metric[statistic] >= 0,
                `${dataset_id}/${metric_name} must record finite ${statistic}`);
        }
        if (metric_name === 'snapshot_serialization') {
            assert.ok(Number.isFinite(metric.bytes) && metric.bytes > 0,
                `${dataset_id}/snapshot_serialization must record positive bytes`);
        } else {
            assert.equal(metric.bytes, null,
                `${dataset_id}/${metric_name} must not report an unmeasured byte count as zero`);
        }
    } else {
        assert.equal(typeof metric.reason, 'string',
            `${dataset_id}/${metric_name} NOT_AVAILABLE requires a reason`);
        assert.ok(metric.reason.length > 0,
            `${dataset_id}/${metric_name} NOT_AVAILABLE requires a non-empty reason`);
        assert.equal(metric.bytes, null,
            `${dataset_id}/${metric_name} NOT_AVAILABLE must not report bytes`);
    }
}

const [report_source, decision_source, decision_log_source, manifest_source] = await Promise.all([
    optional_source(REPORT_PATH),
    optional_source(DECISION_PATH),
    optional_source(DECISION_LOG_PATH),
    optional_source(MANIFEST_PATH),
]);

test('PERF-1B records complete four-dataset evidence from one clean release run', () => {
    const report = required_json(report_source, REPORT_PATH);
    const manifest = required_json(manifest_source, MANIFEST_PATH);

    assert.equal(report.schema_version, 1);
    assert.match(report.context?.commit ?? '', /^[0-9a-f]{40}$/,
        'the report must identify the exact benchmark commit');
    assert.equal(report.context?.worktree_clean, true,
        'the raw benchmark report must be generated before report artifacts are written');
    assert.equal(report.context?.release_build, true);
    assert.equal(report.datasets?.length, EXPECTED_DATASETS.length,
        'the report must contain all four measured datasets, not a 500-atom summary only');
    assert.deepEqual(
        report.datasets.map((dataset) => [dataset.id, dataset.intrinsic_atoms]),
        EXPECTED_DATASETS,
    );
    assert.deepEqual(
        report.datasets.map((dataset) => dataset.sha256),
        manifest.datasets.map((dataset) => dataset.sha256),
        'report dataset hashes must bind to the PERF-1A manifest',
    );

    for (const dataset of report.datasets) {
        assert.ok(dataset.state_versions && typeof dataset.state_versions === 'object');
        assert.equal(dataset.state_versions.snapshot, dataset.state_versions.scene,
            `${dataset.id} snapshot and scene must refer to one canonical state version`);
        for (const metric_name of METRICS) {
            assert_metric(dataset.metrics?.[metric_name], metric_name, dataset.id);
        }
    }

    const summary = report.datasets.find((dataset) => dataset.id === report.summary_dataset_id);
    assert.ok(summary, 'top-level summary must name one reported dataset');
    assert.deepEqual(report.state_versions, summary.state_versions);
    assert.deepEqual(report.metrics, summary.metrics);
    assert.equal(report.diagnostics?.per_frame_frontend_ipc, 0);
    assert.match(report.diagnostics?.per_frame_frontend_ipc_evidence ?? '',
        /phonon-interaction\.test\.mjs passed/);
    assert.equal(report.diagnostics?.production_instrumentation_default, false);
});

test('PERF-1B binds its retain decision to the unedited raw report and observed hardware', () => {
    const report = required_source(report_source, REPORT_PATH);
    const decision = required_source(decision_source, DECISION_PATH);
    const report_sha256 = createHash('sha256').update(report).digest('hex');

    assert.match(decision, /^# CrystalCanvas v0\.6\.2 Performance Report$/m);
    assert.match(decision, new RegExp(`Report SHA-256: .*${report_sha256}`));
    for (const field of ['CPU', 'GPU', 'Metal', 'macOS', 'Power state', 'Viewport']) {
        assert.match(decision, new RegExp(`^- ${field}: (?!NOT_AVAILABLE$).+`, 'm'),
            `PERF-1B must record observed ${field}`);
    }
    assert.match(decision, /^Approved performance budgets: NONE$/m,
        'PERF-1B must not invent a latency budget');
    assert.match(decision, /^Decision: RETAIN_CURRENT_ARCHITECTURE$/m,
        'without an approved budget violation, retain full snapshots, full rebuilds, and CPU picking');
    assert.doesNotMatch(decision, /^ADR-/m,
        'a retain decision must not smuggle an optimization ADR into PERF-1B');
});

test('PERF-1B records the retain decision without claiming a performance guarantee', () => {
    const decision = required_source(decision_source, DECISION_PATH);
    const decision_log = required_source(decision_log_source, DECISION_LOG_PATH);

    assert.match(decision, /no approved budget was violated/i);
    assert.doesNotMatch(decision, /(?:fast enough|performance guarantee|meets.*budget)/i,
        'absence of an approved budget is not a performance guarantee');
    assert.match(decision_log, /PERF-1B[\s\S]*RETAIN_CURRENT_ARCHITECTURE/,
        'DECISION_LOG must preserve the evidence-based architecture decision');
});
