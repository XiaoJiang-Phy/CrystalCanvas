//! INTERACT-1A contract gate: atom dragging is a renderer-owned preview session.
//!
//! These assertions deliberately inspect the IPC boundaries instead of naming a
//! particular implementation type.  A preview update must not reach canonical
//! state, history, settings, event, or scene-rebuild ownership; only the one
//! explicit commit may cross that boundary.

fn command_body<'a>(source: &'a str, command: &str) -> &'a str {
    let signature = format!("pub fn {command}(");
    let start = source
        .find(&signature)
        .unwrap_or_else(|| panic!("missing INTERACT-1A command `{command}`"));
    let remainder = &source[start..];
    let end = remainder
        .find("\n#[tauri::command]")
        .unwrap_or(remainder.len());
    &remainder[..end]
}

fn contract_entry<'a>(source: &'a str, command: &str) -> &'a str {
    let entry = format!("    {command}:");
    let start = source
        .find(&entry)
        .unwrap_or_else(|| panic!("missing TypeScript IPC contract `{command}`"));
    let remainder = &source[start..];
    &remainder[..remainder
        .find('\n')
        .expect("IPC contract entry must terminate on one line")]
}

fn assert_absent(source: &str, forbidden: &[&str], boundary: &str) {
    for token in forbidden {
        assert!(!source.contains(token), "{boundary} must not own `{token}`");
    }
}

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start = source
        .find(start)
        .unwrap_or_else(|| panic!("missing source boundary `{start}`"));
    let remainder = &source[start..];
    let end = remainder
        .find(end)
        .unwrap_or_else(|| panic!("missing source boundary `{end}`"));
    &remainder[..end]
}

fn position(source: &str, needle: &str) -> usize {
    source
        .find(needle)
        .unwrap_or_else(|| panic!("missing `{needle}`"))
}

#[test]
fn drag_session_is_registered_at_rust_typescript_and_inventory_boundaries() {
    let editing = include_str!("../src/commands/editing.rs");
    let main = include_str!("../src/main.rs");
    let contracts = include_str!("../../src/ipc/contracts.ts");
    let inventory = include_str!("../../ipc/inventory.json");

    for command in [
        "begin_atom_drag",
        "update_atom_drag",
        "commit_atom_drag",
        "cancel_atom_drag",
    ] {
        command_body(editing, command);
        assert!(
            main.contains(&format!("commands::{command},")),
            "main invoke handler must register `{command}`"
        );
        assert!(
            inventory.contains(&format!("\"{command}\"")),
            "IPC inventory must declare `{command}`"
        );
    }

    let begin_contract = contract_entry(contracts, "begin_atom_drag");
    assert!(begin_contract.contains("indices: number[]"));
    assert!(
        begin_contract.contains("result: string"),
        "begin_atom_drag must return an opaque session id"
    );
    for command in ["update_atom_drag", "commit_atom_drag", "cancel_atom_drag"] {
        assert!(
            contract_entry(contracts, command).contains("sessionId"),
            "{command} must use the TypeScript sessionId boundary"
        );
    }
    assert!(
        contract_entry(contracts, "update_atom_drag").contains("dx: number")
            && contract_entry(contracts, "update_atom_drag").contains("dy: number"),
        "preview updates must preserve the screen-delta IPC boundary"
    );
}

#[test]
fn preview_update_is_ephemeral_and_has_no_canonical_side_effect_owner() {
    let editing = include_str!("../src/commands/editing.rs");
    let update = command_body(editing, "update_atom_drag");

    for token in ["session_id", "dx", "dy", "renderer_state"] {
        assert!(
            update.contains(token),
            "preview update must consume `{token}`"
        );
    }
    for validation in ["is_finite", "invalid_argument"] {
        assert!(
            update.contains(validation),
            "preview update must reject invalid screen deltas before rendering"
        );
    }
    assert_absent(
        update,
        &[
            "crystal_state",
            "CrystalState",
            "settings_state",
            "AppSettings",
            "undo_state",
            "UndoStack",
            "AppHandle",
            "next_version",
            "commit_version",
            "state_changed",
            "undo_stack_changed",
            "get_crystal_state",
            "build_atoms_with_ghosts",
            "prepare_atom_scene",
            "build_line_scene",
            "serialize",
        ],
        "update_atom_drag",
    );
}

#[test]
fn begin_records_intrinsic_selection_and_a_canonical_source_version() {
    let editing = include_str!("../src/commands/editing.rs");
    let begin = command_body(editing, "begin_atom_drag");
    let instances = include_str!("../src/renderer/instance.rs");

    for token in ["indices", "is_empty", "sort_unstable", "dedup"] {
        assert!(
            begin.contains(token),
            "begin_atom_drag must validate and retain intrinsic source indices (`{token}`)"
        );
    }
    assert!(
        begin.contains("version"),
        "begin_atom_drag must bind its preview to a canonical source version"
    );
    assert!(
        instances.contains("pub source_atom_index: usize")
            && instances.contains("index: instance.source_atom_index"),
        "periodic pick images must map back to intrinsic source_atom_index"
    );
}

#[test]
fn commit_is_the_single_atomic_history_and_state_transition() {
    let editing = include_str!("../src/commands/editing.rs");
    let commit = command_body(editing, "commit_atom_drag");

    for token in [
        "session_id",
        "crystal_state",
        "undo_state",
        "settings_state",
        "renderer_state",
        "version",
        "conflict",
        "commit_version",
        "state_changed",
        "undo_stack_changed",
    ] {
        assert!(
            commit.contains(token),
            "commit_atom_drag must own `{token}`"
        );
    }
    assert!(
        commit.contains("translate_atoms_cartesian"),
        "commit_atom_drag must apply the prepared displacement exactly once to CrystalState"
    );
    let conflict = commit
        .find("conflict")
        .expect("commit must reject a stale source version");
    let version_commit = commit
        .find("commit_version")
        .expect("commit must stamp exactly one canonical version");
    assert!(
        conflict < version_commit,
        "stale-session rejection must occur before any version commit"
    );
}

#[test]
fn cancel_drops_preview_and_failed_commit_rebuilds_from_canonical_state() {
    let editing = include_str!("../src/commands/editing.rs");
    let cancel = command_body(editing, "cancel_atom_drag");
    let commit = command_body(editing, "commit_atom_drag");

    for token in ["session_id", "renderer_state", "cancel_atom_drag"] {
        assert!(
            cancel.contains(token),
            "cancel_atom_drag must own `{token}`"
        );
    }
    assert_absent(
        cancel,
        &[
            "crystal_state",
            "CrystalState",
            "undo_state",
            "UndoStack",
            "next_version",
            "commit_version",
            "state_changed",
            "undo_stack_changed",
        ],
        "cancel_atom_drag",
    );
    assert!(
        commit.contains("restore"),
        "failed or stale commit must restore the renderer from canonical state"
    );
}

#[test]
fn begin_binds_the_renderer_scene_and_source_version_under_one_lock_window() {
    let editing = include_str!("../src/commands/editing.rs");
    let begin = command_body(editing, "begin_atom_drag");
    let crystal_lock = position(begin, "let cs = crystal_state");
    let renderer_lock = position(begin, "let mut renderer = renderer_state");
    let source_version = position(begin, "cs.version");

    assert!(
        crystal_lock < renderer_lock && renderer_lock < source_version,
        "begin must hold CrystalState, then Renderer, while binding the renderer scene to source version"
    );
}

#[test]
fn commit_keeps_the_session_reserved_until_the_canonical_transition_finishes() {
    let editing = include_str!("../src/commands/editing.rs");
    let commit = command_body(editing, "commit_atom_drag");
    let crystal_lock = position(commit, "let mut cs = crystal_state");
    let detach = position(commit, "take_atom_drag");

    assert!(
        crystal_lock < detach,
        "commit may detach the session only after it holds CrystalState, so a second begin cannot enter"
    );
}

#[test]
fn failed_commit_never_writes_a_detached_preview_back_into_a_canonical_scene() {
    let editing = include_str!("../src/commands/editing.rs");
    let restore = source_between(
        editing,
        "fn restore_failed_atom_drag(",
        "fn validate_atom_drag_collision(",
    );

    assert!(
        restore.contains("build_atoms_with_ghosts") && restore.contains("renderer.commit_atoms"),
        "failed commit must rebuild the renderer from current canonical state"
    );
    assert_absent(
        restore,
        &["restore_atom_drag(session)"],
        "failed commit recovery",
    );
}

#[test]
fn zero_displacement_commit_is_a_noop_before_snapshot_or_version_ownership() {
    let editing = include_str!("../src/commands/editing.rs");
    let commit = command_body(editing, "commit_atom_drag");
    let no_op = position(commit, "translation.length_squared() == 0.0");
    let snapshot = position(commit, "StructuralSnapshot::from_crystal_state");
    let version = position(commit, "next_version");

    assert!(
        no_op < snapshot && no_op < version,
        "zero drag must return before creating an undo snapshot or version"
    );
}

#[test]
fn drag_metadata_preallocates_every_periodic_instance_before_population() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let metadata = source_between(renderer, "fn drag_instances(", "\nfn active_atom_drag_mut");

    assert!(
        metadata.contains("try_reserve_exact"),
        "drag metadata must reserve its selected periodic-instance capacity before push"
    );
}

#[test]
fn committed_drag_invalidates_structure_bound_state_and_discards_old_overlays() {
    let editing = include_str!("../src/commands/editing.rs");
    let commit = command_body(editing, "commit_atom_drag");

    let scene = position(
        commit,
        "build_atoms_with_ghosts_with_overlay(&cs, &settings, None)",
    );
    let version = position(commit, "commit_version");
    let invalidate = position(commit, "invalidate_structure_bound_data");
    let clear_overlays = position(commit, "clear_structure_bound_overlays");

    assert!(
        scene < version,
        "the committed atom scene must be prepared without the old structure overlay before version ownership changes"
    );
    assert!(
        version < invalidate && invalidate < clear_overlays,
        "after version commit, drag must invalidate canonical derived data before clearing renderer overlays"
    );
}

#[test]
fn rollback_allocation_failure_restores_the_canonical_renderer_before_returning() {
    let editing = include_str!("../src/commands/editing.rs");
    let commit = command_body(editing, "commit_atom_drag");
    let rollback = source_between(
        commit,
        "let rollback = match",
        "if let Err(error) = validate_atom_drag_collision",
    );

    assert!(
        rollback.contains("Err(_)")
            && rollback.contains("restore_failed_atom_drag")
            && rollback.contains("return Err"),
        "rollback-buffer allocation failure must restore canonical renderer state before the command fails"
    );
}

#[test]
fn non_finite_accumulation_is_rejected_before_mutating_the_drag_session() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let update = source_between(
        renderer,
        "pub(crate) fn update_atom_drag(",
        "\n    pub(crate) fn take_atom_drag",
    );
    let candidate = position(update, "candidate_translation");
    let finite_check = position(update, "candidate_translation.is_finite()");
    let assignment = position(update, "session.translation = candidate_translation");

    assert_absent(
        update,
        &["session.translation +="],
        "atom drag accumulation",
    );
    assert!(
        candidate < finite_check && finite_check < assignment,
        "an overflowed cumulative translation must fail without changing session state"
    );
}

#[test]
fn preview_and_cancel_use_preallocated_renderer_data_with_batched_uploads() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let upload = source_between(renderer, "fn upload_atom_instances(", "\nimpl Renderer");
    let begin = source_between(
        renderer,
        "pub(crate) fn begin_atom_drag(",
        "\n    pub(crate) fn update_atom_drag",
    );
    let update = source_between(
        renderer,
        "pub(crate) fn update_atom_drag(",
        "\n    pub(crate) fn take_atom_drag",
    );
    let cancel = source_between(
        renderer,
        "pub(crate) fn cancel_atom_drag(",
        "\n    fn screen_drag_translation",
    );

    assert!(
        upload.contains("queue.write_buffer") && !upload.contains("for "),
        "the upload primitive must submit one contiguous renderer-owned slice"
    );
    assert_absent(
        update,
        &["queue.write_buffer", "Vec::new", "try_reserve", "push("],
        "preview update hot path",
    );
    assert_absent(
        cancel,
        &[
            "queue.write_buffer",
            "upload_atom_instances(",
            "instance_buffer",
            "transparent_instance_buffer",
            "Vec::new",
            "try_reserve",
            "push(",
        ],
        "preview cancellation hot path",
    );
    assert_eq!(
        update.matches("upload_atom_instances(").count(),
        2,
        "each preview frame may upload opaque and transparent renderer buffers once each"
    );
    assert_eq!(
        cancel.matches("upload_atom_instances(").count(),
        0,
        "cancel must drop session-owned preview buffers without touching canonical GPU buffers"
    );
    assert!(
        cancel.contains("self.atom_drag = None"),
        "cancel must release the active renderer-owned preview session"
    );
    assert_absent(
        begin,
        &[
            "queue.write_buffer",
            "self.instance_buffer",
            "self.transparent_instance_buffer",
            "set_drag_instance_radius",
        ],
        "drag begin canonical atom buffers",
    );
}

#[test]
fn online_preview_uses_session_buffers_while_offscreen_keeps_canonical_ownership() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let online = source_between(
        renderer,
        "// ═══ Normal crystal rendering path ═══════════════════════════════",
        "\n    /// Render the current scene to an off-screen texture",
    );
    let offscreen = source_between(
        renderer,
        "pub fn render_offscreen(",
        "\n    pub fn prepare_volumetric",
    );

    for token in [
        "opaque_stationary_buffer",
        "opaque_preview_buffer",
        "transparent_stationary_buffer",
        "transparent_preview_buffer",
    ] {
        assert!(
            online.contains(token),
            "online drag rendering must draw the session-owned `{token}`"
        );
    }
    assert!(
        online.contains("} else if self.instance_count > 0")
            && online.contains("} else if self.transparent_instance_count > 0"),
        "online rendering must select canonical atom buffers only when no preview session exists"
    );
    assert!(
        offscreen.contains("self.instance_buffer")
            && offscreen.contains("self.transparent_instance_buffer"),
        "offscreen rendering must retain the complete canonical atom buffers"
    );
    assert_absent(
        offscreen,
        &[
            "atom_drag",
            "stationary_buffer",
            "preview_buffer",
            "opaque_preview_instances",
            "transparent_preview_instances",
        ],
        "offscreen canonical export path",
    );
}
