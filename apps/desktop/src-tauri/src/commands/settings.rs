#[tauri::command]
pub fn fallback_policy_label() -> &'static str {
    "prefer_local_ask_before_cloud"
}
