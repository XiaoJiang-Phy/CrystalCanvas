fn main() {}

#[allow(dead_code)]
fn test_menu(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{Menu, MenuItem, Submenu, PredefinedMenuItem};
    let quit = PredefinedMenuItem::quit(app, None::<&str>)?;
    let app_menu = Submenu::with_items(app, "App", true, &[&quit])?;
    let import = MenuItem::with_id(app, "import_cif", "Import CIF...", true, None::<&str>)?;
    let export = MenuItem::with_id(app, "export_poscar", "Export POSCAR...", true, None::<&str>)?;
    let file_menu = Submenu::with_items(app, "File", true, &[&import, &export])?;
    
    let menu = Menu::with_items(app, &[&app_menu, &file_menu])?;
    app.set_menu(menu)?;

    app.on_menu_event(move |app_handle, event| {
        let id = event.id.0.as_str(); // Check if this is the way to get ID string
        use tauri::Emitter;
        let _ = app_handle.emit(&format!("menu_{}", id), ());
    });

    Ok(())
}
