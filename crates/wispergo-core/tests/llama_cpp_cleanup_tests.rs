#![cfg(feature = "llama-cpp")]

use std::path::PathBuf;
use std::time::Duration;

use wispergo_core::cleanup_inprocess::{LlamaCppCleanupConfig, LlamaCppCleanupProvider};
use wispergo_core::providers::{CleanupInput, TextCleanupProvider};

#[tokio::test]
#[ignore = "requires WISPERGO_LLAMA_TEST_GGUF to point at a local generation-capable GGUF"]
async fn punctuation_cleanup_runs_against_real_gguf_when_env_set() {
    let model_path = std::env::var("WISPERGO_LLAMA_TEST_GGUF")
        .expect("set WISPERGO_LLAMA_TEST_GGUF=/path/to/model.gguf to run this test");
    let provider =
        LlamaCppCleanupProvider::new(LlamaCppCleanupConfig::new(PathBuf::from(model_path)));

    let output = provider
        .clean_punctuation_only(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_secs(60),
        })
        .await
        .expect("real GGUF punctuation cleanup should produce output");

    assert!(!output.trim().is_empty());
}
