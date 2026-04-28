mod commands;
#[allow(dead_code)]
mod insertion;
mod state;
#[allow(dead_code)]
mod trigger;

use commands::recording::{cancel_recording, recording_status, start_recording, stop_recording};
use commands::settings::fallback_policy_label;
use state::AppState;

#[tauri::command]
fn app_health() -> &'static str {
    "ok"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            app_health,
            start_recording,
            stop_recording,
            cancel_recording,
            recording_status,
            fallback_policy_label
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
