use std::time::Duration;

use httpmock::prelude::*;
use wispergo_core::domain::{CommandAction, PipelineResult};
use wispergo_core::ollama::{parse_cleanup_json, OllamaCleanupProvider};
use wispergo_core::providers::{CleanupInput, CleanupProvider, ProviderError};

#[test]
fn parses_valid_insert_text_fixture() {
    let fixture = include_str!("fixtures/cleanup_insert_text.json");
    let output = parse_cleanup_json(fixture).expect("parse fixture");

    assert!(matches!(output.result, PipelineResult::InsertText { .. }));
}

#[test]
fn parses_valid_command_fixture() {
    let fixture = include_str!("fixtures/cleanup_command_rewrite.json");
    let output = parse_cleanup_json(fixture).expect("parse fixture");

    assert!(matches!(
        output.result,
        PipelineResult::Command {
            command: CommandAction::RewriteSelection { .. },
            ..
        }
    ));
}

#[test]
fn rejects_unknown_command_fixture() {
    let fixture = include_str!("fixtures/cleanup_invalid.json");
    let error = parse_cleanup_json(fixture).expect_err("invalid command should fail");

    assert!(matches!(error, ProviderError::InvalidOutput { .. }));
}

#[tokio::test]
async fn calls_ollama_chat_api_and_parses_json_content() {
    let server = MockServer::start();
    let body = serde_json::json!({
        "message": {
            "content": include_str!("fixtures/cleanup_insert_text.json")
        }
    });

    let mock = server.mock(|when, then| {
        when.method(POST).path("/api/chat");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });

    let provider = OllamaCleanupProvider::new(server.base_url(), "llama3.2:3b".to_string());
    let output = provider
        .clean(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_secs(2),
        })
        .await
        .expect("cleanup output");

    mock.assert();
    assert!(matches!(output.result, PipelineResult::InsertText { .. }));
}
