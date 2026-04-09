use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::path::PathBuf;
use crystal_canvas::io::chgcar_parser::parse_chgcar;
use crystal_canvas::io::cube_parser::parse_cube;
use crystal_canvas::io::xsf_volumetric_parser::parse_xsf_volumetric;

fn get_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/data")
}

fn bench_chgcar_parser(c: &mut Criterion) {
    let path = get_data_dir().join("BFO/CHGCAR");
    if path.exists() {
        let mut group = c.benchmark_group("Volumetric");
        group.sample_size(10);
        group.bench_function("parse_chgcar", |b| {
            b.iter(|| {
                let res = parse_chgcar(black_box(path.to_str().unwrap())).expect("CHGCAR read failed");
                black_box(res);
            })
        });
    } else {
        println!("Skipping parse_chgcar bench: File not found ({})", path.display());
    }
}

fn bench_cube_parser(c: &mut Criterion) {
    let path = get_data_dir().join("anatase/anatase_00001.cube");
    if path.exists() {
        let mut group = c.benchmark_group("Volumetric");
        group.sample_size(10);
        group.bench_function("parse_cube", |b| {
            b.iter(|| {
                let res = parse_cube(black_box(path.to_str().unwrap())).expect("Cube read failed");
                black_box(res);
            })
        });
    } else {
        println!("Skipping parse_cube bench: File not found ({})", path.display());
    }
}

fn bench_xsf_parser(c: &mut Criterion) {
    let path = get_data_dir().join("anatase/o-3D_QP_BSE.exc_qpt1_3d_1.xsf");
    if path.exists() {
        let mut group = c.benchmark_group("Volumetric");
        group.sample_size(10);
        group.bench_function("parse_xsf", |b| {
            b.iter(|| {
                let res = parse_xsf_volumetric(black_box(path.to_str().unwrap())).expect("XSF read failed");
                black_box(res);
            })
        });
    } else {
        println!("Skipping parse_xsf bench: File not found ({})", path.display());
    }
}

criterion_group!(benches, bench_chgcar_parser, bench_cube_parser, bench_xsf_parser);
criterion_main!(benches);
