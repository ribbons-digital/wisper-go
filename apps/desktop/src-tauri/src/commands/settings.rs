use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};
use wispergo_core::ollama::{OllamaCleanupProvider, DEFAULT_OLLAMA_MODEL};

use crate::audio::AudioInputDevice;
use crate::platform::macos::{self, AccessibilityStatus, MicrophoneStatus};
use crate::state::{AppState, LocalModelSettings, RecognitionLanguage};

const SETTINGS_FILE_NAME: &str = "settings.json";
pub const RECOGNITION_LANGUAGE_CHANGED_EVENT: &str = "wispergo://recognition-language-changed";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaSetupStatus {
    pub cli_installed: bool,
    pub server_running: bool,
    pub model_installed: bool,
    pub model: String,
    pub status: String,
    pub message: Option<String>,
}

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
pub async fn ensure_ollama_setup() -> Result<OllamaSetupStatus, String> {
    let status = tauri::async_runtime::spawn_blocking(ensure_ollama_setup_blocking)
        .await
        .map_err(|err| err.to_string())??;

    if status.cli_installed && status.server_running && status.model_installed {
        let warm_status = status.clone();
        tauri::async_runtime::spawn(async move {
            warm_ollama_model(&warm_status).await;
        });
    }

    Ok(status)
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

async fn warm_ollama_model(status: &OllamaSetupStatus) {
    let base_url = env::var("WISPERGO_OLLAMA_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let provider = OllamaCleanupProvider::new(base_url, status.model.clone());
    if let Err(err) = provider.warm(Duration::from_millis(1500)).await {
        eprintln!("ollama warmup failed: {err}");
    }
}

fn ensure_ollama_setup_blocking() -> Result<OllamaSetupStatus, String> {
    let model =
        env::var("WISPERGO_OLLAMA_MODEL").unwrap_or_else(|_| DEFAULT_OLLAMA_MODEL.to_string());
    let Some(cli_path) = ollama_cli_path() else {
        return Ok(OllamaSetupStatus {
            cli_installed: false,
            server_running: false,
            model_installed: false,
            model,
            status: "cli_missing".to_string(),
            message: Some(
                "Install Ollama from https://ollama.com/download, then reopen Wispergo."
                    .to_string(),
            ),
        });
    };

    let mut server_running = ollama_list(&cli_path).is_ok();
    if !server_running {
        if let Err(err) = start_ollama_server(&cli_path) {
            return Ok(OllamaSetupStatus {
                cli_installed: true,
                server_running: false,
                model_installed: false,
                model,
                status: "server_start_failed".to_string(),
                message: Some(format!(
                    "Ollama is installed, but ollama serve could not start: {err}"
                )),
            });
        }
        server_running = wait_for_ollama_server(&cli_path);
    }

    if !server_running {
        return Ok(OllamaSetupStatus {
            cli_installed: true,
            server_running: false,
            model_installed: false,
            model,
            status: "server_starting".to_string(),
            message: Some(
                "Ollama is starting. Try again in a moment if cleanup is unavailable.".to_string(),
            ),
        });
    }

    let list_output = ollama_list(&cli_path)?;
    if ollama_model_installed(&list_output, &model) {
        return Ok(OllamaSetupStatus {
            cli_installed: true,
            server_running: true,
            model_installed: true,
            model,
            status: "ready".to_string(),
            message: None,
        });
    }

    if let Err(err) = pull_ollama_model(&cli_path, &model) {
        return Ok(OllamaSetupStatus {
            cli_installed: true,
            server_running: true,
            model_installed: false,
            model,
            status: "model_pull_failed".to_string(),
            message: Some(format!("Ollama model pull failed: {err}")),
        });
    }

    let list_output = ollama_list(&cli_path)?;
    let model_installed = ollama_model_installed(&list_output, &model);
    Ok(OllamaSetupStatus {
        cli_installed: true,
        server_running: true,
        model_installed,
        model,
        status: if model_installed {
            "ready"
        } else {
            "model_missing"
        }
        .to_string(),
        message: if model_installed {
            None
        } else {
            Some("Ollama model pull completed, but the model was not listed.".to_string())
        },
    })
}

fn ollama_cli_path() -> Option<PathBuf> {
    env::var_os("WISPERGO_OLLAMA_BIN")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| find_in_path("ollama"))
        .or_else(|| {
            ["/opt/homebrew/bin/ollama", "/usr/local/bin/ollama"]
                .into_iter()
                .map(PathBuf::from)
                .find(|path| path.is_file())
        })
}

fn find_in_path(binary: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

fn ollama_list(cli_path: &Path) -> Result<String, String> {
    let output = Command::new(cli_path)
        .arg("list")
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn start_ollama_server(cli_path: &Path) -> Result<(), String> {
    Command::new(cli_path)
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn wait_for_ollama_server(cli_path: &Path) -> bool {
    for _ in 0..10 {
        thread::sleep(Duration::from_millis(500));
        if ollama_list(cli_path).is_ok() {
            return true;
        }
    }
    false
}

fn pull_ollama_model(cli_path: &Path, model: &str) -> Result<(), String> {
    let output = Command::new(cli_path)
        .arg("pull")
        .arg(model)
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn ollama_model_installed(list_output: &str, model: &str) -> bool {
    list_output
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .any(|listed_model| listed_model == model)
}

fn settings_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|err| err.to_string())?;
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir.join(SETTINGS_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::{ollama_model_installed, OllamaSetupStatus};

    #[test]
    fn detects_installed_ollama_model_from_list_output() {
        let output = "NAME             ID              SIZE      MODIFIED\nqwen2.5:0.5b     abc123          397 MB    1 minute ago\nllama3.2:3b      def456          2.0 GB    1 day ago\n";

        assert!(ollama_model_installed(output, "qwen2.5:0.5b"));
        assert!(!ollama_model_installed(output, "qwen2.5:1.5b"));
    }

    #[test]
    fn ollama_setup_attempts_serve_pull_and_warm_in_backend() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/settings.rs"),
        )
        .expect("settings source");

        assert!(source.contains("arg(\"serve\")"));
        assert!(source.contains("arg(\"pull\")"));
        assert!(source.contains("DEFAULT_OLLAMA_MODEL"));
        assert!(source.contains("warm_ollama_model"));
        assert!(source.contains("Duration::from_millis(1500)"));
    }

    #[test]
    fn ollama_setup_status_serializes_camel_case_for_frontend() {
        let value = serde_json::to_value(OllamaSetupStatus {
            cli_installed: false,
            server_running: false,
            model_installed: false,
            model: "qwen2.5:0.5b".to_string(),
            status: "cli_missing".to_string(),
            message: Some("Install Ollama".to_string()),
        })
        .expect("serialize status");

        assert_eq!(value["cliInstalled"], false);
        assert_eq!(value["serverRunning"], false);
        assert_eq!(value["modelInstalled"], false);
        assert_eq!(value["model"], "qwen2.5:0.5b");
        assert_eq!(value["status"], "cli_missing");
        assert_eq!(value["message"], "Install Ollama");
    }
}
