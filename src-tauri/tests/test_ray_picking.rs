//! [Node 2.2] 射线拾取 (Ray-Picking) 精确度与性能测试
//!
//! 验收标准:
//! - 重叠原子（不同 Z 深度）拾取最靠近相机的原子 Index
//! - 1,000 原子场景拾取计算时间 < 1ms
//!
//! 当前状态: #[ignore] — 等待 ray-picking 模块实现

// TODO: uncomment when ray-picking module is implemented
// use crystal_canvas::rendering::{Ray, Atom, ray_pick, HitResult};

// ===========================================================================
// 精确度测试
// ===========================================================================

/// Two overlapping atoms at different Z depths — must pick the nearest.
#[test]
#[ignore = "Awaiting ray-picking module implementation"]
fn test_pick_nearest_of_overlapping_atoms() {
    // Atom 0: far from camera (z = -5.0)
    // Atom 1: near to camera (z = -3.0)
    // Camera at origin, looking down -Z
    // Both atoms at (x=0, y=0), same radius → ray through center hits both

    // let atoms = vec![
    //     Atom { pos: [0.0, 0.0, -5.0], radius: 1.0, index: 0 },
    //     Atom { pos: [0.0, 0.0, -3.0], radius: 1.0, index: 1 },
    // ];
    // let ray = Ray { origin: [0.0, 0.0, 0.0], direction: [0.0, 0.0, -1.0] };
    // let hit = ray_pick(&atoms, &ray);
    // assert_eq!(hit.unwrap().index, 1, "Should pick the nearest atom (index 1)");
}

/// Ray that misses all atoms should return None.
#[test]
#[ignore = "Awaiting ray-picking module implementation"]
fn test_pick_miss_returns_none() {
    // let atoms = vec![
    //     Atom { pos: [10.0, 10.0, -5.0], radius: 1.0, index: 0 },
    // ];
    // let ray = Ray { origin: [0.0, 0.0, 0.0], direction: [0.0, 0.0, -1.0] };
    // let hit = ray_pick(&atoms, &ray);
    // assert!(hit.is_none(), "Ray missing all atoms should return None");
}

/// Picking with different radii — larger atom at far distance may still
/// be intersected, but nearest intersection point wins.
#[test]
#[ignore = "Awaiting ray-picking module implementation"]
fn test_pick_respects_intersection_point_not_center() {
    // Atom 0: far but large radius (z=-10, r=5.0) → front surface at z=-5
    // Atom 1: near but small radius (z=-3, r=0.5) → front surface at z=-2.5
    // Ray through origin down -Z: hits atom 1 front surface first

    // let atoms = vec![
    //     Atom { pos: [0.0, 0.0, -10.0], radius: 5.0, index: 0 },
    //     Atom { pos: [0.0, 0.0, -3.0], radius: 0.5, index: 1 },
    // ];
    // let ray = Ray { origin: [0.0, 0.0, 0.0], direction: [0.0, 0.0, -1.0] };
    // let hit = ray_pick(&atoms, &ray);
    // assert_eq!(hit.unwrap().index, 1, "Nearest intersection point wins");
}

/// Empty scene should return None.
#[test]
#[ignore = "Awaiting ray-picking module implementation"]
fn test_pick_empty_scene() {
    // let atoms: Vec<Atom> = vec![];
    // let ray = Ray { origin: [0.0, 0.0, 0.0], direction: [0.0, 0.0, -1.0] };
    // let hit = ray_pick(&atoms, &ray);
    // assert!(hit.is_none());
}

// ===========================================================================
// 性能测试
// ===========================================================================

/// 1,000 atom scene: pick computation must complete in < 1ms.
#[test]
#[ignore = "Awaiting ray-picking module implementation"]
fn test_pick_performance_1000_atoms() {
    // Generate 1,000 atoms in a grid
    // let mut atoms = Vec::with_capacity(1000);
    // for i in 0..10 {
    //     for j in 0..10 {
    //         for k in 0..10 {
    //             atoms.push(Atom {
    //                 pos: [i as f32 * 3.0, j as f32 * 3.0, -(k as f32 * 3.0 + 5.0)],
    //                 radius: 1.0,
    //                 index: atoms.len(),
    //             });
    //         }
    //     }
    // }
    //
    // let ray = Ray { origin: [0.0, 0.0, 0.0], direction: [0.0, 0.0, -1.0] };
    //
    // // Warm up
    // let _ = ray_pick(&atoms, &ray);
    //
    // let start = std::time::Instant::now();
    // let _hit = ray_pick(&atoms, &ray);
    // let elapsed = start.elapsed();
    //
    // assert!(
    //     elapsed.as_micros() < 1000,
    //     "Ray-pick on 1,000 atoms took {:?} (must be < 1ms)",
    //     elapsed
    // );
}
