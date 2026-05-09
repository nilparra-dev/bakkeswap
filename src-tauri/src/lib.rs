mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_shell_status])
        .run(tauri::generate_context!())
        .expect("failed to run BakkesSwap Tauri application");
}
