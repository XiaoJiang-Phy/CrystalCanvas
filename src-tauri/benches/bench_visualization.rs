//! Reproducible visualization baseline for performance-only synthetic structures.
//!
//! Run with `cargo bench --manifest-path src-tauri/Cargo.toml --bench bench_visualization`.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group};
use crystal_canvas::crystal_state::CrystalState;
use crystal_canvas::llm::command::CrystalCommand;
use crystal_canvas::renderer::instance::prepare_atom_scene;
use crystal_canvas::renderer::ray_picking::{PickAtom, Ray, ray_pick};
use crystal_canvas::settings::AppSettings;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fmt::Write;
use std::fs::File;
use std::process::Command;
use std::time::{Duration, Instant};

const PRODUCTION_INSTRUMENTATION_DEFAULT: bool = false;
// 1_000 previews is the required renderer-owned drag workload. Until that session can be
// driven without a live WGPU renderer, the benchmark reports the metric as NOT_AVAILABLE.
const DRAG_PREVIEWS_PER_COMMIT: usize = 1_000;
const BENCHMARK_MANIFEST: &str = include_str!("fixtures/visualization-perf-manifest.json");
const BENCHMARK_COMMAND_JSON: &[u8] = br#"{"action":"delete_atoms","params":{"indices":[]}}"#;

const METRICS: [&str; 9] = [
    "command_parse_and_structural_validation",
    "snapshot_serialization",
    "scene_build",
    "gpu_upload",
    "cpu_picking",
    "atom_drag_preview_commit",
    "phonon_pacing_uniform_update",
    "ui_idle_cpu_compositor",
    "listener_count_event_latency",
];

const PROBE_ITERATIONS: usize = 3;

#[derive(Deserialize, Serialize)]
struct BenchmarkManifest {
    schema_version: u32,
    performance_only: bool,
    generator: GeneratorSpec,
    datasets: Vec<DatasetSpec>,
}

#[derive(Deserialize, Serialize)]
struct GeneratorSpec {
    kind: String,
    seed: u64,
}

#[derive(Deserialize, Serialize)]
struct DatasetSpec {
    id: String,
    performance_only: bool,
    intrinsic_atoms: usize,
    grid_dimensions: [usize; 3],
    lattice_angstrom: [f64; 3],
    minimum_distance_angstrom: f64,
    sha256: String,
}

#[derive(Deserialize)]
struct BenchmarkSnapshot {
    name: String,
    cell_a: f64,
    cell_b: f64,
    cell_c: f64,
    cell_alpha: f64,
    cell_beta: f64,
    cell_gamma: f64,
    spacegroup_hm: String,
    spacegroup_number: i32,
    labels: Vec<String>,
    elements: Vec<String>,
    fract_x: Vec<f64>,
    fract_y: Vec<f64>,
    fract_z: Vec<f64>,
    occupancies: Vec<f64>,
    atomic_numbers: Vec<u8>,
    cart_positions: Vec<[f32; 3]>,
    version: u32,
    is_2d: bool,
    vacuum_axis: Option<usize>,
    intrinsic_sites: usize,
    measurements: Vec<crystal_canvas::crystal_state::MeasurementOverlay>,
}

impl BenchmarkSnapshot {
    fn into_crystal_state(self) -> CrystalState {
        let mut state = CrystalState::default();
        state.name = self.name;
        state.cell_a = self.cell_a;
        state.cell_b = self.cell_b;
        state.cell_c = self.cell_c;
        state.cell_alpha = self.cell_alpha;
        state.cell_beta = self.cell_beta;
        state.cell_gamma = self.cell_gamma;
        state.spacegroup_hm = self.spacegroup_hm;
        state.spacegroup_number = self.spacegroup_number;
        state.labels = self.labels;
        state.elements = self.elements;
        state.fract_x = self.fract_x;
        state.fract_y = self.fract_y;
        state.fract_z = self.fract_z;
        state.occupancies = self.occupancies;
        state.atomic_numbers = self.atomic_numbers;
        state.cart_positions = self.cart_positions;
        state.version = self.version;
        state.is_2d = self.is_2d;
        state.vacuum_axis = self.vacuum_axis;
        state.intrinsic_sites = self.intrinsic_sites;
        state.measurements = self.measurements;
        state
    }
}

#[derive(Clone, Serialize)]
struct MetricReport {
    status: &'static str,
    median: Option<f64>,
    p95: Option<f64>,
    max: Option<f64>,
    bytes: Option<usize>,
    reason: Option<String>,
}

#[derive(Serialize)]
struct ReportContext {
    commit: String,
    worktree_clean: bool,
    release_build: bool,
    os: &'static str,
    cpu: String,
    gpu: String,
    architecture: &'static str,
    viewport: String,
    power_state: String,
    warmup_iterations: usize,
    measurement_iterations: usize,
}

#[derive(Clone, Serialize)]
struct StateVersions {
    snapshot: u32,
    scene: u32,
}

#[derive(Serialize)]
struct RunDiagnostics {
    per_frame_frontend_ipc: u64,
    per_frame_frontend_ipc_evidence: String,
    production_instrumentation_default: bool,
}

#[derive(Serialize)]
struct DatasetReport {
    id: String,
    intrinsic_atoms: usize,
    sha256: String,
    state_versions: StateVersions,
    metrics: BTreeMap<&'static str, MetricReport>,
}

#[derive(Serialize)]
struct BenchmarkReport {
    schema_version: u32,
    context: ReportContext,
    summary_dataset_id: String,
    state_versions: StateVersions,
    metrics: BTreeMap<&'static str, MetricReport>,
    diagnostics: RunDiagnostics,
    datasets: Vec<DatasetReport>,
}

fn generated_fractional_positions(dataset: &DatasetSpec) -> Vec<[f64; 3]> {
    let [nx, ny, nz] = dataset.grid_dimensions;
    let count = nx
        .checked_mul(ny)
        .and_then(|value| value.checked_mul(nz))
        .expect("benchmark grid dimensions must not overflow");
    let mut positions = Vec::with_capacity(count);
    for iz in 0..nz {
        for iy in 0..ny {
            for ix in 0..nx {
                positions.push([
                    (ix as f64 + 0.5) / nx as f64,
                    (iy as f64 + 0.5) / ny as f64,
                    (iz as f64 + 0.5) / nz as f64,
                ]);
            }
        }
    }
    positions
}

fn canonical_dataset_sha256(manifest: &BenchmarkManifest, dataset: &DatasetSpec) -> String {
    let [nx, ny, nz] = dataset.grid_dimensions;
    let [a, b, c] = dataset.lattice_angstrom;
    let generator_kind = serde_json::to_string(&manifest.generator.kind)
        .expect("canonical generator kind serialization");
    let dataset_id =
        serde_json::to_string(&dataset.id).expect("canonical dataset id serialization");
    let mut canonical = String::new();
    write!(
        canonical,
        r#"{{"schema_version":{},"generator":{{"kind":{},"seed":{}}},"dataset":{{"id":{},"performance_only":{},"intrinsic_atoms":{},"grid_dimensions":[{nx},{ny},{nz}],"lattice_angstrom":[{a},{b},{c}],"minimum_distance_angstrom":{}}},"fractional_positions":["#,
        manifest.schema_version,
        generator_kind,
        manifest.generator.seed,
        dataset_id,
        dataset.performance_only,
        dataset.intrinsic_atoms,
        dataset.minimum_distance_angstrom,
    )
    .expect("canonical benchmark dataset formatting");
    for (index, [x, y, z]) in generated_fractional_positions(dataset)
        .into_iter()
        .enumerate()
    {
        if index != 0 {
            canonical.push(',');
        }
        write!(canonical, "[{x},{y},{z}]").expect("canonical position formatting");
    }
    canonical.push_str("]}");
    format!("{:x}", Sha256::digest(canonical.as_bytes()))
}

fn validate_dataset(manifest: &BenchmarkManifest, dataset: &DatasetSpec) {
    assert!(dataset.performance_only);
    assert!(
        dataset.minimum_distance_angstrom.is_finite() && dataset.minimum_distance_angstrom > 0.0
    );
    assert!(
        dataset
            .lattice_angstrom
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
    );
    let [nx, ny, nz] = dataset.grid_dimensions;
    let generated_atoms = nx
        .checked_mul(ny)
        .and_then(|value| value.checked_mul(nz))
        .expect("benchmark grid dimensions must not overflow");
    assert_eq!(generated_atoms, dataset.intrinsic_atoms);
    let generated_minimum = [
        dataset.lattice_angstrom[0] / nx as f64,
        dataset.lattice_angstrom[1] / ny as f64,
        dataset.lattice_angstrom[2] / nz as f64,
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min);
    assert!(
        (generated_minimum - dataset.minimum_distance_angstrom).abs() <= 1.0e-12,
        "benchmark minimum distance must describe the generated grid"
    );
    assert_eq!(
        canonical_dataset_sha256(manifest, dataset),
        dataset.sha256,
        "benchmark dataset hash must bind its generated positions"
    );
}

fn load_manifest() -> BenchmarkManifest {
    let manifest: BenchmarkManifest = serde_json::from_str(BENCHMARK_MANIFEST)
        .expect("visualization performance manifest must be valid JSON");
    assert_eq!(manifest.schema_version, 1);
    assert!(manifest.performance_only);
    assert_eq!(manifest.generator.kind, "deterministic_fractional_grid");
    assert_eq!(manifest.datasets.len(), 4);
    for dataset in &manifest.datasets {
        validate_dataset(&manifest, dataset);
    }
    manifest
}

fn synthetic_state(dataset: &DatasetSpec) -> CrystalState {
    let positions = generated_fractional_positions(dataset);
    assert_eq!(positions.len(), dataset.intrinsic_atoms);

    let mut state = CrystalState::default();
    state.name = dataset.id.clone();
    state.cell_a = dataset.lattice_angstrom[0];
    state.cell_b = dataset.lattice_angstrom[1];
    state.cell_c = dataset.lattice_angstrom[2];
    state.intrinsic_sites = dataset.intrinsic_atoms;
    state.labels.reserve(dataset.intrinsic_atoms);
    state.elements.reserve(dataset.intrinsic_atoms);
    state.atomic_numbers.reserve(dataset.intrinsic_atoms);
    state.fract_x.reserve(dataset.intrinsic_atoms);
    state.fract_y.reserve(dataset.intrinsic_atoms);
    state.fract_z.reserve(dataset.intrinsic_atoms);
    state.occupancies.reserve(dataset.intrinsic_atoms);

    for (index, [x, y, z]) in positions.into_iter().enumerate() {
        state.labels.push(format!("Si{index}"));
        state.elements.push("Si".to_owned());
        state.atomic_numbers.push(14);
        state.fract_x.push(x);
        state.fract_y.push(y);
        state.fract_z.push(z);
        state.occupancies.push(1.0);
    }
    state.fractional_to_cartesian();
    state
}

fn unavailable(reason: impl Into<String>) -> MetricReport {
    MetricReport {
        status: "NOT_AVAILABLE",
        median: None,
        p95: None,
        max: None,
        bytes: None,
        reason: Some(reason.into()),
    }
}

fn measure<F>(mut operation: F) -> MetricReport
where
    F: FnMut() -> Option<usize>,
{
    let mut samples = [Duration::ZERO; PROBE_ITERATIONS];
    let mut bytes = None;
    for sample in &mut samples {
        let start = Instant::now();
        bytes = operation();
        *sample = start.elapsed();
    }
    let mut microseconds = samples.map(|sample| sample.as_secs_f64() * 1_000_000.0);
    microseconds.sort_by(f64::total_cmp);
    MetricReport {
        status: "MEASURED",
        median: Some(microseconds[microseconds.len() / 2]),
        p95: Some(microseconds[(microseconds.len() * 95).div_ceil(100) - 1]),
        max: Some(microseconds[microseconds.len() - 1]),
        bytes,
        reason: None,
    }
}

fn parse_command_and_validate_snapshot(snapshot: &[u8]) {
    let command: CrystalCommand = serde_json::from_slice(black_box(BENCHMARK_COMMAND_JSON))
        .expect("benchmark command must parse through the production command schema");
    black_box(command);
    let parsed: BenchmarkSnapshot =
        serde_json::from_slice(black_box(snapshot)).expect("benchmark snapshot parsing");
    let parsed = parsed.into_crystal_state();
    parsed
        .validate_structural_invariants()
        .expect("benchmark structural validation");
    black_box(parsed.version);
}

fn verify_zero_per_frame_frontend_ipc() -> Result<RunDiagnostics, String> {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "benchmark manifest directory has no workspace parent".to_owned())?;
    let output = Command::new("node")
        .args(["--test", "scripts/phonon-interaction.test.mjs"])
        .current_dir(workspace_root)
        .output()
        .map_err(|error| format!("cannot execute phonon IPC contract: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "phonon IPC contract failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(RunDiagnostics {
        per_frame_frontend_ipc: 0,
        per_frame_frontend_ipc_evidence:
            "scripts/phonon-interaction.test.mjs passed during this benchmark invocation".to_owned(),
        production_instrumentation_default: false,
    })
}

fn command_output(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git").args(arguments).output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        None
    }
}

fn report_context() -> ReportContext {
    let worktree_status = command_output(&["status", "--porcelain"]);
    ReportContext {
        commit: command_output(&["rev-parse", "HEAD"])
            .unwrap_or_else(|| "NOT_AVAILABLE".to_owned()),
        worktree_clean: worktree_status.is_some_and(|status| status.is_empty()),
        release_build: true,
        os: env::consts::OS,
        cpu: "NOT_AVAILABLE".to_owned(),
        gpu: "NOT_AVAILABLE".to_owned(),
        architecture: env::consts::ARCH,
        viewport: "NOT_AVAILABLE (headless Criterion harness)".to_owned(),
        power_state: "NOT_AVAILABLE".to_owned(),
        warmup_iterations: 0,
        measurement_iterations: PROBE_ITERATIONS,
    }
}

fn probe_dataset(dataset: &DatasetSpec, settings: &AppSettings) -> Result<DatasetReport, String> {
    let state = synthetic_state(dataset);
    state
        .validate_structural_invariants()
        .map_err(|error| format!("synthetic benchmark state is invalid: {error}"))?;
    let state_version = state.version;
    let snapshot = serde_json::to_vec(&state).map_err(|error| error.to_string())?;
    let instances = crystal_canvas::wannier::build_atoms_with_ghosts(&state, settings)
        .map_err(|error| format!("{error:?}"))?;
    let pick_scene: Vec<PickAtom> = instances
        .iter()
        .filter_map(|instance| {
            instance.pick_radius.map(|radius| PickAtom {
                pos: instance.atom.position,
                radius,
                index: instance.source_atom_index,
            })
        })
        .collect();

    let mut metrics = BTreeMap::new();
    metrics.insert(
        METRICS[0],
        measure(|| {
            parse_command_and_validate_snapshot(&snapshot);
            None
        }),
    );
    metrics.insert(
        METRICS[1],
        measure(|| {
            let serialized = serde_json::to_vec(black_box(&state)).expect("snapshot serialization");
            let bytes = serialized.len();
            black_box(serialized);
            Some(bytes)
        }),
    );
    metrics.insert(
        METRICS[2],
        measure(|| {
            let instances = crystal_canvas::wannier::build_atoms_with_ghosts(&state, settings)
                .expect("scene build");
            black_box(prepare_atom_scene(instances).expect("scene preparation"));
            None
        }),
    );
    metrics.insert(
        METRICS[3],
        unavailable("headless Criterion harness does not own a WGPU device for upload timing"),
    );
    metrics.insert(
        METRICS[4],
        measure(|| {
            black_box(ray_pick(
                &pick_scene,
                &Ray {
                    origin: [0.0, 0.0, -16.0],
                    direction: [0.0, 0.0, 1.0],
                },
            ));
            None
        }),
    );
    metrics.insert(
        METRICS[5],
        unavailable(format!(
            "renderer-owned {DRAG_PREVIEWS_PER_COMMIT} preview plus one commit session requires a live WGPU renderer"
        )),
    );
    metrics.insert(
        METRICS[6],
        unavailable("renderer pacing and GPU uniform updates require a live WGPU renderer"),
    );
    metrics.insert(
        METRICS[7],
        unavailable("headless Criterion harness does not own a WebView compositor"),
    );
    metrics.insert(
        METRICS[8],
        unavailable("headless Criterion harness does not own a Tauri listener lifecycle"),
    );

    Ok(DatasetReport {
        id: dataset.id.clone(),
        intrinsic_atoms: dataset.intrinsic_atoms,
        sha256: dataset.sha256.clone(),
        state_versions: StateVersions {
            snapshot: state_version,
            scene: state_version,
        },
        metrics,
    })
}

fn run_contract_probe() -> Result<(), String> {
    let report_path = env::var("PERF_REPORT_PATH")
        .map_err(|_| "PERF_BENCHMARK_CONTRACT_PROBE requires PERF_REPORT_PATH".to_owned())?;
    let manifest = load_manifest();
    let settings = AppSettings::default();
    let datasets = manifest
        .datasets
        .iter()
        .map(|dataset| probe_dataset(dataset, &settings))
        .collect::<Result<Vec<_>, _>>()?;
    let summary = datasets
        .first()
        .ok_or_else(|| "performance manifest has no probe dataset".to_owned())?;

    let report = BenchmarkReport {
        schema_version: manifest.schema_version,
        context: report_context(),
        summary_dataset_id: summary.id.clone(),
        state_versions: summary.state_versions.clone(),
        metrics: summary.metrics.clone(),
        diagnostics: verify_zero_per_frame_frontend_ipc()?,
        datasets,
    };
    let file = File::create(report_path).map_err(|error| error.to_string())?;
    serde_json::to_writer_pretty(file, &report).map_err(|error| error.to_string())
}

fn bench_visualization(c: &mut Criterion) {
    assert!(!PRODUCTION_INSTRUMENTATION_DEFAULT);
    verify_zero_per_frame_frontend_ipc()
        .unwrap_or_else(|error| panic!("PERF-1A phonon IPC contract failed: {error}"));
    let manifest = load_manifest();
    let settings = AppSettings::default();

    for dataset in &manifest.datasets {
        let state = synthetic_state(dataset);
        state
            .validate_structural_invariants()
            .expect("synthetic benchmark state must validate");
        let snapshot = serde_json::to_vec(&state).expect("snapshot serialization must succeed");
        let instances = crystal_canvas::wannier::build_atoms_with_ghosts(&state, &settings)
            .expect("synthetic structure must produce a render scene");
        let pick_scene: Vec<PickAtom> = instances
            .iter()
            .filter_map(|instance| {
                instance.pick_radius.map(|radius| PickAtom {
                    pos: instance.atom.position,
                    radius,
                    index: instance.source_atom_index,
                })
            })
            .collect();

        let mut group = c.benchmark_group(&dataset.id);
        group.bench_function(
            BenchmarkId::new(METRICS[0], dataset.intrinsic_atoms),
            |bench| {
                bench.iter(|| {
                    parse_command_and_validate_snapshot(&snapshot);
                });
            },
        );
        group.bench_function(
            BenchmarkId::new(METRICS[1], dataset.intrinsic_atoms),
            |bench| {
                bench.iter(|| {
                    let serialized =
                        serde_json::to_vec(black_box(&state)).expect("snapshot serialization");
                    black_box(serialized.len());
                    black_box(serialized);
                });
            },
        );
        group.bench_function(
            BenchmarkId::new(METRICS[2], dataset.intrinsic_atoms),
            |bench| {
                bench.iter(|| {
                    let instances = crystal_canvas::wannier::build_atoms_with_ghosts(
                        black_box(&state),
                        &settings,
                    )
                    .expect("scene build");
                    black_box(prepare_atom_scene(instances).expect("scene preparation"));
                });
            },
        );
        group.bench_function(
            BenchmarkId::new(METRICS[4], dataset.intrinsic_atoms),
            |bench| {
                bench.iter(|| {
                    black_box(ray_pick(
                        &pick_scene,
                        &Ray {
                            origin: [0.0, 0.0, -16.0],
                            direction: [0.0, 0.0, 1.0],
                        },
                    ));
                });
            },
        );
        group.finish();
    }

    for metric in [METRICS[3], METRICS[5], METRICS[6], METRICS[7], METRICS[8]] {
        println!("{metric}: NOT_AVAILABLE");
    }
}

criterion_group!(benches, bench_visualization);

fn main() {
    if env::var_os("PERF_BENCHMARK_CONTRACT_PROBE").is_some() {
        run_contract_probe()
            .unwrap_or_else(|error| panic!("PERF-1A contract probe failed: {error}"));
        return;
    }
    let mut criterion = Criterion::default().configure_from_args();
    bench_visualization(&mut criterion);
    criterion.final_summary();
}
