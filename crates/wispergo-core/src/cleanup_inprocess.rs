//! In-process llama.cpp cleanup provider (Phase 3).
//!
//! Only compiled when the `llama-cpp` cargo feature is enabled, which builds
//! llama.cpp via `llama-cpp-sys-2` (requires cmake + clang). It is gated off
//! by default now that the cleanup sidecar has been retired.
//!
//! ## Slice 3.2 scope
//!
//! This module now contains the in-process `LlamaCppCleanupProvider` behind the
//! existing cleanup traits. It reuses the same prompt/parsing contract as the
//! former HTTP bridge; Phase 3.3 made it the product local cleanup backend.

#![cfg(feature = "llama-cpp")]

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::pin::pin;
use std::sync::Arc;

use async_trait::async_trait;

use crate::cleanup_prompt::{
    cleanup_system_prompt, cleanup_user_prompt,
    parse_cleanup_json as parse_cleanup_json_for_provider,
    parse_punctuation_cleanup_text as parse_punctuation_cleanup_text_for_provider,
    punctuation_system_prompt, punctuation_user_prompt,
};
use crate::providers::{
    CleanupInput, CleanupOutput, CleanupProvider, ProviderError, TextCleanupProvider,
};

const PROVIDER_NAME: &str = "llama_cpp";

/// Return a static string proving the `llama-cpp-2` dependency links and Metal
/// builds on arm64. Used by the 3.1 build-verification test as the slice DoD.
pub fn llama_cpp_linked() -> &'static str {
    "llama-cpp-2 linked"
}

#[derive(Debug, Clone)]
pub struct LlamaCppCleanupConfig {
    model_path: PathBuf,
    context_tokens: NonZeroU32,
    max_generated_tokens: usize,
    n_threads: i32,
}

impl LlamaCppCleanupConfig {
    pub fn new(model_path: PathBuf) -> Self {
        Self {
            model_path,
            context_tokens: NonZeroU32::new(2048).expect("non-zero context size"),
            max_generated_tokens: 512,
            n_threads: 4,
        }
    }

    pub fn model_path(&self) -> &std::path::Path {
        &self.model_path
    }

    pub fn context_tokens(&self) -> NonZeroU32 {
        self.context_tokens
    }

    pub fn max_generated_tokens(&self) -> usize {
        self.max_generated_tokens
    }

    pub fn n_threads(&self) -> i32 {
        self.n_threads
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CleanupChatMessage {
    role: String,
    content: String,
}

impl CleanupChatMessage {
    fn system(content: String) -> Self {
        Self {
            role: "system".to_string(),
            content,
        }
    }

    fn user(content: String) -> Self {
        Self {
            role: "user".to_string(),
            content,
        }
    }
}

#[async_trait]
trait CompletionEngine: Send + Sync {
    async fn complete(
        &self,
        messages: Vec<CleanupChatMessage>,
        timeout: std::time::Duration,
    ) -> Result<String, ProviderError>;
}

pub struct LlamaCppCleanupProvider {
    _config: LlamaCppCleanupConfig,
    completion_engine: Arc<dyn CompletionEngine>,
}

impl LlamaCppCleanupProvider {
    pub fn new(config: LlamaCppCleanupConfig) -> Self {
        Self {
            _config: config.clone(),
            completion_engine: Arc::new(LocalLlamaCompletionEngine { config }),
        }
    }

    #[cfg(test)]
    fn with_completion_engine(
        config: LlamaCppCleanupConfig,
        completion_engine: Arc<dyn CompletionEngine>,
    ) -> Self {
        Self {
            _config: config,
            completion_engine,
        }
    }
}

#[derive(Debug, Clone)]
struct LocalLlamaCompletionEngine {
    config: LlamaCppCleanupConfig,
}

#[async_trait]
impl CompletionEngine for LocalLlamaCompletionEngine {
    async fn complete(
        &self,
        messages: Vec<CleanupChatMessage>,
        timeout: std::time::Duration,
    ) -> Result<String, ProviderError> {
        let config = self.config.clone();
        tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || complete_with_local_llama(config, messages)),
        )
        .await
        .map_err(|_| ProviderError::Timeout {
            provider: PROVIDER_NAME.to_string(),
        })?
        .map_err(|err| ProviderError::Failed {
            provider: PROVIDER_NAME.to_string(),
            message: err.to_string(),
        })?
    }
}

fn complete_with_local_llama(
    config: LlamaCppCleanupConfig,
    messages: Vec<CleanupChatMessage>,
) -> Result<String, ProviderError> {
    use llama_cpp_2::context::params::LlamaContextParams;
    use llama_cpp_2::llama_backend::LlamaBackend;
    use llama_cpp_2::llama_batch::LlamaBatch;
    use llama_cpp_2::model::params::LlamaModelParams;
    use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaModel};
    use llama_cpp_2::sampling::LlamaSampler;

    if !config.model_path().exists() {
        return Err(ProviderError::Unavailable {
            provider: PROVIDER_NAME.to_string(),
            message: Some(format!(
                "cleanup GGUF does not exist: {}",
                config.model_path().display()
            )),
        });
    }

    let backend = LlamaBackend::init().map_err(|err| ProviderError::Unavailable {
        provider: PROVIDER_NAME.to_string(),
        message: Some(err.to_string()),
    })?;

    let model_params = LlamaModelParams::default().with_n_gpu_layers(u32::MAX);
    let model_params = pin!(model_params);
    let model = LlamaModel::load_from_file(&backend, config.model_path(), &model_params).map_err(
        |err| ProviderError::Unavailable {
            provider: PROVIDER_NAME.to_string(),
            message: Some(err.to_string()),
        },
    )?;

    let template = model
        .chat_template(None)
        .map_err(|err| ProviderError::Unavailable {
            provider: PROVIDER_NAME.to_string(),
            message: Some(format!("cleanup model has no usable chat template: {err}")),
        })?;
    let llama_messages = messages
        .into_iter()
        .map(|message| LlamaChatMessage::new(message.role, message.content))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| ProviderError::Failed {
            provider: PROVIDER_NAME.to_string(),
            message: err.to_string(),
        })?;
    let prompt = model
        .apply_chat_template(&template, &llama_messages, true)
        .map_err(|err| ProviderError::Failed {
            provider: PROVIDER_NAME.to_string(),
            message: err.to_string(),
        })?;
    let prompt_tokens =
        model
            .str_to_token(&prompt, AddBos::Always)
            .map_err(|err| ProviderError::Failed {
                provider: PROVIDER_NAME.to_string(),
                message: err.to_string(),
            })?;
    if prompt_tokens.is_empty() {
        return Err(ProviderError::InvalidOutput {
            provider: PROVIDER_NAME.to_string(),
            message: "cleanup prompt tokenized to an empty sequence".to_string(),
        });
    }

    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(config.context_tokens()))
        .with_n_threads(config.n_threads())
        .with_n_threads_batch(config.n_threads())
        .with_no_perf(true);
    let mut ctx =
        model
            .new_context(&backend, ctx_params)
            .map_err(|err| ProviderError::Unavailable {
                provider: PROVIDER_NAME.to_string(),
                message: Some(err.to_string()),
            })?;

    let batch_capacity = prompt_tokens.len().max(1);
    let mut batch = LlamaBatch::new(batch_capacity, 1);
    for (index, token) in prompt_tokens.iter().copied().enumerate() {
        let is_last = index + 1 == prompt_tokens.len();
        batch
            .add(
                token,
                i32::try_from(index).expect("prompt token position fits i32"),
                &[0],
                is_last,
            )
            .map_err(|err| ProviderError::Failed {
                provider: PROVIDER_NAME.to_string(),
                message: err.to_string(),
            })?;
    }
    ctx.decode(&mut batch)
        .map_err(|err| ProviderError::Failed {
            provider: PROVIDER_NAME.to_string(),
            message: err.to_string(),
        })?;

    let mut sampler = LlamaSampler::chain_simple([LlamaSampler::greedy()]);
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut output = String::new();
    let first_generated_pos = i32::try_from(prompt_tokens.len()).expect("prompt length fits i32");

    for n_cur in (first_generated_pos..).take(config.max_generated_tokens()) {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);
        if model.is_eog_token(token) {
            break;
        }

        let piece = model
            .token_to_piece(token, &mut decoder, false, None)
            .map_err(|err| ProviderError::Failed {
                provider: PROVIDER_NAME.to_string(),
                message: err.to_string(),
            })?;
        output.push_str(&piece);

        batch.clear();
        batch
            .add(token, n_cur, &[0], true)
            .map_err(|err| ProviderError::Failed {
                provider: PROVIDER_NAME.to_string(),
                message: err.to_string(),
            })?;
        ctx.decode(&mut batch)
            .map_err(|err| ProviderError::Failed {
                provider: PROVIDER_NAME.to_string(),
                message: err.to_string(),
            })?;
    }

    Ok(output)
}

#[async_trait]
impl TextCleanupProvider for LlamaCppCleanupProvider {
    async fn clean_punctuation_only(&self, input: CleanupInput) -> Result<String, ProviderError> {
        let messages = vec![
            CleanupChatMessage::system(punctuation_system_prompt()),
            CleanupChatMessage::user(punctuation_user_prompt(&input)),
        ];
        let output = self
            .completion_engine
            .complete(messages, input.timeout)
            .await?;

        parse_punctuation_cleanup_text_for_provider(&output, PROVIDER_NAME)
    }
}

#[async_trait]
impl CleanupProvider for LlamaCppCleanupProvider {
    async fn clean(&self, input: CleanupInput) -> Result<CleanupOutput, ProviderError> {
        let messages = vec![
            CleanupChatMessage::system(cleanup_system_prompt()),
            CleanupChatMessage::user(cleanup_user_prompt(&input)),
        ];
        let output = self
            .completion_engine
            .complete(messages, input.timeout)
            .await?;

        parse_cleanup_json_for_provider(&output, PROVIDER_NAME)
    }
}

#[cfg(test)]
#[cfg(feature = "llama-cpp")]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use async_trait::async_trait;

    use super::*;
    use crate::domain::{PipelineResult, ProviderSource};
    use crate::providers::{CleanupInput, CleanupProvider, TextCleanupProvider};

    /// Build-integration smoke test: proves llama-cpp-2 links and Metal builds
    /// on arm64. This is the slice 3.1 DoD.
    #[test]
    fn llama_cpp_links() {
        assert_eq!(llama_cpp_linked(), "llama-cpp-2 linked");
    }

    #[derive(Debug)]
    struct FakeCompletionEngine {
        output: String,
        seen_messages: Arc<Mutex<Vec<Vec<CleanupChatMessage>>>>,
    }

    #[async_trait]
    impl CompletionEngine for FakeCompletionEngine {
        async fn complete(
            &self,
            messages: Vec<CleanupChatMessage>,
            _timeout: Duration,
        ) -> Result<String, ProviderError> {
            self.seen_messages
                .lock()
                .expect("seen messages lock")
                .push(messages);
            Ok(self.output.clone())
        }
    }

    #[tokio::test]
    async fn punctuation_cleanup_uses_shared_prompt_contract_and_parser() {
        let seen_messages = Arc::new(Mutex::new(Vec::new()));
        let provider = LlamaCppCleanupProvider::with_completion_engine(
            LlamaCppCleanupConfig::new(PathBuf::from("/tmp/test.gguf")),
            Arc::new(FakeCompletionEngine {
                output: "Transcript: Hello, world.".to_string(),
                seen_messages: Arc::clone(&seen_messages),
            }),
        );

        let output = provider
            .clean_punctuation_only(CleanupInput {
                transcript: "hello world".to_string(),
                selected_text: Some("ignored".to_string()),
                timeout: Duration::from_secs(3),
            })
            .await
            .expect("punctuation cleanup output");

        assert_eq!(output, "Hello, world.");
        let messages = seen_messages.lock().expect("seen messages lock");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].len(), 2);
        assert_eq!(messages[0][0].role, "system");
        assert!(messages[0][0].content.contains("Punctuation-only cleanup"));
        assert_eq!(messages[0][1].role, "user");
        assert_eq!(messages[0][1].content, "Transcript: hello world");
    }

    #[tokio::test]
    async fn full_cleanup_uses_shared_prompt_contract_and_json_parser() {
        let seen_messages = Arc::new(Mutex::new(Vec::new()));
        let provider = LlamaCppCleanupProvider::with_completion_engine(
            LlamaCppCleanupConfig::new(PathBuf::from("/tmp/test.gguf")),
            Arc::new(FakeCompletionEngine {
                output: include_str!("../tests/fixtures/cleanup_insert_text.json").to_string(),
                seen_messages: Arc::clone(&seen_messages),
            }),
        );

        let output = provider
            .clean(CleanupInput {
                transcript: "hello world".to_string(),
                selected_text: Some("selected text".to_string()),
                timeout: Duration::from_secs(3),
            })
            .await
            .expect("cleanup output");

        assert_eq!(
            output.result,
            PipelineResult::InsertText {
                text: "Hello, world.".to_string(),
                source: ProviderSource::Local,
                confidence: Some(0.91),
            }
        );
        let messages = seen_messages.lock().expect("seen messages lock");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].len(), 2);
        assert_eq!(messages[0][0].role, "system");
        assert!(messages[0][0]
            .content
            .contains("Return only JSON matching the CleanupOutput schema"));
        assert_eq!(messages[0][1].role, "user");
        assert_eq!(
            messages[0][1].content,
            "Transcript: hello world\nSelected text: selected text"
        );
    }
}
