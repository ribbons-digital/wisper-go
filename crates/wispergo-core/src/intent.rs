use crate::domain::{CommandAction, RewriteStyle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentParse {
    Dictation {
        text: String,
    },
    Command {
        command: CommandAction,
        requires_confirmation: bool,
    },
}

#[derive(Debug, Default, Clone)]
pub struct IntentEngine;

impl IntentEngine {
    pub fn parse_rule(&self, transcript: &str) -> IntentParse {
        let trimmed = transcript.trim();
        let normalized = normalize(trimmed);

        if let Some(rest) = normalized.strip_prefix("literal ") {
            return IntentParse::Dictation {
                text: rest.to_string(),
            };
        }

        match normalized.as_str() {
            "new line" => command(CommandAction::NewLine, false),
            "new paragraph" => command(CommandAction::NewParagraph, false),
            "cancel" | "cancel that" | "stop" => command(CommandAction::Cancel, false),
            "delete that" | "delete last phrase" | "delete previous phrase" => {
                command(CommandAction::DeletePreviousPhrase, true)
            }
            "make this a bullet list" | "format this as bullets" | "format as bullets" => {
                command(CommandAction::FormatSelectionBullets, false)
            }
            "make this a numbered list"
            | "format this as a numbered list"
            | "format as numbered list" => command(CommandAction::FormatSelectionNumbered, false),
            "rewrite this casually" => command(
                CommandAction::RewriteSelection {
                    style: RewriteStyle::Casual,
                },
                false,
            ),
            "rewrite this professionally" => command(
                CommandAction::RewriteSelection {
                    style: RewriteStyle::Professional,
                },
                false,
            ),
            "make this shorter" => command(
                CommandAction::RewriteSelection {
                    style: RewriteStyle::Shorter,
                },
                false,
            ),
            "make this longer" => command(
                CommandAction::RewriteSelection {
                    style: RewriteStyle::Longer,
                },
                false,
            ),
            "click submit" | "send this" | "open slack" => command(
                CommandAction::Unsupported {
                    reason: "app_control_out_of_scope".to_string(),
                },
                true,
            ),
            _ => IntentParse::Dictation {
                text: trimmed.to_string(),
            },
        }
    }
}

fn normalize(input: &str) -> String {
    input
        .to_lowercase()
        .trim_matches(|ch: char| ch.is_ascii_punctuation())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn command(command: CommandAction, requires_confirmation: bool) -> IntentParse {
    IntentParse::Command {
        command,
        requires_confirmation,
    }
}
