use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSource {
    Local,
    Cloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandSource {
    Rules,
    LocalLlm,
    CloudLlm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewriteStyle {
    Casual,
    Professional,
    Shorter,
    Longer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommandAction {
    NewLine,
    NewParagraph,
    Cancel,
    LiteralDictation { text: String },
    DeletePreviousPhrase,
    ReplaceSelection { text: String },
    RewriteSelection { style: RewriteStyle },
    FormatSelectionBullets,
    FormatSelectionNumbered,
    Unsupported { reason: String },
}

impl CommandAction {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NewLine => "new_line",
            Self::NewParagraph => "new_paragraph",
            Self::Cancel => "cancel",
            Self::LiteralDictation { .. } => "literal_dictation",
            Self::DeletePreviousPhrase => "delete_previous_phrase",
            Self::ReplaceSelection { .. } => "replace_selection",
            Self::RewriteSelection {
                style: RewriteStyle::Casual,
            } => "rewrite_selection_casual",
            Self::RewriteSelection {
                style: RewriteStyle::Professional,
            } => "rewrite_selection_professional",
            Self::RewriteSelection {
                style: RewriteStyle::Shorter,
            } => "rewrite_selection_shorter",
            Self::RewriteSelection {
                style: RewriteStyle::Longer,
            } => "rewrite_selection_longer",
            Self::FormatSelectionBullets => "format_selection_bullets",
            Self::FormatSelectionNumbered => "format_selection_numbered",
            Self::Unsupported { .. } => "unsupported",
        }
    }

    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            Self::DeletePreviousPhrase | Self::ReplaceSelection { .. } | Self::Unsupported { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PipelineResult {
    InsertText {
        text: String,
        source: ProviderSource,
        confidence: Option<f32>,
    },
    Command {
        command: CommandAction,
        requires_confirmation: bool,
        source: CommandSource,
    },
    Cancelled {
        reason: String,
    },
    Error {
        recoverable: bool,
        message: String,
    },
}

impl PipelineResult {
    pub fn requires_confirmation(&self) -> bool {
        matches!(
            self,
            Self::Command {
                requires_confirmation: true,
                ..
            }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingMode {
    PressAndHold,
    Toggle,
    FloatingButton,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveContext {
    pub app_id: String,
    pub app_name: String,
    pub window_title: Option<String>,
    pub selected_text: Option<String>,
    pub style_profile: Option<String>,
}
