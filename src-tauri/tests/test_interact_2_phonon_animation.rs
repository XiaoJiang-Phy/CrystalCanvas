//! INTERACT-2A backend contract gate: phonon animation is renderer-owned presentation state.
//!
//! The numerical fixture is synthetic and makes no physical normalization or
//! time-evolution claim. Numerical assertions call production Rust seams; the
//! test does not carry a duplicate implementation of the display relation.

use crystal_canvas::renderer::instance::{
    apply_phonon_frame, validate_phonon_display_envelope, AtomInstance,
};
use crystal_canvas::renderer::renderer::PhononPlayback;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

const TOLERANCE: f64 = 1.0e-6;

fn command_body<'a>(source: &'a str, command: &str) -> &'a str {
    let signature = format!("pub fn {command}(");
    let start = source
        .find(&signature)
        .unwrap_or_else(|| panic!("missing INTERACT-2A command `{command}`"));
    let remainder = &source[start..];
    let end = remainder
        .find("\n#[tauri::command]")
        .unwrap_or(remainder.len());
    &remainder[..end]
}

fn braced_item(source: &str, start: usize) -> Option<&str> {
    let opening = start + source[start..].find('{')?;
    let mut depth = 0_usize;
    for (offset, byte) in source.as_bytes()[opening..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(&source[start..=opening + offset]);
                }
            }
            _ => {}
        }
    }
    None
}

fn function_body<'a>(source: &'a str, function: &str) -> &'a str {
    let signature = format!("fn {function}(");
    let start = source
        .find(&signature)
        .unwrap_or_else(|| panic!("missing production function `{function}`"));
    braced_item(source, start).unwrap_or_else(|| panic!("unterminated function `{function}`"))
}

fn named_item<'a>(source: &'a str, declaration: &str) -> &'a str {
    let start = source
        .find(declaration)
        .unwrap_or_else(|| panic!("missing production item `{declaration}`"));
    braced_item(source, start).unwrap_or_else(|| panic!("unterminated item `{declaration}`"))
}

fn assert_absent(source: &str, forbidden: &[&str], boundary: &str) {
    for token in forbidden {
        assert!(!source.contains(token), "{boundary} must not own `{token}`");
    }
}

fn assert_in_order(source: &str, required: &[&str], boundary: &str) {
    let mut cursor = 0;
    for token in required {
        let position = source[cursor..]
            .find(token)
            .unwrap_or_else(|| panic!("{boundary} must contain `{token}` in order"));
        cursor += position + token.len();
    }
}

fn atom(position: [f32; 3], radius: f32, color: [f32; 4]) -> AtomInstance {
    AtomInstance {
        position,
        radius,
        color,
    }
}

fn assert_position(actual: [f32; 3], expected: [f64; 3]) {
    for axis in 0..3 {
        assert!(
            (f64::from(actual[axis]) - expected[axis]).abs() <= TOLERANCE,
            "axis {axis}: expected {}, got {}",
            expected[axis],
            actual[axis]
        );
    }
}

fn assert_atoms_bitwise_equal(actual: &[AtomInstance], expected: &[AtomInstance]) {
    assert_eq!(
        bytemuck::cast_slice::<AtomInstance, u8>(actual),
        bytemuck::cast_slice::<AtomInstance, u8>(expected),
        "atom instances must remain bitwise identical"
    );
}

#[test]
fn synthetic_fixture_declares_units_conventions_and_no_physical_claim() {
    let fixture = include_str!("fixtures/interact_2a_phonon_display.yaml");
    let manifest = include_str!("fixtures/interact_2a_phonon_display.manifest.json");

    for declaration in [
        "base_position_unit: angstrom",
        "display_scale_unit: angstrom",
        "eigenvector_unit: dimensionless",
        "phase_unit: radian",
        "physical_claim: none",
    ] {
        assert!(
            fixture.contains(declaration),
            "fixture must declare `{declaration}`"
        );
    }
    for declaration in [
        "phonon_yaml_v1",
        "synthetic presentation-only regression; no physical validation claim",
        "physical_time_interpretation",
        "periodic_source_mapping",
    ] {
        assert!(
            manifest.contains(declaration),
            "manifest must declare `{declaration}`"
        );
    }
}

#[test]
fn production_kernel_matches_all_four_cardinal_phases() {
    let base = [atom([1.0, -2.0, 0.5], 0.75, [0.1, 0.2, 0.3, 1.0])];
    let source_atom_indices = [0_usize];
    let displacements = [[0.25_f32, -0.5, 1.0]];
    let mut output = [atom([99.0; 3], 99.0, [9.0; 4])];

    for (phase, expected) in [
        (0.0, [1.0, -2.0, 0.5]),
        (FRAC_PI_2, [1.1, -2.2, 0.9]),
        (PI, [1.0, -2.0, 0.5]),
        (3.0 * FRAC_PI_2, [0.9, -1.8, 0.1]),
    ] {
        apply_phonon_frame(
            &base,
            &source_atom_indices,
            &displacements,
            phase,
            0.4,
            &mut output,
        )
        .expect("declared cardinal phase must be renderable");
        assert_position(output[0].position, expected);
        assert_eq!(output[0].radius, base[0].radius);
        assert_eq!(output[0].color, base[0].color);
    }
}

#[test]
fn production_kernel_maps_periodic_images_to_one_intrinsic_displacement() {
    let base = [
        atom([1.0, -2.0, 0.5], 0.5, [1.0; 4]),
        atom([4.0, -2.0, 0.5], 0.5, [1.0; 4]),
    ];
    let source_atom_indices = [0_usize, 0_usize];
    let displacements = [[0.25_f32, -0.5, 1.0]];
    let mut output = base;

    apply_phonon_frame(
        &base,
        &source_atom_indices,
        &displacements,
        FRAC_PI_2,
        0.4,
        &mut output,
    )
    .expect("periodic images must share their intrinsic displacement");

    let intrinsic_delta = [
        output[0].position[0] - base[0].position[0],
        output[0].position[1] - base[0].position[1],
        output[0].position[2] - base[0].position[2],
    ];
    let image_delta = [
        output[1].position[0] - base[1].position[0],
        output[1].position[1] - base[1].position[1],
        output[1].position[2] - base[1].position[2],
    ];
    assert_position(intrinsic_delta, [0.1, -0.2, 0.4]);
    assert_position(image_delta, [0.1, -0.2, 0.4]);
    assert_position(
        [
            output[1].position[0] - output[0].position[0],
            output[1].position[1] - output[0].position[1],
            output[1].position[2] - output[0].position[2],
        ],
        [3.0, 0.0, 0.0],
    );
}

#[test]
fn repeated_cycles_preserve_base_bits_and_reuse_output_storage() {
    let base = vec![
        atom([1.0, 2.0, 3.0], 0.4, [0.1, 0.2, 0.3, 1.0]),
        atom([-1.0, -2.0, -3.0], 0.6, [0.9, 0.8, 0.7, 0.5]),
    ];
    let base_before = base.clone();
    let source_atom_indices = [0_usize, 1_usize];
    let displacements = [[0.25_f32, 0.5, 0.75], [-0.5, 0.25, -0.125]];
    let mut output = base.clone();
    let output_ptr = output.as_ptr();
    let output_capacity = output.capacity();

    for cycle in 0..4096 {
        let phase = f64::from(cycle) * 0.03125;
        let display_scale = if cycle % 2 == 0 { 0.0 } else { 5.0 };
        apply_phonon_frame(
            &base,
            &source_atom_indices,
            &displacements,
            phase,
            display_scale,
            &mut output,
        )
        .expect("repeated presentation cycles must remain valid");
        assert_eq!(output.as_ptr(), output_ptr);
        assert_eq!(output.capacity(), output_capacity);
    }

    assert_atoms_bitwise_equal(&base, &base_before);
    apply_phonon_frame(
        &base,
        &source_atom_indices,
        &displacements,
        0.0,
        0.0,
        &mut output,
    )
    .expect("reset must restore the bitwise base instances");
    assert_atoms_bitwise_equal(&output, &base);
}

#[test]
fn empty_unset_mode_is_a_safe_noop() {
    let mut output: [AtomInstance; 0] = [];
    apply_phonon_frame(&[], &[], &[], 0.0, 1.0, &mut output)
        .expect("an unset empty mode must not create invalid GPU data");
}

#[test]
fn invalid_shapes_indices_and_numbers_fail_before_output_mutation() {
    let base = [
        atom([1.0, 2.0, 3.0], 0.4, [0.1, 0.2, 0.3, 1.0]),
        atom([4.0, 5.0, 6.0], 0.5, [0.4, 0.5, 0.6, 1.0]),
    ];
    let sentinel = [
        atom([91.0, 92.0, 93.0], 9.0, [9.0; 4]),
        atom([94.0, 95.0, 96.0], 8.0, [8.0; 4]),
    ];
    let valid_sources = [0_usize, 1_usize];
    let valid_displacements = [[0.1_f32, 0.2, 0.3], [0.4, 0.5, 0.6]];

    let cases: &[(&[usize], &[[f32; 3]], f64, f64, usize)] = &[
        (&[0], &valid_displacements, 0.0, 1.0, 2),
        (&valid_sources, &[[0.1, 0.2, 0.3]], 0.0, 1.0, 2),
        (&[0, usize::MAX], &valid_displacements, 0.0, 1.0, 2),
        (&valid_sources, &valid_displacements, f64::NAN, 1.0, 2),
        (&valid_sources, &valid_displacements, f64::INFINITY, 1.0, 2),
        (&valid_sources, &valid_displacements, 0.0, f64::NAN, 2),
        (
            &valid_sources,
            &[[f32::NAN, 0.0, 0.0], [0.0; 3]],
            0.0,
            1.0,
            2,
        ),
        (&valid_sources, &valid_displacements, FRAC_PI_2, f64::MAX, 2),
        (&valid_sources, &valid_displacements, 0.0, 1.0, 1),
    ];

    for &(sources, displacements, phase, scale, output_len) in cases {
        let mut output = sentinel;
        let result = apply_phonon_frame(
            &base,
            sources,
            displacements,
            phase,
            scale,
            &mut output[..output_len],
        );
        assert!(result.is_err(), "invalid frame input must be rejected");
        assert_atoms_bitwise_equal(&output, &sentinel);
    }
}

#[test]
fn huge_finite_phase_is_reduced_before_renderer_precision_conversion() {
    let base = [atom([0.0; 3], 1.0, [1.0; 4])];
    let sources = [0_usize];
    let displacements = [[1.0_f32, -1.0, 0.5]];
    let mut output = base;

    apply_phonon_frame(&base, &sources, &displacements, f64::MAX, 1.0, &mut output)
        .expect("a huge finite phase must be reduced in f64 before conversion");
    assert!(
        output
            .iter()
            .flat_map(|instance| instance.position)
            .all(f32::is_finite),
        "a reduced finite phase must produce only finite GPU coordinates"
    );
}

#[test]
fn display_envelope_rejects_a_scale_before_a_later_playback_phase_can_overflow() {
    let base = [atom([1.0, -2.0, 0.5], 0.75, [0.1, 0.2, 0.3, 1.0])];
    let source_atom_indices = [0_usize];
    let displacements = [[1.0_f32, -0.5, 0.25]];
    let mut output = base;

    // A phase of zero is harmless even at this scale. The gate must reject the
    // request before normal playback reaches pi/2 and attempts an invalid GPU
    // coordinate.
    apply_phonon_frame(
        &base,
        &source_atom_indices,
        &displacements,
        0.0,
        f64::MAX,
        &mut output,
    )
    .expect("the current zero-phase frame is still representable");
    assert_atoms_bitwise_equal(&output, &base);

    assert!(
        validate_phonon_display_envelope(&base, &source_atom_indices, &displacements, f64::MAX,)
            .is_err(),
        "a finite amplitude that would overflow at a later phase must be refused before playback"
    );

    let before_rejected_frame = output;
    assert!(
        apply_phonon_frame(
            &base,
            &source_atom_indices,
            &displacements,
            FRAC_PI_2,
            f64::MAX,
            &mut output,
        )
        .is_err(),
        "the dynamic frame kernel confirms the later phase would be invalid"
    );
    assert_atoms_bitwise_equal(&output, &before_rejected_frame);
}

#[test]
fn display_envelope_permits_extreme_finite_scale_when_the_mode_is_stationary() {
    let base = [atom([1.0, -2.0, 0.5], 0.75, [0.1, 0.2, 0.3, 1.0])];
    let source_atom_indices = [0_usize];
    let stationary_displacements = [[0.0_f32; 3]];
    let mut output = base;

    validate_phonon_display_envelope(
        &base,
        &source_atom_indices,
        &stationary_displacements,
        f64::MAX,
    )
    .expect("a finite scale must remain valid when every possible frame is the base scene");
    apply_phonon_frame(
        &base,
        &source_atom_indices,
        &stationary_displacements,
        FRAC_PI_2,
        f64::MAX,
        &mut output,
    )
    .expect("a stationary mode must remain renderable at every phase");
    assert_atoms_bitwise_equal(&output, &base);
}

#[test]
fn playback_clock_starts_stops_and_resumes_without_catching_up_missed_time() {
    let mut playback = PhononPlayback::new(TAU).expect("finite display rate must be valid");
    assert!(!playback.is_playing());
    assert!((playback.phase_at(10.0).unwrap() - 0.0).abs() <= TOLERANCE);

    playback.start(10.0).unwrap();
    assert!(playback.is_playing());
    assert!((playback.phase_at(10.25).unwrap() - FRAC_PI_2).abs() <= TOLERANCE);
    playback.stop(10.5).unwrap();
    assert!(!playback.is_playing());
    assert!((playback.phase_at(100.0).unwrap() - PI).abs() <= TOLERANCE);

    playback.start(100.0).unwrap();
    assert!((playback.phase_at(100.25).unwrap() - 3.0 * FRAC_PI_2).abs() <= TOLERANCE);
    playback.stop(100.5).unwrap();
    assert!((playback.phase_at(1_000_000.0).unwrap() - 0.0).abs() <= TOLERANCE);
}

#[test]
fn playback_clock_rejects_nonfinite_or_backward_time_without_state_corruption() {
    assert!(PhononPlayback::new(f64::NAN).is_err());
    assert!(PhononPlayback::new(f64::INFINITY).is_err());

    let mut playback = PhononPlayback::new(TAU).unwrap();
    assert!(playback.start(f64::NAN).is_err());
    assert!(!playback.is_playing());
    playback.start(10.0).unwrap();
    assert!(playback.phase_at(9.0).is_err());
    assert!(playback.stop(9.0).is_err());
    assert!(playback.is_playing());
    assert!((playback.phase_at(10.25).unwrap() - FRAC_PI_2).abs() <= TOLERANCE);
    assert!(playback.seek(f64::INFINITY, 10.25).is_err());
    assert!((playback.phase_at(10.25).unwrap() - FRAC_PI_2).abs() <= TOLERANCE);
    assert!(playback.phase_at(1.0e100).unwrap().is_finite());
}

#[test]
fn production_kernel_and_frame_hot_path_are_allocation_free_by_construction() {
    let instances = include_str!("../src/renderer/instance.rs");
    let renderer = include_str!("../src/renderer/renderer.rs");
    let kernel = function_body(instances, "apply_phonon_frame");
    let frame = function_body(renderer, "render");

    for (source, boundary) in [
        (kernel, "production phonon kernel"),
        (frame, "renderer frame path"),
    ] {
        assert_absent(
            source,
            &[
                "Vec::new",
                "Vec::with_capacity",
                ".to_vec()",
                ".clone()",
                ".collect(",
                "Box::new",
                "reserve(",
                "reserve_exact(",
                "create_buffer_init",
                "prepare_atom_scene",
                "build_atoms_with_ghosts",
                "commit_atoms",
            ],
            boundary,
        );
    }
}

#[test]
fn renderer_render_loop_consumes_clock_and_production_kernel_not_a_name_match() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let render = function_body(renderer, "render");

    assert!(
        render.contains("phase_at("),
        "Renderer::render must derive the current phase from renderer-owned playback time"
    );
    assert!(
        render.contains("apply_phonon_frame("),
        "Renderer::render must call the dynamically tested production kernel"
    );
    assert_absent(
        render,
        &[
            "CrystalState",
            "UndoStack",
            "state_changed",
            "commit_version",
            "VecDeque",
            "build_atoms_with_ghosts",
            "prepare_atom_scene",
            "commit_atoms",
        ],
        "Renderer::render phonon tick",
    );
}

#[test]
fn presentation_setup_uses_fallible_allocation_instead_of_to_vec_clones() {
    let renderer = include_str!("../src/renderer/renderer.rs");
    let presentation = named_item(renderer, "impl PhononPresentation");

    assert_absent(
        presentation,
        &[".to_vec()", ".clone()"],
        "PhononPresentation setup",
    );
    assert!(
        presentation.contains("try_reserve"),
        "presentation allocation must be fallible and reported through IpcResult"
    );
}

#[test]
fn phase_ipc_is_a_renderer_control_boundary_not_a_canonical_update() {
    let analysis = include_str!("../src/commands/analysis.rs");
    let phase = command_body(analysis, "set_phonon_phase");

    for token in ["phase", "amplitude", "renderer_state"] {
        assert!(
            phase.contains(token),
            "set_phonon_phase must retain `{token}`"
        );
    }
    assert_absent(
        phase,
        &[
            "crystal_state",
            "settings_state",
            "cart_positions",
            "eigenvectors",
            "Vec::new",
            "clone()",
            "build_atoms_with_ghosts",
            "prepare_atom_scene",
            "commit_atoms",
            "phonon_phase =",
            "next_version",
            "commit_version",
            "UndoStack",
            "state_changed",
            "undo_stack_changed",
            ".emit(",
        ],
        "set_phonon_phase hot path",
    );
}

#[test]
fn display_scale_ipc_is_a_renderer_only_phase_preserving_boundary() {
    let analysis = include_str!("../src/commands/analysis.rs");
    let renderer = include_str!("../src/renderer/renderer.rs");
    let scale_command = command_body(analysis, "set_phonon_display_scale");
    let scale_method = function_body(renderer, "set_phonon_display_scale");

    assert_in_order(
        scale_command,
        &[
            "renderer_state",
            ".try_lock()",
            "renderer.set_phonon_display_scale(display_scale)",
        ],
        "display-scale IPC must delegate through the renderer lock",
    );
    assert_absent(
        scale_command,
        &[
            "crystal_state",
            "settings_state",
            "phonon_frame_wake",
            "cart_positions",
            "commit_version",
            ".emit(",
        ],
        "display-scale IPC must not acquire canonical-state, wake, or event ownership",
    );

    assert_in_order(
        scale_method,
        &[
            "if !display_scale.is_finite()",
            "validate_phonon_display_envelope(",
            "presentation.display_scale = display_scale",
            "presentation.dirty = true",
        ],
        "display scale must validate before changing renderer presentation state",
    );
    assert_absent(
        scale_method,
        &[
            "presentation.playback.seek(",
            "presentation.playback.start(",
            "presentation.playback.stop(",
            "self.phonon_presentation =",
            "CrystalState",
            "UndoStack",
            "cart_positions",
            "frac_positions",
            "commit_atoms",
            "build_atoms_with_ghosts",
            "prepare_atom_scene",
            "next_version",
            "commit_version",
            "state_changed",
            "undo_stack_changed",
            ".emit(",
        ],
        "display scale must not take playback, canonical-state, snapshot, or event ownership",
    );
}

#[test]
fn mode_stop_and_clear_paths_cannot_mutate_canonical_coordinates() {
    let analysis = include_str!("../src/commands/analysis.rs");
    let mode = command_body(analysis, "set_phonon_mode");
    let phase = command_body(analysis, "set_phonon_phase");
    let boundaries = [mode, phase].concat();

    assert_absent(
        &boundaries,
        &[
            "cart_positions =",
            "cart_positions[",
            "frac_positions =",
            "translate_atoms_cartesian",
            "commit_version",
            "next_version",
            "UndoStack",
            "state_changed",
            "undo_stack_changed",
        ],
        "phonon start/stop/phase commands",
    );
}

#[test]
fn frame_wake_has_one_generation_owner_and_explicit_stop_paths() {
    let commands = include_str!("../src/commands/mod.rs");
    let analysis = include_str!("../src/commands/analysis.rs");
    let renderer = include_str!("../src/renderer/renderer.rs");
    let main = include_str!("../src/main.rs");
    let wake = named_item(commands, "impl PhononFrameWake");
    let start = function_body(wake, "start");
    let stop = function_body(wake, "stop");
    let playing = command_body(analysis, "set_phonon_playing");
    let mode = command_body(analysis, "set_phonon_mode");
    let render = function_body(renderer, "render");

    assert!(
        commands.contains("generation: std::sync::Arc<std::sync::atomic::AtomicU64>"),
        "the wake owner must expose one shared generation, not independent timer state"
    );
    assert_in_order(
        start,
        &[
            "fetch_add",
            "wrapping_add(1)",
            ".spawn(move ||",
            "while wake_generation.load",
            "std::thread::sleep(Self::INTERVAL)",
            "wake_generation.load",
            "!= generation",
            "app.run_on_main_thread(|| {})",
        ],
        "phonon frame wake start",
    );
    assert!(
        start.contains(".name(\"phonon-frame-wake\".into())"),
        "the wake thread must remain identifiable in diagnostics"
    );
    assert!(
        stop.contains("fetch_add"),
        "stopping playback must invalidate the active generation"
    );
    assert_absent(
        wake,
        &[
            "requestAnimationFrame",
            "setInterval",
            "CrystalState",
            "UndoStack",
            "renderer.render(",
        ],
        "renderer-owned phonon wake",
    );

    assert_in_order(
        playing,
        &[
            "renderer.set_phonon_playing(playing)?",
            "if playing",
            "phonon_frame_wake.start(app)",
        ],
        "set_phonon_playing start path",
    );
    assert_in_order(
        playing,
        &[
            "phonon_frame_wake.start(app)",
            "renderer.set_phonon_playing(false)",
            "return Err(error)",
        ],
        "set_phonon_playing failed-start rollback",
    );
    assert!(
        playing.contains("} else {\n        phonon_frame_wake.stop();"),
        "explicit playback stop must stop the wake owner"
    );
    assert!(
        mode.contains("phonon_frame_wake.stop();"),
        "mode replacement or clearing must stop the prior playback wake"
    );
    assert!(
        render.contains("presentation.playback.halt();"),
        "a rejected presentation frame must halt playback before the next wake"
    );
    assert_in_order(
        main,
        &[
            "if !renderer.phonon_is_playing()",
            "phonon_frame_wake.stop();",
            "tauri::RunEvent::Exit",
            "phonon_frame_wake.stop();",
        ],
        "main-loop wake shutdown",
    );
}

#[test]
fn repeated_play_requests_keep_the_existing_wake_owner_running() {
    let analysis = include_str!("../src/commands/analysis.rs");
    let renderer = include_str!("../src/renderer/renderer.rs");
    let playing = command_body(analysis, "set_phonon_playing");
    let playback = function_body(renderer, "start");

    // The renderer clock already treats a repeated start as idempotent. The
    // command layer must preserve that property instead of creating one wake
    // thread per duplicate native request.
    assert_in_order(
        playback,
        &["if self.playing", "self.phase_at(now)?", "return Ok(())"],
        "renderer playback repeated-start path",
    );
    assert_in_order(
        playing,
        &[
            "renderer.phonon_is_playing()",
            "renderer.set_phonon_playing(playing)?",
            "if playing {",
        ],
        "phonon playback control ordering",
    );

    let play_branch_start = playing
        .find("if playing {")
        .expect("missing outer `playing` branch");
    let play_branch = braced_item(playing, play_branch_start)
        .expect("outer `playing` branch must have a balanced body");

    // A duplicate `true` request must neither start another waker nor stop the
    // current owner. Only the false-to-true transition can cross start().
    assert_in_order(
        play_branch,
        &["if !", "phonon_frame_wake.start(app)"],
        "phonon wake start transition",
    );
    assert_absent(
        play_branch,
        &["phonon_frame_wake.stop()"],
        "duplicate playing request must retain the wake owner",
    );

    let stop_index = playing
        .find("phonon_frame_wake.stop()")
        .expect("false playback request must stop the wake owner");
    assert!(
        stop_index > play_branch_start + play_branch.len(),
        "wake stop must be exclusive to the outer false branch"
    );
    assert!(
        playing[play_branch_start + play_branch.len()..stop_index].contains("else"),
        "wake stop must be owned by the outer false branch"
    );
    assert_eq!(
        playing.matches("phonon_frame_wake.start(app)").count(),
        1,
        "the transition guard must own the only wake spawn boundary"
    );
}
