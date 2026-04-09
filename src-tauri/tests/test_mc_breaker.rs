//! Breaker Tests for Marching Cubes (Node 11.4a)
//! 
//! [Breaker Mode] Tests zero matrices, empty grids, pathological condition numbers,
//! and strict 1e-3 epsilon tolerances.

use crystal_canvas::renderer::isosurface::{marching_cubes_cpu, euler_characteristic_for_test};
use crystal_canvas::volumetric::{VolumetricData, VolumetricFormat};

#[test]
fn test_breaker_strict_epsilon_sphere() {
    // 100^3 grid on a 2.0 Angstrom cell to achieve high resolution.
    // h = 2.0 / 99 = 0.0202
    // Max interpolation error should be < 1e-3.
    let n = 100usize;
    let cell_len = 2.0f64;
    let r = 0.5f64;
    let mut data = vec![0.0f32; n * n * n];
    let half = cell_len / 2.0;

    for iz in 0..n {
        for iy in 0..n {
            for ix in 0..n {
                let x = ix as f64 / (n - 1) as f64 * cell_len;
                let y = iy as f64 / (n - 1) as f64 * cell_len;
                let z = iz as f64 / (n - 1) as f64 * cell_len;
                data[ix + iy * n + iz * n * n] = ((x - half).powi(2) + (y - half).powi(2) + (z - half).powi(2) - r * r) as f32;
            }
        }
    }

    let data_min = data.iter().cloned().fold(f32::INFINITY, f32::min);
    let data_max = data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    let vol = VolumetricData {
        grid_dims: [n, n, n],
        lattice: [cell_len, 0.0, 0.0, 0.0, cell_len, 0.0, 0.0, 0.0, cell_len],
        data_min,
        data_max,
        data,
        source_format: VolumetricFormat::VaspChgcar,
        origin: [0.0, 0.0, 0.0],
    };

    let verts = marching_cubes_cpu(&vol, 0.0);
    assert!(!verts.is_empty(), "Mesh should be non-empty");

    let chi = euler_characteristic_for_test(&verts);
    assert_eq!(chi, 2, "Euler characteristic must be 2 for a closed sphere, got {}", chi);

    let mut max_err = 0.0f32;
    for v in &verts {
        let [x, y, z] = v.position;
        let dist = ((x - half as f32).powi(2) + (y - half as f32).powi(2) + (z - half as f32).powi(2)).sqrt();
        let err = (dist - r as f32).abs();
        if err > max_err { max_err = err; }
    }
    assert!(max_err <= 1e-3, "Vertex error {} exceeded strict 1e-3 epsilon", max_err);
}

#[test]
fn test_breaker_empty_grid() {
    let vol = VolumetricData {
        grid_dims: [0, 0, 0],
        lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        data_min: 0.0,
        data_max: 0.0,
        data: vec![],
        source_format: VolumetricFormat::VaspChgcar,
        origin: [0.0, 0.0, 0.0],
    };

    let verts = marching_cubes_cpu(&vol, 0.0);
    assert!(verts.is_empty(), "Empty grid should return 0 vertices without panic");
}

#[test]
fn test_breaker_single_voxel() {
    let vol = VolumetricData {
        grid_dims: [1, 1, 1],
        lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        data_min: 1.0,
        data_max: 1.0,
        data: vec![1.0],
        source_format: VolumetricFormat::VaspChgcar,
        origin: [0.0, 0.0, 0.0],
    };

    let verts = marching_cubes_cpu(&vol, 0.0);
    assert!(verts.is_empty(), "Single element grid should return 0 vertices without panic");
}

#[test]
fn test_breaker_non_cubic_dims() {
    let nx = 2;
    let ny = 3;
    let nz = 100;
    let vol = VolumetricData {
        grid_dims: [nx, ny, nz],
        lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        data_min: 0.0,
        data_max: 0.0,
        data: vec![0.0; nx * ny * nz],
        source_format: VolumetricFormat::VaspChgcar,
        origin: [0.0, 0.0, 0.0],
    };
    
    // Test that extreme aspect ratios don't cause an out-of-bounds flat array access
    let verts = marching_cubes_cpu(&vol, -1.0);
    assert!(verts.is_empty(), "Non-cubic dims should be processed without panic");
}

#[test]
fn test_breaker_zero_field() {
    let n = 5;
    let vol = VolumetricData {
        grid_dims: [n, n, n],
        lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        data_min: 0.0,
        data_max: 0.0,
        data: vec![0.0; n * n * n],
        source_format: VolumetricFormat::VaspChgcar,
        origin: [0.0, 0.0, 0.0],
    };

    let verts = marching_cubes_cpu(&vol, 0.0);
    for v in &verts {
        assert!(v.normal.iter().all(|n| n.is_finite()), "Zero field gradient should not cause NaN normal");
    }
}

#[test]
fn test_breaker_zero_matrix() {
    let n = 5;
    let mut data = vec![0.0f32; n * n * n];
    for i in 0..data.len() {
        data[i] = if i % 2 == 0 { 1.0 } else { -1.0 };
    }
    let vol = VolumetricData {
        grid_dims: [n, n, n],
        lattice: [0.0; 9],
        data_min: -1.0,
        data_max: 1.0,
        data,
        source_format: VolumetricFormat::VaspChgcar,
        origin: [0.0, 0.0, 0.0],
    };

    let verts = marching_cubes_cpu(&vol, 0.0);
    assert!(!verts.is_empty(), "Mixed data should yield triangles");
    for v in &verts {
        assert_eq!(v.position, [0.0, 0.0, 0.0], "Zero lattice must collapse all positions to origin");
        assert!(v.normal.iter().all(|n| n.is_finite()), "Collapsed vertices must still have finite normals");
    }
}

#[test]
fn test_breaker_nan_and_infinity_threshold() {
    let n = 5;
    let vol = VolumetricData {
        grid_dims: [n, n, n],
        lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        data_min: 0.0,
        data_max: 1.0,
        data: vec![0.5; n * n * n],
        source_format: VolumetricFormat::VaspChgcar,
        origin: [0.0, 0.0, 0.0],
    };

    let verts_nan = marching_cubes_cpu(&vol, f32::NAN);
    assert!(verts_nan.is_empty(), "NaN threshold should yield no vertices without panic");

    let verts_inf = marching_cubes_cpu(&vol, f32::INFINITY);
    assert!(verts_inf.is_empty(), "Infinity threshold should yield no vertices without panic");
    
    let verts_neg_inf = marching_cubes_cpu(&vol, f32::NEG_INFINITY);
    // NEG_INFINITY means all > threshold, so it might emit. Either way it shouldn't panic.
    assert!(verts_neg_inf.is_empty(), "NEG_INFINITY threshold means all > threshold, so it yields case 0xFF (empty)");
}

#[test]
fn test_breaker_pathological_lattice() {
    let n = 10;
    let mut data = vec![0.0f32; n * n * n];
    for i in 0..n*n*n {
        data[i] = (i as f32) - 500.0; // Mix of positive and negative
    }

    let vol = VolumetricData {
        grid_dims: [n, n, n],
        // Nearly singular matrix: column 'a' and 'b' almost collinear, 'c' is tiny
        // ColMajor: [a_x, a_y, a_z,  b_x, b_y, b_z,  c_x, c_y, c_z]
        lattice: [1.0, 1.0, 1e-10, 1.0, 1.0000001, 0.0, 0.0, 0.0, 1e-10],
        data_min: -500.0,
        data_max: 500.0,
        data,
        source_format: VolumetricFormat::VaspChgcar,
        origin: [0.0, 0.0, 0.0],
    };

    let verts = marching_cubes_cpu(&vol, 0.0);
    // Should complete without panic and not output NaN vertices despite terrible lattice
    for v in &verts {
        assert!(v.position.iter().all(|p| p.is_finite()), "Pathological lattice output NaN position");
    }
}

