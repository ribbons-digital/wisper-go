use wispergo_core::cleanup_prompt::{
    cleanup_system_prompt, cleanup_user_prompt, parse_cleanup_json, parse_punctuation_cleanup_text,
    punctuation_system_prompt, punctuation_user_prompt,
};
use wispergo_core::domain::{CommandAction, CommandSource, PipelineResult};
use wispergo_core::providers::{CleanupInput, ProviderError};

#[test]
fn shared_punctuation_prompt_preserves_existing_contract() {
    let input = CleanupInput {
        transcript: "hello world".to_string(),
        selected_text: Some("ignored selection".to_string()),
        timeout: std::time::Duration::from_secs(1),
    };

    assert!(punctuation_system_prompt().contains("Punctuation-only cleanup"));
    assert!(
        punctuation_system_prompt().contains("Return only the corrected transcript as plain text")
    );
    assert!(punctuation_system_prompt().contains("Preserve the exact words, language, and script"));
    assert!(punctuation_system_prompt().contains("Do not translate, paraphrase"));
    assert_eq!(punctuation_user_prompt(&input), "Transcript: hello world");
}

#[test]
fn shared_cleanup_prompt_preserves_existing_contract() {
    let input = CleanupInput {
        transcript: "hello world".to_string(),
        selected_text: Some("selected text".to_string()),
        timeout: std::time::Duration::from_secs(1),
    };

    assert!(cleanup_system_prompt().contains("Return only JSON matching the CleanupOutput schema"));
    assert!(cleanup_system_prompt().contains("Do not execute commands"));
    assert!(
        cleanup_system_prompt().contains("Preserve the transcript's original language and script")
    );
    assert_eq!(
        cleanup_user_prompt(&input),
        "Transcript: hello world\nSelected text: selected text"
    );
}

#[test]
fn shared_punctuation_parser_uses_supplied_provider_name() {
    let output = parse_punctuation_cleanup_text("Transcript: Hello, world.", "llama_cpp")
        .expect("parse punctuation response");
    assert_eq!(output, "Hello, world.");

    let error = parse_punctuation_cleanup_text(" \n ", "llama_cpp")
        .expect_err("empty punctuation output should fail");
    assert!(matches!(
        error,
        ProviderError::InvalidOutput { provider, .. } if provider == "llama_cpp"
    ));
}

#[test]
fn shared_cleanup_json_parser_uses_supplied_provider_name_and_forces_destructive_confirmation() {
    let output = parse_cleanup_json(
        r#"{
          "result": {
            "kind": "command",
            "command": { "kind": "delete_previous_phrase" },
            "requires_confirmation": false,
            "source": "local_llm"
          }
        }"#,
        "llama_cpp",
    )
    .expect("parse output");

    assert_eq!(
        output.result,
        PipelineResult::Command {
            command: CommandAction::DeletePreviousPhrase,
            requires_confirmation: true,
            source: CommandSource::LocalLlm,
        }
    );

    let error = parse_cleanup_json("not json", "llama_cpp")
        .expect_err("invalid json should fail with provider name");
    assert!(matches!(
        error,
        ProviderError::InvalidOutput { provider, .. } if provider == "llama_cpp"
    ));
}
