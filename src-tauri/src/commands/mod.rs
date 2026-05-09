use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AppShellStatus {
    pub application: String,
    pub phase: String,
    pub cli_first: bool,
    pub runtime_write_support: bool,
}

#[tauri::command]
pub fn app_shell_status() -> AppShellStatus {
    AppShellStatus {
        application: "BakkesSwap".to_string(),
        phase: "skeleton-only".to_string(),
        cli_first: true,
        runtime_write_support: false,
    }
}
