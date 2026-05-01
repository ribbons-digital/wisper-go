use std::time::Duration;

use httpmock::prelude::*;
use wispergo_core::domain::{PipelineResult, ProviderSource};
use wispergo_core::llama_server::{
    parse_punctuation_cleanup_text, LlamaServerCleanupProvider, DEFAULT_LLAMA_SERVER_MODEL,
};
use wispergo_core::providers::{CleanupInput, CleanupProvider, ProviderError, TextCleanupProvider};

#[tokio::test]
async fn calls_openai_chat_endpoint_for_punctuation_cleanup() {
    let server = MockServer::start();
    let body = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": "Hello, world."
                }
            }
        ]
    });

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/chat/completions")
            .body_contains(DEFAULT_LLAMA_SERVER_MODEL)
            .body_contains("Punctuation-only cleanup")
            .body_contains("Return only the corrected transcript as plain text")
            .body_contains("Preserve the exact words, language, and script")
            .body_contains("Do not translate, paraphrase")
            .body_contains("Transcript: hello world");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });

    let provider =
        LlamaServerCleanupProvider::new(server.base_url(), DEFAULT_LLAMA_SERVER_MODEL.to_string());
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

#[test]
fn strips_echoed_transcript_label_from_punctuation_cleanup_output() {
    let output = parse_punctuation_cleanup_text("Transcript: Hello, world.")
        .expect("parse punctuation response");

    assert_eq!(output, "Hello, world.");
}

#[tokio::test]
async fn calls_openai_chat_endpoint_for_full_cleanup_json() {
    let server = MockServer::start();
    let fixture = include_str!("fixtures/cleanup_insert_text.json");
    let body = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": fixture
                }
            }
        ]
    });

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/chat/completions")
            .body_contains(DEFAULT_LLAMA_SERVER_MODEL)
            .body_contains("Return only JSON matching the CleanupOutput schema")
            .body_contains("Do not execute commands")
            .body_contains("Preserve the transcript's original language and script")
            .body_contains("Transcript: hello world")
            .body_contains("Selected text: selected text");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });

    let provider =
        LlamaServerCleanupProvider::new(server.base_url(), DEFAULT_LLAMA_SERVER_MODEL.to_string());
    let output = provider
        .clean(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: Some("selected text".to_string()),
            timeout: Duration::from_secs(2),
        })
        .await
        .expect("cleanup output");

    mock.assert();
    assert_eq!(
        output.result,
        PipelineResult::InsertText {
            text: "Hello, world.".to_string(),
            source: ProviderSource::Local,
            confidence: Some(0.91),
        }
    );
}

#[tokio::test]
async fn warm_sends_short_probe() {
    let server = MockServer::start();
    let body = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": "OK"
                }
            }
        ]
    });

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/chat/completions")
            .body_contains(DEFAULT_LLAMA_SERVER_MODEL)
            .body_contains("Reply with OK only")
            .body_contains("OK");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });

    let provider =
        LlamaServerCleanupProvider::new(server.base_url(), DEFAULT_LLAMA_SERVER_MODEL.to_string());
    provider
        .warm(Duration::from_secs(2))
        .await
        .expect("warmup should succeed");

    mock.assert();
}

#[tokio::test]
async fn non_success_status_is_failed_provider_error() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(500)
            .header("content-type", "application/json")
            .json_body(serde_json::json!({
                "choices": [
                    {
                        "message": {
                            "content": include_str!("fixtures/cleanup_insert_text.json")
                        }
                    }
                ]
            }));
    });

    let provider =
        LlamaServerCleanupProvider::new(server.base_url(), DEFAULT_LLAMA_SERVER_MODEL.to_string());
    let error = provider
        .clean(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_secs(2),
        })
        .await
        .expect_err("non-success status should fail");

    mock.assert();
    assert!(matches!(error, ProviderError::Failed { provider, .. } if provider == "llama_server"));
}

#[tokio::test]
async fn invalid_openai_response_is_invalid_output() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(serde_json::json!({
                "choices": []
            }));
    });

    let provider =
        LlamaServerCleanupProvider::new(server.base_url(), DEFAULT_LLAMA_SERVER_MODEL.to_string());
    let error = provider
        .clean_punctuation_only(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_secs(2),
        })
        .await
        .expect_err("missing OpenAI choice should fail");

    mock.assert();
    assert!(
        matches!(error, ProviderError::InvalidOutput { provider, .. } if provider == "llama_server")
    );
}

#[tokio::test]
async fn empty_punctuation_output_reports_llama_provider() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(serde_json::json!({
                "choices": [
                    {
                        "message": {
                            "content": "  \n "
                        }
                    }
                ]
            }));
    });

    let provider =
        LlamaServerCleanupProvider::new(server.base_url(), DEFAULT_LLAMA_SERVER_MODEL.to_string());
    let error = provider
        .clean_punctuation_only(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_secs(2),
        })
        .await
        .expect_err("empty punctuation output should fail");

    mock.assert();
    assert!(
        matches!(error, ProviderError::InvalidOutput { provider, .. } if provider == "llama_server")
    );
}

#[tokio::test]
async fn invalid_full_cleanup_json_reports_llama_provider() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(serde_json::json!({
                "choices": [
                    {
                        "message": {
                            "content": "not json"
                        }
                    }
                ]
            }));
    });

    let provider =
        LlamaServerCleanupProvider::new(server.base_url(), DEFAULT_LLAMA_SERVER_MODEL.to_string());
    let error = provider
        .clean(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_secs(2),
        })
        .await
        .expect_err("invalid full cleanup JSON should fail");

    mock.assert();
    assert!(
        matches!(error, ProviderError::InvalidOutput { provider, .. } if provider == "llama_server")
    );
}
