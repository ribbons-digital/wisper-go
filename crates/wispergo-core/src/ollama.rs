use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::domain::PipelineResult;
use crate::providers::{CleanupInput, CleanupOutput, CleanupProvider, ProviderError};

pub const DEFAULT_OLLAMA_MODEL: &str = "qwen2.5:0.5b";

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
                provider: "ollama".to_string(),
            })??;
        Ok(())
    }

    pub async fn clean_punctuation_only(
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
                provider: "ollama".to_string(),
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
                provider: "ollama".to_string(),
                message: Some(err.to_string()),
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::Failed {
                provider: "ollama".to_string(),
                message: format!("ollama returned HTTP status {status}"),
            });
        }

        response
            .json()
            .await
            .map_err(|err| ProviderError::InvalidOutput {
                provider: "ollama".to_string(),
                message: err.to_string(),
            })
    }
}

pub fn parse_punctuation_cleanup_text(input: &str) -> Result<String, ProviderError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(ProviderError::InvalidOutput {
            provider: "ollama".to_string(),
            message: "empty punctuation cleanup output".to_string(),
        });
    }

    Ok(text.to_string())
}

pub fn parse_cleanup_json(input: &str) -> Result<CleanupOutput, ProviderError> {
    let mut output = serde_json::from_str::<CleanupOutput>(input).map_err(|err| {
        ProviderError::InvalidOutput {
            provider: "ollama".to_string(),
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
