use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};
use wispergo_core::asset_manifest::{AssetEntry, AssetManifest, AssetRole};
use wispergo_core::asset_storage::AssetStorage;
use wispergo_core::downloader::{repair_asset, verify_asset, AssetIntegrity};
use wispergo_core::ollama::{OllamaCleanupProvider, DEFAULT_OLLAMA_MODEL};

use crate::audio::AudioInputDevice;
use crate::commands::assets::{
    load_bundled_manifest, AssetClient, AssetDownloadStatus, ASSET_DOWNLOAD_EVENT,
};
use crate::inference::app_support_asset_storage;
use crate::inference::manager::{
    AsrEngineConfig, CleanupEngineConfig, CleanupInferenceMode, InferenceManager,
    InferenceRuntimeStatus,
};
use crate::inference::resources::InferenceResourcePaths;
use crate::platform::macos::{self, AccessibilityStatus, MicrophoneStatus};
use crate::state::{AppState, CleanupMode, LocalModelSettings, RecognitionLanguage};

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
pub fn cleanup_runtime_status(
    inference_manager: State<'_, InferenceManager>,
) -> InferenceRuntimeStatus {
    inference_manager.cleanup().status()
}

pub fn managed_cleanup_runtime_enabled(settings: &LocalModelSettings) -> bool {
    managed_cleanup_runtime_enabled_for_backend(
        settings,
        env::var("WISPERGO_CLEANUP_BACKEND").ok().as_deref(),
    )
}

fn managed_cleanup_runtime_enabled_for_backend(
    settings: &LocalModelSettings,
    cleanup_backend: Option<&str>,
) -> bool {
    settings.cleanup_mode != CleanupMode::Off && cleanup_backend != Some("ollama")
}

pub fn sync_inference_manager_for_settings(
    app: &AppHandle,
    inference_manager: &InferenceManager,
    settings: &LocalModelSettings,
) {
    let manifest = load_bundled_manifest(app);
    let storage = app_support_asset_storage(app).ok();
    let resources = match app.path().resource_dir() {
        Ok(resource_root) => Some(InferenceResourcePaths::from_resource_root(resource_root)),
        Err(err) => {
            eprintln!("inference resource directory unavailable: {err}");
            None
        }
    };

    sync_asr_for_settings(
        inference_manager,
        settings,
        resources.as_ref(),
        &manifest,
        storage.as_ref(),
    );
    sync_cleanup_for_settings(
        inference_manager,
        settings,
        resources.as_ref(),
        &manifest,
        storage.as_ref(),
    );
}

fn sync_asr_for_settings(
    inference_manager: &InferenceManager,
    settings: &LocalModelSettings,
    resources: Option<&InferenceResourcePaths>,
    manifest: &AssetManifest,
    storage: Option<&AssetStorage>,
) {
    match resolve_asr_model_path_for_settings(settings, resources, manifest, storage) {
        Ok(model_path) => {
            if let Err(err) = inference_manager.asr().arm(AsrEngineConfig {
                model_path,
                language: settings
                    .recognition_language
                    .whisper_code()
                    .map(str::to_string),
            }) {
                eprintln!("ASR inference manager arm failed: {err}");
            }
        }
        Err(message) => {
            if let Err(err) = inference_manager.asr().mark_unavailable(message) {
                eprintln!("ASR inference manager unavailable sync failed: {err}");
            }
        }
    }
}

fn sync_cleanup_for_settings(
    inference_manager: &InferenceManager,
    settings: &LocalModelSettings,
    resources: Option<&InferenceResourcePaths>,
    manifest: &AssetManifest,
    storage: Option<&AssetStorage>,
) {
    if !managed_cleanup_runtime_enabled(settings) {
        if let Err(err) = inference_manager.cleanup().disable() {
            eprintln!("cleanup inference manager disable failed: {err}");
        }
        return;
    }

    let cleanup_model_path =
        match resolve_cleanup_model_path_for_settings(settings, resources, manifest, storage) {
            Ok(path) => path,
            Err(message) => {
                if let Err(err) = inference_manager.cleanup().mark_unavailable(message) {
                    eprintln!("cleanup inference manager unavailable sync failed: {err}");
                }
                return;
            }
        };

    if let Err(err) = inference_manager.cleanup().arm(CleanupEngineConfig {
        model_path: cleanup_model_path,
        mode: match settings.cleanup_mode {
            CleanupMode::Off => unreachable!("cleanup off handled above"),
            CleanupMode::PunctuationOnly => CleanupInferenceMode::PunctuationOnly,
            CleanupMode::FullCleanup => CleanupInferenceMode::FullCleanup,
        },
    }) {
        eprintln!("cleanup inference manager arm failed: {err}");
    }
}

fn resolve_cleanup_model_path_for_settings(
    settings: &LocalModelSettings,
    resources: Option<&InferenceResourcePaths>,
    manifest: &AssetManifest,
    storage: Option<&AssetStorage>,
) -> Result<PathBuf, String> {
    let role = match settings.cleanup_mode {
        CleanupMode::Off => return Err("cleanup is disabled".to_string()),
        CleanupMode::PunctuationOnly => AssetRole::CleanupPunctuation,
        CleanupMode::FullCleanup => AssetRole::CleanupFull,
    };

    if !manifest.assets.is_empty() {
        let storage = storage.ok_or_else(|| {
            "Local cleanup asset storage is unavailable. Reopen Wispergo and try again.".to_string()
        })?;
        let asset = selected_cleanup_asset(manifest, role)?;
        let path = storage.asset_path(&asset.id, asset.role);
        return match verify_asset(asset, storage) {
            AssetIntegrity::Valid => Ok(path),
            AssetIntegrity::Missing => Err(match role {
                AssetRole::CleanupPunctuation => format!(
                    "cleanup punctuation asset '{}' is not downloaded yet",
                    asset.id
                ),
                AssetRole::CleanupFull => {
                    format!("full cleanup asset '{}' is not downloaded yet", asset.id)
                }
                AssetRole::Asr => unreachable!("cleanup role cannot be ASR"),
            }),
            AssetIntegrity::Corrupt => Err(match role {
                AssetRole::CleanupPunctuation => {
                    format!("cleanup punctuation asset '{}' is corrupt", asset.id)
                }
                AssetRole::CleanupFull => format!("full cleanup asset '{}' is corrupt", asset.id),
                AssetRole::Asr => unreachable!("cleanup role cannot be ASR"),
            }),
        };
    }

    resources
        .filter(|resources| resources.cleanup_model_path.exists())
        .map(|resources| resources.cleanup_model_path.clone())
        .ok_or_else(|| {
            "Local cleanup is not configured. Download cleanup models or reinstall Wispergo."
                .to_string()
        })
}

fn selected_cleanup_asset(
    manifest: &AssetManifest,
    role: AssetRole,
) -> Result<&AssetEntry, String> {
    match role {
        AssetRole::CleanupPunctuation => manifest
            .by_role(role)
            .find(|asset| asset.default)
            .ok_or_else(|| "no default cleanup punctuation asset is configured".to_string()),
        AssetRole::CleanupFull => manifest
            .by_role(role)
            .next()
            .ok_or_else(|| "no full cleanup asset is configured".to_string()),
        AssetRole::Asr => unreachable!("cleanup role cannot be ASR"),
    }
}

fn resolve_asr_model_path_for_settings(
    settings: &LocalModelSettings,
    resources: Option<&InferenceResourcePaths>,
    manifest: &AssetManifest,
    storage: Option<&AssetStorage>,
) -> Result<PathBuf, String> {
    if let Some(env_model) = env::var_os("WISPERGO_WHISPER_MODEL").map(PathBuf::from) {
        return Ok(env_model);
    }

    if !manifest.assets.is_empty() {
        let storage = storage.ok_or_else(|| {
            "Local ASR asset storage is unavailable. Reopen Wispergo and try again.".to_string()
        })?;
        let asset = selected_asr_asset(manifest, settings)?;
        let path = storage.asset_path(&asset.id, asset.role);
        return match verify_asset(asset, storage) {
            AssetIntegrity::Valid => Ok(path),
            AssetIntegrity::Missing => Err(format!(
                "ASR model '{}' is not downloaded yet. Download models before dictating.",
                asset.display_name
            )),
            AssetIntegrity::Corrupt => Err(format!(
                "ASR model '{}' is corrupt. Repair or re-download models before dictating.",
                asset.display_name
            )),
        };
    }

    let bundled_model_path = resources
        .filter(|resources| resources.asr_model_path.exists())
        .map(|resources| resources.asr_model_path.clone());
    let settings_model_path = settings
        .whisper_model_path
        .as_ref()
        .map(|path| PathBuf::from(path.trim()))
        .filter(|path| path.exists());
    let model_path = bundled_model_path.or(settings_model_path);

    match model_path {
        Some(path) => Ok(path),
        None => Err(
            "Local ASR is not configured. Reinstall Wispergo or set WISPERGO_WHISPER_MODEL."
                .to_string(),
        ),
    }
}

fn selected_asr_asset<'a>(
    manifest: &'a AssetManifest,
    settings: &LocalModelSettings,
) -> Result<&'a AssetEntry, String> {
    let asset = manifest
        .find(&settings.asr_model_id)
        .ok_or_else(|| format!("unknown ASR model id: {}", settings.asr_model_id))?;
    if asset.role != AssetRole::Asr {
        return Err(format!(
            "asset {} is not an ASR model (role: {:?})",
            asset.id, asset.role
        ));
    }
    Ok(asset)
}

async fn ensure_assets_for_settings(
    app: &AppHandle,
    client_state: &AssetClient,
    settings: &LocalModelSettings,
) -> Result<(), String> {
    let manifest = load_bundled_manifest(app);
    if manifest.assets.is_empty() {
        return Ok(());
    }

    let storage = app_support_asset_storage(app)?;
    let asset = selected_asr_asset(&manifest, settings)?.clone();
    match verify_asset(&asset, &storage) {
        AssetIntegrity::Valid => Ok(()),
        AssetIntegrity::Missing | AssetIntegrity::Corrupt => {
            let _ = app.emit(
                ASSET_DOWNLOAD_EVENT,
                AssetDownloadStatus::Downloading {
                    asset_id: asset.id.clone(),
                    display_name: asset.display_name.clone(),
                },
            );
            match repair_asset(&asset, &storage, &client_state.get()).await {
                Ok(_) => {
                    let _ = app.emit(ASSET_DOWNLOAD_EVENT, AssetDownloadStatus::Ready);
                    Ok(())
                }
                Err(err) => {
                    let message = format!("failed to download {}: {err}", asset.display_name);
                    let _ = app.emit(
                        ASSET_DOWNLOAD_EVENT,
                        AssetDownloadStatus::Failed {
                            message: message.clone(),
                        },
                    );
                    Err(message)
                }
            }
        }
    }
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
pub async fn set_local_model_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    inference_manager: State<'_, InferenceManager>,
    asset_client: State<'_, AssetClient>,
    settings: LocalModelSettings,
) -> Result<LocalModelSettings, String> {
    let previous = state.local_model_settings();
    let settings = settings.normalized();
    ensure_assets_for_settings(&app, asset_client.inner(), &settings).await?;
    state.set_local_model_settings(settings.clone());
    save_persisted_settings(&app, &settings)?;
    app.emit(
        RECOGNITION_LANGUAGE_CHANGED_EVENT,
        settings.recognition_language,
    )
    .map_err(|err| err.to_string())?;
    if previous != settings {
        sync_inference_manager_for_settings(&app, inference_manager.inner(), &settings);
    }
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
    inference_manager: State<'_, InferenceManager>,
    language: RecognitionLanguage,
) -> Result<RecognitionLanguage, String> {
    let mut settings = state.local_model_settings();
    settings.recognition_language = language;
    state.set_local_model_settings(settings.clone());
    save_persisted_settings(&app, &settings)?;
    app.emit(RECOGNITION_LANGUAGE_CHANGED_EVENT, language)
        .map_err(|err| err.to_string())?;
    sync_inference_manager_for_settings(&app, inference_manager.inner(), &settings);
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::{
        managed_cleanup_runtime_enabled_for_backend, ollama_model_installed, CleanupMode,
        LocalModelSettings, OllamaSetupStatus,
    };
    use crate::inference::manager::{
        AsrEngineConfig, AsrInferenceOutput, AsrInferenceRequest, CleanupEngineConfig,
        CleanupInferenceOutput, CleanupInferenceRequest, InferenceManager, InferenceManagerError,
        InferenceRuntimeState, ManagedInferenceEngine,
    };
    use crate::inference::resources::{CpuArchitecture, InferenceResourcePaths};
    use crate::state::RecognitionLanguage;
    use wispergo_core::asset_manifest::AssetRole;
    use wispergo_core::asset_storage::AssetStorage;
    use wispergo_core::domain::{PipelineResult, ProviderSource};

    struct TestAsrEngine {
        config: AsrEngineConfig,
    }

    impl ManagedInferenceEngine<AsrInferenceRequest, AsrInferenceOutput> for TestAsrEngine {
        fn infer(
            &mut self,
            _payload: AsrInferenceRequest,
        ) -> Result<AsrInferenceOutput, InferenceManagerError> {
            Ok(AsrInferenceOutput {
                transcript: self
                    .config
                    .language
                    .clone()
                    .unwrap_or_else(|| "auto".to_string()),
                confidence: None,
                source: ProviderSource::Local,
            })
        }
    }

    struct TestCleanupEngine;

    impl ManagedInferenceEngine<CleanupInferenceRequest, CleanupInferenceOutput> for TestCleanupEngine {
        fn infer(
            &mut self,
            payload: CleanupInferenceRequest,
        ) -> Result<CleanupInferenceOutput, InferenceManagerError> {
            Ok(CleanupInferenceOutput {
                result: PipelineResult::InsertText {
                    text: payload.transcript,
                    source: ProviderSource::Local,
                    confidence: None,
                },
            })
        }
    }

    fn empty_manifest() -> wispergo_core::asset_manifest::AssetManifest {
        wispergo_core::asset_manifest::AssetManifest {
            schema_version: 1,
            assets: Vec::new(),
        }
    }

    fn test_asr_manifest(id: &str) -> wispergo_core::asset_manifest::AssetManifest {
        wispergo_core::asset_manifest::AssetManifest {
            schema_version: 1,
            assets: vec![wispergo_core::asset_manifest::AssetEntry {
                id: id.to_string(),
                role: wispergo_core::asset_manifest::AssetRole::Asr,
                display_name: id.to_string(),
                url: format!("https://example.test/{id}.bin"),
                size: 10,
                sha256: "dc41663fad7e4d1e9d5767b61ec63919d3a120dc3e12f34bb5375658bbaccfb1"
                    .to_string(),
                default: id == "medium",
            }],
        }
    }

    fn test_cleanup_manifest(
        id: &str,
        role: AssetRole,
    ) -> wispergo_core::asset_manifest::AssetManifest {
        test_cleanup_manifest_with_default(id, role, true)
    }

    fn test_cleanup_manifest_with_default(
        id: &str,
        role: AssetRole,
        default: bool,
    ) -> wispergo_core::asset_manifest::AssetManifest {
        wispergo_core::asset_manifest::AssetManifest {
            schema_version: 1,
            assets: vec![wispergo_core::asset_manifest::AssetEntry {
                id: id.to_string(),
                role,
                display_name: id.to_string(),
                url: format!("https://example.test/{id}.gguf"),
                size: 10,
                sha256: "dc41663fad7e4d1e9d5767b61ec63919d3a120dc3e12f34bb5375658bbaccfb1"
                    .to_string(),
                default,
            }],
        }
    }

    fn create_file(path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, "test asset").expect("write test asset");
    }

    fn test_manager(
        asr_loads: Arc<AtomicUsize>,
        cleanup_loads: Arc<AtomicUsize>,
        seen_asr_configs: Arc<Mutex<Vec<AsrEngineConfig>>>,
    ) -> InferenceManager {
        InferenceManager::new(
            move |config: &AsrEngineConfig| {
                asr_loads.fetch_add(1, Ordering::SeqCst);
                seen_asr_configs
                    .lock()
                    .expect("seen asr configs")
                    .push(config.clone());
                Ok(Box::new(TestAsrEngine {
                    config: config.clone(),
                }))
            },
            move |_config: &CleanupEngineConfig| {
                cleanup_loads.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new(TestCleanupEngine))
            },
        )
    }

    #[test]
    fn settings_sync_arms_without_loading_and_first_asr_request_loads() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let resources = InferenceResourcePaths::from_resource_root_for_arch(
            tempdir.path().to_path_buf(),
            CpuArchitecture::Aarch64,
        );
        create_file(&resources.asr_model_path);
        create_file(&resources.cleanup_model_path);
        let asr_loads = Arc::new(AtomicUsize::new(0));
        let cleanup_loads = Arc::new(AtomicUsize::new(0));
        let seen_asr_configs = Arc::new(Mutex::new(Vec::new()));
        let manager = test_manager(
            Arc::clone(&asr_loads),
            Arc::clone(&cleanup_loads),
            Arc::clone(&seen_asr_configs),
        );
        let settings = LocalModelSettings::default();

        let manifest = empty_manifest();
        super::sync_asr_for_settings(&manager, &settings, Some(&resources), &manifest, None);
        super::sync_cleanup_for_settings(&manager, &settings, Some(&resources), &manifest, None);

        assert_eq!(manager.asr().status().state, InferenceRuntimeState::Ready);
        assert_eq!(
            manager.cleanup().status().state,
            InferenceRuntimeState::Ready
        );
        assert!(!manager.asr().snapshot().loaded);
        assert!(!manager.cleanup().snapshot().loaded);
        assert_eq!(asr_loads.load(Ordering::SeqCst), 0);
        assert_eq!(cleanup_loads.load(Ordering::SeqCst), 0);

        let asr = manager
            .asr()
            .request(AsrInferenceRequest { audio: vec![0.1] })
            .expect("asr request");

        assert_eq!(asr.transcript, "auto");
        assert_eq!(asr_loads.load(Ordering::SeqCst), 1);
        assert!(manager.asr().snapshot().loaded);
        manager.shutdown().expect("shutdown");
    }

    #[test]
    fn asr_sync_uses_verified_app_support_asset_when_manifest_is_populated() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = AssetStorage::new(tempdir.path().join("models"));
        let manifest = test_asr_manifest("large-v3-turbo");
        let asset_path = storage.asset_path("large-v3-turbo", AssetRole::Asr);
        create_file(&asset_path);
        let asr_loads = Arc::new(AtomicUsize::new(0));
        let seen_asr_configs = Arc::new(Mutex::new(Vec::new()));
        let manager = test_manager(
            Arc::clone(&asr_loads),
            Arc::new(AtomicUsize::new(0)),
            Arc::clone(&seen_asr_configs),
        );
        let settings = LocalModelSettings {
            asr_model_id: "large-v3-turbo".to_string(),
            ..LocalModelSettings::default()
        };

        super::sync_asr_for_settings(&manager, &settings, None, &manifest, Some(&storage));

        assert_eq!(manager.asr().status().state, InferenceRuntimeState::Ready);
        assert!(!manager.asr().snapshot().loaded);
        let output = manager
            .asr()
            .request(AsrInferenceRequest { audio: vec![0.1] })
            .expect("asr request");
        assert_eq!(output.transcript, "auto");
        assert_eq!(asr_loads.load(Ordering::SeqCst), 1);
        assert_eq!(
            seen_asr_configs
                .lock()
                .expect("seen asr configs")
                .last()
                .expect("loaded config")
                .model_path,
            asset_path
        );
        manager.shutdown().expect("shutdown");
    }

    #[test]
    fn cleanup_uses_verified_app_support_punctuation_asset_when_manifest_populated() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = AssetStorage::new(tempdir.path().join("models"));
        let manifest =
            test_cleanup_manifest("qwen2.5-0.5b-instruct", AssetRole::CleanupPunctuation);
        let asset_path = storage.asset_path("qwen2.5-0.5b-instruct", AssetRole::CleanupPunctuation);
        create_file(&asset_path);
        let settings = LocalModelSettings {
            cleanup_mode: CleanupMode::PunctuationOnly,
            ..LocalModelSettings::default()
        };

        let path = super::resolve_cleanup_model_path_for_settings(
            &settings,
            None,
            &manifest,
            Some(&storage),
        )
        .expect("cleanup path");

        assert_eq!(path, asset_path);
    }

    #[test]
    fn cleanup_punctuation_missing_asset_reports_unavailable_path_error() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = AssetStorage::new(tempdir.path().join("models"));
        let manifest =
            test_cleanup_manifest("qwen2.5-0.5b-instruct", AssetRole::CleanupPunctuation);
        let settings = LocalModelSettings {
            cleanup_mode: CleanupMode::PunctuationOnly,
            ..LocalModelSettings::default()
        };

        let error = super::resolve_cleanup_model_path_for_settings(
            &settings,
            None,
            &manifest,
            Some(&storage),
        )
        .expect_err("missing cleanup asset should report unavailable");

        assert!(error
            .contains("cleanup punctuation asset 'qwen2.5-0.5b-instruct' is not downloaded yet"));
    }

    #[test]
    fn cleanup_punctuation_corrupt_asset_reports_unavailable_path_error() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = AssetStorage::new(tempdir.path().join("models"));
        let manifest =
            test_cleanup_manifest("qwen2.5-0.5b-instruct", AssetRole::CleanupPunctuation);
        let asset_path = storage.asset_path("qwen2.5-0.5b-instruct", AssetRole::CleanupPunctuation);
        if let Some(parent) = asset_path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&asset_path, "corrupt asset").expect("write corrupt asset");
        let settings = LocalModelSettings {
            cleanup_mode: CleanupMode::PunctuationOnly,
            ..LocalModelSettings::default()
        };

        let error = super::resolve_cleanup_model_path_for_settings(
            &settings,
            None,
            &manifest,
            Some(&storage),
        )
        .expect_err("corrupt cleanup asset should report unavailable");

        assert!(error.contains("cleanup punctuation asset 'qwen2.5-0.5b-instruct' is corrupt"));
    }

    #[test]
    fn full_cleanup_uses_verified_app_support_full_asset_when_manifest_populated() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = AssetStorage::new(tempdir.path().join("models"));
        let manifest = test_cleanup_manifest_with_default(
            "qwen2.5-3b-instruct",
            AssetRole::CleanupFull,
            false,
        );
        let asset_path = storage.asset_path("qwen2.5-3b-instruct", AssetRole::CleanupFull);
        create_file(&asset_path);
        let settings = LocalModelSettings {
            cleanup_mode: CleanupMode::FullCleanup,
            ..LocalModelSettings::default()
        };

        let path = super::resolve_cleanup_model_path_for_settings(
            &settings,
            None,
            &manifest,
            Some(&storage),
        )
        .expect("full cleanup path");

        assert_eq!(path, asset_path);
    }

    #[test]
    fn full_cleanup_missing_full_asset_reports_unavailable_path_error() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = AssetStorage::new(tempdir.path().join("models"));
        let manifest = test_cleanup_manifest_with_default(
            "qwen2.5-3b-instruct",
            AssetRole::CleanupFull,
            false,
        );
        let settings = LocalModelSettings {
            cleanup_mode: CleanupMode::FullCleanup,
            ..LocalModelSettings::default()
        };

        let error = super::resolve_cleanup_model_path_for_settings(
            &settings,
            None,
            &manifest,
            Some(&storage),
        )
        .expect_err("missing full cleanup asset should report unavailable");

        assert!(error.contains("full cleanup asset 'qwen2.5-3b-instruct' is not downloaded yet"));
    }

    #[test]
    fn full_cleanup_does_not_use_punctuation_asset_when_manifest_populated() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = AssetStorage::new(tempdir.path().join("models"));
        let manifest =
            test_cleanup_manifest("qwen2.5-0.5b-instruct", AssetRole::CleanupPunctuation);
        let asset_path = storage.asset_path("qwen2.5-0.5b-instruct", AssetRole::CleanupPunctuation);
        create_file(&asset_path);
        let settings = LocalModelSettings {
            cleanup_mode: CleanupMode::FullCleanup,
            ..LocalModelSettings::default()
        };

        let error = super::resolve_cleanup_model_path_for_settings(
            &settings,
            None,
            &manifest,
            Some(&storage),
        )
        .expect_err("full cleanup should require a cleanup_full asset");

        assert!(error.contains("no full cleanup asset is configured"));
    }

    #[test]
    fn cleanup_uses_bundled_path_only_when_manifest_is_empty() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let resources = InferenceResourcePaths::from_resource_root_for_arch(
            tempdir.path().to_path_buf(),
            CpuArchitecture::Aarch64,
        );
        create_file(&resources.cleanup_model_path);
        let settings = LocalModelSettings {
            cleanup_mode: CleanupMode::PunctuationOnly,
            ..LocalModelSettings::default()
        };
        let manifest = empty_manifest();

        let path = super::resolve_cleanup_model_path_for_settings(
            &settings,
            Some(&resources),
            &manifest,
            None,
        )
        .expect("bundled cleanup path");

        assert_eq!(path, resources.cleanup_model_path);
    }

    #[test]
    fn cleanup_mode_off_disables_cleanup_slot() {
        let manager = test_manager(
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(0)),
            Arc::new(Mutex::new(Vec::new())),
        );
        let settings = LocalModelSettings {
            cleanup_mode: CleanupMode::Off,
            ..LocalModelSettings::default()
        };

        let manifest = empty_manifest();
        super::sync_cleanup_for_settings(&manager, &settings, None, &manifest, None);

        assert_eq!(
            manager.cleanup().status().state,
            InferenceRuntimeState::Disabled
        );
        assert!(!manager.cleanup().snapshot().loaded);
        manager.shutdown().expect("shutdown");
    }

    #[test]
    fn recognition_language_sync_rearms_asr_without_loading() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let resources = InferenceResourcePaths::from_resource_root_for_arch(
            tempdir.path().to_path_buf(),
            CpuArchitecture::Aarch64,
        );
        create_file(&resources.asr_model_path);
        let asr_loads = Arc::new(AtomicUsize::new(0));
        let seen_asr_configs = Arc::new(Mutex::new(Vec::new()));
        let manager = test_manager(
            Arc::clone(&asr_loads),
            Arc::new(AtomicUsize::new(0)),
            Arc::clone(&seen_asr_configs),
        );

        let manifest = empty_manifest();
        super::sync_asr_for_settings(
            &manager,
            &LocalModelSettings {
                recognition_language: RecognitionLanguage::En,
                ..LocalModelSettings::default()
            },
            Some(&resources),
            &manifest,
            None,
        );
        let first_generation = manager.asr().snapshot().generation;
        super::sync_asr_for_settings(
            &manager,
            &LocalModelSettings {
                recognition_language: RecognitionLanguage::Zh,
                ..LocalModelSettings::default()
            },
            Some(&resources),
            &manifest,
            None,
        );

        assert_eq!(manager.asr().snapshot().generation, first_generation + 1);
        assert!(!manager.asr().snapshot().loaded);
        assert_eq!(asr_loads.load(Ordering::SeqCst), 0);

        let output = manager
            .asr()
            .request(AsrInferenceRequest { audio: vec![0.1] })
            .expect("asr request");
        assert_eq!(output.transcript, "zh");
        assert_eq!(asr_loads.load(Ordering::SeqCst), 1);
        assert_eq!(
            seen_asr_configs
                .lock()
                .expect("seen asr configs")
                .last()
                .expect("loaded config")
                .language
                .as_deref(),
            Some("zh")
        );
        manager.shutdown().expect("shutdown");
    }

    #[test]
    fn managed_cleanup_runtime_enabled_for_punctuation_without_backend_override() {
        let settings = LocalModelSettings {
            cleanup_mode: CleanupMode::PunctuationOnly,
            ..LocalModelSettings::default()
        };

        assert!(managed_cleanup_runtime_enabled_for_backend(&settings, None));
    }

    #[test]
    fn managed_cleanup_runtime_enabled_false_when_cleanup_off() {
        let settings = LocalModelSettings {
            cleanup_mode: CleanupMode::Off,
            ..LocalModelSettings::default()
        };

        assert!(!managed_cleanup_runtime_enabled_for_backend(
            &settings, None
        ));
    }

    #[test]
    fn managed_cleanup_runtime_enabled_false_for_ollama_backend() {
        let settings = LocalModelSettings {
            cleanup_mode: CleanupMode::PunctuationOnly,
            ..LocalModelSettings::default()
        };

        assert!(!managed_cleanup_runtime_enabled_for_backend(
            &settings,
            Some("ollama")
        ));
    }

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
