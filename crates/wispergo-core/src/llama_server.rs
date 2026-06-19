use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::cleanup_prompt::{
    cleanup_system_prompt, cleanup_user_prompt,
    parse_cleanup_json as parse_cleanup_json_for_provider,
    parse_punctuation_cleanup_text as parse_punctuation_cleanup_text_for_provider,
    punctuation_system_prompt, punctuation_user_prompt,
};
use crate::providers::{
    CleanupInput, CleanupOutput, CleanupProvider, ProviderError, TextCleanupProvider,
};

pub const DEFAULT_LLAMA_SERVER_MODEL: &str = "qwen2.5-3b-instruct";
const PROVIDER_NAME: &str = "llama_server";

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

    pub async fn warm(&self, timeout: Duration) -> Result<(), ProviderError> {
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
                provider: PROVIDER_NAME.to_string(),
            })??;
        Ok(())
    }

    async fn send_chat(
        &self,
        request: OpenAiChatRequest,
    ) -> Result<OpenAiChatResponse, ProviderError> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|err| ProviderError::Unavailable {
                provider: PROVIDER_NAME.to_string(),
                message: Some(err.to_string()),
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::Failed {
                provider: PROVIDER_NAME.to_string(),
                message: format!("llama-server returned HTTP status {status}"),
            });
        }

        response
            .json()
            .await
            .map_err(|err| ProviderError::InvalidOutput {
                provider: PROVIDER_NAME.to_string(),
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
                provider: PROVIDER_NAME.to_string(),
            })??;

        parse_punctuation_cleanup_text(&first_choice_content(body)?)
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
                provider: PROVIDER_NAME.to_string(),
            })??;

        parse_cleanup_json(&first_choice_content(body)?)
    }
}

fn first_choice_content(response: OpenAiChatResponse) -> Result<String, ProviderError> {
    response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .ok_or_else(|| ProviderError::InvalidOutput {
            provider: PROVIDER_NAME.to_string(),
            message: "OpenAI chat response contained no choices".to_string(),
        })
}

pub fn parse_punctuation_cleanup_text(input: &str) -> Result<String, ProviderError> {
    parse_punctuation_cleanup_text_for_provider(input, PROVIDER_NAME)
}

pub fn parse_cleanup_json(input: &str) -> Result<CleanupOutput, ProviderError> {
    parse_cleanup_json_for_provider(input, PROVIDER_NAME)
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
