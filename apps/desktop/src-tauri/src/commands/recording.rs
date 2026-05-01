use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager, State};
use wispergo_core::audio::{trim_silence, VadConfig};
use wispergo_core::domain::PipelineResult;
use wispergo_core::ollama::{OllamaCleanupProvider, DEFAULT_OLLAMA_MODEL};
use wispergo_core::providers::{
    AsrOutput, AsrProvider, CleanupInput, ProviderError, TextCleanupProvider,
};
use wispergo_core::whisper_sidecar::WhisperSidecarProvider;

use crate::audio::{capture_stats, AudioCaptureStats};
use crate::inference::cleanup_runtime::CleanupRuntimeManager;
use crate::inference::resources::InferenceResourcePaths;
use crate::insertion::clipboard::{insert_text_detailed, InsertionDiagnostics, InsertionResult};
use crate::state::{AppState, CleanupMode, LocalModelSettings, RecordingStatus};

#[derive(Debug, serde::Serialize)]
pub struct StopRecordingOutput {
    pub result: PipelineResult,
    pub insertion: InsertionResult,
}

const PUNCTUATION_CLEANUP_TIMEOUT: Duration = Duration::from_millis(1200);
const FULL_CLEANUP_TIMEOUT: Duration = Duration::from_secs(3);

struct ProcessedRecording {
    result: PipelineResult,
    timings: ProcessRecordingTimings,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessRecordingTimings {
    sample_count: usize,
    duration_ms: u64,
    peak: f32,
    rms: f32,
    capture_ms: u128,
    trim_ms: u128,
    asr_ms: u128,
    cleanup_ms: u128,
    total_ms: u128,
    cleanup_mode: CleanupMode,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordingTimingDiagnostics {
    timestamp_ms: u128,
    reason: String,
    cleanup_mode: CleanupMode,
    sample_count: usize,
    duration_ms: u64,
    peak: f32,
    rms: f32,
    stop_ms: u128,
    capture_ms: u128,
    trim_ms: u128,
    asr_ms: u128,
    cleanup_ms: u128,
    process_ms: u128,
    insertion_ms: u128,
    total_ms: u128,
}

#[tauri::command]
pub fn start_recording(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    state.start_recording(&mode)
}

#[tauri::command]
pub async fn stop_recording(
    app: AppHandle,
    state: State<'_, AppState>,
    cleanup_runtime: State<'_, CleanupRuntimeManager>,
    reason: String,
) -> Result<StopRecordingOutput, String> {
    let total_start = Instant::now();
    let stop_start = Instant::now();
    let audio = state.stop_recording(&reason)?;
    let stop_ms = stop_start.elapsed().as_millis();
    let bundled_resources = bundled_inference_resources(&app);
    let cleanup_provider = cleanup_provider_for_recording(cleanup_runtime.inner());

    let process_start = Instant::now();
    let processed = process_recording(
        audio,
        state.local_model_settings(),
        bundled_resources.as_ref(),
        cleanup_provider.as_deref(),
    )
    .await?;
    let process_ms = process_start.elapsed().as_millis();
    let result = processed.result;

    let insertion_start = Instant::now();
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
    let insertion_ms = insertion_start.elapsed().as_millis();
    let total_ms = total_start.elapsed().as_millis();
    let timing_diagnostics = RecordingTimingDiagnostics {
        timestamp_ms: current_timestamp_ms(),
        reason: reason.clone(),
        cleanup_mode: processed.timings.cleanup_mode,
        sample_count: processed.timings.sample_count,
        duration_ms: processed.timings.duration_ms,
        peak: processed.timings.peak,
        rms: processed.timings.rms,
        stop_ms,
        capture_ms: processed.timings.capture_ms,
        trim_ms: processed.timings.trim_ms,
        asr_ms: processed.timings.asr_ms,
        cleanup_ms: processed.timings.cleanup_ms,
        process_ms,
        insertion_ms,
        total_ms,
    };
    log_recording_timing_diagnostics(&app, &timing_diagnostics);
    eprintln!(
        "wispergo timing: stop_recording reason={} stop_ms={} process_ms={} insertion_ms={} total_ms={}",
        reason, stop_ms, process_ms, insertion_ms, total_ms
    );

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

fn bundled_inference_resources(app: &AppHandle) -> Option<InferenceResourcePaths> {
    match app.path().resource_dir() {
        Ok(resource_root) => Some(InferenceResourcePaths::from_resource_root(resource_root)),
        Err(err) => {
            eprintln!("bundled inference resource directory unavailable: {err}");
            None
        }
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

fn log_recording_timing_diagnostics(app: &AppHandle, diagnostics: &RecordingTimingDiagnostics) {
    let Ok(app_data_dir) = app.path().app_data_dir() else {
        eprintln!("recording timing log failed: app data directory is unavailable");
        return;
    };

    if let Err(err) = append_recording_timing_diagnostics(&app_data_dir, diagnostics) {
        eprintln!("recording timing log failed: {err}");
    }
}

fn append_insertion_diagnostics(
    app_data_dir: &Path,
    diagnostics: &InsertionDiagnostics,
) -> Result<PathBuf, String> {
    append_json_line(
        app_data_dir,
        "insertion-diagnostics.log",
        &InsertionDiagnosticLogRecord {
            timestamp_ms: current_timestamp_ms(),
            diagnostics,
        },
    )
}

fn append_recording_timing_diagnostics(
    app_data_dir: &Path,
    diagnostics: &RecordingTimingDiagnostics,
) -> Result<PathBuf, String> {
    append_json_line(app_data_dir, "recording-timings.log", diagnostics)
}

fn append_json_line<T: serde::Serialize>(
    app_data_dir: &Path,
    file_name: &str,
    record: &T,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(app_data_dir).map_err(|err| err.to_string())?;
    let log_path = app_data_dir.join(file_name);
    let json = serde_json::to_string(record).map_err(|err| err.to_string())?;
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
    use crate::inference::resources::{CpuArchitecture, InferenceResourcePaths};
    use crate::insertion::clipboard::{
        FocusedTargetMetadata, FocusedTextTarget, InsertionDiagnostics, InsertionResult,
        InsertionStepStatus,
    };
    use wispergo_core::domain::{PipelineResult, ProviderSource};
    use wispergo_core::providers::{
        AsrOutput, CleanupOutput, FakeTextCleanupProvider, ProviderError, TextCleanupProvider,
    };

    use crate::state::LocalModelSettings;
    use crate::state::{AppState, CleanupMode, RecordingSession, RecordingStatus};

    fn create_file(path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, "test asset").expect("write test asset");
    }

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
    fn configured_asr_paths_are_used_when_bundled_resources_are_unavailable() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let whisper_binary_path = tempdir.path().join("whisper-cli");
        let whisper_model_path = tempdir.path().join("model.bin");
        create_file(&whisper_binary_path);
        create_file(&whisper_model_path);

        let paths = super::resolve_asr_paths_with_sources(
            &LocalModelSettings {
                whisper_binary_path: Some(whisper_binary_path.display().to_string()),
                whisper_model_path: Some(whisper_model_path.display().to_string()),
                recognition_language: crate::state::RecognitionLanguage::Auto,
                cleanup_mode: CleanupMode::PunctuationOnly,
            },
            None,
            None,
            None,
        )
        .expect("resolve paths");

        assert_eq!(paths.binary_path, whisper_binary_path);
        assert_eq!(paths.model_path, whisper_model_path);
    }

    #[test]
    fn bundled_asr_paths_are_used_when_settings_and_env_are_empty() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let resources = InferenceResourcePaths::from_resource_root_for_arch(
            tempdir.path().to_path_buf(),
            CpuArchitecture::Aarch64,
        );
        create_file(&resources.whisper_binary_path);
        create_file(&resources.asr_model_path);

        let paths = super::resolve_asr_paths_with_sources(
            &LocalModelSettings {
                whisper_binary_path: None,
                whisper_model_path: None,
                recognition_language: crate::state::RecognitionLanguage::Auto,
                cleanup_mode: CleanupMode::PunctuationOnly,
            },
            Some(&resources),
            None,
            None,
        )
        .expect("resolve paths");

        assert_eq!(paths.binary_path, resources.whisper_binary_path);
        assert_eq!(paths.model_path, resources.asr_model_path);
    }

    #[test]
    fn environment_asr_paths_override_bundled_assets() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let resources = InferenceResourcePaths::from_resource_root_for_arch(
            tempdir.path().to_path_buf(),
            CpuArchitecture::Aarch64,
        );
        create_file(&resources.whisper_binary_path);
        create_file(&resources.asr_model_path);

        let paths = super::resolve_asr_paths_with_sources(
            &LocalModelSettings {
                whisper_binary_path: None,
                whisper_model_path: None,
                recognition_language: crate::state::RecognitionLanguage::Auto,
                cleanup_mode: CleanupMode::PunctuationOnly,
            },
            Some(&resources),
            Some(PathBuf::from("/custom/whisper-cli")),
            Some(PathBuf::from("/custom/model.bin")),
        )
        .expect("resolve paths");

        assert_eq!(paths.binary_path, PathBuf::from("/custom/whisper-cli"));
        assert_eq!(paths.model_path, PathBuf::from("/custom/model.bin"));
    }

    #[test]
    fn stale_persisted_asr_settings_fall_back_to_bundled_assets() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let resources = InferenceResourcePaths::from_resource_root_for_arch(
            tempdir.path().to_path_buf(),
            CpuArchitecture::Aarch64,
        );
        create_file(&resources.whisper_binary_path);
        create_file(&resources.asr_model_path);

        let paths = super::resolve_asr_paths_with_sources(
            &LocalModelSettings {
                whisper_binary_path: Some("/missing/legacy-whisper-cli".to_string()),
                whisper_model_path: Some("/missing/legacy-model.bin".to_string()),
                recognition_language: crate::state::RecognitionLanguage::Auto,
                cleanup_mode: CleanupMode::PunctuationOnly,
            },
            Some(&resources),
            None,
            None,
        )
        .expect("resolve paths");

        assert_eq!(paths.binary_path, resources.whisper_binary_path);
        assert_eq!(paths.model_path, resources.asr_model_path);
    }

    #[test]
    fn partial_model_override_uses_bundled_binary() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let resources = InferenceResourcePaths::from_resource_root_for_arch(
            tempdir.path().to_path_buf(),
            CpuArchitecture::Aarch64,
        );
        create_file(&resources.whisper_binary_path);
        create_file(&resources.asr_model_path);

        let paths = super::resolve_asr_paths_with_sources(
            &LocalModelSettings {
                whisper_binary_path: None,
                whisper_model_path: None,
                recognition_language: crate::state::RecognitionLanguage::Auto,
                cleanup_mode: CleanupMode::PunctuationOnly,
            },
            Some(&resources),
            None,
            Some(PathBuf::from("/custom/model.bin")),
        )
        .expect("resolve paths");

        assert_eq!(paths.binary_path, resources.whisper_binary_path);
        assert_eq!(paths.model_path, PathBuf::from("/custom/model.bin"));
    }

    #[test]
    fn partial_binary_env_override_uses_bundled_model() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let resources = InferenceResourcePaths::from_resource_root_for_arch(
            tempdir.path().to_path_buf(),
            CpuArchitecture::Aarch64,
        );
        create_file(&resources.whisper_binary_path);
        create_file(&resources.asr_model_path);

        let paths = super::resolve_asr_paths_with_sources(
            &LocalModelSettings {
                whisper_binary_path: None,
                whisper_model_path: None,
                recognition_language: crate::state::RecognitionLanguage::Auto,
                cleanup_mode: CleanupMode::PunctuationOnly,
            },
            Some(&resources),
            Some(PathBuf::from("/custom/whisper-cli")),
            None,
        )
        .expect("resolve paths");

        assert_eq!(paths.binary_path, PathBuf::from("/custom/whisper-cli"));
        assert_eq!(paths.model_path, resources.asr_model_path);
    }

    #[test]
    fn missing_bundled_asr_assets_return_damaged_install_error() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let resources = InferenceResourcePaths::from_resource_root_for_arch(
            tempdir.path().to_path_buf(),
            CpuArchitecture::Aarch64,
        );

        let error = super::resolve_asr_paths_with_sources(
            &LocalModelSettings {
                whisper_binary_path: None,
                whisper_model_path: None,
                recognition_language: crate::state::RecognitionLanguage::Auto,
                cleanup_mode: CleanupMode::PunctuationOnly,
            },
            Some(&resources),
            None,
            None,
        )
        .expect_err("missing bundled assets");

        assert!(error.contains("Wispergo installation is missing bundled ASR assets"));
        assert!(error.contains("bin/macos-aarch64/whisper-cli"));
        assert!(error.contains("models/asr/ggml-large-v3-turbo.bin"));
    }

    #[test]
    fn chinese_language_maps_to_whisper_code() {
        assert_eq!(
            crate::state::RecognitionLanguage::Zh.whisper_code(),
            Some("zh")
        );
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
    fn recording_pipeline_branches_on_cleanup_mode() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/recording.rs"),
        )
        .expect("recording source");

        assert!(source.contains("CleanupMode::Off => raw_result"));
        assert!(source.contains("CleanupMode::PunctuationOnly"));
        assert!(source.contains("clean_punctuation_only"));
        assert!(source.contains("CleanupMode::FullCleanup"));
        assert!(source.contains(".clean(CleanupInput"));
    }

    #[test]
    fn recording_pipeline_logs_stage_timings_and_skips_ollama_for_cleanup_off() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/recording.rs"),
        )
        .expect("recording source");

        assert!(source.contains("wispergo timing: stop_recording"));
        assert!(source.contains("wispergo timing: process_recording"));
        assert!(source.contains("cleanup_mode={:?}"));
        assert!(source.contains("recording-timings.log"));
        assert!(source
            .contains("CleanupMode::Off => apply_cleanup_mode(asr, CleanupMode::Off, None).await"));
    }

    #[tokio::test]
    async fn punctuation_cleanup_uses_text_cleanup_provider() {
        let provider = FakeTextCleanupProvider::new(
            Ok("Hello, world.".to_string()),
            Ok(CleanupOutput {
                result: PipelineResult::InsertText {
                    text: "unused".to_string(),
                    source: ProviderSource::Local,
                    confidence: None,
                },
            }),
        );
        let asr = AsrOutput {
            transcript: "hello world".to_string(),
            confidence: Some(0.82),
            source: ProviderSource::Local,
        };

        let result = super::apply_cleanup_mode(
            asr,
            CleanupMode::PunctuationOnly,
            Some(&provider as &dyn TextCleanupProvider),
        )
        .await;

        assert_eq!(
            result,
            PipelineResult::InsertText {
                text: "Hello, world.".to_string(),
                source: ProviderSource::Local,
                confidence: Some(0.82),
            }
        );
    }

    #[tokio::test]
    async fn punctuation_cleanup_timeout_falls_back_to_raw_asr() {
        let provider = FakeTextCleanupProvider::new(
            Err(ProviderError::Timeout {
                provider: "fake".to_string(),
            }),
            Ok(CleanupOutput {
                result: PipelineResult::InsertText {
                    text: "unused".to_string(),
                    source: ProviderSource::Local,
                    confidence: None,
                },
            }),
        );
        let asr = AsrOutput {
            transcript: "hello world".to_string(),
            confidence: Some(0.82),
            source: ProviderSource::Local,
        };

        let result = super::apply_cleanup_mode(
            asr,
            CleanupMode::PunctuationOnly,
            Some(&provider as &dyn TextCleanupProvider),
        )
        .await;

        assert_eq!(
            result,
            PipelineResult::InsertText {
                text: "hello world".to_string(),
                source: ProviderSource::Local,
                confidence: Some(0.82),
            }
        );
    }

    #[tokio::test]
    async fn punctuation_cleanup_invalid_output_falls_back_to_raw_asr() {
        let provider = FakeTextCleanupProvider::new(
            Err(ProviderError::InvalidOutput {
                provider: "fake".to_string(),
                message: "invalid".to_string(),
            }),
            Ok(CleanupOutput {
                result: PipelineResult::InsertText {
                    text: "unused".to_string(),
                    source: ProviderSource::Local,
                    confidence: None,
                },
            }),
        );
        let asr = AsrOutput {
            transcript: "hello world".to_string(),
            confidence: Some(0.82),
            source: ProviderSource::Local,
        };

        let result = super::apply_cleanup_mode(
            asr,
            CleanupMode::PunctuationOnly,
            Some(&provider as &dyn TextCleanupProvider),
        )
        .await;

        assert_eq!(
            result,
            PipelineResult::InsertText {
                text: "hello world".to_string(),
                source: ProviderSource::Local,
                confidence: Some(0.82),
            }
        );
    }

    #[tokio::test]
    async fn punctuation_cleanup_without_provider_falls_back_to_raw_asr() {
        let asr = AsrOutput {
            transcript: "hello world".to_string(),
            confidence: Some(0.82),
            source: ProviderSource::Local,
        };

        let result = super::apply_cleanup_mode(asr, CleanupMode::PunctuationOnly, None).await;

        assert_eq!(
            result,
            PipelineResult::InsertText {
                text: "hello world".to_string(),
                source: ProviderSource::Local,
                confidence: Some(0.82),
            }
        );
    }

    #[test]
    fn punctuation_cleanup_uses_short_timeout_and_skips_when_already_punctuated() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/recording.rs"),
        )
        .expect("recording source");

        assert!(source.contains("PUNCTUATION_CLEANUP_TIMEOUT"));
        assert!(source.contains("Duration::from_millis(1200)"));
        assert!(source.contains("FULL_CLEANUP_TIMEOUT"));
        assert!(source.contains("looks_reasonably_punctuated(&asr.transcript)"));
    }

    #[test]
    fn detects_reasonably_punctuated_transcripts() {
        assert!(super::looks_reasonably_punctuated("Hello, world."));
        assert!(super::looks_reasonably_punctuated("你好世界。"));
        assert!(super::looks_reasonably_punctuated("真的嗎？"));
        assert!(!super::looks_reasonably_punctuated("hello world"));
        assert!(!super::looks_reasonably_punctuated("你好世界"));
    }

    #[test]
    fn appends_recording_timing_diagnostics_json_line() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wispergo-timings-test-{unique}"));
        let diagnostics = super::RecordingTimingDiagnostics {
            timestamp_ms: 123,
            reason: "global_shortcut".to_string(),
            cleanup_mode: CleanupMode::PunctuationOnly,
            sample_count: 16_000,
            duration_ms: 1_000,
            peak: 0.5,
            rms: 0.1,
            stop_ms: 1,
            capture_ms: 2,
            trim_ms: 3,
            asr_ms: 4,
            cleanup_ms: 5,
            process_ms: 6,
            insertion_ms: 7,
            total_ms: 8,
        };

        let log_path =
            super::append_recording_timing_diagnostics(&dir, &diagnostics).expect("append");
        let log = std::fs::read_to_string(&log_path).expect("read log");
        let value: serde_json::Value = serde_json::from_str(log.trim()).expect("json line");

        assert_eq!(log_path, dir.join("recording-timings.log"));
        assert_eq!(value["cleanupMode"], "punctuation_only");
        assert_eq!(value["asrMs"], 4);
        assert_eq!(value["cleanupMs"], 5);
        assert_eq!(value["insertionMs"], 7);
        assert!(value.get("transcript").is_none());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ollama_cleanup_provider_uses_default_model_without_env_override() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/recording.rs"),
        )
        .expect("recording source");

        assert!(source.contains("DEFAULT_OLLAMA_MODEL.to_string()"));
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
    bundled_resources: Option<&InferenceResourcePaths>,
    cleanup_provider: Option<&dyn TextCleanupProvider>,
) -> Result<ProcessedRecording, String> {
    let total_start = Instant::now();
    let capture_start = Instant::now();
    let capture = capture_stats(&audio);
    let capture_ms = capture_start.elapsed().as_millis();
    eprintln!(
        "wispergo audio capture: samples={} duration_ms={} peak={:.4} rms={:.4}",
        capture.sample_count, capture.duration_ms, capture.peak, capture.rms
    );

    let trim_start = Instant::now();
    let audio = trim_silence(&audio, dictation_vad_config());
    let trim_ms = trim_start.elapsed().as_millis();
    if audio.is_empty() {
        return Err(no_speech_error(capture));
    }

    let asr_start = Instant::now();
    let asr = local_asr_provider(&settings, bundled_resources)?
        .transcribe(audio)
        .await
        .map_err(provider_error_message)?;
    let asr_ms = asr_start.elapsed().as_millis();

    let cleanup_mode = settings.cleanup_mode;
    let cleanup_start = Instant::now();
    let result = match cleanup_mode {
        CleanupMode::Off => apply_cleanup_mode(asr, CleanupMode::Off, None).await,
        CleanupMode::PunctuationOnly | CleanupMode::FullCleanup => {
            apply_cleanup_mode(asr, cleanup_mode, cleanup_provider).await
        }
    };
    let cleanup_ms = cleanup_start.elapsed().as_millis();
    let total_ms = total_start.elapsed().as_millis();
    eprintln!(
        "wispergo timing: process_recording capture_ms={} trim_ms={} asr_ms={} cleanup_ms={} total_ms={} cleanup_mode={:?}",
        capture_ms, trim_ms, asr_ms, cleanup_ms, total_ms, cleanup_mode
    );

    Ok(ProcessedRecording {
        result,
        timings: ProcessRecordingTimings {
            sample_count: capture.sample_count,
            duration_ms: capture.duration_ms,
            peak: capture.peak,
            rms: capture.rms,
            capture_ms,
            trim_ms,
            asr_ms,
            cleanup_ms,
            total_ms,
            cleanup_mode,
        },
    })
}

async fn apply_cleanup_mode(
    asr: AsrOutput,
    cleanup_mode: CleanupMode,
    cleanup: Option<&dyn TextCleanupProvider>,
) -> PipelineResult {
    let raw_result = PipelineResult::InsertText {
        text: asr.transcript.clone(),
        source: asr.source.clone(),
        confidence: asr.confidence,
    };

    match cleanup_mode {
        CleanupMode::Off => raw_result,
        CleanupMode::PunctuationOnly => {
            if looks_reasonably_punctuated(&asr.transcript) {
                return raw_result;
            }
            let Some(cleanup) = cleanup else {
                return raw_result;
            };
            cleanup
                .clean_punctuation_only(CleanupInput {
                    transcript: asr.transcript,
                    selected_text: None,
                    timeout: PUNCTUATION_CLEANUP_TIMEOUT,
                })
                .await
                .map(|text| PipelineResult::InsertText {
                    text,
                    source: asr.source,
                    confidence: asr.confidence,
                })
                .unwrap_or(raw_result)
        }
        CleanupMode::FullCleanup => {
            let Some(cleanup) = cleanup else {
                return raw_result;
            };
            cleanup
                .clean(CleanupInput {
                    transcript: asr.transcript,
                    selected_text: None,
                    timeout: FULL_CLEANUP_TIMEOUT,
                })
                .await
                .map(|output| output.result)
                .unwrap_or(raw_result)
        }
    }
}

fn looks_reasonably_punctuated(text: &str) -> bool {
    let trimmed = text.trim_end();
    trimmed.ends_with(['.', '!', '?', '。', '！', '？'])
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

#[derive(Debug)]
struct AsrPaths {
    binary_path: PathBuf,
    model_path: PathBuf,
}

fn local_asr_provider(
    settings: &LocalModelSettings,
    bundled_resources: Option<&InferenceResourcePaths>,
) -> Result<WhisperSidecarProvider, String> {
    let paths = resolve_asr_paths_with_resources(settings, bundled_resources)?;

    Ok(
        WhisperSidecarProvider::new(paths.binary_path, Some(paths.model_path))
            .with_language(
                settings
                    .recognition_language
                    .whisper_code()
                    .map(str::to_string),
            )
            .with_timeout(Duration::from_secs(30)),
    )
}

fn resolve_asr_paths_with_resources(
    settings: &LocalModelSettings,
    bundled_resources: Option<&InferenceResourcePaths>,
) -> Result<AsrPaths, String> {
    resolve_asr_paths_with_sources(
        settings,
        bundled_resources,
        env::var_os("WISPERGO_WHISPER_BIN").map(PathBuf::from),
        env::var_os("WISPERGO_WHISPER_MODEL").map(PathBuf::from),
    )
}

fn resolve_asr_paths_with_sources(
    settings: &LocalModelSettings,
    bundled_resources: Option<&InferenceResourcePaths>,
    env_binary: Option<PathBuf>,
    env_model: Option<PathBuf>,
) -> Result<AsrPaths, String> {
    let bundled_binary_path = bundled_resources
        .filter(|resources| resources.whisper_binary_path.exists())
        .map(|resources| resources.whisper_binary_path.clone());
    let bundled_model_path = bundled_resources
        .filter(|resources| resources.asr_model_path.exists())
        .map(|resources| resources.asr_model_path.clone());

    let binary_path = env_binary
        .or(bundled_binary_path)
        .or_else(|| existing_settings_path(&settings.whisper_binary_path))
        .or_else(|| {
            if bundled_resources.is_none() {
                find_in_path("whisper-cli").or_else(|| find_in_path("whisper-cpp"))
            } else {
                None
            }
        });
    let model_path = env_model
        .or(bundled_model_path)
        .or_else(|| existing_settings_path(&settings.whisper_model_path));

    match (binary_path, model_path) {
        (Some(binary_path), Some(model_path)) => Ok(AsrPaths {
            binary_path,
            model_path,
        }),
        _ => {
            if let Some(resources) = bundled_resources {
                let mut missing = Vec::new();
                if !resources.whisper_binary_path.exists() {
                    missing.push(format_bundled_asset_path(
                        &resources.resource_root,
                        &resources.whisper_binary_path,
                    ));
                }
                if !resources.asr_model_path.exists() {
                    missing.push(format_bundled_asset_path(
                        &resources.resource_root,
                        &resources.asr_model_path,
                    ));
                }
                if !missing.is_empty() {
                    return Err(format!(
                        "Wispergo installation is missing bundled ASR assets: {}",
                        missing.join(", ")
                    ));
                }
            }

            Err("Local ASR is not configured. Reinstall Wispergo or set WISPERGO_WHISPER_BIN and WISPERGO_WHISPER_MODEL.".to_string())
        }
    }
}

fn format_bundled_asset_path(resource_root: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(resource_root)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn existing_settings_path(path: &Option<String>) -> Option<PathBuf> {
    settings_path(path).filter(|path| path.exists())
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

fn cleanup_provider_for_recording(
    cleanup_runtime: &CleanupRuntimeManager,
) -> Option<Box<dyn TextCleanupProvider>> {
    if env::var("WISPERGO_CLEANUP_BACKEND").ok().as_deref() == Some("ollama") {
        return ollama_cleanup_provider()
            .map(|provider| Box::new(provider) as Box<dyn TextCleanupProvider>);
    }

    cleanup_runtime
        .provider()
        .map(|provider| Box::new(provider) as Box<dyn TextCleanupProvider>)
}

fn ollama_cleanup_provider() -> Option<OllamaCleanupProvider> {
    let model =
        env::var("WISPERGO_OLLAMA_MODEL").unwrap_or_else(|_| DEFAULT_OLLAMA_MODEL.to_string());
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
