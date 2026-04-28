use std::time::Duration;

use crate::domain::{ActiveContext, CommandSource, PipelineResult, ProviderSource};
use crate::intent::{IntentEngine, IntentParse};
use crate::privacy::PrivacyPolicy;
use crate::providers::{AsrProvider, CleanupInput, CleanupProvider, ProviderError};

#[derive(Debug, Clone)]
pub struct PipelineInput {
    pub audio: Vec<f32>,
    pub context: ActiveContext,
    pub cleanup_timeout: Duration,
}

pub struct Pipeline<A, C>
where
    A: AsrProvider,
    C: CleanupProvider,
{
    asr: A,
    cleanup: C,
    intent: IntentEngine,
    _policy: PrivacyPolicy,
}

impl<A, C> Pipeline<A, C>
where
    A: AsrProvider,
    C: CleanupProvider,
{
    pub fn new(asr: A, cleanup: C, policy: PrivacyPolicy) -> Self {
        Self {
            asr,
            cleanup,
            intent: IntentEngine,
            _policy: policy,
        }
    }

    pub async fn run(&self, input: PipelineInput) -> PipelineResult {
        let asr = match self.asr.transcribe(input.audio).await {
            Ok(output) => output,
            Err(err) => {
                return PipelineResult::Error {
                    recoverable: err.is_recoverable(),
                    message: err.to_string(),
                }
            }
        };

        match self.intent.parse_rule(&asr.transcript) {
            IntentParse::Command {
                command,
                requires_confirmation,
            } => PipelineResult::Command {
                command,
                requires_confirmation,
                source: CommandSource::Rules,
            },
            IntentParse::Dictation { text } => {
                let cleanup_result = self
                    .cleanup
                    .clean(CleanupInput {
                        transcript: text.clone(),
                        selected_text: input.context.selected_text,
                        timeout: input.cleanup_timeout,
                    })
                    .await;

                match cleanup_result {
                    Ok(output) => output.result,
                    Err(ProviderError::Timeout { .. }) => PipelineResult::InsertText {
                        text,
                        source: ProviderSource::Local,
                        confidence: asr.confidence,
                    },
                    Err(err) => PipelineResult::Error {
                        recoverable: err.is_recoverable(),
                        message: err.to_string(),
                    },
                }
            }
        }
    }
}
