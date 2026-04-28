use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::Command;
use tokio::time;

use crate::domain::ProviderSource;
use crate::providers::{
    AsrOutput, AsrProvider, ProviderError, ASR_INPUT_CHANNELS, ASR_INPUT_SAMPLE_RATE_HZ,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
pub const WHISPER_SIDECAR_BITS_PER_SAMPLE: u16 = 16;

#[derive(Debug, Clone)]
pub struct WhisperSidecarProvider {
    binary_path: PathBuf,
    model_path: Option<PathBuf>,
    timeout: Duration,
}

impl WhisperSidecarProvider {
    pub fn new(binary_path: PathBuf, model_path: Option<PathBuf>) -> Self {
        Self {
            binary_path,
            model_path,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl AsrProvider for WhisperSidecarProvider {
    async fn transcribe(&self, audio: Vec<f32>) -> Result<AsrOutput, ProviderError> {
        let wav = write_temp_wav(&audio)?;
        let mut command = Command::new(&self.binary_path);

        if let Some(model_path) = &self.model_path {
            command.arg("--model").arg(model_path);
        }
        command.arg("--file").arg(wav.path());
        command.kill_on_drop(true);

        let output = match time::timeout(self.timeout, command.output()).await {
            Ok(output) => output.map_err(|err| ProviderError::Unavailable {
                provider: "whisper_sidecar".to_string(),
                message: Some(err.to_string()),
            })?,
            Err(_) => {
                return Err(ProviderError::Timeout {
                    provider: "whisper_sidecar".to_string(),
                })
            }
        };

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

fn write_temp_wav(samples: &[f32]) -> Result<tempfile::NamedTempFile, ProviderError> {
    if samples.is_empty() {
        return Err(ProviderError::InvalidOutput {
            provider: "whisper_sidecar".to_string(),
            message: "empty audio".to_string(),
        });
    }

    let mut file = tempfile::Builder::new()
        .prefix("wispergo-")
        .suffix(".wav")
        .tempfile()
        .map_err(|err| ProviderError::Failed {
            provider: "whisper_sidecar".to_string(),
            message: err.to_string(),
        })?;

    write_wav_16khz_mono(file.as_file_mut(), samples).map_err(|err| ProviderError::Failed {
        provider: "whisper_sidecar".to_string(),
        message: err.to_string(),
    })?;

    Ok(file)
}

fn write_wav_16khz_mono(writer: &mut impl Write, samples: &[f32]) -> std::io::Result<()> {
    const BYTES_PER_SAMPLE: u16 = WHISPER_SIDECAR_BITS_PER_SAMPLE / 8;

    let data_len = samples.len() as u32 * u32::from(BYTES_PER_SAMPLE);
    writer.write_all(b"RIFF")?;
    writer.write_all(&(36 + data_len).to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?;
    writer.write_all(&1u16.to_le_bytes())?;
    writer.write_all(&ASR_INPUT_CHANNELS.to_le_bytes())?;
    writer.write_all(&ASR_INPUT_SAMPLE_RATE_HZ.to_le_bytes())?;
    writer.write_all(
        &(ASR_INPUT_SAMPLE_RATE_HZ * u32::from(ASR_INPUT_CHANNELS) * u32::from(BYTES_PER_SAMPLE))
            .to_le_bytes(),
    )?;
    writer.write_all(&(ASR_INPUT_CHANNELS * BYTES_PER_SAMPLE).to_le_bytes())?;
    writer.write_all(&WHISPER_SIDECAR_BITS_PER_SAMPLE.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_len.to_le_bytes())?;

    for sample in samples {
        let pcm = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        writer.write_all(&pcm.to_le_bytes())?;
    }

    writer.flush()
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
