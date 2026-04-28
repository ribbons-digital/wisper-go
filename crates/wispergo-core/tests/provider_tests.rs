use std::sync::{Arc, Mutex};
use std::time::Duration;

use wispergo_core::domain::{CommandAction, CommandSource, PipelineResult, ProviderSource};
use wispergo_core::providers::{
    AsrOutput, AsrProvider, CleanupInput, CleanupOutput, CleanupProvider, FakeAsrProvider,
    FakeCleanupProvider, ProviderError,
};

#[tokio::test]
async fn fake_asr_returns_configured_transcript() {
    let provider = FakeAsrProvider::new(Ok(AsrOutput {
        transcript: "hello world".to_string(),
        confidence: Some(0.8),
        source: ProviderSource::Local,
    }));

    let result = provider
        .transcribe(vec![0.0, 0.1])
        .await
        .expect("asr output");

    assert_eq!(result.transcript, "hello world");
    assert_eq!(result.source, ProviderSource::Local);
}

#[tokio::test]
async fn fake_cleanup_returns_structured_result() {
    let provider = FakeCleanupProvider::new(Ok(CleanupOutput {
        result: PipelineResult::Command {
            command: CommandAction::NewParagraph,
            requires_confirmation: false,
            source: CommandSource::LocalLlm,
        },
    }));

    let result = provider
        .clean(CleanupInput {
            transcript: "new paragraph".to_string(),
            selected_text: None,
            timeout: Duration::from_millis(500),
        })
        .await
        .expect("cleanup output");

    assert!(matches!(result.result, PipelineResult::Command { .. }));
}

#[tokio::test]
async fn provider_errors_distinguish_timeout_and_unavailable() {
    let timeout = ProviderError::Timeout {
        provider: "local_asr".to_string(),
    };
    let unavailable = ProviderError::Unavailable {
        provider: "ollama".to_string(),
        message: None,
    };

    assert!(timeout.is_recoverable());
    assert!(unavailable.is_recoverable());
}

#[tokio::test]
async fn provider_error_display_redacts_raw_diagnostics() {
    let error = ProviderError::InvalidOutput {
        provider: "ollama".to_string(),
        message: "transcript: secret words".to_string(),
    };

    assert_eq!(
        error.to_string(),
        "ollama returned invalid output; diagnostic details are redacted"
    );
    assert_eq!(error.diagnostic_message(), Some("transcript: secret words"));
}

#[tokio::test]
async fn fake_providers_record_call_counts() {
    let calls = Arc::new(Mutex::new(0));
    let provider = FakeAsrProvider::with_counter(
        Ok(AsrOutput {
            transcript: "hi".to_string(),
            confidence: None,
            source: ProviderSource::Local,
        }),
        calls.clone(),
    );

    let _ = provider.transcribe(vec![0.2]).await;

    assert_eq!(*calls.lock().expect("counter lock"), 1);
}
