use crystal_canvas::{
    commands::editing,
    crystal_state::CrystalState,
    renderer::{instance, renderer::Renderer},
    settings::AppSettings,
    undo::UndoStack,
    wannier,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tauri::{Listener, Manager};

struct Probe {
    state: serde_json::Value,
    version: u32,
    can_undo: bool,
    can_redo: bool,
    pick_scene: Arc<Vec<crystal_canvas::renderer::ray_picking::PickAtom>>,
    state_events: usize,
    undo_events: usize,
}

fn valid_state() -> CrystalState {
    let mut state = CrystalState::default();
    state.name = "INTERACT-1A fixture".into();
    state.cell_a = 4.0;
    state.cell_b = 4.0;
    state.cell_c = 4.0;
    state.labels = vec!["C1".into()];
    state.elements = vec!["C".into()];
    state.fract_x = vec![0.25];
    state.fract_y = vec![0.25];
    state.fract_z = vec![0.25];
    state.occupancies = vec![1.0];
    state.atomic_numbers = vec![6];
    state.intrinsic_sites = 1;
    state.version = 73;
    state.fractional_to_cartesian();
    state.validate_structural_invariants().unwrap();
    state
}

fn build_fixture() -> tauri::App {
    let state = valid_state();
    let settings = AppSettings::default();
    let app = tauri::Builder::default()
        .build(tauri::generate_context!())
        .expect("INTERACT-1A fixture must build a Tauri application");
    let window = tauri::WebviewWindowBuilder::new(
        &app,
        "interact-1a-drag-session",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .inner_size(64.0, 64.0)
    .visible(false)
    .build()
    .expect("INTERACT-1A fixture requires a real hidden window");
    let mut renderer = Renderer::new(Arc::new(window), 64, 64);
    let atom_scene = wannier::build_atoms_with_ghosts(&state, &settings)
        .and_then(instance::prepare_atom_scene)
        .expect("INTERACT-1A fixture must prepare atom scene");
    let line_scene = instance::build_line_scene(&state, &settings)
        .expect("INTERACT-1A fixture must prepare line scene");
    renderer.commit_atoms(atom_scene);
    renderer.update_lines(&line_scene);

    assert!(app.manage(Mutex::new(state)));
    assert!(app.manage(Mutex::new(settings)));
    assert!(app.manage(Mutex::new(UndoStack::new(32))));
    assert!(app.manage(Mutex::new(renderer)));
    app
}

fn probe(app: &tauri::App, state_events: &AtomicUsize, undo_events: &AtomicUsize) -> Probe {
    let crystal_state = app.state::<Mutex<CrystalState>>();
    let crystal_state = crystal_state.lock().unwrap();
    let state = serde_json::to_value(&*crystal_state).unwrap();
    let version = crystal_state.version;
    drop(crystal_state);

    let undo_state = app.state::<Mutex<UndoStack>>();
    let undo_state = undo_state.lock().unwrap();
    let can_undo = undo_state.can_undo();
    let can_redo = undo_state.can_redo();
    drop(undo_state);

    let renderer = app.state::<Mutex<Renderer>>();
    let pick_scene = renderer.lock().unwrap().pick_scene_snapshot();

    Probe {
        state,
        version,
        can_undo,
        can_redo,
        pick_scene,
        state_events: state_events.load(Ordering::SeqCst),
        undo_events: undo_events.load(Ordering::SeqCst),
    }
}

fn assert_unchanged(
    app: &tauri::App,
    before: &Probe,
    state_events: &AtomicUsize,
    undo_events: &AtomicUsize,
) {
    let after = probe(app, state_events, undo_events);
    assert_eq!(after.state, before.state);
    assert_eq!(after.version, before.version);
    assert_eq!(after.can_undo, before.can_undo);
    assert_eq!(after.can_redo, before.can_redo);
    assert!(Arc::ptr_eq(&after.pick_scene, &before.pick_scene));
    assert_eq!(after.state_events, before.state_events);
    assert_eq!(after.undo_events, before.undo_events);
}

fn main() {
    if std::env::var_os("CRYSTAL_CANVAS_RUN_INTERACT_1A_SMOKE").is_none() {
        eprintln!(
            "SKIPPED_NO_DESKTOP_SESSION: set CRYSTAL_CANVAS_RUN_INTERACT_1A_SMOKE=1 on a macOS desktop session"
        );
        return;
    }
    run_drag_session_smoke();
}

fn run_drag_session_smoke() {
    let app = build_fixture();
    let state_events = Arc::new(AtomicUsize::new(0));
    let undo_events = Arc::new(AtomicUsize::new(0));
    let state_counter = state_events.clone();
    let undo_counter = undo_events.clone();
    let state_listener = app.listen("state_changed", move |_| {
        state_counter.fetch_add(1, Ordering::SeqCst);
    });
    let undo_listener = app.listen("undo_stack_changed", move |_| {
        undo_counter.fetch_add(1, Ordering::SeqCst);
    });

    let preview_before = probe(&app, &state_events, &undo_events);
    let session = editing::begin_atom_drag(vec![0], app.state(), app.state())
        .expect("begin must accept one intrinsic atom");
    for _ in 0..1_000 {
        editing::update_atom_drag(session.clone(), 0.001, -0.001, app.state())
            .expect("finite preview update must succeed");
    }
    assert_unchanged(&app, &preview_before, &state_events, &undo_events);
    editing::cancel_atom_drag(session, app.state()).expect("cancel must restore the preview");
    assert_unchanged(&app, &preview_before, &state_events, &undo_events);

    let invalid_before = probe(&app, &state_events, &undo_events);
    let invalid_session = editing::begin_atom_drag(vec![0], app.state(), app.state()).unwrap();
    assert!(
        editing::update_atom_drag(invalid_session.clone(), f32::NAN, 0.0, app.state()).is_err()
    );
    editing::cancel_atom_drag(invalid_session, app.state()).unwrap();
    assert_unchanged(&app, &invalid_before, &state_events, &undo_events);

    let no_op_before = probe(&app, &state_events, &undo_events);
    let no_op_session = editing::begin_atom_drag(vec![0], app.state(), app.state()).unwrap();
    editing::commit_atom_drag(
        no_op_session,
        app.handle().clone(),
        app.state(),
        app.state(),
        app.state(),
        app.state(),
    )
    .expect("zero displacement commit must be accepted as a no-op");
    assert_unchanged(&app, &no_op_before, &state_events, &undo_events);

    let commit_before = probe(&app, &state_events, &undo_events);
    let commit_session = editing::begin_atom_drag(vec![0], app.state(), app.state()).unwrap();
    editing::update_atom_drag(commit_session.clone(), 0.1, 0.0, app.state()).unwrap();
    editing::commit_atom_drag(
        commit_session,
        app.handle().clone(),
        app.state(),
        app.state(),
        app.state(),
        app.state(),
    )
    .expect("finite drag commit must succeed");
    let committed = probe(&app, &state_events, &undo_events);
    assert_eq!(committed.version, commit_before.version + 1);
    assert!(committed.can_undo);
    assert_eq!(committed.state_events, commit_before.state_events + 1);
    assert_eq!(committed.undo_events, commit_before.undo_events + 1);

    let stale_session = editing::begin_atom_drag(vec![0], app.state(), app.state()).unwrap();
    {
        let crystal_state = app.state::<Mutex<CrystalState>>();
        crystal_state.lock().unwrap().version += 1;
    }
    let stale_before = probe(&app, &state_events, &undo_events);
    assert!(
        editing::commit_atom_drag(
            stale_session,
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
            app.state(),
        )
        .is_err()
    );
    let stale_after = probe(&app, &state_events, &undo_events);
    assert_eq!(stale_after.state, stale_before.state);
    assert_eq!(stale_after.version, stale_before.version);
    assert_eq!(stale_after.can_undo, stale_before.can_undo);
    assert_eq!(stale_after.can_redo, stale_before.can_redo);
    assert_eq!(stale_after.state_events, stale_before.state_events);
    assert_eq!(stale_after.undo_events, stale_before.undo_events);

    app.unlisten(state_listener);
    app.unlisten(undo_listener);
}
