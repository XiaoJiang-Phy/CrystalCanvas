use crystal_canvas::{
    commands::{analysis, editing, geometry, viewport},
    crystal_state::CrystalState,
    renderer::renderer::Renderer,
    settings::AppSettings,
    transaction::StateChangedPayload,
    undo::UndoStack,
};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Listener, Manager};

fn valid_state() -> CrystalState {
    let mut state = crystal_canvas::io::import::load_file(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/data/nacl.cif"
    ))
    .expect("SYNC-1C fixture must load the audited NaCl structure");
    state.version = 17;
    state.validate_structural_invariants().unwrap();
    state
}

fn build_fixture() -> tauri::App {
    let app = tauri::Builder::default()
        .build(tauri::generate_context!())
        .expect("SYNC-1C fixture must build a Tauri application");
    let window = tauri::WebviewWindowBuilder::new(
        &app,
        "sync-1c-smoke",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .inner_size(64.0, 64.0)
    .visible(false)
    .build()
    .expect("SYNC-1C fixture requires a real hidden window");

    assert!(app.manage(Mutex::new(valid_state())));
    assert!(app.manage(Mutex::new(AppSettings::default())));
    assert!(app.manage(Mutex::new(UndoStack::new(32))));
    assert!(app.manage(Mutex::new(Renderer::new(Arc::new(window), 64, 64))));
    app
}

fn version(app: &tauri::App) -> u32 {
    app.state::<Mutex<CrystalState>>().lock().unwrap().version
}

fn assert_single_commit<T>(
    app: &tauri::App,
    state_event_versions: &Mutex<Vec<u32>>,
    mutation: impl FnOnce() -> crystal_canvas::ipc::IpcResult<T>,
) {
    let before_version = version(app);
    let before_events = state_event_versions.lock().unwrap().len();
    mutation().expect("committed mutation must succeed");
    let committed_version = version(app);
    let state_event_versions = state_event_versions.lock().unwrap();
    assert_eq!(committed_version, before_version + 1);
    assert_eq!(state_event_versions.len(), before_events + 1);
    assert_eq!(state_event_versions[before_events], committed_version);
}

fn assert_rejected_without_event<T>(
    app: &tauri::App,
    state_event_versions: &Mutex<Vec<u32>>,
    mutation: impl FnOnce() -> crystal_canvas::ipc::IpcResult<T>,
) {
    let before_version = version(app);
    let before_events = state_event_versions.lock().unwrap().len();
    assert!(mutation().is_err());
    assert_eq!(version(app), before_version);
    assert_eq!(state_event_versions.lock().unwrap().len(), before_events);
}

fn main() {
    if std::env::var_os("CRYSTAL_CANVAS_RUN_SYNC_SMOKE").is_none() {
        eprintln!(
            "SKIPPED_NO_DESKTOP_SESSION: set CRYSTAL_CANVAS_RUN_SYNC_SMOKE=1 on a macOS desktop session"
        );
        return;
    }

    run_sync_smoke();
}

fn run_sync_smoke() {
    let app = build_fixture();
    let state_event_versions = Arc::new(Mutex::new(Vec::new()));
    let event_versions = state_event_versions.clone();
    let listener = app.listen("state_changed", move |event| {
        let payload: serde_json::Value = serde_json::from_str(event.payload()).unwrap();
        let version = payload["version"].as_u64().and_then(|value| u32::try_from(value).ok()).unwrap();
        event_versions.lock().unwrap().push(version);
    });

    assert_single_commit(&app, state_event_versions.as_ref(), || {
        geometry::apply_slab(
            [1, 0, 0],
            1,
            5.0,
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
            app.state(),
        )
    });
    assert_single_commit(&app, state_event_versions.as_ref(), || {
        editing::undo(
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
            app.state(),
        )
    });
    assert_single_commit(&app, state_event_versions.as_ref(), || {
        geometry::apply_supercell(
            [[2, 0, 0], [0, 1, 0], [0, 0, 1]],
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
            app.state(),
        )
    });
    assert_single_commit(&app, state_event_versions.as_ref(), || {
        editing::undo(
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
            app.state(),
        )
    });
    assert_single_commit(&app, state_event_versions.as_ref(), || {
        editing::redo(
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
            app.state(),
        )
    });

    let added_index = app
        .state::<Mutex<CrystalState>>()
        .lock()
        .unwrap()
        .intrinsic_sites;
    assert_single_commit(&app, state_event_versions.as_ref(), || {
        editing::add_atom(
            "O".into(),
            8,
            [0.123, 0.234, 0.345],
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
            app.state(),
        )
    });
    assert_single_commit(&app, state_event_versions.as_ref(), || {
        editing::delete_atoms(
            vec![added_index],
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
            app.state(),
        )
    });
    assert_single_commit(&app, state_event_versions.as_ref(), || {
        editing::substitute_atoms(
            vec![0],
            "N".into(),
            7,
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
            app.state(),
        )
    });
    assert_single_commit(&app, state_event_versions.as_ref(), || {
        analysis::add_measurement(
            vec![0, 1],
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
            app.state(),
        )
    });
    assert_single_commit(&app, state_event_versions.as_ref(), || {
        analysis::clear_measurements(
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
            app.state(),
        )
    });

    assert_single_commit(&app, state_event_versions.as_ref(), || {
        editing::translate_atoms_screen(
            vec![0],
            1.0,
            -1.0,
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
        )
    });

    assert_rejected_without_event(&app, state_event_versions.as_ref(), || {
        editing::delete_atoms(
            Vec::new(),
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
            app.state(),
        )
    });
    assert_rejected_without_event(&app, state_event_versions.as_ref(), || {
        geometry::apply_slab(
            [0, 0, 0],
            1,
            5.0,
            app.handle().clone(),
            app.state(),
            app.state(),
            app.state(),
            app.state(),
        )
    });

    let before_camera_version = version(&app);
    let before_camera_events = state_event_versions.lock().unwrap().len();
    viewport::rotate_camera(1.0, -1.0, app.state()).unwrap();
    viewport::pan_camera(1.0, -1.0, app.state()).unwrap();
    viewport::zoom_camera(1.0, app.state()).unwrap();
    assert_eq!(version(&app), before_camera_version);
    assert_eq!(state_event_versions.lock().unwrap().len(), before_camera_events);

    let before_duplicate_version = version(&app);
    let before_duplicate_events = state_event_versions.lock().unwrap().len();
    app.emit(
        "state_changed",
        StateChangedPayload {
            version: before_duplicate_version,
        },
    )
    .unwrap();
    assert_eq!(version(&app), before_duplicate_version);
    let state_event_versions = state_event_versions.lock().unwrap();
    assert_eq!(state_event_versions.len(), before_duplicate_events + 1);
    assert_eq!(state_event_versions[before_duplicate_events], before_duplicate_version);

    app.unlisten(listener);
}
