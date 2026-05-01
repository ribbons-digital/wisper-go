use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::domain::PipelineResult;
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
    let text = input.trim();
    if text.is_empty() {
        return Err(ProviderError::InvalidOutput {
            provider: PROVIDER_NAME.to_string(),
            message: "empty punctuation cleanup output".to_string(),
        });
    }

    Ok(text.to_string())
}

pub fn parse_cleanup_json(input: &str) -> Result<CleanupOutput, ProviderError> {
    let mut output = serde_json::from_str::<CleanupOutput>(input).map_err(|err| {
        ProviderError::InvalidOutput {
            provider: PROVIDER_NAME.to_string(),
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
