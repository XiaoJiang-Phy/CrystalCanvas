//! [Node 4.1] 非法物理结构拦截器 (Overlap Detection) 测试
//!
//! 验收标准:
//! - 插入与现有原子距离 ≤ 0.5Å 的新原子 → CollisionError
//! - 引擎拒绝更新 GPU Buffer
//! - 安全距离的原子可正常插入
//!
//! 当前状态: #[ignore] — 等待碰撞检测模块实现

// TODO: uncomment when overlap detection module is implemented
// use crystal_canvas::crystal_state::{CrystalState, CollisionError};

// ===========================================================================
// Helpers (to be replaced with actual constructors)
// ===========================================================================

// fn make_test_state_with_atom_at_origin() -> CrystalState {
//     // Create a minimal CrystalState with one atom at fractional (0, 0, 0)
//     // in a cubic cell a=b=c=5.0Å
//     todo!("Create test fixture")
// }

// ===========================================================================
// 碰撞检测拦截测试
// ===========================================================================

/// Insert atom at distance ~0.42Å from existing atom → must be rejected.
/// Distance: sqrt(0.3² + 0.3² + 0.0²) ≈ 0.424Å < 0.5Å threshold
#[test]
#[ignore = "Awaiting overlap detection module implementation"]
fn test_insert_overlapping_atom_rejected() {
    // let mut state = make_test_state_with_atom_at_origin();
    // let initial_version = state.version;
    //
    // // Try to insert C atom at ~0.42Å from the origin atom
    // let result = state.try_add_atom("C", [0.06, 0.06, 0.0]); // fract coords in 5Å cell
    //
    // assert!(
    //     matches!(result, Err(CollisionError { .. })),
    //     "Inserting atom at ~0.42Å should trigger CollisionError"
    // );
    //
    // // State version must NOT have changed (GPU buffer not updated)
    // assert_eq!(state.version, initial_version, "Version must not change on rejected insert");
}

/// Insert atom exactly at 0.5Å threshold → borderline, should be rejected.
#[test]
#[ignore = "Awaiting overlap detection module implementation"]
fn test_insert_at_exact_threshold_rejected() {
    // let mut state = make_test_state_with_atom_at_origin();
    //
    // // Place atom exactly at 0.5Å distance
    // // In a 5Å cubic cell, fract distance = 0.5/5.0 = 0.1
    // let result = state.try_add_atom("O", [0.1, 0.0, 0.0]); // 0.5Å away
    //
    // assert!(result.is_err(), "Atom at exactly 0.5Å should be rejected");
}

/// Insert atom at safe distance (> 1.0Å) → should succeed.
#[test]
#[ignore = "Awaiting overlap detection module implementation"]
fn test_insert_valid_atom_accepted() {
    // let mut state = make_test_state_with_atom_at_origin();
    // let initial_version = state.version;
    //
    // // Insert at ~2.5Å away (well above 0.5Å threshold)
    // let result = state.try_add_atom("O", [0.5, 0.0, 0.0]); // 2.5Å in 5Å cell
    //
    // assert!(result.is_ok(), "Insert at safe distance should succeed");
    // assert_eq!(state.version, initial_version + 1, "Version should increment");
    // assert_eq!(state.num_atoms(), 2);
}

/// Insert atom into empty state should always succeed.
#[test]
#[ignore = "Awaiting overlap detection module implementation"]
fn test_insert_into_empty_state() {
    // let mut state = CrystalState::empty_cubic(5.0);
    // let result = state.try_add_atom("Si", [0.0, 0.0, 0.0]);
    // assert!(result.is_ok(), "First atom in empty state should always succeed");
}

/// Multiple overlapping insertions: only the first should succeed.
#[test]
#[ignore = "Awaiting overlap detection module implementation"]
fn test_multiple_overlapping_insertions() {
    // let mut state = CrystalState::empty_cubic(5.0);
    //
    // let r1 = state.try_add_atom("Fe", [0.0, 0.0, 0.0]);
    // assert!(r1.is_ok());
    //
    // // Second atom at ~0.3Å away
    // let r2 = state.try_add_atom("Fe", [0.04, 0.04, 0.0]);
    // assert!(r2.is_err(), "Second atom too close should be rejected");
    //
    // // Third atom far away should succeed
    // let r3 = state.try_add_atom("Fe", [0.5, 0.5, 0.5]);
    // assert!(r3.is_ok());
    //
    // assert_eq!(state.num_atoms(), 2, "Only 2 of 3 atoms should be accepted");
}
