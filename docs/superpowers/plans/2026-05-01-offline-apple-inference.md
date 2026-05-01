# Offline Apple Inference Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the product dependency on external Whisper/Ollama setup with Wispergo-managed offline macOS inference: bundled whisper.cpp ASR, bundled llama.cpp cleanup, and graceful raw-ASR fallback.

**Architecture:** Keep inference execution behind provider boundaries. Desktop/Tauri code owns bundled resource resolution and runtime lifecycle; `wispergo-core` owns ASR/cleanup provider protocols, prompts, HTTP parsing, and provider errors. Ollama remains as a hidden developer backend while product defaults move to bundled assets and a managed llama.cpp server.

**Tech Stack:** Rust, Tauri v2, whisper.cpp sidecar, llama.cpp `llama-server` OpenAI-compatible HTTP API, GGUF models, React/Vitest settings UI, Cargo tests, pnpm build scripts.

---

## Working Directory

Use the existing isolated worktree:

```bash
cd /Users/shiang/.config/superpowers/worktrees/wispergo/cleanup-mode
```

The approved design spec is:

```text
docs/superpowers/specs/2026-05-01-offline-apple-inference-design.md
```

## Scope and Sequencing

This is intentionally split into small commits. Do not commit multi-gigabyte model files to git. The product build will bundle files staged under `apps/desktop/src-tauri/resources/`, while git tracks the directory layout, staging/verification scripts, and code that resolves those resources.

Execution order:

1. Core cleanup abstraction.
2. llama.cpp HTTP provider.
3. Bundled resource resolver.
4. ASR bundled defaults.
5. Cleanup runtime manager.
6. Recording pipeline wiring.
7. Frontend/product UX cleanup.
8. Bundle resource config and asset verification scripts.
9. README and manual evaluation doc.
10. Full verification and review.

## File Structure

Create or modify these focused units:

- `crates/wispergo-core/src/providers.rs` — shared provider traits and fake providers.
- `crates/wispergo-core/src/ollama.rs` — keep developer Ollama provider, implement shared punctuation trait.
- `crates/wispergo-core/src/llama_server.rs` — new llama.cpp HTTP cleanup provider.
- `crates/wispergo-core/src/lib.rs` — export the new provider module.
- `crates/wispergo-core/tests/llama_server_tests.rs` — llama provider request/response tests.
- `crates/wispergo-core/tests/provider_tests.rs` — fake punctuation provider tests.
- `apps/desktop/src-tauri/src/inference/mod.rs` — desktop inference module root.
- `apps/desktop/src-tauri/src/inference/resources.rs` — bundled resource path and architecture resolver.
- `apps/desktop/src-tauri/src/inference/cleanup_runtime.rs` — managed llama-server lifecycle.
- `apps/desktop/src-tauri/src/commands/recording.rs` — use bundled ASR paths and cleanup abstraction.
- `apps/desktop/src-tauri/src/commands/settings.rs` — replace user-facing Ollama setup with generic cleanup runtime status.
- `apps/desktop/src-tauri/src/state.rs` — existing app settings state; runtime manager is registered as separate Tauri managed state.
- `apps/desktop/src-tauri/src/lib.rs` — manage/start cleanup runtime.
- `apps/desktop/src-tauri/tauri.conf.json` — include `resources/` in the macOS bundle.
- `apps/desktop/src/app/App.tsx`, `apps/desktop/src/features/settings/SettingsPanel.tsx`, `apps/desktop/src/lib/tauriApi.ts`, `apps/desktop/src/types/pipeline.ts` — remove normal-user Ollama UX and show generic offline cleanup state.
- `scripts/verify-inference-assets.sh` — release asset validation.
- `apps/desktop/src-tauri/resources/` — tracked directory layout, with large assets ignored.
- `README.md` — document offline product behavior and developer overrides.
- `docs/manual/offline-cleanup-eval.md` — manual English/Chinese punctuation quality fixture.

---

### Task 1: Add a punctuation-capable cleanup provider trait

**Files:**
- Modify: `crates/wispergo-core/src/providers.rs`
- Modify: `crates/wispergo-core/src/ollama.rs`
- Modify: `crates/wispergo-core/tests/provider_tests.rs`
- Test: `crates/wispergo-core/tests/ollama_tests.rs`

- [ ] **Step 1: Write failing tests for a fake punctuation cleanup provider**

Append this test to `crates/wispergo-core/tests/provider_tests.rs`. Update the existing domain import to include `ProviderSource`, and update the provider import to include `TextCleanupProvider` and `FakeTextCleanupProvider`:

```rust
use wispergo_core::providers::{
    AsrOutput, AsrProvider, CleanupInput, CleanupOutput, CleanupProvider, FakeAsrProvider,
    FakeCleanupProvider, FakeTextCleanupProvider, ProviderError, TextCleanupProvider,
};

#[tokio::test]
async fn fake_text_cleanup_returns_plain_punctuation_response() {
    let provider = FakeTextCleanupProvider::new(
        Ok("Hello, world.".to_string()),
        Ok(CleanupOutput {
            result: PipelineResult::InsertText {
                text: "Hello, world.".to_string(),
                source: ProviderSource::Local,
                confidence: None,
            },
        }),
    );

    let output = provider
        .clean_punctuation_only(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_millis(500),
        })
        .await
        .expect("punctuation output");

    assert_eq!(output, "Hello, world.");
}
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run:

```bash
cargo test -p wispergo-core --test provider_tests fake_text_cleanup_returns_plain_punctuation_response
```

Expected: compile failure because `TextCleanupProvider` and `FakeTextCleanupProvider` do not exist.

- [ ] **Step 3: Add the shared punctuation trait and fake provider**

In `crates/wispergo-core/src/providers.rs`, add this trait immediately after `CleanupProvider`:

```rust
#[async_trait]
pub trait TextCleanupProvider: CleanupProvider {
    async fn clean_punctuation_only(&self, input: CleanupInput) -> Result<String, ProviderError>;
}
```

Add this fake provider after `FakeCleanupProvider`:

```rust
#[derive(Debug, Clone)]
pub struct FakeTextCleanupProvider {
    punctuation_response: Result<String, ProviderError>,
    cleanup_response: Result<CleanupOutput, ProviderError>,
    punctuation_calls: Option<Arc<Mutex<usize>>>,
}

impl FakeTextCleanupProvider {
    pub fn new(
        punctuation_response: Result<String, ProviderError>,
        cleanup_response: Result<CleanupOutput, ProviderError>,
    ) -> Self {
        Self {
            punctuation_response,
            cleanup_response,
            punctuation_calls: None,
        }
    }

    pub fn with_punctuation_counter(
        punctuation_response: Result<String, ProviderError>,
        cleanup_response: Result<CleanupOutput, ProviderError>,
        punctuation_calls: Arc<Mutex<usize>>,
    ) -> Self {
        Self {
            punctuation_response,
            cleanup_response,
            punctuation_calls: Some(punctuation_calls),
        }
    }
}

#[async_trait]
impl CleanupProvider for FakeTextCleanupProvider {
    async fn clean(&self, _input: CleanupInput) -> Result<CleanupOutput, ProviderError> {
        self.cleanup_response.clone()
    }
}

#[async_trait]
impl TextCleanupProvider for FakeTextCleanupProvider {
    async fn clean_punctuation_only(&self, _input: CleanupInput) -> Result<String, ProviderError> {
        if let Some(calls) = &self.punctuation_calls {
            *calls.lock().expect("fake text cleanup counter lock") += 1;
        }
        self.punctuation_response.clone()
    }
}
```

- [ ] **Step 4: Implement the trait for Ollama**

In `crates/wispergo-core/src/ollama.rs`, change the provider import to include `TextCleanupProvider`:

```rust
use crate::providers::{CleanupInput, CleanupOutput, CleanupProvider, ProviderError, TextCleanupProvider};
```

Move the existing inherent `clean_punctuation_only` body into a `TextCleanupProvider` impl. Keep `warm` as an inherent method. The implementation should be:

```rust
#[async_trait]
impl TextCleanupProvider for OllamaCleanupProvider {
    async fn clean_punctuation_only(
        &self,
        input: CleanupInput,
    ) -> Result<String, ProviderError> {
        let request = OllamaChatRequest {
            model: self.model.clone(),
            stream: false,
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: punctuation_system_prompt(),
                },
                OllamaMessage {
                    role: "user".to_string(),
                    content: punctuation_user_prompt(&input),
                },
            ],
        };

        let body = tokio::time::timeout(input.timeout, self.send_chat(request))
            .await
            .map_err(|_| ProviderError::Timeout {
                provider: "ollama".to_string(),
            })??;

        parse_punctuation_cleanup_text(&body.message.content)
    }
}
```

- [ ] **Step 5: Import the new trait in Ollama tests**

In `crates/wispergo-core/tests/ollama_tests.rs`, update the providers import from:

```rust
use wispergo_core::providers::{CleanupInput, CleanupProvider, ProviderError};
```

to:

```rust
use wispergo_core::providers::{CleanupInput, CleanupProvider, ProviderError, TextCleanupProvider};
```

This keeps existing calls such as `provider.clean_punctuation_only(...)` valid after the method moves to the trait.

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test -p wispergo-core --test provider_tests fake_text_cleanup_returns_plain_punctuation_response
cargo test -p wispergo-core --test ollama_tests calls_ollama_chat_api_for_punctuation_only_cleanup
```

Expected: both pass.

- [ ] **Step 7: Commit Task 1**

```bash
git add crates/wispergo-core/src/providers.rs crates/wispergo-core/src/ollama.rs crates/wispergo-core/tests/provider_tests.rs crates/wispergo-core/tests/ollama_tests.rs
git commit -m "refactor: add text cleanup provider trait"
```

---

### Task 2: Add a llama.cpp server cleanup provider

**Files:**
- Create: `crates/wispergo-core/src/llama_server.rs`
- Create: `crates/wispergo-core/tests/llama_server_tests.rs`
- Modify: `crates/wispergo-core/src/lib.rs`
- Modify: `crates/wispergo-core/src/ollama.rs` if prompt helpers need sharing

- [ ] **Step 1: Write failing llama provider tests**

Create `crates/wispergo-core/tests/llama_server_tests.rs`:

```rust
use std::time::Duration;

use httpmock::prelude::*;
use wispergo_core::domain::PipelineResult;
use wispergo_core::llama_server::{LlamaServerCleanupProvider, DEFAULT_LLAMA_SERVER_MODEL};
use wispergo_core::providers::{CleanupInput, CleanupProvider, ProviderError, TextCleanupProvider};

#[tokio::test]
async fn calls_openai_chat_endpoint_for_punctuation_cleanup() {
    let server = MockServer::start();
    let body = serde_json::json!({
        "choices": [
            { "message": { "content": "Hello, world." } }
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

    let provider = LlamaServerCleanupProvider::new(
        server.base_url(),
        DEFAULT_LLAMA_SERVER_MODEL.to_string(),
    );

    let output = provider
        .clean_punctuation_only(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: Some("ignored".to_string()),
            timeout: Duration::from_secs(2),
        })
        .await
        .expect("punctuation output");

    mock.assert();
    assert_eq!(output, "Hello, world.");
}

#[tokio::test]
async fn calls_openai_chat_endpoint_for_full_cleanup_json() {
    let server = MockServer::start();
    let body = serde_json::json!({
        "choices": [
            { "message": { "content": include_str!("fixtures/cleanup_insert_text.json") } }
        ]
    });

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/chat/completions")
            .body_contains("Return only JSON matching the CleanupOutput schema")
            .body_contains("Transcript: hello world");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });

    let provider = LlamaServerCleanupProvider::new(server.base_url(), "qwen-test".to_string());
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

#[tokio::test]
async fn warm_sends_short_probe() {
    let server = MockServer::start();
    let body = serde_json::json!({
        "choices": [
            { "message": { "content": "OK" } }
        ]
    });

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/chat/completions")
            .body_contains("Reply with OK only")
            .body_contains("OK");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });

    let provider = LlamaServerCleanupProvider::new(server.base_url(), "qwen-test".to_string());
    provider.warm(Duration::from_secs(2)).await.expect("warmup");

    mock.assert();
}

#[tokio::test]
async fn non_success_status_is_failed_provider_error() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(500).body("server error");
    });

    let provider = LlamaServerCleanupProvider::new(server.base_url(), "qwen-test".to_string());
    let error = provider
        .clean_punctuation_only(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_secs(2),
        })
        .await
        .expect_err("status should fail");

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
            .json_body(serde_json::json!({ "choices": [] }));
    });

    let provider = LlamaServerCleanupProvider::new(server.base_url(), "qwen-test".to_string());
    let error = provider
        .clean_punctuation_only(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_secs(2),
        })
        .await
        .expect_err("empty choices should fail");

    mock.assert();
    assert!(matches!(error, ProviderError::InvalidOutput { provider, .. } if provider == "llama_server"));
}

#[tokio::test]
async fn empty_punctuation_output_reports_llama_provider() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(serde_json::json!({
                "choices": [{ "message": { "content": "  \n" } }]
            }));
    });

    let provider = LlamaServerCleanupProvider::new(server.base_url(), "qwen-test".to_string());
    let error = provider
        .clean_punctuation_only(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_secs(2),
        })
        .await
        .expect_err("empty punctuation output should fail");

    mock.assert();
    assert!(matches!(error, ProviderError::InvalidOutput { provider, .. } if provider == "llama_server"));
}

#[tokio::test]
async fn invalid_full_cleanup_json_reports_llama_provider() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(serde_json::json!({
                "choices": [{ "message": { "content": "not json" } }]
            }));
    });

    let provider = LlamaServerCleanupProvider::new(server.base_url(), "qwen-test".to_string());
    let error = provider
        .clean(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_secs(2),
        })
        .await
        .expect_err("invalid JSON should fail");

    mock.assert();
    assert!(matches!(error, ProviderError::InvalidOutput { provider, .. } if provider == "llama_server"));
}
```

- [ ] **Step 2: Run tests and verify they fail**

```bash
cargo test -p wispergo-core --test llama_server_tests
```

Expected: compile failure because `wispergo_core::llama_server` does not exist.

- [ ] **Step 3: Implement `llama_server.rs`**

Create `crates/wispergo-core/src/llama_server.rs` with this implementation:

```rust
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::domain::PipelineResult;
use crate::providers::{CleanupInput, CleanupOutput, CleanupProvider, ProviderError, TextCleanupProvider};

pub const DEFAULT_LLAMA_SERVER_MODEL: &str = "qwen2.5-3b-instruct";

#[derive(Debug, Clone)]
pub struct LlamaServerCleanupProvider {
    base_url: String,
    model: String,
    client: Client,
}

impl LlamaServerCleanupProvider {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            client: Client::new(),
        }
    }

    pub async fn warm(&self, timeout: std::time::Duration) -> Result<(), ProviderError> {
        let request = OpenAiChatRequest {
            model: self.model.clone(),
            stream: false,
            temperature: 0.0,
            messages: vec![
                OpenAiMessage {
                    role: "system".to_string(),
                    content: "Reply with OK only.".to_string(),
                },
                OpenAiMessage {
                    role: "user".to_string(),
                    content: "OK".to_string(),
                },
            ],
        };

        tokio::time::timeout(timeout, self.send_chat(request))
            .await
            .map_err(|_| ProviderError::Timeout {
                provider: "llama_server".to_string(),
            })??;
        Ok(())
    }

    async fn send_chat(&self, request: OpenAiChatRequest) -> Result<OpenAiChatResponse, ProviderError> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|err| ProviderError::Unavailable {
                provider: "llama_server".to_string(),
                message: Some(err.to_string()),
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::Failed {
                provider: "llama_server".to_string(),
                message: format!("llama-server returned HTTP status {status}"),
            });
        }

        response.json().await.map_err(|err| ProviderError::InvalidOutput {
            provider: "llama_server".to_string(),
            message: err.to_string(),
        })
    }
}

#[async_trait]
impl TextCleanupProvider for LlamaServerCleanupProvider {
    async fn clean_punctuation_only(&self, input: CleanupInput) -> Result<String, ProviderError> {
        let request = OpenAiChatRequest {
            model: self.model.clone(),
            stream: false,
            temperature: 0.0,
            messages: vec![
                OpenAiMessage {
                    role: "system".to_string(),
                    content: punctuation_system_prompt(),
                },
                OpenAiMessage {
                    role: "user".to_string(),
                    content: punctuation_user_prompt(&input),
                },
            ],
        };

        let body = tokio::time::timeout(input.timeout, self.send_chat(request))
            .await
            .map_err(|_| ProviderError::Timeout {
                provider: "llama_server".to_string(),
            })??;

        parse_punctuation_cleanup_text(first_message_content(&body)?)
    }
}

#[async_trait]
impl CleanupProvider for LlamaServerCleanupProvider {
    async fn clean(&self, input: CleanupInput) -> Result<CleanupOutput, ProviderError> {
        let request = OpenAiChatRequest {
            model: self.model.clone(),
            stream: false,
            temperature: 0.0,
            messages: vec![
                OpenAiMessage {
                    role: "system".to_string(),
                    content: cleanup_system_prompt(),
                },
                OpenAiMessage {
                    role: "user".to_string(),
                    content: cleanup_user_prompt(&input),
                },
            ],
        };

        let body = tokio::time::timeout(input.timeout, self.send_chat(request))
            .await
            .map_err(|_| ProviderError::Timeout {
                provider: "llama_server".to_string(),
            })??;

        parse_cleanup_json(first_message_content(&body)?)
    }
}

fn first_message_content(response: &OpenAiChatResponse) -> Result<&str, ProviderError> {
    response
        .choices
        .first()
        .map(|choice| choice.message.content.as_str())
        .ok_or_else(|| ProviderError::InvalidOutput {
            provider: "llama_server".to_string(),
            message: "missing first chat completion choice".to_string(),
        })
}

fn parse_punctuation_cleanup_text(input: &str) -> Result<String, ProviderError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(ProviderError::InvalidOutput {
            provider: "llama_server".to_string(),
            message: "empty punctuation cleanup output".to_string(),
        });
    }

    Ok(text.to_string())
}

fn parse_cleanup_json(input: &str) -> Result<CleanupOutput, ProviderError> {
    let mut output = serde_json::from_str::<CleanupOutput>(input).map_err(|err| {
        ProviderError::InvalidOutput {
            provider: "llama_server".to_string(),
            message: err.to_string(),
        }
    })?;

    if let PipelineResult::Command {
        command,
        requires_confirmation,
        ..
    } = &mut output.result
    {
        if command.is_destructive() {
            *requires_confirmation = true;
        }
    }

    Ok(output)
}

fn cleanup_system_prompt() -> String {
    "Return only JSON matching the CleanupOutput schema. Do not execute commands. Classify user intent into insert_text, command, cancelled, or error results. Preserve the transcript's original language and script; do not translate between languages.".to_string()
}

fn cleanup_user_prompt(input: &CleanupInput) -> String {
    format!(
        "Transcript: {}\nSelected text: {}",
        input.transcript,
        input.selected_text.as_deref().unwrap_or("")
    )
}

fn punctuation_system_prompt() -> String {
    "Punctuation-only cleanup. Return only the corrected transcript as plain text. Add punctuation and capitalization only. Preserve the exact words, language, and script from the transcript. Do not translate, paraphrase, summarize, add or remove words, classify commands, or execute commands.".to_string()
}

fn punctuation_user_prompt(input: &CleanupInput) -> String {
    format!("Transcript: {}", input.transcript)
}

#[derive(Debug, Serialize)]
struct OpenAiChatRequest {
    model: String,
    stream: bool,
    temperature: f32,
    messages: Vec<OpenAiMessage>,
}

#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessageResponse,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessageResponse {
    content: String,
}
```

- [ ] **Step 4: Export the module**

Add to `crates/wispergo-core/src/lib.rs`:

```rust
pub mod llama_server;
```

- [ ] **Step 5: Run focused tests**

```bash
cargo test -p wispergo-core --test llama_server_tests
cargo test -p wispergo-core --test ollama_tests
```

Expected: all pass.

- [ ] **Step 6: Commit Task 2**

```bash
git add crates/wispergo-core/src/lib.rs crates/wispergo-core/src/llama_server.rs crates/wispergo-core/tests/llama_server_tests.rs
git commit -m "feat: add llama server cleanup provider"
```

---

### Task 3: Add bundled inference resource resolution

**Files:**
- Create: `apps/desktop/src-tauri/src/inference/mod.rs`
- Create: `apps/desktop/src-tauri/src/inference/resources.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing resource resolver tests**

Create `apps/desktop/src-tauri/src/inference/mod.rs`:

```rust
pub mod resources;
```

Create `apps/desktop/src-tauri/src/inference/resources.rs` with only the tests first:

```rust
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_aarch64_resource_paths() {
        let root = PathBuf::from("/Applications/Wispergo.app/Contents/Resources");
        let paths = InferenceResourcePaths::from_resource_root_for_arch(
            root.clone(),
            CpuArchitecture::Aarch64,
        );

        assert_eq!(
            paths.whisper_binary_path,
            root.join("bin/macos-aarch64/whisper-cli")
        );
        assert_eq!(
            paths.llama_server_binary_path,
            root.join("bin/macos-aarch64/llama-server")
        );
        assert_eq!(
            paths.asr_model_path,
            root.join("models/asr/ggml-large-v3-turbo.bin")
        );
        assert_eq!(
            paths.cleanup_model_path,
            root.join("models/cleanup/qwen2.5-3b-instruct-q4_k_m.gguf")
        );
    }

    #[test]
    fn resolves_x86_64_resource_paths() {
        let root = PathBuf::from("/Applications/Wispergo.app/Contents/Resources");
        let paths = InferenceResourcePaths::from_resource_root_for_arch(
            root.clone(),
            CpuArchitecture::X86_64,
        );

        assert_eq!(
            paths.whisper_binary_path,
            root.join("bin/macos-x86_64/whisper-cli")
        );
        assert_eq!(
            paths.llama_server_binary_path,
            root.join("bin/macos-x86_64/llama-server")
        );
    }

    #[test]
    fn missing_resource_validation_lists_exact_missing_paths() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("wispergo-missing-assets-{unique}"));
        std::fs::create_dir_all(&root).expect("create root");
        let paths = InferenceResourcePaths::from_resource_root_for_arch(
            root.clone(),
            CpuArchitecture::Aarch64,
        );

        let error = paths.validate_required_assets().expect_err("missing assets");

        assert!(error.contains("Wispergo installation is missing bundled inference assets"));
        assert!(error.contains("bin/macos-aarch64/whisper-cli"));
        assert!(error.contains("models/asr/ggml-large-v3-turbo.bin"));

        let _ = std::fs::remove_dir_all(root);
    }
}
```

Add `mod inference;` to `apps/desktop/src-tauri/src/lib.rs` near the other module declarations.

- [ ] **Step 2: Run tests and verify they fail**

```bash
cargo test -p wispergo-desktop --lib inference::resources::tests
```

Expected: compile failure because `CpuArchitecture` and `InferenceResourcePaths` do not exist.

- [ ] **Step 3: Implement the resolver**

Replace `apps/desktop/src-tauri/src/inference/resources.rs` with this implementation, preserving the tests at the bottom:

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuArchitecture {
    Aarch64,
    X86_64,
}

impl CpuArchitecture {
    pub fn current() -> Self {
        if cfg!(target_arch = "aarch64") {
            Self::Aarch64
        } else {
            Self::X86_64
        }
    }

    pub fn resource_dir_name(self) -> &'static str {
        match self {
            Self::Aarch64 => "macos-aarch64",
            Self::X86_64 => "macos-x86_64",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceResourcePaths {
    pub resource_root: PathBuf,
    pub whisper_binary_path: PathBuf,
    pub llama_server_binary_path: PathBuf,
    pub asr_model_path: PathBuf,
    pub cleanup_model_path: PathBuf,
}

impl InferenceResourcePaths {
    pub fn from_resource_root(resource_root: PathBuf) -> Self {
        Self::from_resource_root_for_arch(resource_root, CpuArchitecture::current())
    }

    pub fn from_resource_root_for_arch(
        resource_root: PathBuf,
        architecture: CpuArchitecture,
    ) -> Self {
        let bin_root = resource_root
            .join("bin")
            .join(architecture.resource_dir_name());
        Self {
            whisper_binary_path: bin_root.join("whisper-cli"),
            llama_server_binary_path: bin_root.join("llama-server"),
            asr_model_path: resource_root.join("models/asr/ggml-large-v3-turbo.bin"),
            cleanup_model_path: resource_root
                .join("models/cleanup/qwen2.5-3b-instruct-q4_k_m.gguf"),
            resource_root,
        }
    }

    pub fn validate_required_assets(&self) -> Result<(), String> {
        let required = [
            &self.whisper_binary_path,
            &self.llama_server_binary_path,
            &self.asr_model_path,
            &self.cleanup_model_path,
        ];
        let missing = required
            .iter()
            .filter(|path| !path.exists())
            .map(|path| display_relative_or_absolute(&self.resource_root, path))
            .collect::<Vec<_>>();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Wispergo installation is missing bundled inference assets: {}",
                missing.join(", ")
            ))
        }
    }
}

fn display_relative_or_absolute(root: &PathBuf, path: &PathBuf) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}
```

Keep the tests below this implementation. Use only the imports that compile; the implementation needs `PathBuf` and does not need `Path`.

- [ ] **Step 4: Run focused tests**

```bash
cargo test -p wispergo-desktop --lib inference::resources::tests
```

Expected: all resource resolver tests pass.

- [ ] **Step 5: Commit Task 3**

```bash
git add apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/inference/mod.rs apps/desktop/src-tauri/src/inference/resources.rs
git commit -m "feat: resolve bundled inference assets"
```

---

### Task 4: Prefer bundled ASR assets while preserving developer overrides

**Files:**
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/src/commands/recording.rs`
- Test: `apps/desktop/src-tauri/src/commands/recording.rs`

- [ ] **Step 1: Add desktop test dependency for temporary asset directories**

In `apps/desktop/src-tauri/Cargo.toml`, add this section at the end of the file when it is not already present:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Add failing tests for ASR resolver precedence**

In the test module in `apps/desktop/src-tauri/src/commands/recording.rs`, add `use crate::inference::resources::{CpuArchitecture, InferenceResourcePaths};` and append these tests:

```rust
#[test]
fn bundled_asr_paths_are_used_when_settings_and_env_are_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let resources = InferenceResourcePaths::from_resource_root_for_arch(
        root.clone(),
        CpuArchitecture::Aarch64,
    );
    create_file(&resources.whisper_binary_path);
    create_file(&resources.asr_model_path);

    let paths = super::resolve_asr_paths_with_resources(
        &LocalModelSettings {
            whisper_binary_path: None,
            whisper_model_path: None,
            recognition_language: crate::state::RecognitionLanguage::Auto,
            cleanup_mode: CleanupMode::PunctuationOnly,
        },
        Some(&resources),
    )
    .expect("resolve bundled paths");

    assert_eq!(paths.binary_path, root.join("bin/macos-aarch64/whisper-cli"));
    assert_eq!(paths.model_path, root.join("models/asr/ggml-large-v3-turbo.bin"));
}

#[test]
fn explicit_settings_asr_paths_override_bundled_assets() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let resources = InferenceResourcePaths::from_resource_root_for_arch(
        root,
        CpuArchitecture::Aarch64,
    );

    let paths = super::resolve_asr_paths_with_resources(
        &LocalModelSettings {
            whisper_binary_path: Some("/custom/whisper-cli".to_string()),
            whisper_model_path: Some("/custom/model.bin".to_string()),
            recognition_language: crate::state::RecognitionLanguage::Auto,
            cleanup_mode: CleanupMode::PunctuationOnly,
        },
        Some(&resources),
    )
    .expect("resolve custom paths");

    assert_eq!(paths.binary_path, std::path::PathBuf::from("/custom/whisper-cli"));
    assert_eq!(paths.model_path, std::path::PathBuf::from("/custom/model.bin"));
}

#[test]
fn missing_bundled_asr_assets_return_damaged_install_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let resources = InferenceResourcePaths::from_resource_root_for_arch(
        dir.path().to_path_buf(),
        CpuArchitecture::Aarch64,
    );

    let error = super::resolve_asr_paths_with_resources(
        &LocalModelSettings {
            whisper_binary_path: None,
            whisper_model_path: None,
            recognition_language: crate::state::RecognitionLanguage::Auto,
            cleanup_mode: CleanupMode::PunctuationOnly,
        },
        Some(&resources),
    )
    .expect_err("missing bundled ASR should fail clearly");

    assert!(error.contains("Wispergo installation is missing bundled ASR assets"));
    assert!(error.contains("bin/macos-aarch64/whisper-cli"));
    assert!(error.contains("models/asr/ggml-large-v3-turbo.bin"));
}

fn create_file(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(path, "test asset").expect("write test asset");
}
```

- [ ] **Step 3: Run tests and verify they fail**

```bash
cargo test -p wispergo-desktop --lib bundled_asr_paths_are_used_when_settings_and_env_are_empty
cargo test -p wispergo-desktop --lib explicit_settings_asr_paths_override_bundled_assets
cargo test -p wispergo-desktop --lib missing_bundled_asr_assets_return_damaged_install_error
```

Expected: compile failure because `resolve_asr_paths_with_resources` does not exist.

- [ ] **Step 4: Thread resource paths into processing**

Modify `stop_recording` in `apps/desktop/src-tauri/src/commands/recording.rs` so it resolves resource paths before `process_recording`:

```rust
    let bundled_resources = bundled_inference_resources(&app);

    let process_start = Instant::now();
    let processed = process_recording(
        audio,
        state.local_model_settings(),
        bundled_resources.as_ref(),
    )
    .await?;
```

Change `process_recording` signature to:

```rust
async fn process_recording(
    audio: Vec<f32>,
    settings: LocalModelSettings,
    bundled_resources: Option<&InferenceResourcePaths>,
) -> Result<ProcessedRecording, String> {
```

Update the ASR call inside `process_recording`:

```rust
    let asr = local_asr_provider(&settings, bundled_resources)?
        .transcribe(audio)
        .await
        .map_err(provider_error_message)?;
```

Add imports at the top:

```rust
use crate::inference::resources::InferenceResourcePaths;
```

Add this helper near `ollama_cleanup_provider`:

```rust
fn bundled_inference_resources(app: &AppHandle) -> Option<InferenceResourcePaths> {
    match app.path().resource_dir() {
        Ok(resource_root) => Some(InferenceResourcePaths::from_resource_root(resource_root)),
        Err(err) => {
            eprintln!("bundled inference resource directory unavailable: {err}");
            None
        }
    }
}
```

- [ ] **Step 5: Implement resolver with bundled fallback**

Change `local_asr_provider` to accept resources:

```rust
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
```

Replace `resolve_asr_paths` with:

```rust
fn resolve_asr_paths(settings: &LocalModelSettings) -> Result<AsrPaths, String> {
    resolve_asr_paths_with_resources(settings, None)
}

fn resolve_asr_paths_with_resources(
    settings: &LocalModelSettings,
    bundled_resources: Option<&InferenceResourcePaths>,
) -> Result<AsrPaths, String> {
    let override_binary = settings_path(&settings.whisper_binary_path)
        .or_else(|| env::var_os("WISPERGO_WHISPER_BIN").map(PathBuf::from));
    let override_model = settings_path(&settings.whisper_model_path)
        .or_else(|| env::var_os("WISPERGO_WHISPER_MODEL").map(PathBuf::from));

    if let (Some(binary_path), Some(model_path)) = (override_binary, override_model) {
        return Ok(AsrPaths {
            binary_path,
            model_path,
        });
    }

    if let Some(resources) = bundled_resources {
        let missing = [
            &resources.whisper_binary_path,
            &resources.asr_model_path,
        ]
        .into_iter()
        .filter(|path| !path.exists())
        .map(|path| {
            path.strip_prefix(&resources.resource_root)
                .map(|relative| relative.display().to_string())
                .unwrap_or_else(|_| path.display().to_string())
        })
        .collect::<Vec<_>>();

        if missing.is_empty() {
            return Ok(AsrPaths {
                binary_path: resources.whisper_binary_path.clone(),
                model_path: resources.asr_model_path.clone(),
            });
        }

        return Err(format!(
            "Wispergo installation is missing bundled ASR assets: {}",
            missing.join(", ")
        ));
    }

    let binary_path = find_in_path("whisper-cli")
        .or_else(|| find_in_path("whisper-cpp"))
        .ok_or_else(|| {
            "Local ASR is not configured and bundled whisper.cpp is unavailable. Reinstall Wispergo or set WISPERGO_WHISPER_BIN and WISPERGO_WHISPER_MODEL.".to_string()
        })?;
    let model_path = env::var_os("WISPERGO_WHISPER_MODEL")
        .map(PathBuf::from)
        .ok_or_else(|| {
            "Local ASR model is missing. Reinstall Wispergo or set WISPERGO_WHISPER_MODEL to a local whisper.cpp model path.".to_string()
        })?;

    Ok(AsrPaths {
        binary_path,
        model_path,
    })
}
```

- [ ] **Step 6: Update existing tests that call `process_recording` or `local_asr_provider`**

Run this search:

```bash
rg "process_recording\(|local_asr_provider\(" apps/desktop/src-tauri/src/commands/recording.rs
```

Every direct test call to `process_recording(audio, settings)` must become:

```rust
process_recording(audio, settings, None).await
```

Every direct test call to `local_asr_provider(&settings)` must become:

```rust
local_asr_provider(&settings, None)
```

- [ ] **Step 7: Run focused tests**

```bash
cargo test -p wispergo-desktop --lib bundled_asr_paths_are_used_when_settings_and_env_are_empty
cargo test -p wispergo-desktop --lib explicit_settings_asr_paths_override_bundled_assets
cargo test -p wispergo-desktop --lib configured_asr_paths_take_precedence
```

Expected: all pass.

- [ ] **Step 8: Commit Task 4**

```bash
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/src/commands/recording.rs
git commit -m "feat: prefer bundled whisper assets"
```

---

### Task 5: Add a managed cleanup runtime process manager

**Files:**
- Create: `apps/desktop/src-tauri/src/inference/cleanup_runtime.rs`
- Modify: `apps/desktop/src-tauri/src/inference/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/Cargo.toml`

- [ ] **Step 1: Write state and command tests before process implementation**

Create `apps/desktop/src-tauri/src/inference/cleanup_runtime.rs` with these tests first:

```rust
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::resources::{CpuArchitecture, InferenceResourcePaths};

    #[test]
    fn default_status_is_sanitized_unavailable() {
        let manager = CleanupRuntimeManager::default();
        let status = manager.status();

        assert_eq!(status.state, CleanupRuntimeState::Unavailable);
        assert_eq!(status.message.as_deref(), Some("Offline punctuation is not ready."));
    }

    #[test]
    fn ready_status_does_not_expose_port_or_model_details() {
        let manager = CleanupRuntimeManager::default();
        manager.mark_ready_for_test("http://127.0.0.1:43210".to_string());

        let status = manager.status();

        assert_eq!(status.state, CleanupRuntimeState::Ready);
        assert_eq!(status.message, None);
        assert!(manager.provider().is_some());
    }

    #[test]
    fn server_command_uses_bundled_binary_model_and_localhost() {
        let root = PathBuf::from("/bundle/Resources");
        let resources = InferenceResourcePaths::from_resource_root_for_arch(
            root.clone(),
            CpuArchitecture::Aarch64,
        );

        let command = CleanupRuntimeCommand::new(&resources, 43210);

        assert_eq!(command.program, root.join("bin/macos-aarch64/llama-server"));
        assert!(command.args.contains(&"-m".to_string()));
        assert!(command.args.contains(&root.join("models/cleanup/qwen2.5-3b-instruct-q4_k_m.gguf").display().to_string()));
        assert!(command.args.contains(&"--host".to_string()));
        assert!(command.args.contains(&"127.0.0.1".to_string()));
        assert!(command.args.contains(&"--port".to_string()));
        assert!(command.args.contains(&"43210".to_string()));
        assert!(command.args.contains(&"--n-gpu-layers".to_string()));
        assert!(command.args.contains(&"999".to_string()));
    }

    #[test]
    fn stopped_child_transitions_to_failed_status() {
        let manager = CleanupRuntimeManager::default();
        manager.mark_failed_for_test("Offline punctuation stopped unexpectedly.".to_string());

        let status = manager.status();

        assert_eq!(status.state, CleanupRuntimeState::Failed);
        assert_eq!(status.message.as_deref(), Some("Offline punctuation stopped unexpectedly."));
    }
}
```

- [ ] **Step 2: Run tests and verify they fail**

```bash
cargo test -p wispergo-desktop --lib inference::cleanup_runtime::tests
```

Expected: compile failure because runtime types do not exist.

- [ ] **Step 3: Add desktop Tokio time dependency**

In `apps/desktop/src-tauri/Cargo.toml`, add this dependency next to the other runtime dependencies:

```toml
tokio = { version = "1", features = ["time"] }
```

- [ ] **Step 4: Implement sanitized status, command construction, readiness polling, crash monitor, and shutdown**

Replace `apps/desktop/src-tauri/src/inference/cleanup_runtime.rs` with this implementation and keep the tests at the bottom:

```rust
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wispergo_core::llama_server::{LlamaServerCleanupProvider, DEFAULT_LLAMA_SERVER_MODEL};

use crate::inference::resources::InferenceResourcePaths;

const CLEANUP_WARMUP_ATTEMPT_TIMEOUT: Duration = Duration::from_millis(750);
const CLEANUP_READY_TIMEOUT: Duration = Duration::from_secs(30);
const CLEANUP_READY_POLL_INTERVAL: Duration = Duration::from_millis(250);
const CLEANUP_CRASH_POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupRuntimeState {
    Disabled,
    Starting,
    Ready,
    Unavailable,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupRuntimeStatus {
    pub state: CleanupRuntimeState,
    pub message: Option<String>,
}

#[derive(Debug)]
struct CleanupRuntimeInner {
    status: CleanupRuntimeStatus,
    base_url: Option<String>,
    child: Option<Child>,
    generation: u64,
}

#[derive(Debug, Clone)]
pub struct CleanupRuntimeManager {
    inner: Arc<Mutex<CleanupRuntimeInner>>,
}

impl Default for CleanupRuntimeManager {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(CleanupRuntimeInner {
                status: CleanupRuntimeStatus {
                    state: CleanupRuntimeState::Unavailable,
                    message: Some("Offline punctuation is not ready.".to_string()),
                },
                base_url: None,
                child: None,
                generation: 0,
            })),
        }
    }
}

impl CleanupRuntimeManager {
    pub fn status(&self) -> CleanupRuntimeStatus {
        self.inner.lock().expect("cleanup runtime lock").status.clone()
    }

    pub fn provider(&self) -> Option<LlamaServerCleanupProvider> {
        let inner = self.inner.lock().expect("cleanup runtime lock");
        if inner.status.state != CleanupRuntimeState::Ready {
            return None;
        }
        let base_url = inner.base_url.clone()?;
        Some(LlamaServerCleanupProvider::new(
            base_url,
            DEFAULT_LLAMA_SERVER_MODEL.to_string(),
        ))
    }

    pub fn start_background(&self, resources: InferenceResourcePaths) {
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            manager.start(resources).await;
        });
    }

    async fn start(&self, resources: InferenceResourcePaths) {
        if let Err(_err) = resources.validate_required_assets() {
            self.set_status(
                CleanupRuntimeState::Unavailable,
                None,
                Some("Offline punctuation assets are missing. Reinstall Wispergo.".to_string()),
            );
            return;
        }

        let generation = self.next_generation();
        self.set_status(
            CleanupRuntimeState::Starting,
            None,
            Some("Preparing offline punctuation.".to_string()),
        );

        let port = choose_local_port();
        let base_url = format!("http://127.0.0.1:{port}");
        let command = CleanupRuntimeCommand::new(&resources, port);

        let child = match spawn_runtime_process(&command) {
            Ok(child) => child,
            Err(_err) => {
                self.set_status(
                    CleanupRuntimeState::Failed,
                    None,
                    Some("Offline punctuation could not start.".to_string()),
                );
                return;
            }
        };

        {
            let mut inner = self.inner.lock().expect("cleanup runtime lock");
            inner.child = Some(child);
            inner.base_url = Some(base_url.clone());
        }

        self.monitor_child(resources.clone(), generation);

        let provider = LlamaServerCleanupProvider::new(
            base_url.clone(),
            DEFAULT_LLAMA_SERVER_MODEL.to_string(),
        );
        match wait_until_ready(&provider).await {
            Ok(()) => self.set_status(CleanupRuntimeState::Ready, Some(base_url), None),
            Err(()) => self.set_status(
                CleanupRuntimeState::Failed,
                None,
                Some("Offline punctuation did not become ready in time.".to_string()),
            ),
        }
    }

    fn monitor_child(&self, resources: InferenceResourcePaths, generation: u64) {
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(CLEANUP_CRASH_POLL_INTERVAL).await;
                let should_restart = {
                    let mut inner = manager.inner.lock().expect("cleanup runtime lock");
                    if inner.generation != generation {
                        return;
                    }
                    match inner.child.as_mut() {
                        Some(child) => match child.try_wait() {
                            Ok(Some(_status)) => {
                                inner.child = None;
                                inner.base_url = None;
                                inner.status = CleanupRuntimeStatus {
                                    state: CleanupRuntimeState::Failed,
                                    message: Some("Offline punctuation stopped unexpectedly.".to_string()),
                                };
                                true
                            }
                            Ok(None) => false,
                            Err(_err) => {
                                inner.status = CleanupRuntimeStatus {
                                    state: CleanupRuntimeState::Failed,
                                    message: Some("Offline punctuation status is unavailable.".to_string()),
                                };
                                true
                            }
                        },
                        None => false,
                    }
                };

                if should_restart {
                    manager.start_background(resources.clone());
                    return;
                }
            }
        });
    }

    pub fn shutdown(&self) {
        let mut inner = self.inner.lock().expect("cleanup runtime lock");
        inner.generation = inner.generation.saturating_add(1);
        if let Some(mut child) = inner.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        inner.base_url = None;
        inner.status = CleanupRuntimeStatus {
            state: CleanupRuntimeState::Unavailable,
            message: Some("Offline punctuation is stopped.".to_string()),
        };
    }

    fn next_generation(&self) -> u64 {
        let mut inner = self.inner.lock().expect("cleanup runtime lock");
        inner.generation = inner.generation.saturating_add(1);
        inner.generation
    }

    fn set_status(&self, state: CleanupRuntimeState, base_url: Option<String>, message: Option<String>) {
        let mut inner = self.inner.lock().expect("cleanup runtime lock");
        inner.status = CleanupRuntimeStatus { state, message };
        inner.base_url = base_url;
    }

    #[cfg(test)]
    pub fn mark_ready_for_test(&self, base_url: String) {
        self.set_status(CleanupRuntimeState::Ready, Some(base_url), None);
    }

    #[cfg(test)]
    pub fn mark_failed_for_test(&self, message: String) {
        self.set_status(CleanupRuntimeState::Failed, None, Some(message));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupRuntimeCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl CleanupRuntimeCommand {
    pub fn new(resources: &InferenceResourcePaths, port: u16) -> Self {
        Self {
            program: resources.llama_server_binary_path.clone(),
            args: vec![
                "-m".to_string(),
                resources.cleanup_model_path.display().to_string(),
                "--host".to_string(),
                "127.0.0.1".to_string(),
                "--port".to_string(),
                port.to_string(),
                "--ctx-size".to_string(),
                "2048".to_string(),
                "--n-gpu-layers".to_string(),
                "999".to_string(),
            ],
        }
    }
}

async fn wait_until_ready(provider: &LlamaServerCleanupProvider) -> Result<(), ()> {
    let deadline = Instant::now() + CLEANUP_READY_TIMEOUT;
    while Instant::now() < deadline {
        if provider.warm(CLEANUP_WARMUP_ATTEMPT_TIMEOUT).await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(CLEANUP_READY_POLL_INTERVAL).await;
    }
    Err(())
}

fn spawn_runtime_process(command: &CleanupRuntimeCommand) -> Result<Child, String> {
    Command::new(&command.program)
        .args(&command.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("Failed to start bundled cleanup runtime: {err}"))
}

fn choose_local_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .unwrap_or(41173)
}
```

- [ ] **Step 5: Export the module**

Change `apps/desktop/src-tauri/src/inference/mod.rs` to:

```rust
pub mod cleanup_runtime;
pub mod resources;
```

- [ ] **Step 6: Run focused tests**

```bash
cargo test -p wispergo-desktop --lib inference::cleanup_runtime::tests
```

Expected: all pass.

- [ ] **Step 7: Commit Task 5**

```bash
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/src/inference/mod.rs apps/desktop/src-tauri/src/inference/cleanup_runtime.rs
git commit -m "feat: manage bundled cleanup runtime"
```

### Task 6: Wire cleanup runtime and provider selection into recording

**Files:**
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/commands/recording.rs`
- Modify: `apps/desktop/src-tauri/src/commands/settings.rs`

- [ ] **Step 1: Add failing recording tests for generic text cleanup**

In `apps/desktop/src-tauri/src/commands/recording.rs` tests, add imports:

```rust
use wispergo_core::domain::{PipelineResult, ProviderSource};
use wispergo_core::providers::{AsrOutput, CleanupOutput, FakeTextCleanupProvider, ProviderError, TextCleanupProvider};
```

Append these tests:

```rust
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
        confidence: Some(0.9),
        source: ProviderSource::Local,
    };

    let result = super::apply_cleanup_mode(asr, CleanupMode::PunctuationOnly, Some(&provider)).await;

    assert_eq!(
        result,
        PipelineResult::InsertText {
            text: "Hello, world.".to_string(),
            source: ProviderSource::Local,
            confidence: Some(0.9),
        }
    );
}

#[tokio::test]
async fn punctuation_cleanup_timeout_falls_back_to_raw_asr() {
    let provider = FakeTextCleanupProvider::new(
        Err(ProviderError::Timeout {
            provider: "llama_server".to_string(),
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
        confidence: None,
        source: ProviderSource::Local,
    };

    let result = super::apply_cleanup_mode(asr, CleanupMode::PunctuationOnly, Some(&provider)).await;

    assert_eq!(
        result,
        PipelineResult::InsertText {
            text: "hello world".to_string(),
            source: ProviderSource::Local,
            confidence: None,
        }
    );
}

#[tokio::test]
async fn punctuation_cleanup_invalid_output_falls_back_to_raw_asr() {
    let provider = FakeTextCleanupProvider::new(
        Err(ProviderError::InvalidOutput {
            provider: "llama_server".to_string(),
            message: "empty output".to_string(),
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
        confidence: None,
        source: ProviderSource::Local,
    };

    let result = super::apply_cleanup_mode(asr, CleanupMode::PunctuationOnly, Some(&provider)).await;

    assert_eq!(
        result,
        PipelineResult::InsertText {
            text: "hello world".to_string(),
            source: ProviderSource::Local,
            confidence: None,
        }
    );
}

#[tokio::test]
async fn punctuation_cleanup_without_provider_falls_back_to_raw_asr() {
    let asr = AsrOutput {
        transcript: "hello world".to_string(),
        confidence: None,
        source: ProviderSource::Local,
    };

    let result = super::apply_cleanup_mode(asr, CleanupMode::PunctuationOnly, None).await;

    assert_eq!(
        result,
        PipelineResult::InsertText {
            text: "hello world".to_string(),
            source: ProviderSource::Local,
            confidence: None,
        }
    );
}
```

- [ ] **Step 2: Run tests and verify they fail**

```bash
cargo test -p wispergo-desktop --lib punctuation_cleanup_uses_text_cleanup_provider
cargo test -p wispergo-desktop --lib punctuation_cleanup_timeout_falls_back_to_raw_asr
cargo test -p wispergo-desktop --lib punctuation_cleanup_invalid_output_falls_back_to_raw_asr
cargo test -p wispergo-desktop --lib punctuation_cleanup_without_provider_falls_back_to_raw_asr
```

Expected: compile failure because `apply_cleanup_mode` still takes `Option<&OllamaCleanupProvider>`.

- [ ] **Step 3: Generalize `apply_cleanup_mode`**

In `apps/desktop/src-tauri/src/commands/recording.rs`, change imports:

```rust
use wispergo_core::providers::{
    AsrOutput, AsrProvider, CleanupInput, CleanupProvider, ProviderError, TextCleanupProvider,
};
```

Change the function signature:

```rust
async fn apply_cleanup_mode(
    asr: AsrOutput,
    cleanup_mode: CleanupMode,
    cleanup: Option<&dyn TextCleanupProvider>,
) -> PipelineResult {
```

No other body changes should be needed after Task 1 because both punctuation and full cleanup are methods on `TextCleanupProvider` via its `CleanupProvider` supertrait.

- [ ] **Step 4: Manage cleanup runtime in Tauri setup**

In `apps/desktop/src-tauri/src/lib.rs`, add imports:

```rust
use inference::cleanup_runtime::CleanupRuntimeManager;
use inference::resources::InferenceResourcePaths;
```

Before `.setup`, manage the runtime:

```rust
        .manage(AppState::default())
        .manage(CleanupRuntimeManager::default())
```

Because the existing code currently calls `.manage(AppState::default())` near the end of the builder chain, keep one `AppState` manage call only. The intended builder order is `.manage(AppState::default()).manage(CleanupRuntimeManager::default()).setup(...)`.

Inside `.setup`, after settings load, start the runtime when resources resolve:

```rust
            if let Ok(resource_root) = app.path().resource_dir() {
                let resources = InferenceResourcePaths::from_resource_root(resource_root);
                app.state::<CleanupRuntimeManager>()
                    .inner()
                    .start_background(resources);
            }
```

Change the end of `run()` from direct builder `.run(...).expect(...)` to build the app and intercept exit events:

The current code ends the builder chain with:

```rust
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
```

Replace only that ending with:

```rust
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app_handle.state::<CleanupRuntimeManager>().shutdown();
            }
        });
```

Preserve all existing setup, managed state, window event, and command handler code before the replaced ending.

- [ ] **Step 5: Pass runtime state into `stop_recording`**

Change the `stop_recording` command signature:

```rust
pub async fn stop_recording(
    app: AppHandle,
    state: State<'_, AppState>,
    cleanup_runtime: State<'_, CleanupRuntimeManager>,
    reason: String,
) -> Result<StopRecordingOutput, String> {
```

Import:

```rust
use crate::inference::cleanup_runtime::CleanupRuntimeManager;
```

Change the process call:

```rust
    let cleanup_provider = cleanup_provider_for_recording(cleanup_runtime.inner());
    let processed = process_recording(
        audio,
        state.local_model_settings(),
        bundled_resources.as_ref(),
        cleanup_provider.as_deref(),
    )
    .await?;
```

Change `process_recording` signature:

```rust
async fn process_recording(
    audio: Vec<f32>,
    settings: LocalModelSettings,
    bundled_resources: Option<&InferenceResourcePaths>,
    cleanup_provider: Option<&dyn TextCleanupProvider>,
) -> Result<ProcessedRecording, String> {
```

Change cleanup selection inside `process_recording`:

```rust
    let result = match cleanup_mode {
        CleanupMode::Off => apply_cleanup_mode(asr, CleanupMode::Off, None).await,
        CleanupMode::PunctuationOnly | CleanupMode::FullCleanup => {
            apply_cleanup_mode(asr, cleanup_mode, cleanup_provider).await
        }
    };
```

Add this helper:

```rust
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
```

Keep `ollama_cleanup_provider()` for developer override only.

- [ ] **Step 6: Add cleanup runtime status command**

In `apps/desktop/src-tauri/src/commands/settings.rs`, import:

```rust
use crate::inference::cleanup_runtime::{CleanupRuntimeManager, CleanupRuntimeStatus};
```

Add command:

```rust
#[tauri::command]
pub fn cleanup_runtime_status(
    cleanup_runtime: State<'_, CleanupRuntimeManager>,
) -> CleanupRuntimeStatus {
    cleanup_runtime.status()
}
```

In `apps/desktop/src-tauri/src/lib.rs`, import and register `cleanup_runtime_status` in `generate_handler!`.

- [ ] **Step 7: Run backend tests**

```bash
cargo test -p wispergo-desktop --lib punctuation_cleanup_uses_text_cleanup_provider
cargo test -p wispergo-desktop --lib punctuation_cleanup_timeout_falls_back_to_raw_asr
cargo test -p wispergo-desktop --lib punctuation_cleanup_invalid_output_falls_back_to_raw_asr
cargo test -p wispergo-desktop --lib punctuation_cleanup_without_provider_falls_back_to_raw_asr
cargo test -p wispergo-desktop --lib
```

Expected: all desktop lib tests pass.

- [ ] **Step 8: Commit Task 6**

```bash
git add apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/commands/recording.rs apps/desktop/src-tauri/src/commands/settings.rs
git commit -m "feat: use managed cleanup runtime in recording"
```

---

### Task 7: Replace user-facing Ollama setup UX with offline cleanup status

**Files:**
- Modify: `apps/desktop/src/types/pipeline.ts`
- Modify: `apps/desktop/src/lib/tauriApi.ts`
- Modify: `apps/desktop/src/app/App.tsx`
- Modify: `apps/desktop/src/features/settings/SettingsPanel.tsx`
- Modify: `apps/desktop/src/app/App.test.tsx`
- Modify: `apps/desktop/src/features/settings/SettingsPanel.test.tsx`

- [ ] **Step 1: Update frontend types**

In `apps/desktop/src/types/pipeline.ts`, replace `OllamaSetupStatus` with:

```ts
export type CleanupRuntimeState = "disabled" | "starting" | "ready" | "unavailable" | "failed";

export type CleanupRuntimeStatus = {
  state: CleanupRuntimeState;
  message?: string | null;
};
```

Remove `OllamaSetupStatus` from product component props. Product components must use `CleanupRuntimeStatus` and must not display model names, filenames, ports, or base URLs.

- [ ] **Step 2: Update Tauri API wrapper**

In `apps/desktop/src/lib/tauriApi.ts`, add:

```ts
export function cleanupRuntimeStatus(): Promise<CleanupRuntimeStatus> {
  return invoke<CleanupRuntimeStatus>("cleanup_runtime_status");
}
```

Remove normal product imports/calls to `ensureOllamaSetup` from UI code. Keep the wrapper only if backend command still exists for developer use.

- [ ] **Step 3: Update App state and polling**

In `apps/desktop/src/app/App.tsx`:

- Replace `ensureOllamaSetup` import with `cleanupRuntimeStatus`.
- Replace `OllamaSetupStatus` type import with `CleanupRuntimeStatus`.
- Rename state:

```ts
const [cleanupRuntime, setCleanupRuntime] = useState<CleanupRuntimeStatus | null>(null);
```

Replace the Ollama setup `useEffect` with:

```ts
  useEffect(() => {
    if (isRecorderSurface || isLanguageSurface || !modelSettingsLoaded) {
      return;
    }

    if (modelSettings.cleanupMode === "off") {
      setCleanupRuntime(null);
      return;
    }

    let mounted = true;
    const refresh = () => {
      void cleanupRuntimeStatus()
        .then((status) => {
          if (mounted) {
            setCleanupRuntime(status);
          }
        })
        .catch(() => {
          if (mounted) {
            setCleanupRuntime({
              state: "unavailable",
              message: "Offline punctuation is unavailable.",
            });
          }
        });
    };

    refresh();
    const interval = window.setInterval(refresh, 2_000);

    return () => {
      mounted = false;
      window.clearInterval(interval);
    };
  }, [isRecorderSurface, isLanguageSurface, modelSettingsLoaded, modelSettings.cleanupMode]);
```

Pass `cleanupRuntime={cleanupRuntime}` to `SettingsPanel`.

- [ ] **Step 4: Update SettingsPanel props and notice**

In `SettingsPanel.tsx`:

- Replace `OllamaSetupStatus` import with `CleanupRuntimeStatus`.
- Rename prop `ollamaSetup` to `cleanupRuntime`.
- Replace `OllamaSetupNotice` with:

```tsx
function CleanupRuntimeNotice({ status }: { status: CleanupRuntimeStatus }) {
  if (status.state === "ready") {
    return (
      <div className="cleanup-runtime" aria-live="polite">
        Offline punctuation ready.
      </div>
    );
  }

  if (status.state === "starting") {
    return (
      <div className="cleanup-runtime" aria-live="polite">
        Preparing offline punctuation. Wispergo will use raw transcripts until it is ready.
      </div>
    );
  }

  return (
    <div className="cleanup-runtime" aria-live="polite">
      {status.message ?? "Offline punctuation is unavailable."} Wispergo will use raw transcripts.
    </div>
  );
}
```

Render it with:

```tsx
{cleanupEnabled && cleanupRuntime ? <CleanupRuntimeNotice status={cleanupRuntime} /> : null}
```

Remove the user-facing Ollama install link, Ollama model text, Whisper binary path input, and Whisper model path input. Keep developer overrides available only through environment variables and backend code.

- [ ] **Step 5: Update frontend tests**

In `App.test.tsx` and `SettingsPanel.test.tsx`, replace expectations mentioning Ollama with these product-level expectations:

```ts
expect(screen.queryByText(/Install Ollama/i)).not.toBeInTheDocument();
expect(screen.getByText(/Offline punctuation ready/i)).toBeInTheDocument();
```

Add a SettingsPanel test confirming normal users do not see model path fields:

```tsx
it("hides developer model path fields from normal settings", () => {
  render(defaultSettingsPanel());

  expect(screen.queryByLabelText(/Whisper binary path/i)).not.toBeInTheDocument();
  expect(screen.queryByLabelText(/Whisper model path/i)).not.toBeInTheDocument();
});
```

Add this helper above the test when the file does not already define one:

```tsx
function defaultSettingsPanel() {
  return (
    <SettingsPanel
      fallbackPolicy="local_only"
      microphones={[]}
      selectedMicrophoneId={null}
      microphone={{ granted: true, canPrompt: false }}
      accessibility={{ granted: true, canPrompt: false }}
      modelSettings={{
        whisperBinaryPath: "",
        whisperModelPath: "",
        recognitionLanguage: "auto",
        cleanupMode: "punctuation_only",
      }}
      cleanupRuntime={{ state: "ready", message: null }}
      onMicrophoneChange={() => undefined}
      onRefreshMicrophones={() => undefined}
      onRefreshAccessibility={() => undefined}
      onRequestMicrophoneAccess={() => undefined}
      onRequestAccessibility={() => undefined}
      onModelSettingsSave={() => undefined}
    />
  );
}
```

Add a SettingsPanel test for unavailable cleanup:

```tsx
it("shows raw transcript fallback when offline punctuation is unavailable", () => {
  render(
    <SettingsPanel
      fallbackPolicy="local_only"
      microphones={[]}
      selectedMicrophoneId={null}
      microphone={{ granted: true, canPrompt: false }}
      accessibility={{ granted: true, canPrompt: false }}
      modelSettings={{
        whisperBinaryPath: "",
        whisperModelPath: "",
        recognitionLanguage: "auto",
        cleanupMode: "punctuation_only",
      }}
      cleanupRuntime={{
        state: "unavailable",
        message: "Offline punctuation is unavailable.",
      }}
      onMicrophoneChange={() => undefined}
      onRefreshMicrophones={() => undefined}
      onRefreshAccessibility={() => undefined}
      onRequestMicrophoneAccess={() => undefined}
      onRequestAccessibility={() => undefined}
      onModelSettingsSave={() => undefined}
    />,
  );

  expect(screen.getByText(/Offline punctuation is unavailable/i)).toBeInTheDocument();
  expect(screen.getByText(/raw transcripts/i)).toBeInTheDocument();
});
```

- [ ] **Step 6: Run frontend tests**

```bash
pnpm --dir apps/desktop test
```

Expected: all Vitest tests pass.

- [ ] **Step 7: Commit Task 7**

```bash
git add apps/desktop/src/types/pipeline.ts apps/desktop/src/lib/tauriApi.ts apps/desktop/src/app/App.tsx apps/desktop/src/features/settings/SettingsPanel.tsx apps/desktop/src/app/App.test.tsx apps/desktop/src/features/settings/SettingsPanel.test.tsx
git commit -m "feat: show offline cleanup runtime status"
```

---

### Task 8: Add bundle resources layout and asset verification

**Files:**
- Modify: `.gitignore`
- Modify: `apps/desktop/src-tauri/tauri.conf.json`
- Create: `apps/desktop/src-tauri/resources/bin/macos-aarch64/.gitkeep`
- Create: `apps/desktop/src-tauri/resources/bin/macos-x86_64/.gitkeep`
- Create: `apps/desktop/src-tauri/resources/models/asr/.gitkeep`
- Create: `apps/desktop/src-tauri/resources/models/cleanup/.gitkeep`
- Create: `scripts/verify-inference-assets.sh`
- Create: `scripts/check-macos-bundle-inference-layout.sh`
- Modify: `package.json`

- [ ] **Step 1: Create tracked resource directories**

Run:

```bash
mkdir -p apps/desktop/src-tauri/resources/bin/macos-aarch64 \
  apps/desktop/src-tauri/resources/bin/macos-x86_64 \
  apps/desktop/src-tauri/resources/models/asr \
  apps/desktop/src-tauri/resources/models/cleanup

touch apps/desktop/src-tauri/resources/bin/macos-aarch64/.gitkeep \
  apps/desktop/src-tauri/resources/bin/macos-x86_64/.gitkeep \
  apps/desktop/src-tauri/resources/models/asr/.gitkeep \
  apps/desktop/src-tauri/resources/models/cleanup/.gitkeep
```

- [ ] **Step 2: Ignore large staged binaries/models while keeping `.gitkeep` files**

Append to `.gitignore`:

```gitignore
apps/desktop/src-tauri/resources/bin/macos-aarch64/*
apps/desktop/src-tauri/resources/bin/macos-x86_64/*
apps/desktop/src-tauri/resources/models/asr/*
apps/desktop/src-tauri/resources/models/cleanup/*
!apps/desktop/src-tauri/resources/bin/macos-aarch64/.gitkeep
!apps/desktop/src-tauri/resources/bin/macos-x86_64/.gitkeep
!apps/desktop/src-tauri/resources/models/asr/.gitkeep
!apps/desktop/src-tauri/resources/models/cleanup/.gitkeep
```

- [ ] **Step 3: Add Tauri resources mapping**

In `apps/desktop/src-tauri/tauri.conf.json`, add this property inside the existing `bundle` object:

```json
"resources": {
  "resources/": ""
},
```

The resulting `bundle` object should contain `active`, `targets`, `icon`, `resources`, and `macOS`.

- [ ] **Step 4: Add asset verification script**

Create `scripts/verify-inference-assets.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESOURCE_DIR="$ROOT_DIR/apps/desktop/src-tauri/resources"

required=(
  "bin/macos-aarch64/whisper-cli"
  "bin/macos-aarch64/llama-server"
  "bin/macos-x86_64/whisper-cli"
  "bin/macos-x86_64/llama-server"
  "models/asr/ggml-large-v3-turbo.bin"
  "models/cleanup/qwen2.5-3b-instruct-q4_k_m.gguf"
)

missing=()
for relative in "${required[@]}"; do
  path="$RESOURCE_DIR/$relative"
  if [[ ! -e "$path" ]]; then
    missing+=("$relative")
  fi
  if [[ "$relative" == bin/* && -e "$path" && ! -x "$path" ]]; then
    echo "Inference binary is not executable: $relative" >&2
    exit 1
  fi
done

if (( ${#missing[@]} > 0 )); then
  echo "Missing bundled inference assets:" >&2
  printf '  - %s\n' "${missing[@]}" >&2
  echo "Stage whisper.cpp, llama.cpp, ggml-large-v3-turbo, and Qwen2.5-3B GGUF assets before release packaging." >&2
  exit 1
fi

echo "Bundled inference assets verified."
```

Run:

```bash
chmod +x scripts/verify-inference-assets.sh
```

- [ ] **Step 5: Add bundle layout check script**

Create `scripts/check-macos-bundle-inference-layout.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_PATH="${1:-$ROOT_DIR/target/release/bundle/macos/Wispergo.app}"
RESOURCE_DIR="$APP_PATH/Contents/Resources"

required_dirs=(
  "bin/macos-aarch64"
  "bin/macos-x86_64"
  "models/asr"
  "models/cleanup"
)

if [[ ! -d "$APP_PATH" ]]; then
  echo "Built app bundle not found: $APP_PATH" >&2
  exit 1
fi

missing=()
for relative in "${required_dirs[@]}"; do
  if [[ ! -d "$RESOURCE_DIR/$relative" ]]; then
    missing+=("$relative")
  fi
done

if (( ${#missing[@]} > 0 )); then
  echo "Built app bundle is missing inference resource directories:" >&2
  printf '  - Contents/Resources/%s\n' "${missing[@]}" >&2
  exit 1
fi

echo "Built app inference resource layout verified."
```

Run:

```bash
chmod +x scripts/check-macos-bundle-inference-layout.sh
```

- [ ] **Step 6: Add package scripts**

In root `package.json`, add scripts without changing existing scripts:

```json
"verify:inference-assets": "./scripts/verify-inference-assets.sh",
"check:bundle-inference-layout": "./scripts/check-macos-bundle-inference-layout.sh",
"desktop:build:offline-release": "pnpm verify:inference-assets && pnpm desktop:build && pnpm check:bundle-inference-layout"
```

Keep `desktop:build` unchanged so developer builds can still run without multi-gigabyte staged assets.

- [ ] **Step 7: Verify developer build still works without staged assets**

Run:

```bash
pnpm --dir apps/desktop build
```

Expected: frontend build passes. Do not run `pnpm desktop:build:offline-release` until real assets are staged.

- [ ] **Step 8: Verify the asset script fails clearly without assets**

Run:

```bash
./scripts/verify-inference-assets.sh
```

Expected: exit code 1 and output listing the six missing assets. This is a successful test for the script in a dev checkout.

- [ ] **Step 9: Verify bundle layout after a desktop build**

Run this after `pnpm desktop:build` has produced an app bundle:

```bash
./scripts/check-macos-bundle-inference-layout.sh
```

Expected: `Built app inference resource layout verified.`

- [ ] **Step 10: Commit Task 8**

```bash
git add .gitignore package.json apps/desktop/src-tauri/tauri.conf.json scripts/verify-inference-assets.sh scripts/check-macos-bundle-inference-layout.sh apps/desktop/src-tauri/resources/bin/macos-aarch64/.gitkeep apps/desktop/src-tauri/resources/bin/macos-x86_64/.gitkeep apps/desktop/src-tauri/resources/models/asr/.gitkeep apps/desktop/src-tauri/resources/models/cleanup/.gitkeep
git commit -m "build: prepare bundled inference assets"
```

---

### Task 9: Update documentation and manual quality evaluation fixture

**Files:**
- Modify: `README.md`
- Create: `docs/manual/offline-cleanup-eval.md`

- [ ] **Step 1: Update README product behavior**

In `README.md`, replace user-facing Ollama setup language with this text:

```markdown
### Offline inference

Wispergo is designed to run fully offline on macOS. Product builds bundle:

- whisper.cpp for speech recognition
- `ggml-large-v3-turbo` for ASR
- llama.cpp `llama-server` for cleanup
- a Qwen2.5-3B-Instruct GGUF cleanup model

Normal users should not need to install Ollama, whisper.cpp, llama.cpp, or model files separately.

Developer builds can still override local inference paths with:

- `WISPERGO_WHISPER_BIN`
- `WISPERGO_WHISPER_MODEL`
- `WISPERGO_CLEANUP_BACKEND=ollama`
- `WISPERGO_OLLAMA_BASE_URL`
- `WISPERGO_OLLAMA_MODEL`

For release packaging, stage bundled inference assets under `apps/desktop/src-tauri/resources/` and run:

```bash
pnpm desktop:build:offline-release
```
```

- [ ] **Step 2: Add manual evaluation fixture**

Create `docs/manual/offline-cleanup-eval.md`:

```markdown
# Offline Cleanup Manual Evaluation

Use this fixture after staging the bundled llama.cpp cleanup model.

## Environment

- App build:
- Mac model:
- Architecture:
- ASR model: ggml-large-v3-turbo
- Cleanup model: qwen2.5-3b-instruct-q4_k_m.gguf
- Cleanup mode: punctuation_only

## Cases

| Case | Spoken content | Expected cleanup behavior | Raw ASR | Cleanup output | ASR ms | Cleanup ms | Notes |
|---|---|---|---|---|---:|---:|---|
| English sentence | hello world this is a test | Adds comma/period only, preserves words | | | | | |
| English question | can you send this to me tomorrow | Adds question mark if model infers question | | | | | |
| Chinese sentence | 你好世界這是一個測試 | Adds Chinese punctuation, preserves script | | | | | |
| Chinese question | 你明天可以傳給我嗎 | Adds Chinese question mark | | | | | |
| Mixed English Chinese | 今天review the pull request可以嗎 | Preserves mixed language and adds punctuation | | | | | |
| Already punctuated | Hello, world. | Cleanup should be skipped by heuristic | | | | | |

## Pass criteria

- Cleanup does not translate Chinese to English or English to Chinese.
- Cleanup does not add or remove meaningful words.
- Cleanup latency is acceptable on Apple Silicon.
- Intel Macs may fall back to raw ASR if cleanup exceeds the internal timeout.
```

- [ ] **Step 3: Commit Task 9**

```bash
git add README.md docs/manual/offline-cleanup-eval.md
git commit -m "docs: describe bundled offline inference"
```

---

### Task 10: Full verification and review

**Files:**
- No intended code changes unless verification finds defects.

- [ ] **Step 1: Run Rust formatting and tests**

```bash
cargo fmt --check
cargo test --workspace
```

Expected: formatting passes and all Rust tests pass.

- [ ] **Step 2: Run frontend tests and build**

```bash
pnpm --dir apps/desktop test
pnpm --dir apps/desktop build
```

Expected: all Vitest tests pass and Vite build succeeds.

- [ ] **Step 3: Run developer desktop build**

```bash
pnpm desktop:build
```

Expected: developer build succeeds even if real inference assets are not staged. The `.gitkeep` files keep resource directories present so the bundle layout check can verify directory placement.

Run:

```bash
./scripts/check-macos-bundle-inference-layout.sh
```

Expected: `Built app inference resource layout verified.`

- [ ] **Step 4: Run release asset verification script**

Without staged assets, run:

```bash
./scripts/verify-inference-assets.sh
```

Expected: exit code 1 listing missing assets. With staged assets, expected output is:

```text
Bundled inference assets verified.
```

- [ ] **Step 5: Request final code review with subagents**

Use two fresh reviewer subagents:

1. Spec compliance reviewer: verify implementation matches `docs/superpowers/specs/2026-05-01-offline-apple-inference-design.md` and this plan.
2. Code quality reviewer: inspect runtime lifecycle, provider abstraction, Tauri state, error handling, tests, and packaging scripts.

- [ ] **Step 6: Commit any review fixes**

When reviewers request changes, fix them in focused commits. Re-run the relevant verification commands before claiming completion.

---

## Validation Summary

Final implementation is not complete until these pass:

```bash
cargo fmt --check
cargo test --workspace
pnpm --dir apps/desktop test
pnpm --dir apps/desktop build
pnpm desktop:build
```

Release packaging additionally requires staged assets and:

```bash
pnpm desktop:build:offline-release
```

## Notes for Subagents

- Do not start implementation on `main`; use this `feature/cleanup-mode` worktree.
- Do not run implementation subagents in parallel because tasks touch shared files.
- Use TDD: write failing tests first, then implementation, then focused tests, then commit.
- Keep Ollama as a developer override only; do not show Ollama install instructions to normal users.
- Do not log transcripts in timing diagnostics or runtime status messages.
- Do not commit large model or sidecar binary files; stage them under ignored resource paths for release builds.
