//! [Node 2.2] Ray-Picking accuracy and performance tests
//!
//! Acceptance criteria:
//! - Overlapping atoms (different Z depths): pick the nearest atom by intersection point
//! - 1,000 atom scene: pick computation < 1ms
//!
//! Status: ACTIVE — ray-picking module implemented

use crystal_canvas::renderer::ray_picking::{PickAtom, Ray, ray_pick};

// ===========================================================================
// Accuracy tests
// ===========================================================================

/// Two overlapping atoms at different Z depths — must pick the nearest.
#[test]
fn test_pick_nearest_of_overlapping_atoms() {
    // Atom 0: far from camera (z = -5.0)
    // Atom 1: near to camera (z = -3.0)
    // Camera at origin, looking down -Z
    // Both atoms at (x=0, y=0), same radius → ray through center hits both

    let atoms = vec![
        PickAtom {
            pos: [0.0, 0.0, -5.0],
            radius: 1.0,
            index: 0,
        }, // far
        PickAtom {
            pos: [0.0, 0.0, -3.0],
            radius: 1.0,
            index: 1,
        }, // near
    ];
    let ray = Ray {
        origin: [0.0, 0.0, 0.0],
        direction: [0.0, 0.0, -1.0],
    };
    let hit = ray_pick(&atoms, &ray);
    assert_eq!(
        hit.unwrap().index,
        1,
        "Should pick the nearest atom (index 1)"
    );
}

/// Ray that misses all atoms should return None.
#[test]
fn test_pick_miss_returns_none() {
    let atoms = vec![PickAtom {
        pos: [10.0, 10.0, -5.0],
        radius: 1.0,
        index: 0,
    }];
    let ray = Ray {
        origin: [0.0, 0.0, 0.0],
        direction: [0.0, 0.0, -1.0],
    };
    let hit = ray_pick(&atoms, &ray);
    assert!(hit.is_none(), "Ray missing all atoms should return None");
}

/// Picking with different radii — larger atom at far distance may still
/// be intersected, but nearest intersection point wins.
#[test]
fn test_pick_respects_intersection_point_not_center() {
    // Atom 0: far but large radius (z=-10, r=5.0) → front surface at z=-5
    // Atom 1: near but small radius (z=-3, r=0.5) → front surface at z=-2.5
    // Ray through origin down -Z: hits atom 1 front surface first

    let atoms = vec![
        PickAtom {
            pos: [0.0, 0.0, -10.0],
            radius: 5.0,
            index: 0,
        },
        PickAtom {
            pos: [0.0, 0.0, -3.0],
            radius: 0.5,
            index: 1,
        },
    ];
    let ray = Ray {
        origin: [0.0, 0.0, 0.0],
        direction: [0.0, 0.0, -1.0],
    };
    let hit = ray_pick(&atoms, &ray);
    assert_eq!(hit.unwrap().index, 1, "Nearest intersection point wins");
}

/// Empty scene should return None.
#[test]
fn test_pick_empty_scene() {
    let atoms: Vec<PickAtom> = vec![];
    let ray = Ray {
        origin: [0.0, 0.0, 0.0],
        direction: [0.0, 0.0, -1.0],
    };
    let hit = ray_pick(&atoms, &ray);
    assert!(hit.is_none());
}

// ===========================================================================
// Performance tests
// ===========================================================================

/// 1,000 atom scene: pick computation must complete in < 1ms.
#[test]
fn test_pick_performance_1000_atoms() {
    // Generate 1,000 atoms in a 10x10x10 grid
    let mut atoms = Vec::with_capacity(1000);
    for i in 0..10 {
        for j in 0..10 {
            for k in 0..10 {
                atoms.push(PickAtom {
                    pos: [i as f32 * 3.0, j as f32 * 3.0, -(k as f32 * 3.0 + 5.0)],
                    radius: 1.0,
                    index: atoms.len(),
                });
            }
        }
    }

    let ray = Ray {
        origin: [0.0, 0.0, 0.0],
        direction: [0.0, 0.0, -1.0],
    };

    // Warm up
    let _ = ray_pick(&atoms, &ray);

    let start = std::time::Instant::now();
    let _hit = ray_pick(&atoms, &ray);
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_micros() < 1000,
        "Ray-pick on 1,000 atoms took {:?} (must be < 1ms)",
        elapsed
    );
}
