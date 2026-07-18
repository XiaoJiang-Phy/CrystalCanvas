fn position(source: &str, needle: &str) -> usize {
    source.find(needle).unwrap_or_else(|| panic!("missing {needle}"))
}

#[test]
fn prepared_candidate_rejection_precedes_every_commit_effect() {
    let source = include_str!("../src/transaction.rs");
    let validate = position(source, "prepared\n        .validate_structural_invariants()");
    let version = position(source, "let version = stamp_version");
    let renderer = position(source, "let mut renderer = renderer_state");
    let commit_atoms = position(source, "renderer.commit_atoms(atom_scene);");
    let state = position(source, "*cs = prepared;");
    let history = position(source, "u_stack.push(previous_state);");
    let event = position(source, "\"state_changed\",");

    assert!(validate < version);
    assert!(validate < renderer);
    assert!(validate < commit_atoms);
    assert!(validate < state);
    assert!(validate < history);
    assert!(validate < event);
}

#[test]
fn in_place_failure_paths_restore_before_commit_or_event() {
    let source = include_str!("../src/transaction.rs");
    let mutation_failure = position(source, "if let Err(error) = f(&mut cs)");
    let first_restore = position(source, "pre_mutation_snapshot.restore_for_rollback(&mut cs);");
    let renderer_lock = position(source, "let mut renderer = match renderer_state.lock()");
    let version = position(source, "let version = commit_version(&mut cs, pending_version)?;");
    let history = position(source, "u_stack.push(pre_mutation_snapshot);");
    let event = source.rfind("app.emit(\"state_changed\"").unwrap();

    assert!(mutation_failure < first_restore);
    assert!(first_restore < renderer_lock);
    assert!(first_restore < version);
    assert!(first_restore < history);
    assert!(first_restore < event);
    assert!(source.matches("pre_mutation_snapshot.restore_for_rollback(&mut cs);").count() >= 5);
}

#[test]
fn history_candidate_rejection_swaps_back_before_history_or_event() {
    let source = include_str!("../src/commands/editing.rs");
    let undo_swap = position(source, "candidate.swap_structural_fields(&mut cs);\n    if let Err(error) = cs.validate_structural_invariants()");
    let undo_restore = position(source, "candidate.swap_structural_fields(&mut cs);\n        }\n        return Err");
    let undo_commit = position(source, "u_stack.commit_undo()");
    let undo_event = position(source, "\"undo_stack_changed\",");

    assert!(undo_swap < undo_restore);
    assert!(undo_restore < undo_commit);
    assert!(undo_restore < undo_event);

    let redo_start = position(source, "pub fn redo(");
    let redo = &source[redo_start..];
    let redo_swap = position(redo, "candidate.swap_structural_fields(&mut cs);\n    if let Err(error) = cs.validate_structural_invariants()");
    let redo_restore = position(redo, "candidate.swap_structural_fields(&mut cs);\n        }\n        return Err");
    let redo_commit = position(redo, "u_stack.commit_redo()");
    let redo_event = position(redo, "\"undo_stack_changed\",");

    assert!(redo_swap < redo_restore);
    assert!(redo_restore < redo_commit);
    assert!(redo_restore < redo_event);
}

#[test]
fn invalid_parser_candidates_fail_before_lock_or_state_events() {
    let source = include_str!("../src/commands/analysis.rs");
    let interactive_start = position(source, "pub fn load_phonon_interactive");
    let interactive_end = position(source, "pub fn load_axsf_phonon");
    let interactive = &source[interactive_start..interactive_end];
    let structure = position(interactive, "new_state\n        .validate_structural_invariants()");
    let phonon_mismatch = position(interactive, "new_state.intrinsic_sites != data.n_atoms");
    let lock = position(interactive, "let mut cs = crystal_state");
    let state_event = position(interactive, "\"state_changed\",");
    let undo_event = position(interactive, "\"undo_stack_changed\",");

    assert!(structure < lock);
    assert!(phonon_mismatch < lock);
    assert!(structure < state_event);
    assert!(phonon_mismatch < undo_event);

    let axsf = &source[interactive_end..];
    let axsf_validate = position(axsf, "new_state\n        .validate_structural_invariants()");
    let axsf_lock = position(axsf, "let mut cs = crystal_state");
    assert!(axsf_validate < axsf_lock);
}
