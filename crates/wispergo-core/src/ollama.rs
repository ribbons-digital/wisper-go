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

pub const DEFAULT_OLLAMA_MODEL: &str = "qwen2.5:0.5b";
const PROVIDER_NAME: &str = "ollama";

#[derive(Debug, Clone)]
pub struct OllamaCleanupProvider {
    base_url: String,
    model: String,
    client: Client,
}

impl OllamaCleanupProvider {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            client: Client::new(),
        }
    }

    pub async fn warm(&self, timeout: std::time::Duration) -> Result<(), ProviderError> {
        let request = OllamaChatRequest {
            model: self.model.clone(),
            stream: false,
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: "Reply with OK only.".to_string(),
                },
                OllamaMessage {
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
}

#[async_trait]
impl TextCleanupProvider for OllamaCleanupProvider {
    async fn clean_punctuation_only(&self, input: CleanupInput) -> Result<String, ProviderError> {
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
                provider: PROVIDER_NAME.to_string(),
            })??;

        parse_punctuation_cleanup_text(&body.message.content)
    }
}

#[async_trait]
impl CleanupProvider for OllamaCleanupProvider {
    async fn clean(&self, input: CleanupInput) -> Result<CleanupOutput, ProviderError> {
        let request = OllamaChatRequest {
            model: self.model.clone(),
            stream: false,
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: cleanup_system_prompt(),
                },
                OllamaMessage {
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

        parse_cleanup_json(&body.message.content)
    }
}

impl OllamaCleanupProvider {
    async fn send_chat(
        &self,
        request: OllamaChatRequest,
    ) -> Result<OllamaChatResponse, ProviderError> {
        let url = format!("{}/api/chat", self.base_url);
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
                message: format!("ollama returned HTTP status {status}"),
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

pub fn parse_punctuation_cleanup_text(input: &str) -> Result<String, ProviderError> {
    parse_punctuation_cleanup_text_for_provider(input, PROVIDER_NAME)
}

pub fn parse_cleanup_json(input: &str) -> Result<CleanupOutput, ProviderError> {
    parse_cleanup_json_for_provider(input, PROVIDER_NAME)
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    stream: bool,
    messages: Vec<OllamaMessage>,
}

#[derive(Debug, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    content: String,
}
