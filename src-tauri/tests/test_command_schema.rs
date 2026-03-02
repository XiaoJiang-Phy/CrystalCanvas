//! [Node 5.1] LLM command injection protection test (Command Schema Validation)
//!
//! Acceptance Criteria:
//! - Malicious or malformed JSON commands must be rejected by the Serde layer (Result::Err)
//! - Valid commands must be correctly deserialized
//! - CrystalCommand uses deny_unknown_fields to prevent injection attempts
//!
//! Current Status: Active — All tests enabled after M9 Command Bus implementation
//! Schema Source: crystal_canvas::llm::command::CrystalCommand

// ===========================================================================
// Schema Validation Tests
// imported from src/llm/command.rs
// ===========================================================================

use crystal_canvas::llm::command::*;

// ===========================================================================
// Malicious Input Rejection Tests
// ===========================================================================

/// Negative index in delete_atoms → Serde must reject (u32 cannot be negative).
#[test]
fn test_negative_index_rejected() {
    let json = r#"{"action": "delete_atoms", "params": {"indices": [-1]}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "Negative index should be rejected by u32 type"
    );
}

/// Missing required field (indices) in delete_atoms → Err.
#[test]
fn test_missing_required_field_rejected() {
    let json = r#"{"action": "delete_atoms", "params": {}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "Missing 'indices' field should be rejected"
    );
}

/// Unknown action type → Err.
#[test]
fn test_unknown_action_rejected() {
    let json = r#"{"action": "hack_system", "params": {}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Unknown action should be rejected");
}

/// Extra/unknown fields in params (injection attempt) → Err.
#[test]
fn test_unknown_fields_in_params_rejected() {
    let json = r#"{"action": "delete_atoms", "params": {"indices": [0], "evil": true}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "Unknown fields should be rejected (deny_unknown_fields)"
    );
}

/// Extra fields at top level → Err.
#[test]
fn test_unknown_top_level_fields_rejected() {
    let json = r#"{"action": "delete_atoms", "params": {"indices": [0]}, "inject": "payload"}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "Unknown top-level fields should be rejected"
    );
}

/// Completely malformed JSON → Err.
#[test]
fn test_malformed_json_rejected() {
    let json = r#"{"action": "delete_atoms", INVALID"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Malformed JSON should be rejected");
}

/// Empty string → Err.
#[test]
fn test_empty_string_rejected() {
    let json = "";
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Empty string should be rejected");
}

/// Wrong type for miller indices (strings instead of ints) → Err.
#[test]
fn test_wrong_type_miller_rejected() {
    let json = r#"{"action": "cleave_slab", "params": {"miller": ["a","b","c"], "layers": 3, "vacuum_a": 15.0}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_err(), "String miller indices should be rejected");
}

/// Params is null → Err.
#[test]
fn test_null_params_rejected() {
    let json = r#"{"action": "delete_atoms", "params": null}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Null params should be rejected");
}

// ===========================================================================
// Valid Input Acceptance Tests
// ===========================================================================

/// Valid delete_atoms command.
#[test]
fn test_valid_delete_atoms_accepted() {
    let json = r#"{"action": "delete_atoms", "params": {"indices": [0, 1, 42]}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_ok(), "Valid delete_atoms should be accepted");
    if let Ok(CrystalCommand::DeleteAtoms(params)) = result {
        assert_eq!(params.indices, vec![0, 1, 42]);
    }
}

/// Valid add_atom command.
#[test]
fn test_valid_add_atom_accepted() {
    let json =
        r#"{"action": "add_atom", "params": {"element": "Si", "frac_pos": [0.25, 0.25, 0.25]}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_ok(), "Valid add_atom should be accepted");
}

/// Valid make_supercell command.
#[test]
fn test_valid_make_supercell_accepted() {
    let json = r#"{"action": "make_supercell", "params": {"matrix": [[2,0,0],[0,2,0],[0,0,2]]}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_ok(), "Valid make_supercell should be accepted");
}

/// Valid cleave_slab command.
#[test]
fn test_valid_cleave_slab_accepted() {
    let json = r#"{"action": "cleave_slab", "params": {"miller": [1,1,1], "layers": 3, "vacuum_a": 15.0}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_ok(), "Valid cleave_slab should be accepted");
}

/// Valid export_file command.
#[test]
fn test_valid_export_file_accepted() {
    let json =
        r#"{"action": "export_file", "params": {"format": "POSCAR", "path": "/tmp/POSCAR"}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "Valid export_file should be accepted: {:?}",
        result.err()
    );
}

/// Valid batch command.
#[test]
fn test_valid_batch_accepted() {
    let json = r#"{"action": "batch", "params": {"commands": [{"action": "delete_atoms", "params": {"indices": [0]}}]}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_ok(), "Valid batch should be accepted");
}

// ===========================================================================
// Physics Sandbox Tests
// ===========================================================================

#[test]
fn test_sandbox_index_out_of_bounds() {
    use crystal_canvas::crystal_state::CrystalState;
    use crystal_canvas::llm::sandbox::validate_command;

    let state = CrystalState {
        elements: vec!["Si".to_string(), "Si".to_string()],
        fract_x: vec![0.0, 0.5],
        fract_y: vec![0.0, 0.5],
        fract_z: vec![0.0, 0.5],
        ..Default::default()
    };

    // Deleting index 2 should fail
    let cmd = CrystalCommand::DeleteAtoms(DeleteAtomsParams { indices: vec![2] });
    assert!(validate_command(&cmd, &state).is_err());
}

#[test]
fn test_sandbox_vacuum_too_small() {
    use crystal_canvas::crystal_state::CrystalState;
    use crystal_canvas::llm::sandbox::validate_command;

    let state = CrystalState::default();
    let cmd = CrystalCommand::CleaveSlab(CleavSlabParams {
        miller: [1, 0, 0],
        layers: 1,
        vacuum_a: 4.9,
    });
    assert!(validate_command(&cmd, &state).is_err());
}

#[test]
fn test_sandbox_vacuum_too_large() {
    use crystal_canvas::crystal_state::CrystalState;
    use crystal_canvas::llm::sandbox::validate_command;

    let state = CrystalState::default();
    let cmd = CrystalCommand::CleaveSlab(CleavSlabParams {
        miller: [1, 0, 0],
        layers: 1,
        vacuum_a: 100.1,
    });
    assert!(validate_command(&cmd, &state).is_err());
}

#[test]
fn test_sandbox_supercell_negative_det() {
    use crystal_canvas::crystal_state::CrystalState;
    use crystal_canvas::llm::sandbox::validate_command;

    let state = CrystalState::default();
    let cmd = CrystalCommand::MakeSupercell(MakeSupercellParams {
        matrix: [[-1, 0, 0], [0, 1, 0], [0, 0, 1]],
    });
    assert!(validate_command(&cmd, &state).is_err());
}

// ===========================================================================
// End-to-End M9 Integration Test
// ===========================================================================

#[test]
fn test_end_to_end_mock_llm() {
    use crystal_canvas::crystal_state::CrystalState;
    use crystal_canvas::llm::sandbox::validate_command;
    use crystal_canvas::llm::router::execute_command;
    use crystal_canvas::llm::command::CrystalCommand;

    // 1. Initial State
    let mut state = CrystalState::default();
    assert_eq!(state.num_atoms(), 0);

    // 2. Mock LLM JSON output (Add one Si atom at origin)
    let json_response = r#"{
        "action": "add_atom",
        "params": {
            "element": "Si",
            "frac_pos": [0.0, 0.0, 0.0]
        }
    }"#;

    // 3. Layer 1: Schema Validation (Parse gate)
    let command: CrystalCommand = serde_json::from_str(json_response).expect("Schema validation failed");

    // 4. Layer 2: Physics Sandbox
    validate_command(&command, &state).expect("Physics sandbox validation failed");

    // 5. Layer 3: Dispatch to Engine (Router)
    execute_command(command, &mut state).expect("Execution failed");

    // 6. Verify state update
    assert_eq!(state.num_atoms(), 1);
    assert_eq!(state.elements[0], "Si");
    assert_eq!(state.fract_x[0], 0.0);
}
