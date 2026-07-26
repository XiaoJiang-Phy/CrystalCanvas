import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';

const BENCHMARK_PATH = 'src-tauri/benches/bench_visualization.rs';
const MANIFEST_PATH = 'src-tauri/benches/fixtures/visualization-perf-manifest.json';
const REPORT_SCHEMA_PATH = 'src-tauri/benches/fixtures/visualization-perf-report.schema.json';
const CARGO_TOML_PATH = 'src-tauri/Cargo.toml';
const EXPECTED_INTRINSIC_ATOMS = [500, 1_000, 5_000, 10_000];
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
const REPOSITORY_ROOT = resolve(new URL('..', import.meta.url).pathname);
const PROJECT_CARGO_HOME = join(REPOSITORY_ROOT, '.cargo');
const PROJECT_RUSTUP_HOME = join(REPOSITORY_ROOT, '.rustup');
const PROJECT_CARGO = join(PROJECT_CARGO_HOME, 'bin', 'cargo');
const RUN_DYNAMIC_REPORT_PROBE = process.env.RUN_PERF_BENCHMARK_CONTRACT_PROBE === '1';

async function optional_source(path) {
    try {
        return await readFile(new URL(`../${path}`, import.meta.url), 'utf8');
    } catch (error) {
        if (error && typeof error === 'object' && error.code === 'ENOENT') return null;
        throw error;
    }
}

const [benchmark, manifest_source, report_schema_source, cargo_toml] = await Promise.all([
    optional_source(BENCHMARK_PATH),
    optional_source(MANIFEST_PATH),
    optional_source(REPORT_SCHEMA_PATH),
    optional_source(CARGO_TOML_PATH),
]);

function required_source(source, path) {
    assert.ok(source, `PERF-1A requires ${path}; no existing benchmark may stand in for the visualization matrix`);
    return source;
}

function required_json(source, path) {
    return JSON.parse(required_source(source, path));
}

function generated_fractional_grid(dataset) {
    const [nx, ny, nz] = dataset.grid_dimensions;
    const atoms = [];
    for (let iz = 0; iz < nz; iz += 1) {
        for (let iy = 0; iy < ny; iy += 1) {
            for (let ix = 0; ix < nx; ix += 1) {
                atoms.push([
                    (ix + 0.5) / nx,
                    (iy + 0.5) / ny,
                    (iz + 0.5) / nz,
                ]);
            }
        }
    }
    return atoms;
}

function canonical_dataset_hash(manifest, dataset) {
    const { sha256: ignoredHash, ...declared } = dataset;
    const canonical = JSON.stringify({
        schema_version: manifest.schema_version,
        generator: manifest.generator,
        dataset: declared,
        fractional_positions: generated_fractional_grid(dataset),
    });
    return createHash('sha256').update(canonical).digest('hex');
}

function actual_minimum_grid_distance(dataset) {
    const [nx, ny, nz] = dataset.grid_dimensions;
    const [a, b, c] = dataset.lattice_angstrom;
    return Math.min(a / nx, b / ny, c / nz);
}

function benchmark_report_command(reportPath) {
    return spawnSync(
        PROJECT_CARGO,
        [
            'bench',
            '--manifest-path', 'src-tauri/Cargo.toml',
            '--bench', 'bench_visualization',
            '--',
            '--help',
        ],
        {
            cwd: REPOSITORY_ROOT,
            encoding: 'utf8',
            timeout: 180_000,
            env: {
                ...process.env,
                CARGO_HOME: PROJECT_CARGO_HOME,
                RUSTUP_HOME: PROJECT_RUSTUP_HOME,
                PATH: `${join(PROJECT_CARGO_HOME, 'bin')}:${process.env.PATH ?? ''}`,
                PERF_REPORT_PATH: reportPath,
                PERF_BENCHMARK_CONTRACT_PROBE: '1',
            },
        },
    );
}

test('PERF-1A fixes a deterministic four-size intrinsic-atom dataset matrix', () => {
    const manifest = required_json(manifest_source, MANIFEST_PATH);

    assert.equal(manifest.schema_version, 1);
    assert.equal(manifest.performance_only, true,
        'benchmark structures must be explicitly excluded from physical claims');
    assert.ok(manifest.generator && typeof manifest.generator === 'object',
        'the manifest must record a deterministic generation method');
    assert.equal(typeof manifest.generator.kind, 'string');
    assert.ok(manifest.generator.kind.length > 0);
    assert.ok(Number.isInteger(manifest.generator.seed),
        'the generator seed must be recorded so every matrix entry is reproducible');
    assert.deepEqual(manifest.datasets.map((dataset) => dataset.intrinsic_atoms), EXPECTED_INTRINSIC_ATOMS,
        'the matrix must contain exactly 500, 1,000, 5,000, and 10,000 intrinsic atoms in order');

    for (const dataset of manifest.datasets) {
        assert.equal(dataset.performance_only, true);
        assert.equal(typeof dataset.id, 'string');
        assert.match(dataset.id, /^visualization-(?:500|1000|5000|10000)$/);
        const generatedPositions = generated_fractional_grid(dataset);
        assert.equal(generatedPositions.length, dataset.intrinsic_atoms,
            `${dataset.id} grid dimensions must generate exactly its declared intrinsic atom count`);
        assert.ok(generatedPositions.every((position) => position.length === 3
            && position.every((component) => Number.isFinite(component) && component > 0 && component < 1)),
        `${dataset.id} generator must not emit non-finite or boundary fractional coordinates`);
        assert.deepEqual(dataset.lattice_angstrom?.length, 3,
            `${dataset.id} must record all lattice lengths`);
        assert.ok(dataset.lattice_angstrom.every((length) => Number.isFinite(length) && length > 0),
            `${dataset.id} lattice lengths must be finite and positive`);
        assert.ok(Number.isFinite(dataset.minimum_distance_angstrom) && dataset.minimum_distance_angstrom > 0,
            `${dataset.id} must record a finite positive minimum distance`);
        assert.equal(dataset.minimum_distance_angstrom, actual_minimum_grid_distance(dataset),
            `${dataset.id} declared minimum distance must equal the generated periodic grid spacing`);
        assert.equal(dataset.sha256, canonical_dataset_hash(manifest, dataset),
            `${dataset.id} SHA-256 must bind the generator, declared dataset, and generated positions`);
    }
});

test('PERF-1A exposes a release Criterion target with all required measurements', () => {
    const source = required_source(benchmark, BENCHMARK_PATH);

    assert.match(cargo_toml, /\[\[bench\]\]\s*name\s*=\s*"bench_visualization"\s*harness\s*=\s*false/s,
        'Cargo must expose the visualization benchmark as a Criterion target');
    assert.match(source, /criterion_(?:group|main)!/,
        'the target must be executable through cargo bench rather than a source-only checklist');
    assert.match(source, /visualization-perf-manifest\.json/,
        'the benchmark must consume the recorded matrix instead of synthesizing untracked data');

    assert.match(source, /datasets[\s\S]*intrinsic_atoms|intrinsic_atoms[\s\S]*datasets/,
        'the executable target must iterate manifest-provided datasets instead of hard-coding a substitute matrix');

    for (const metric of METRICS) {
        assert.match(source, new RegExp(`['\"]${metric}['\"]`),
            `the benchmark must declare the ${metric} measurement explicitly`);
    }

    assert.match(source, /1_000\s*(?:preview|previews)/i,
        'the drag workload must exercise 1,000 previews before its single terminal commit');
    assert.match(source, /per_frame_frontend_ipc\s*[:=]\s*0/,
        'the phonon workload must record zero per-frame frontend IPC');
});

test('PERF-1A executes a report-producing release benchmark rather than a source-only checklist', {
    timeout: 200_000,
    skip: !RUN_DYNAMIC_REPORT_PROBE
        && 'run with RUN_PERF_BENCHMARK_CONTRACT_PROBE=1 after the report probe is implemented',
}, async () => {
    const reportDirectory = await mkdtemp(join(tmpdir(), 'crystal-canvas-perf-'));
    const reportPath = join(reportDirectory, 'visualization-perf-report.json');
    try {
        const result = benchmark_report_command(reportPath);
        assert.equal(result.error, undefined,
            `release benchmark process must start: ${result.error?.message ?? 'unknown process error'}`);
        assert.equal(result.status, 0,
            `release benchmark must honor PERF_BENCHMARK_CONTRACT_PROBE before Criterion handles --help\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`);
        assert.ok(existsSync(reportPath),
            'release benchmark must write the requested structured report instead of only Criterion console output');

        const schema = required_json(report_schema_source, REPORT_SCHEMA_PATH);
        const report = JSON.parse(await readFile(reportPath, 'utf8'));
        assert.equal(report.schema_version, schema.schema_version);
        assert.ok(report.context && typeof report.context === 'object',
            'report must contain an observed execution context');
        for (const field of schema.required_context) {
            assert.ok(Object.hasOwn(report.context, field),
                `report context must record ${field}`);
        }
        assert.equal(report.context.release_build, true,
            'PERF-1A report must come from a release benchmark run');

        assert.ok(report.state_versions && typeof report.state_versions === 'object',
            'report must identify the state version used by snapshot and scene measurements');
        assert.equal(report.state_versions.snapshot, report.state_versions.scene,
            'snapshot and scene measurements must refer to one canonical state version');

        assert.ok(report.metrics && typeof report.metrics === 'object',
            'report must contain a result or an explicit unavailable reason for every metric');
        for (const metric of METRICS) {
            const resultEntry = report.metrics[metric];
            assert.ok(resultEntry && typeof resultEntry === 'object', `report is missing ${metric}`);
            assert.ok(['MEASURED', 'NOT_AVAILABLE'].includes(resultEntry.status),
                `${metric} must be MEASURED or NOT_AVAILABLE, never an invented zero`);
            if (resultEntry.status === 'MEASURED') {
                for (const statistic of schema.required_summary_statistics) {
                    assert.ok(Number.isFinite(resultEntry[statistic]) && resultEntry[statistic] >= 0,
                        `${metric} must record a finite ${statistic}`);
                }
            } else {
                assert.equal(typeof resultEntry.reason, 'string',
                    `${metric} NOT_AVAILABLE requires an observed reason`);
                assert.ok(resultEntry.reason.length > 0,
                    `${metric} NOT_AVAILABLE requires a non-empty reason`);
            }
        }
        assert.ok(Number.isFinite(report.metrics.snapshot_serialization.bytes)
            && report.metrics.snapshot_serialization.bytes > 0,
        'snapshot serialization must record a positive serialized byte count');
        assert.equal(report.diagnostics?.per_frame_frontend_ipc, 0,
            'phonon baseline must record zero per-frame frontend IPC');
        assert.equal(report.diagnostics?.production_instrumentation_default, false,
            'benchmark instrumentation must remain disabled in production by default');
    } finally {
        await rm(reportDirectory, { recursive: true, force: true });
    }
});

test('PERF-1A report schema forbids invented zeroes and ties snapshot to scene version', () => {
    const schema = required_json(report_schema_source, REPORT_SCHEMA_PATH);

    assert.equal(schema.schema_version, 1);
    assert.deepEqual(schema.required_context, [
        'commit',
        'worktree_clean',
        'release_build',
        'os',
        'cpu',
        'gpu',
        'architecture',
        'viewport',
        'power_state',
        'warmup_iterations',
        'measurement_iterations',
    ]);
    assert.deepEqual(schema.required_summary_statistics, ['median', 'p95', 'max']);
    assert.equal(schema.unavailable_metric_status, 'NOT_AVAILABLE',
        'unavailable metrics must remain unavailable rather than masquerading as zero');
    assert.equal(schema.same_state_version_required, true,
        'snapshot serialization and scene build must describe one committed state version');
    assert.equal(schema.production_instrumentation_default, 'disabled');
});

test('PERF-1A keeps benchmark instrumentation outside product ownership and event semantics', () => {
    const source = required_source(benchmark, BENCHMARK_PATH);

    assert.doesNotMatch(source, /\b(?:state_changed|emit_all|safeInvoke|safeListen|requestAnimationFrame)\b/,
        'the benchmark harness must not fabricate IPC events, WebView loops, or a second listener owner');
    assert.doesNotMatch(source, /\b(?:delta_snapshot|dirty_range|spatial_index|zero_copy|chunk(?:ed|ing)?)\b/i,
        'PERF-1A measures the retained architecture; it must not smuggle in an optimization');
    assert.match(source, /production_instrumentation_default\s*[:=]\s*(?:false|"disabled")/,
        'instrumentation must be disabled by default outside an explicit benchmark invocation');
});
