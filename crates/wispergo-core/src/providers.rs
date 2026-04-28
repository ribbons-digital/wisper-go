use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::{PipelineResult, ProviderSource};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AsrOutput {
    pub transcript: String,
    pub confidence: Option<f32>,
    pub source: ProviderSource,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CleanupInput {
    pub transcript: String,
    pub selected_text: Option<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CleanupOutput {
    pub result: PipelineResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    Unavailable {
        provider: String,
        message: Option<String>,
    },
    Timeout { provider: String },
    InvalidOutput { provider: String, message: String },
    Failed { provider: String, message: String },
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable { provider, .. } => write!(formatter, "{provider} is unavailable"),
            Self::Timeout { provider } => write!(formatter, "{provider} timed out"),
            Self::InvalidOutput { provider, .. } => {
                write!(
                    formatter,
                    "{provider} returned invalid output; diagnostic details are redacted"
                )
            }
            Self::Failed { provider, .. } => {
                write!(formatter, "{provider} failed; diagnostic details are redacted")
            }
        }
    }
}

impl std::error::Error for ProviderError {}

impl ProviderError {
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Unavailable { .. } | Self::Timeout { .. } | Self::InvalidOutput { .. }
        )
    }

    pub fn diagnostic_message(&self) -> Option<&str> {
        match self {
            Self::Unavailable {
                message: Some(message),
                ..
            }
            | Self::InvalidOutput { message, .. }
            | Self::Failed { message, .. } => Some(message),
            Self::Unavailable { message: None, .. } | Self::Timeout { .. } => None,
        }
    }
}

#[async_trait]
pub trait AsrProvider: Send + Sync {
    async fn transcribe(&self, audio: Vec<f32>) -> Result<AsrOutput, ProviderError>;
}

#[async_trait]
pub trait CleanupProvider: Send + Sync {
    async fn clean(&self, input: CleanupInput) -> Result<CleanupOutput, ProviderError>;
}

#[derive(Debug, Clone)]
pub struct FakeAsrProvider {
    response: Result<AsrOutput, ProviderError>,
    calls: Option<Arc<Mutex<usize>>>,
}

impl FakeAsrProvider {
    pub fn new(response: Result<AsrOutput, ProviderError>) -> Self {
        Self {
            response,
            calls: None,
        }
    }

    pub fn with_counter(
        response: Result<AsrOutput, ProviderError>,
        calls: Arc<Mutex<usize>>,
    ) -> Self {
        Self {
            response,
            calls: Some(calls),
        }
    }
}

#[async_trait]
impl AsrProvider for FakeAsrProvider {
    async fn transcribe(&self, _audio: Vec<f32>) -> Result<AsrOutput, ProviderError> {
        if let Some(calls) = &self.calls {
            *calls.lock().expect("fake asr counter lock") += 1;
        }
        self.response.clone()
    }
}

#[derive(Debug, Clone)]
pub struct FakeCleanupProvider {
    response: Result<CleanupOutput, ProviderError>,
}

impl FakeCleanupProvider {
    pub fn new(response: Result<CleanupOutput, ProviderError>) -> Self {
        Self { response }
    }
}

#[async_trait]
impl CleanupProvider for FakeCleanupProvider {
    async fn clean(&self, _input: CleanupInput) -> Result<CleanupOutput, ProviderError> {
        self.response.clone()
    }
}
