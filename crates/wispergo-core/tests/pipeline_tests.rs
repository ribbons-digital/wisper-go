use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use wispergo_core::domain::{
    ActiveContext, CommandAction, CommandSource, PipelineResult, ProviderSource,
};
use wispergo_core::pipeline::{Pipeline, PipelineInput};
use wispergo_core::privacy::PrivacyPolicy;
use wispergo_core::providers::{
    AsrOutput, CleanupInput, CleanupOutput, CleanupProvider, FakeAsrProvider, FakeCleanupProvider,
    ProviderError,
};

fn context() -> ActiveContext {
    ActiveContext {
        app_id: "com.apple.Notes".to_string(),
        app_name: "Notes".to_string(),
        window_title: None,
        selected_text: None,
        style_profile: None,
    }
}

#[derive(Debug, Clone)]
struct SpyCleanupProvider {
    response: Result<CleanupOutput, ProviderError>,
    last_input: Arc<Mutex<Option<CleanupInput>>>,
}

impl SpyCleanupProvider {
    fn new(
        response: Result<CleanupOutput, ProviderError>,
        last_input: Arc<Mutex<Option<CleanupInput>>>,
    ) -> Self {
        Self {
            response,
            last_input,
        }
    }
}

#[async_trait]
impl CleanupProvider for SpyCleanupProvider {
    async fn clean(&self, input: CleanupInput) -> Result<CleanupOutput, ProviderError> {
        *self.last_input.lock().expect("spy cleanup input lock") = Some(input);
        self.response.clone()
    }
}

#[tokio::test]
async fn rule_command_skips_cleanup_provider() {
    let pipeline = Pipeline::new(
        FakeAsrProvider::new(Ok(AsrOutput {
            transcript: "new paragraph".to_string(),
            confidence: Some(0.9),
            source: ProviderSource::Local,
        })),
        FakeCleanupProvider::new(Err(ProviderError::Failed {
            provider: "cleanup".to_string(),
            message: "should not be called".to_string(),
        })),
        PrivacyPolicy::default(),
    );

    let result = pipeline
        .run(PipelineInput {
            audio: vec![0.1],
            context: context(),
            cleanup_timeout: Duration::from_millis(500),
        })
        .await;

    assert!(matches!(
        result,
        PipelineResult::Command {
            command: CommandAction::NewParagraph,
            ..
        }
    ));
}

#[tokio::test]
async fn dictation_flows_through_cleanup_provider() {
    let pipeline = Pipeline::new(
        FakeAsrProvider::new(Ok(AsrOutput {
            transcript: "hello world".to_string(),
            confidence: Some(0.9),
            source: ProviderSource::Local,
        })),
        FakeCleanupProvider::new(Ok(CleanupOutput {
            result: PipelineResult::InsertText {
                text: "Hello, world.".to_string(),
                source: ProviderSource::Local,
                confidence: Some(0.9),
            },
        })),
        PrivacyPolicy::default(),
    );

    let result = pipeline
        .run(PipelineInput {
            audio: vec![0.1],
            context: context(),
            cleanup_timeout: Duration::from_millis(500),
        })
        .await;

    assert_eq!(
        result,
        PipelineResult::InsertText {
            text: "Hello, world.".to_string(),
            source: ProviderSource::Local,
            confidence: Some(0.9)
        }
    );
}

#[tokio::test]
async fn cleanup_timeout_inserts_raw_asr_for_plain_dictation() {
    let pipeline = Pipeline::new(
        FakeAsrProvider::new(Ok(AsrOutput {
            transcript: "plain dictation".to_string(),
            confidence: Some(0.7),
            source: ProviderSource::Local,
        })),
        FakeCleanupProvider::new(Err(ProviderError::Timeout {
            provider: "ollama".to_string(),
        })),
        PrivacyPolicy::default(),
    );

    let result = pipeline
        .run(PipelineInput {
            audio: vec![0.1],
            context: context(),
            cleanup_timeout: Duration::from_millis(100),
        })
        .await;

    assert_eq!(
        result,
        PipelineResult::InsertText {
            text: "plain dictation".to_string(),
            source: ProviderSource::Local,
            confidence: Some(0.7)
        }
    );
}

#[tokio::test]
async fn cleanup_timeout_preserves_asr_source() {
    let pipeline = Pipeline::new(
        FakeAsrProvider::new(Ok(AsrOutput {
            transcript: "cloud dictation".to_string(),
            confidence: Some(0.6),
            source: ProviderSource::Cloud,
        })),
        FakeCleanupProvider::new(Err(ProviderError::Timeout {
            provider: "cleanup".to_string(),
        })),
        PrivacyPolicy::default(),
    );

    let result = pipeline
        .run(PipelineInput {
            audio: vec![0.1],
            context: context(),
            cleanup_timeout: Duration::from_millis(100),
        })
        .await;

    assert_eq!(
        result,
        PipelineResult::InsertText {
            text: "cloud dictation".to_string(),
            source: ProviderSource::Cloud,
            confidence: Some(0.6)
        }
    );
}

#[tokio::test]
async fn context_disabled_app_omits_selected_text_from_cleanup() {
    let last_input = Arc::new(Mutex::new(None));
    let pipeline = Pipeline::new(
        FakeAsrProvider::new(Ok(AsrOutput {
            transcript: "rewrite this".to_string(),
            confidence: Some(0.8),
            source: ProviderSource::Local,
        })),
        SpyCleanupProvider::new(
            Ok(CleanupOutput {
                result: PipelineResult::InsertText {
                    text: "Rewrite this.".to_string(),
                    source: ProviderSource::Local,
                    confidence: Some(0.8),
                },
            }),
            Arc::clone(&last_input),
        ),
        PrivacyPolicy {
            context_disabled_apps: vec!["com.company.SecretApp".to_string()],
            ..PrivacyPolicy::default()
        },
    );

    let mut context = context();
    context.app_id = "com.company.SecretApp".to_string();
    context.selected_text = Some("private selection".to_string());

    let _ = pipeline
        .run(PipelineInput {
            audio: vec![0.1],
            context,
            cleanup_timeout: Duration::from_millis(100),
        })
        .await;

    assert_eq!(
        last_input
            .lock()
            .expect("spy cleanup input lock")
            .as_ref()
            .expect("cleanup input")
            .selected_text,
        None
    );
}

#[tokio::test]
async fn destructive_cleanup_command_requires_confirmation() {
    let pipeline = Pipeline::new(
        FakeAsrProvider::new(Ok(AsrOutput {
            transcript: "plain dictation".to_string(),
            confidence: Some(0.9),
            source: ProviderSource::Local,
        })),
        FakeCleanupProvider::new(Ok(CleanupOutput {
            result: PipelineResult::Command {
                command: CommandAction::DeletePreviousPhrase,
                requires_confirmation: false,
                source: CommandSource::LocalLlm,
            },
        })),
        PrivacyPolicy::default(),
    );

    let result = pipeline
        .run(PipelineInput {
            audio: vec![0.1],
            context: context(),
            cleanup_timeout: Duration::from_millis(100),
        })
        .await;

    assert!(matches!(
        result,
        PipelineResult::Command {
            command: CommandAction::DeletePreviousPhrase,
            requires_confirmation: true,
            source: CommandSource::LocalLlm,
        }
    ));
}

#[tokio::test]
async fn asr_errors_become_pipeline_errors() {
    let pipeline = Pipeline::new(
        FakeAsrProvider::new(Err(ProviderError::Unavailable {
            provider: "asr".to_string(),
            message: Some("microphone denied".to_string()),
        })),
        FakeCleanupProvider::new(Err(ProviderError::Failed {
            provider: "cleanup".to_string(),
            message: "should not be called".to_string(),
        })),
        PrivacyPolicy::default(),
    );

    let result = pipeline
        .run(PipelineInput {
            audio: vec![0.1],
            context: context(),
            cleanup_timeout: Duration::from_millis(100),
        })
        .await;

    assert_eq!(
        result,
        PipelineResult::Error {
            recoverable: true,
            message: "asr is unavailable".to_string(),
        }
    );
}

#[tokio::test]
async fn cleanup_failures_become_pipeline_errors() {
    let pipeline = Pipeline::new(
        FakeAsrProvider::new(Ok(AsrOutput {
            transcript: "plain dictation".to_string(),
            confidence: Some(0.7),
            source: ProviderSource::Local,
        })),
        FakeCleanupProvider::new(Err(ProviderError::Failed {
            provider: "cleanup".to_string(),
            message: "raw diagnostic".to_string(),
        })),
        PrivacyPolicy::default(),
    );

    let result = pipeline
        .run(PipelineInput {
            audio: vec![0.1],
            context: context(),
            cleanup_timeout: Duration::from_millis(100),
        })
        .await;

    assert_eq!(
        result,
        PipelineResult::Error {
            recoverable: false,
            message: "cleanup failed; diagnostic details are redacted".to_string(),
        }
    );
}
