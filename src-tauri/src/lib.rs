pub mod ai;
pub mod app;
pub mod error;
pub mod resume;
pub mod session;
pub mod store;
pub mod transcript;
pub mod typesafe;

pub fn run() {
    let store = store::Store::new(store::Store::default_dir());
    #[cfg(target_os = "macos")]
    if let Some(home) = dirs::home_dir() {
        let _ = store.import_swift_prefs(&home.join("Library/Preferences/local.butler.desktop.plist"));
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .manage(app::AppState::new(store))
        .invoke_handler(tauri::generate_handler![
            app::snapshot,
            app::start_listening,
            app::stop_listening,
            app::suggest,
            app::cancel,
            app::clear,
            app::export,
            app::set_notes,
            app::get_settings,
            app::save_settings,
            app::read_key,
            app::save_key,
            app::test_provider,
            app::test_typesafe,
            app::import_resume,
            app::list_interviews,
            app::delete_interview,
            app::download_whisper_model,
            app::toggle_floating,
            app::show_main,
        ])
        .setup(|a| Ok(app::tray(a.handle())?))
        .on_window_event(app::on_window_event)
        .build(tauri::generate_context!())
        .expect("error while running Cue")
        .run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
                app::save_now(handle);
            }
        });
}
