use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager, State};
use wispergo_core::audio::{trim_silence, VadConfig};
use wispergo_core::domain::PipelineResult;
use wispergo_core::ollama::OllamaCleanupProvider;
use wispergo_core::providers::{AsrProvider, CleanupInput, CleanupProvider, ProviderError};
use wispergo_core::whisper_sidecar::WhisperSidecarProvider;

use crate::audio::{capture_stats, AudioCaptureStats};
use crate::insertion::clipboard::{insert_text_detailed, InsertionDiagnostics, InsertionResult};
use crate::state::{AppState, LocalModelSettings, RecordingStatus};

#[derive(Debug, serde::Serialize)]
pub struct StopRecordingOutput {
    pub result: PipelineResult,
    pub insertion: InsertionResult,
}

#[tauri::command]
pub fn start_recording(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    state.start_recording(&mode)
}

#[tauri::command]
pub async fn stop_recording(
    app: AppHandle,
    state: State<'_, AppState>,
    reason: String,
) -> Result<StopRecordingOutput, String> {
    let audio = state.stop_recording(&reason)?;
    let result = process_recording(audio, state.local_model_settings()).await?;
    let insertion = match &result {
        PipelineResult::InsertText { text, .. } => {
            let outcome = insert_text_detailed(text)?;
            log_insertion_diagnostics(&app, &outcome.diagnostics);
            outcome.result
        }
        PipelineResult::Command { command, .. } => {
            let outcome = insert_text_detailed(command.label())?;
            log_insertion_diagnostics(&app, &outcome.diagnostics);
            outcome.result
        }
        PipelineResult::Cancelled { .. } => InsertionResult::CopiedOnly,
        PipelineResult::Error { message, .. } => return Err(message.clone()),
    };

    Ok(StopRecordingOutput { result, insertion })
}

#[tauri::command]
pub fn cancel_recording(state: State<'_, AppState>, reason: String) -> Result<(), String> {
    state.cancel_recording(&reason)
}

#[tauri::command]
pub fn recording_status(state: State<'_, AppState>) -> &'static str {
    match state.recording_status() {
        RecordingStatus::Idle => "idle",
        RecordingStatus::Recording => "recording",
    }
}

fn log_insertion_diagnostics(app: &AppHandle, diagnostics: &InsertionDiagnostics) {
    let Ok(app_data_dir) = app.path().app_data_dir() else {
        eprintln!("insertion diagnostics log failed: app data directory is unavailable");
        return;
    };

    if let Err(err) = append_insertion_diagnostics(&app_data_dir, diagnostics) {
        eprintln!("insertion diagnostics log failed: {err}");
    }
}

fn append_insertion_diagnostics(
    app_data_dir: &Path,
    diagnostics: &InsertionDiagnostics,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(app_data_dir).map_err(|err| err.to_string())?;
    let log_path = app_data_dir.join("insertion-diagnostics.log");
    let record = InsertionDiagnosticLogRecord {
        timestamp_ms: current_timestamp_ms(),
        diagnostics,
    };
    let json = serde_json::to_string(&record).map_err(|err| err.to_string())?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|err| err.to_string())?;
    writeln!(file, "{json}").map_err(|err| err.to_string())?;
    Ok(log_path)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct InsertionDiagnosticLogRecord<'a> {
    timestamp_ms: u128,
    diagnostics: &'a InsertionDiagnostics,
}

fn current_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::audio::AudioCaptureStats;
    use crate::insertion::clipboard::{
        FocusedTargetMetadata, FocusedTextTarget, InsertionDiagnostics, InsertionResult,
        InsertionStepStatus,
    };
    use crate::state::LocalModelSettings;
    use crate::state::{AppState, RecordingSession, RecordingStatus};

    #[test]
    fn start_and_cancel_recording_update_state() {
        let state = AppState::default();

        state
            .start_recording_with_session(RecordingSession::buffered(vec![0.1]))
            .expect("start");
        assert_eq!(state.recording_status(), RecordingStatus::Recording);

        state.cancel_recording("user_cancelled").expect("cancel");
        assert_eq!(state.recording_status(), RecordingStatus::Idle);
    }

    #[test]
    fn configured_asr_paths_take_precedence() {
        let paths = super::resolve_asr_paths(&LocalModelSettings {
            whisper_binary_path: Some("/settings/whisper-cli".to_string()),
            whisper_model_path: Some("/settings/model.bin".to_string()),
        })
        .expect("resolve paths");

        assert_eq!(paths.binary_path, PathBuf::from("/settings/whisper-cli"));
        assert_eq!(paths.model_path, PathBuf::from("/settings/model.bin"));
    }

    #[test]
    fn dictation_vad_keeps_context_for_short_utterances() {
        let config = super::dictation_vad_config();

        assert_eq!(config.silence_threshold, 0.01);
        assert_eq!(config.padding_samples, 4_000);
    }

    #[test]
    fn appends_insertion_diagnostics_json_line() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wispergo-diagnostics-test-{unique}"));
        let diagnostics = InsertionDiagnostics {
            target_status: FocusedTextTarget::NoEditableTarget,
            target: FocusedTargetMetadata {
                process_id: Some(123),
                role: Some("AXGroup".to_string()),
                subrole: None,
                selected_text_settable: Some(false),
                value_settable: Some(false),
                text_selection_available: Some(false),
            },
            clipboard: InsertionStepStatus::Success,
            paste: InsertionStepStatus::NotAttempted,
            direct_insert: InsertionStepStatus::NotAttempted,
            final_result: InsertionResult::NoEditableTarget,
        };

        let log_path = super::append_insertion_diagnostics(&dir, &diagnostics).expect("append");
        let log = std::fs::read_to_string(&log_path).expect("read log");
        let value: serde_json::Value = serde_json::from_str(log.trim()).expect("json line");

        assert_eq!(log_path, dir.join("insertion-diagnostics.log"));
        assert_eq!(value["diagnostics"]["finalResult"], "no_editable_target");
        assert_eq!(value["diagnostics"]["target"]["role"], "AXGroup");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn no_speech_error_distinguishes_empty_capture_from_silence() {
        assert_eq!(
            super::no_speech_error(AudioCaptureStats {
                sample_count: 0,
                duration_ms: 0,
                peak: 0.0,
                rms: 0.0
            }),
            "No microphone audio was captured. Check microphone permission and the selected input device."
        );

        let quiet_error = super::no_speech_error(AudioCaptureStats {
            sample_count: 16_000,
            duration_ms: 1_000,
            peak: 0.001,
            rms: 0.0005,
        });
        assert!(quiet_error.contains("No speech was detected"));
        assert!(quiet_error.contains("Captured 1.0s"));
        assert!(quiet_error.contains("peak 0.0010"));
    }
}

async fn process_recording(
    audio: Vec<f32>,
    settings: LocalModelSettings,
) -> Result<PipelineResult, String> {
    let capture = capture_stats(&audio);
    eprintln!(
        "wispergo audio capture: samples={} duration_ms={} peak={:.4} rms={:.4}",
        capture.sample_count, capture.duration_ms, capture.peak, capture.rms
    );

    let audio = trim_silence(&audio, dictation_vad_config());
    if audio.is_empty() {
        return Err(no_speech_error(capture));
    }

    let asr = local_asr_provider(&settings)?
        .transcribe(audio)
        .await
        .map_err(provider_error_message)?;

    let raw_result = PipelineResult::InsertText {
        text: asr.transcript.clone(),
        source: asr.source,
        confidence: asr.confidence,
    };

    let Some(cleanup) = ollama_cleanup_provider() else {
        return Ok(raw_result);
    };

    let cleanup = cleanup
        .clean(CleanupInput {
            transcript: asr.transcript,
            selected_text: None,
            timeout: Duration::from_secs(3),
        })
        .await;

    Ok(cleanup.map(|output| output.result).unwrap_or(raw_result))
}

fn dictation_vad_config() -> VadConfig {
    VadConfig::dictation()
}

fn no_speech_error(capture: AudioCaptureStats) -> String {
    if capture.sample_count == 0 {
        return "No microphone audio was captured. Check microphone permission and the selected input device.".to_string();
    }

    format!(
        "No speech was detected in the microphone audio. Captured {:.1}s with peak {:.4} and RMS {:.4}; check the selected input device and input level.",
        capture.duration_ms as f32 / 1_000.0,
        capture.peak,
        capture.rms
    )
}

struct AsrPaths {
    binary_path: PathBuf,
    model_path: PathBuf,
}

fn local_asr_provider(settings: &LocalModelSettings) -> Result<WhisperSidecarProvider, String> {
    let paths = resolve_asr_paths(settings)?;

    Ok(
        WhisperSidecarProvider::new(paths.binary_path, Some(paths.model_path))
            .with_timeout(Duration::from_secs(30)),
    )
}

fn resolve_asr_paths(settings: &LocalModelSettings) -> Result<AsrPaths, String> {
    let binary_path = settings_path(&settings.whisper_binary_path)
        .or_else(|| env::var_os("WISPERGO_WHISPER_BIN").map(PathBuf::from))
        .or_else(|| find_in_path("whisper-cli"))
        .or_else(|| find_in_path("whisper-cpp"))
        .ok_or_else(|| {
        "Local ASR is not configured. Set WISPERGO_WHISPER_BIN to a whisper.cpp compatible binary and WISPERGO_WHISPER_MODEL to a local model path.".to_string()
    })?;
    let model_path = settings_path(&settings.whisper_model_path)
        .or_else(|| env::var_os("WISPERGO_WHISPER_MODEL").map(PathBuf::from))
        .ok_or_else(|| {
            "Local ASR model is not configured. Set WISPERGO_WHISPER_MODEL to a local whisper.cpp model path.".to_string()
        })?;

    Ok(AsrPaths {
        binary_path,
        model_path,
    })
}

fn settings_path(path: &Option<String>) -> Option<PathBuf> {
    let path = path.as_ref()?.trim();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn find_in_path(binary: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

fn ollama_cleanup_provider() -> Option<OllamaCleanupProvider> {
    let model = env::var("WISPERGO_OLLAMA_MODEL").ok()?;
    let base_url = env::var("WISPERGO_OLLAMA_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    Some(OllamaCleanupProvider::new(base_url, model))
}

fn provider_error_message(err: ProviderError) -> String {
    match err.diagnostic_message() {
        Some(message) if !message.trim().is_empty() => format!("{err}: {}", message.trim()),
        _ => err.to_string(),
    }
}
