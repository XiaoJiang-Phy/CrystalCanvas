//! [Node 5.1] LLM command injection protection test (Command Schema Validation)
//!
//! Acceptance Criteria:
//! - Malicious or malformed JSON commands must be rejected by the Serde layer (Result::Err)
//! - Valid commands must be correctly deserialized
//! - CrystalCommand uses deny_unknown_fields to prevent injection attempts
//!
//! Current Status: #[ignore] — Awaiting Command Bus module implementation
//! Note: The CrystalCommand schema is temporarily defined in this file and will be moved once the formal module is developed.

use serde::Deserialize;

// ===========================================================================
// TDD Skeleton: CrystalCommand Protocol Definition
// These types will be moved to the src/commands/ module during official implementation.
// ===========================================================================

/// Parameter types for each command action
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DeleteAtomsParams {
    /// Atom indices to delete — must be non-negative
    indices: Vec<u32>, // u32 enforces non-negative at type level
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct AddAtomParams {
    element: String,
    frac_pos: [f64; 3],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct SubstituteParams {
    indices: Vec<u32>,
    new_element: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct CleavSlabParams {
    miller: [i32; 3],
    layers: u32,
    vacuum_a: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct MakeSupercellParams {
    matrix: [[i32; 3]; 3],
}

/// The top-level command envelope
#[derive(Debug, Deserialize)]
#[serde(tag = "action", content = "params", deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
enum CrystalCommand {
    DeleteAtoms(DeleteAtomsParams),
    AddAtom(AddAtomParams),
    Substitute(SubstituteParams),
    CleaveSlab(CleavSlabParams),
    MakeSupercell(MakeSupercellParams),
}

// ===========================================================================
// Malicious Input Rejection Tests
// ===========================================================================

/// Negative index in delete_atoms → Serde must reject (u32 cannot be negative).
#[test]
fn test_negative_index_rejected() {
    let json = r#"{"action": "delete_atoms", "params": {"indices": [-1]}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Negative index should be rejected by u32 type");
}

/// Missing required field (indices) in delete_atoms → Err.
#[test]
fn test_missing_required_field_rejected() {
    let json = r#"{"action": "delete_atoms", "params": {}}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Missing 'indices' field should be rejected");
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
    assert!(result.is_err(), "Unknown fields should be rejected (deny_unknown_fields)");
}

/// Extra fields at top level → Err.
#[test]
fn test_unknown_top_level_fields_rejected() {
    let json = r#"{"action": "delete_atoms", "params": {"indices": [0]}, "inject": "payload"}"#;
    let result: Result<CrystalCommand, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Unknown top-level fields should be rejected");
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
    let json = r#"{"action": "add_atom", "params": {"element": "Si", "frac_pos": [0.25, 0.25, 0.25]}}"#;
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
