use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::audio::AudioInputDevice;
use crate::platform::macos::{self, AccessibilityStatus, MicrophoneStatus};
use crate::state::{AppState, LocalModelSettings, RecognitionLanguage};

const SETTINGS_FILE_NAME: &str = "settings.json";
pub const RECOGNITION_LANGUAGE_CHANGED_EVENT: &str = "wispergo://recognition-language-changed";

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedSettings {
    local_model: LocalModelSettings,
}

#[tauri::command]
pub fn fallback_policy_label() -> &'static str {
    "prefer_local_ask_before_cloud"
}

#[tauri::command]
pub fn list_microphones() -> Result<Vec<AudioInputDevice>, String> {
    crate::audio::list_input_devices()
}

#[tauri::command]
pub fn selected_microphone_id(state: State<'_, AppState>) -> Option<String> {
    state.selected_microphone_id()
}

#[tauri::command]
pub fn set_microphone_device(
    state: State<'_, AppState>,
    device_id: Option<String>,
) -> Result<(), String> {
    state.set_selected_microphone_id(device_id);
    Ok(())
}

#[tauri::command]
pub fn microphone_status() -> MicrophoneStatus {
    macos::microphone_status()
}

#[tauri::command]
pub async fn request_microphone_access() -> Result<MicrophoneStatus, String> {
    tauri::async_runtime::spawn_blocking(macos::request_microphone_access)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn local_model_settings(state: State<'_, AppState>) -> LocalModelSettings {
    state.local_model_settings().to_frontend()
}

#[tauri::command]
pub fn set_local_model_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: LocalModelSettings,
) -> Result<LocalModelSettings, String> {
    let settings = settings.normalized();
    state.set_local_model_settings(settings.clone());
    save_persisted_settings(&app, &settings)?;
    app.emit(
        RECOGNITION_LANGUAGE_CHANGED_EVENT,
        settings.recognition_language,
    )
    .map_err(|err| err.to_string())?;
    Ok(settings.to_frontend())
}

#[tauri::command]
pub fn recognition_language(state: State<'_, AppState>) -> RecognitionLanguage {
    state.local_model_settings().recognition_language
}

#[tauri::command]
pub fn set_recognition_language(
    app: AppHandle,
    state: State<'_, AppState>,
    language: RecognitionLanguage,
) -> Result<RecognitionLanguage, String> {
    let mut settings = state.local_model_settings();
    settings.recognition_language = language;
    state.set_local_model_settings(settings.clone());
    save_persisted_settings(&app, &settings)?;
    app.emit(RECOGNITION_LANGUAGE_CHANGED_EVENT, language)
        .map_err(|err| err.to_string())?;
    Ok(language)
}

#[tauri::command]
pub fn accessibility_status() -> AccessibilityStatus {
    macos::accessibility_status()
}

#[tauri::command]
pub fn request_accessibility() -> AccessibilityStatus {
    macos::request_accessibility()
}

pub fn load_persisted_settings(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let path = settings_file_path(app)?;
    if !path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let settings = serde_json::from_str::<PersistedSettings>(&content)
        .map_err(|err| err.to_string())?
        .local_model
        .normalized();
    state.set_local_model_settings(settings);
    Ok(())
}

fn save_persisted_settings(
    app: &AppHandle,
    local_model: &LocalModelSettings,
) -> Result<(), String> {
    let path = settings_file_path(app)?;
    let persisted = PersistedSettings {
        local_model: local_model.clone(),
    };
    let content = serde_json::to_string_pretty(&persisted).map_err(|err| err.to_string())?;
    fs::write(path, content).map_err(|err| err.to_string())
}

fn settings_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|err| err.to_string())?;
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir.join(SETTINGS_FILE_NAME))
}
