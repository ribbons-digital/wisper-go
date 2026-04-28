use std::time::Duration;

use wispergo_core::domain::{ActiveContext, CommandAction, PipelineResult, ProviderSource};
use wispergo_core::pipeline::{Pipeline, PipelineInput};
use wispergo_core::privacy::PrivacyPolicy;
use wispergo_core::providers::{
    AsrOutput, CleanupOutput, FakeAsrProvider, FakeCleanupProvider, ProviderError,
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
