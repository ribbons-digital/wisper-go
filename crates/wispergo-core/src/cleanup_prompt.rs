use crate::domain::PipelineResult;
use crate::providers::{CleanupInput, CleanupOutput, ProviderError};

pub fn parse_punctuation_cleanup_text(
    input: &str,
    provider_name: &str,
) -> Result<String, ProviderError> {
    let text = strip_echoed_transcript_label(input.trim()).trim();
    if text.is_empty() {
        return Err(ProviderError::InvalidOutput {
            provider: provider_name.to_string(),
            message: "empty punctuation cleanup output".to_string(),
        });
    }

    Ok(text.to_string())
}

fn strip_echoed_transcript_label(text: &str) -> &str {
    if text.to_ascii_lowercase().starts_with("transcript:") {
        &text["transcript:".len()..]
    } else {
        text
    }
}

pub fn parse_cleanup_json(
    input: &str,
    provider_name: &str,
) -> Result<CleanupOutput, ProviderError> {
    let mut output = serde_json::from_str::<CleanupOutput>(input).map_err(|err| {
        ProviderError::InvalidOutput {
            provider: provider_name.to_string(),
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

pub fn cleanup_system_prompt() -> String {
    "Return only JSON matching the CleanupOutput schema. Do not execute commands. Classify user intent into insert_text, command, cancelled, or error results. Preserve the transcript's original language and script; do not translate between languages.".to_string()
}

pub fn cleanup_user_prompt(input: &CleanupInput) -> String {
    format!(
        "Transcript: {}\nSelected text: {}",
        input.transcript,
        input.selected_text.as_deref().unwrap_or("")
    )
}

pub fn punctuation_system_prompt() -> String {
    "Punctuation-only cleanup. Return only the corrected transcript as plain text. Add punctuation and capitalization only. Preserve the exact words, language, and script from the transcript. Do not translate, paraphrase, summarize, add or remove words, classify commands, or execute commands.".to_string()
}

pub fn punctuation_user_prompt(input: &CleanupInput) -> String {
    format!("Transcript: {}", input.transcript)
}
