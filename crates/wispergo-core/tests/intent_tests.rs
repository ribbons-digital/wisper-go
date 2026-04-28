use wispergo_core::domain::{CommandAction, RewriteStyle};
use wispergo_core::intent::{IntentEngine, IntentParse};

#[test]
fn explicit_new_line_is_rule_command() {
    let result = IntentEngine::default().parse_rule("new line");

    assert_eq!(
        result,
        IntentParse::Command {
            command: CommandAction::NewLine,
            requires_confirmation: false
        }
    );
}

#[test]
fn literal_mode_keeps_command_words_as_text() {
    let result = IntentEngine::default().parse_rule("literal new paragraph");

    assert_eq!(
        result,
        IntentParse::Dictation {
            text: "new paragraph".to_string()
        }
    );
}

#[test]
fn destructive_delete_requires_confirmation() {
    let result = IntentEngine::default().parse_rule("delete that");

    assert_eq!(
        result,
        IntentParse::Command {
            command: CommandAction::DeletePreviousPhrase,
            requires_confirmation: true
        }
    );
}

#[test]
fn rewrite_selection_maps_to_professional_style() {
    let result = IntentEngine::default().parse_rule("rewrite this professionally");

    assert_eq!(
        result,
        IntentParse::Command {
            command: CommandAction::RewriteSelection {
                style: RewriteStyle::Professional
            },
            requires_confirmation: false
        }
    );
}

#[test]
fn unsupported_app_control_is_safe_command() {
    let result = IntentEngine::default().parse_rule("click submit");

    assert_eq!(
        result,
        IntentParse::Command {
            command: CommandAction::Unsupported {
                reason: "app_control_out_of_scope".to_string()
            },
            requires_confirmation: true
        }
    );
}

#[test]
fn ordinary_sentence_remains_dictation() {
    let result = IntentEngine::default().parse_rule("I need a new line of business next quarter");

    assert_eq!(
        result,
        IntentParse::Dictation {
            text: "I need a new line of business next quarter".to_string()
        }
    );
}
