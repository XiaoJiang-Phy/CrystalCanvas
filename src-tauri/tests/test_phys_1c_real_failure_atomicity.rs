use crystal_canvas::{
    crystal_state::CrystalState,
    renderer::renderer::Renderer,
    settings::AppSettings,
    transaction,
    undo::{StructuralSnapshot, UndoStack},
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tauri::{Listener, Manager};

struct AtomicityProbe {
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
    state.name = "PHYS-1C fixture".into();
    state.cell_a = 4.0;
    state.cell_b = 4.0;
    state.cell_c = 4.0;
    state.labels = vec!["C1".into()];
    state.elements = vec!["C".into()];
    state.fract_x = vec![0.0];
    state.fract_y = vec![0.0];
    state.fract_z = vec![0.0];
    state.occupancies = vec![1.0];
    state.atomic_numbers = vec![6];
    state.intrinsic_sites = 1;
    state.version = 41;
    state.fractional_to_cartesian();
    state.validate_structural_invariants().unwrap();
    state
}

fn seeded_undo_stack(state: &CrystalState) -> UndoStack {
    let mut first = StructuralSnapshot::from_crystal_state(state);
    first.version = state.version - 2;
    let mut second = StructuralSnapshot::from_crystal_state(state);
    second.version = state.version - 1;
    let current = StructuralSnapshot::from_crystal_state(state);
    let mut undo = UndoStack::new(8);
    undo.push(first);
    undo.push(second);
    assert!(undo.undo(current).is_some());
    assert!(undo.can_undo());
    assert!(undo.can_redo());
    undo
}

fn build_real_fixture() -> tauri::App {
    let app = tauri::Builder::default()
        .build(tauri::generate_context!())
        .expect("real Tauri application must build for the PHYS-1C gate");
    let window = tauri::WebviewWindowBuilder::new(
        &app,
        "phys-1c-atomicity",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .inner_size(64.0, 64.0)
    .visible(false)
    .build()
    .expect("PHYS-1C fixture requires a real hidden window");

    assert!(app.manage(Mutex::new(valid_state())));
    assert!(app.manage(Mutex::new(AppSettings::default())));
    assert!(app.manage(Mutex::new(seeded_undo_stack(&valid_state()))));
    assert!(app.manage(Mutex::new(Renderer::new(Arc::new(window), 64, 64))));
    app
}

fn probe(
    app: &tauri::App,
    state_events: &AtomicUsize,
    undo_events: &AtomicUsize,
) -> AtomicityProbe {
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

    let renderer_state = app.state::<Mutex<Renderer>>();
    let pick_scene = renderer_state.lock().unwrap().pick_scene_snapshot();

    AtomicityProbe {
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
    before: &AtomicityProbe,
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

fn history_depth_after_failure(app: &tauri::App) -> (usize, usize) {
    let crystal_state = app.state::<Mutex<CrystalState>>();
    let current = StructuralSnapshot::from_crystal_state(&crystal_state.lock().unwrap());
    drop(crystal_state);

    let undo_state = app.state::<Mutex<UndoStack>>();
    let mut undo_state = undo_state.lock().unwrap();
    let mut future_depth = 0;
    while undo_state.redo(current.clone()).is_some() {
        future_depth += 1;
    }
    let mut total_depth = 0;
    while undo_state.undo(current.clone()).is_some() {
        total_depth += 1;
    }
    (total_depth - future_depth, future_depth)
}

fn main() {
    if std::env::var_os("CRYSTAL_CANVAS_RUN_REAL_GPU_TEST").is_none() {
        eprintln!(
            "SKIPPED_NO_DESKTOP_SESSION: set CRYSTAL_CANVAS_RUN_REAL_GPU_TEST=1 on a macOS desktop session"
        );
        return;
    }

    run_real_failure_atomicity_test();
}

fn run_real_failure_atomicity_test() {
    let app = build_real_fixture();
    let handle = app.handle();
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

    let crystal_state = app.state::<Mutex<CrystalState>>();
    let settings = app.state::<Mutex<AppSettings>>();
    let renderer = app.state::<Mutex<Renderer>>();
    let undo = app.state::<Mutex<UndoStack>>();

    let before_prepared = probe(&app, &state_events, &undo_events);
    let prepared_result = transaction::with_prepared_state_update(
        &handle,
        &crystal_state,
        &settings,
        &renderer,
        &undo,
        |current| {
            let mut invalid = current.clone();
            invalid.fract_x[0] = f64::NAN;
            Ok(invalid)
        },
    );
    assert!(prepared_result.is_err());
    assert_unchanged(&app, &before_prepared, &state_events, &undo_events);
    assert_eq!(history_depth_after_failure(&app), (1, 1));
    *undo.lock().unwrap() = seeded_undo_stack(&valid_state());

    let before_in_place = probe(&app, &state_events, &undo_events);
    let in_place_result = transaction::with_state_update(
        &handle,
        &crystal_state,
        &settings,
        &renderer,
        &undo,
        |_| Ok(true),
        |current| {
            current.fract_x[0] = f64::NAN;
            Ok(())
        },
    );
    assert!(in_place_result.is_err());
    assert_unchanged(&app, &before_in_place, &state_events, &undo_events);
    assert_eq!(history_depth_after_failure(&app), (1, 1));

    app.unlisten(state_listener);
    app.unlisten(undo_listener);
}
