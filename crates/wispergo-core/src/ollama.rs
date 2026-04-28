use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::providers::{CleanupInput, CleanupOutput, CleanupProvider, ProviderError};

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

        let url = format!("{}/api/chat", self.base_url);
        let response =
            tokio::time::timeout(input.timeout, self.client.post(url).json(&request).send())
                .await
                .map_err(|_| ProviderError::Timeout {
                    provider: "ollama".to_string(),
                })?
                .map_err(|err| ProviderError::Unavailable {
                    provider: format!("ollama: {err}"),
                })?;

        let body: OllamaChatResponse =
            response
                .json()
                .await
                .map_err(|err| ProviderError::InvalidOutput {
                    provider: "ollama".to_string(),
                    message: err.to_string(),
                })?;

        parse_cleanup_json(&body.message.content)
    }
}

pub fn parse_cleanup_json(input: &str) -> Result<CleanupOutput, ProviderError> {
    serde_json::from_str::<CleanupOutput>(input).map_err(|err| ProviderError::InvalidOutput {
        provider: "ollama".to_string(),
        message: err.to_string(),
    })
}

fn cleanup_system_prompt() -> String {
    "Return only JSON matching the CleanupOutput schema. Do not execute commands. Classify user intent into insert_text, command, cancelled, or error results.".to_string()
}

fn cleanup_user_prompt(input: &CleanupInput) -> String {
    format!(
        "Transcript: {}\nSelected text: {}",
        input.transcript,
        input.selected_text.as_deref().unwrap_or("")
    )
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
