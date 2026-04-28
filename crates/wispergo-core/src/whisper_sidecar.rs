use std::path::PathBuf;

use async_trait::async_trait;
use tokio::process::Command;

use crate::domain::ProviderSource;
use crate::providers::{AsrOutput, AsrProvider, ProviderError};

#[derive(Debug, Clone)]
pub struct WhisperSidecarProvider {
    binary_path: PathBuf,
    model_path: Option<PathBuf>,
}

impl WhisperSidecarProvider {
    pub fn new(binary_path: PathBuf, model_path: Option<PathBuf>) -> Self {
        Self {
            binary_path,
            model_path,
        }
    }
}

#[async_trait]
impl AsrProvider for WhisperSidecarProvider {
    async fn transcribe(&self, _audio: Vec<f32>) -> Result<AsrOutput, ProviderError> {
        let mut command = Command::new(&self.binary_path);

        if let Some(model_path) = &self.model_path {
            command.arg("--model").arg(model_path);
        }

        let output = command
            .output()
            .await
            .map_err(|err| ProviderError::Unavailable {
                provider: "whisper_sidecar".to_string(),
                message: Some(err.to_string()),
            })?;

        if !output.status.success() {
            return Err(ProviderError::Failed {
                provider: "whisper_sidecar".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(AsrOutput {
            transcript: parse_whisper_output(&stdout)?,
            confidence: None,
            source: ProviderSource::Local,
        })
    }
}

pub fn parse_whisper_output(output: &str) -> Result<String, ProviderError> {
    let transcript = output.trim().to_string();
    if transcript.is_empty() {
        return Err(ProviderError::InvalidOutput {
            provider: "whisper_sidecar".to_string(),
            message: "empty transcript".to_string(),
        });
    }
    Ok(transcript)
}
