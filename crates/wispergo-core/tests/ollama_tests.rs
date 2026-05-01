use std::time::Duration;

use httpmock::prelude::*;
use wispergo_core::domain::{CommandAction, CommandSource, PipelineResult};
use wispergo_core::ollama::{
    parse_cleanup_json, parse_punctuation_cleanup_text, OllamaCleanupProvider, DEFAULT_OLLAMA_MODEL,
};
use wispergo_core::providers::{CleanupInput, CleanupProvider, ProviderError, TextCleanupProvider};

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

#[test]
fn parses_punctuation_only_plain_text_response() {
    let output =
        parse_punctuation_cleanup_text("  Hello, world.\n").expect("parse punctuation response");

    assert_eq!(output, "Hello, world.");
}

#[test]
fn strips_echoed_transcript_label_from_punctuation_cleanup_output() {
    let output = parse_punctuation_cleanup_text("Transcript: Hello, world.")
        .expect("parse punctuation response");

    assert_eq!(output, "Hello, world.");
}

#[test]
fn rejects_empty_punctuation_only_plain_text_response() {
    let error =
        parse_punctuation_cleanup_text(" \n ").expect_err("empty punctuation output should fail");

    assert!(matches!(error, ProviderError::InvalidOutput { provider, .. } if provider == "ollama"));
}

#[tokio::test]
async fn calls_ollama_chat_api_for_punctuation_only_cleanup() {
    let server = MockServer::start();
    let body = serde_json::json!({
        "message": {
            "content": "Hello, world."
        }
    });

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/chat")
            .body_contains(DEFAULT_OLLAMA_MODEL)
            .body_contains("Punctuation-only cleanup")
            .body_contains("Return only the corrected transcript as plain text")
            .body_contains("Preserve the exact words, language, and script")
            .body_contains("Do not translate, paraphrase")
            .body_contains("Transcript: hello world");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });

    let provider = OllamaCleanupProvider::new(server.base_url(), DEFAULT_OLLAMA_MODEL.to_string());
    let output = provider
        .clean_punctuation_only(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: Some("ignored selection".to_string()),
            timeout: Duration::from_secs(2),
        })
        .await
        .expect("punctuation cleanup output");

    mock.assert();
    assert_eq!(output, "Hello, world.");
}

#[tokio::test]
async fn warms_ollama_model_with_short_plain_text_probe() {
    let server = MockServer::start();
    let body = serde_json::json!({
        "message": {
            "content": "OK"
        }
    });

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/chat")
            .body_contains(DEFAULT_OLLAMA_MODEL)
            .body_contains("Reply with OK only")
            .body_contains("OK");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });

    let provider = OllamaCleanupProvider::new(server.base_url(), DEFAULT_OLLAMA_MODEL.to_string());
    provider
        .warm(Duration::from_secs(2))
        .await
        .expect("warmup should succeed");

    mock.assert();
}

#[tokio::test]
async fn rejects_non_success_http_status() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/api/chat");
        then.status(500)
            .header("content-type", "application/json")
            .json_body(serde_json::json!({
                "message": {
                    "content": include_str!("fixtures/cleanup_insert_text.json")
                }
            }));
    });

    let provider = OllamaCleanupProvider::new(server.base_url(), "llama3.2:3b".to_string());
    let error = provider
        .clean(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_secs(2),
        })
        .await
        .expect_err("non-success status should fail");

    mock.assert();
    assert!(matches!(error, ProviderError::Failed { provider, .. } if provider == "ollama"));
}

#[test]
fn destructive_model_commands_require_confirmation() {
    let output = parse_cleanup_json(
        r#"{
          "result": {
            "kind": "command",
            "command": { "kind": "delete_previous_phrase" },
            "requires_confirmation": false,
            "source": "local_llm"
          }
        }"#,
    )
    .expect("parse output");

    assert_eq!(
        output.result,
        PipelineResult::Command {
            command: CommandAction::DeletePreviousPhrase,
            requires_confirmation: true,
            source: CommandSource::LocalLlm,
        }
    );
}

#[tokio::test]
async fn reqwest_errors_keep_provider_name_stable() {
    let provider = OllamaCleanupProvider::new(
        "http://127.0.0.1:1/private-endpoint".to_string(),
        "llama3.2:3b".to_string(),
    );
    let error = provider
        .clean(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_millis(50),
        })
        .await
        .expect_err("connection should fail");

    assert!(matches!(error, ProviderError::Unavailable { provider, .. } if provider == "ollama"));
}
