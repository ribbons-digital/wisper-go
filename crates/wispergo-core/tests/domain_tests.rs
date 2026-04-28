use wispergo_core::domain::{
    CommandAction, CommandSource, PipelineResult, ProviderSource, RewriteStyle,
};

#[test]
fn insert_text_result_round_trips_through_json() {
    let result = PipelineResult::InsertText {
        text: "Hello world.".to_string(),
        source: ProviderSource::Local,
        confidence: Some(0.92),
    };

    let json = serde_json::to_string(&result).expect("serialize result");
    assert!(json.contains("\"kind\":\"insert_text\""));

    let decoded: PipelineResult = serde_json::from_str(&json).expect("deserialize result");
    assert_eq!(decoded, result);
}

#[test]
fn destructive_command_requires_confirmation() {
    let result = PipelineResult::Command {
        command: CommandAction::DeletePreviousPhrase,
        requires_confirmation: true,
        source: CommandSource::Rules,
    };

    assert!(result.requires_confirmation());
}

#[test]
fn rewrite_command_carries_style() {
    let command = CommandAction::RewriteSelection {
        style: RewriteStyle::Professional,
    };

    assert_eq!(command.label(), "rewrite_selection_professional");
}
