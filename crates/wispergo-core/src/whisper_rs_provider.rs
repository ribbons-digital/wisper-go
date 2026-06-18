//! In-process whisper.cpp ASR provider (Phase 2).
//!
//! Only compiled when the `whisper-rs` cargo feature is enabled. Implements the
//! same [`AsrProvider`](crate::providers::AsrProvider) contract as the retired
//! [`WhisperSidecarProvider`](crate::whisper_sidecar::WhisperSidecarProvider),
//! but runs whisper.cpp in-process via `whisper-rs` instead of spawning
//! `whisper-cli` per utterance.
//!
//! ## Slice 2.2 scope
//!
//! This slice delivers the provider itself: it takes `f32` PCM directly (no
//! temp WAV), holds a **persistent** `WhisperContext` loaded once and reused
//! across `transcribe` calls (the dominant latency win over the sidecar, which
//! reloaded the model every utterance), and accepts the language (Auto/EN/ZH)
//! as a context parameter matching the sidecar's `--language` mapping. The
//! lazy-load / idle-unload lifecycle is Phase 4 (`InferenceManager`); here the
//! context loads on first `transcribe` and stays resident for the provider's
//! lifetime.
//!
//! The provider is **not wired into the pipeline** in this slice (that's 2.3).
//! The `whisper-rs` cargo feature remains off by default.
//!
//! ## Concurrency
//!
//! The persistent context is held in a `Mutex`. A single `transcribe` call
//! holds the lock for its full duration (load-if-needed + create_state + full +
//! read segments), which serializes transcriptions. This is intentional and
//! correct for a single-user dictation app where only one utterance is in
//! flight at a time; the Phase 4 `InferenceManager` will own idle-unload, not
//! parallelism.

#![cfg(feature = "whisper-rs")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
};

use crate::domain::ProviderSource;
use crate::providers::{AsrOutput, AsrProvider, ProviderError};

const PROVIDER_NAME: &str = "whisper_rs";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// In-process whisper.cpp ASR provider.
///
/// Holds a persistent [`WhisperContext`] (loaded lazily on first transcription)
/// behind an `Arc<Mutex<…>>` so it can be moved into a blocking task. The model
/// path and language are fixed at construction; switching language or model
/// rebuilds the provider. This mirrors the sidecar's `with_language` /
/// `with_timeout` builder shape.
pub struct WhisperRsProvider {
    model_path: PathBuf,
    language: Option<String>,
    timeout: Duration,
    context: Arc<Mutex<Option<WhisperContext>>>,
}

impl std::fmt::Debug for WhisperRsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WhisperRsProvider")
            .field("model_path", &self.model_path)
            .field("language", &self.language)
            .field("timeout", &self.timeout)
            .field(
                "context_loaded",
                &self.context.lock().map(|c| c.is_some()).unwrap_or(false),
            )
            .finish()
    }
}

impl WhisperRsProvider {
    /// Build a provider for `model_path` with Auto language detection.
    pub fn new<P: Into<PathBuf>>(model_path: P) -> Self {
        Self {
            model_path: model_path.into(),
            language: None,
            timeout: DEFAULT_TIMEOUT,
            context: Arc::new(Mutex::new(None)),
        }
    }

    /// Set the recognition language code (`None` or `"auto"` for auto-detect,
    /// `"en"` / `"zh"` to force). Matches the sidecar's `--language` mapping.
    pub fn with_language(mut self, language_code: Option<String>) -> Self {
        self.language = normalize_language(language_code);
        self
    }

    /// Override the per-utterance transcription timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl AsrProvider for WhisperRsProvider {
    async fn transcribe(&self, audio: Vec<f32>) -> Result<AsrOutput, ProviderError> {
        if audio.is_empty() {
            return Err(ProviderError::InvalidOutput {
                provider: PROVIDER_NAME.to_string(),
                message: "empty audio".to_string(),
            });
        }

        // Model load + transcription are CPU/GPU-bound synchronous work. The
        // persistent context is shared via Arc so the blocking task can own a
        // reference without borrowing `self`.
        let context = self.context.clone();
        let model_path = self.model_path.clone();
        let language = self.language.clone();
        let timeout = self.timeout;

        let join = tokio::task::spawn_blocking(move || -> Result<String, ProviderError> {
            transcribe_blocking(&model_path, &language, &audio, &context)
        });

        let transcript = match tokio::time::timeout(timeout, join).await {
            Ok(Ok(Ok(text))) => text,
            Ok(Ok(Err(err))) => return Err(err),
            Ok(Err(join_err)) => {
                return Err(ProviderError::Failed {
                    provider: PROVIDER_NAME.to_string(),
                    message: format!("transcription task panicked: {join_err}"),
                })
            }
            Err(_) => {
                return Err(ProviderError::Timeout {
                    provider: PROVIDER_NAME.to_string(),
                })
            }
        };

        Ok(AsrOutput {
            transcript,
            confidence: None,
            source: ProviderSource::Local,
        })
    }
}

/// Synchronous transcription on a blocking thread. Loads the persistent context
/// on first call (cached in `context`), reuses it on subsequent calls, then
/// runs the full pipeline and assembles the transcript.
fn transcribe_blocking(
    model_path: &Path,
    language: &Option<String>,
    audio: &[f32],
    context: &Mutex<Option<WhisperContext>>,
) -> Result<String, ProviderError> {
    let mut guard = context.lock().expect("whisper-rs context lock");
    if guard.is_none() {
        let loaded = load_context(model_path)?;
        *guard = Some(loaded);
    }

    // Borrow the cached context for the duration of the call. The lock is held
    // until the end of this function, serializing transcriptions (see module
    // docs: intentional for single-user dictation).
    let ctx = guard
        .as_ref()
        .expect("context was just loaded or cached");
    let mut state = ctx.create_state().map_err(|err| ProviderError::Failed {
        provider: PROVIDER_NAME.to_string(),
        message: format!("failed to create whisper state: {err}"),
    })?;

    let mut full_params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    full_params.set_language(language.as_deref());
    full_params.set_translate(false);
    full_params.set_no_timestamps(true);
    full_params.set_print_progress(false);
    full_params.set_print_special(false);
    full_params.set_print_realtime(false);
    full_params.set_print_timestamps(false);

    state
        .full(full_params, audio)
        .map_err(|err| ProviderError::Failed {
            provider: PROVIDER_NAME.to_string(),
            message: format!("whisper full transcription failed: {err}"),
        })?;

    let n_segments = state.full_n_segments();
    let mut segments: Vec<String> = Vec::with_capacity(n_segments.max(0) as usize);
    for i in 0..n_segments {
        let Some(segment) = state.get_segment(i) else {
            continue;
        };
        let text = segment
            .to_str_lossy()
            .map_err(|err| ProviderError::InvalidOutput {
                provider: PROVIDER_NAME.to_string(),
                message: format!("segment text decode failed: {err}"),
            })?
            .trim()
            .to_string();
        if !text.is_empty() {
            segments.push(text);
        }
    }

    let transcript = segments.join(" ");
    if transcript.is_empty() {
        return Err(ProviderError::InvalidOutput {
            provider: PROVIDER_NAME.to_string(),
            message: "empty transcript".to_string(),
        });
    }
    if is_no_speech_transcript(&transcript) {
        return Err(ProviderError::InvalidOutput {
            provider: PROVIDER_NAME.to_string(),
            message: "no speech detected".to_string(),
        });
    }
    Ok(transcript)
}

fn load_context(model_path: &Path) -> Result<WhisperContext, ProviderError> {
    if !model_path.exists() {
        return Err(ProviderError::Unavailable {
            provider: PROVIDER_NAME.to_string(),
            message: Some(format!(
                "model file not found: {}",
                model_path.display()
            )),
        });
    }
    let params = WhisperContextParameters::default();
    WhisperContext::new_with_params(model_path.display().to_string(), params).map_err(|err| {
        ProviderError::Failed {
            provider: PROVIDER_NAME.to_string(),
            message: format!("failed to load whisper context: {err}"),
        }
    })
}

/// Normalize a raw language code to the form whisper.cpp expects, matching the
/// sidecar's `with_language` semantics: `None`, empty, or `"auto"` → `None`
/// (auto-detect); otherwise the trimmed lowercase code.
pub fn normalize_language(language_code: Option<String>) -> Option<String> {
    let code = language_code?.trim().to_string();
    if code.is_empty() || code.eq_ignore_ascii_case("auto") {
        None
    } else {
        Some(code.to_ascii_lowercase())
    }
}

/// Detect whisper.cpp's "no speech" sentinel tokens in the transcript. Shared
/// shape with the sidecar provider's detection so both providers behave
/// identically on empty/silent audio.
pub fn is_no_speech_transcript(transcript: &str) -> bool {
    let normalized = transcript.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "[blank_audio]" | "[no_speech]" | "[silence]" | "(silence)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_language_none_for_auto_empty_or_none() {
        assert_eq!(normalize_language(None), None);
        assert_eq!(normalize_language(Some(String::new())), None);
        assert_eq!(normalize_language(Some("   ".to_string())), None);
        assert_eq!(normalize_language(Some("auto".to_string())), None);
        assert_eq!(normalize_language(Some("AUTO".to_string())), None);
    }

    #[test]
    fn normalize_language_lowercases_and_trims_explicit_codes() {
        assert_eq!(normalize_language(Some("en".to_string())), Some("en".to_string()));
        assert_eq!(
            normalize_language(Some("  ZH  ".to_string())),
            Some("zh".to_string())
        );
    }

    #[test]
    fn is_no_speech_detects_sentinels_case_insensitively() {
        assert!(is_no_speech_transcript("[blank_audio]"));
        assert!(is_no_speech_transcript("[NO_SPEECH]"));
        assert!(is_no_speech_transcript("  [silence]  "));
        assert!(is_no_speech_transcript("(silence)"));
    }

    #[test]
    fn is_no_speech_does_not_match_real_transcripts() {
        assert!(!is_no_speech_transcript("hello world"));
        assert!(!is_no_speech_transcript("the silence was long"));
    }

    #[test]
    fn provider_stores_language_and_timeout_via_builders() {
        let provider = WhisperRsProvider::new("/nonexistent/model.bin")
            .with_language(Some("ZH".to_string()))
            .with_timeout(Duration::from_secs(10));

        assert_eq!(provider.language, Some("zh".to_string()));
        assert_eq!(provider.timeout, Duration::from_secs(10));
    }

    #[test]
    fn provider_defaults_to_auto_language_and_default_timeout() {
        let provider = WhisperRsProvider::new("/nonexistent/model.bin");
        assert_eq!(provider.language, None);
        assert_eq!(provider.timeout, DEFAULT_TIMEOUT);
    }

    /// Language handling relative to the retired `WhisperSidecarProvider`.
    ///
    /// Both providers receive `None` for Auto and `"en"`/`"zh"` for the explicit
    /// modes (Wispergo's `RecognitionLanguage::whisper_code()` emits exactly
    /// those). `WhisperRsProvider` additionally lowercases explicit codes so
    /// the value passed to whisper.cpp's `set_language` is always lowercase
    /// (the form whisper.cpp documents), whereas the sidecar passed the raw
    /// string to `--language` (the CLI is case-insensitive). This is an
    /// intentional, safe normalization, not a regression.
    #[test]
    fn normalizes_explicit_language_codes_to_lowercase() {
        // Wispergo never passes "auto"; it passes None for Auto.
        assert_eq!(normalize_language(None), None);
        assert_eq!(normalize_language(Some("en".to_string())), Some("en".to_string()));
        assert_eq!(normalize_language(Some("zh".to_string())), Some("zh".to_string()));
        assert_eq!(
            normalize_language(Some("  ZH  ".to_string())),
            Some("zh".to_string())
        );
    }
}
