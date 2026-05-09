mod commands;

#[cfg(test)]
mod gui_sandbox_smoke;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::get_config,
            commands::set_game_path,
            commands::validate_game_path,
            commands::import_codered,
            commands::refresh_db,
            commands::search_items,
            commands::create_plan,
            commands::build_plan,
            commands::install_preview,
            commands::install_confirmed,
            commands::restore_preview,
            commands::restore_confirmed,
            commands::backup_originals_status,
            commands::backup_originals_verify,
            commands::list_installed_swaps,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run BakkesSwap Tauri application");
}
