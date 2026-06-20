# Punctuation Safety Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Punctuation-only cleanup safe by accepting LLM punctuation suggestions only when deterministic validation proves they did not rewrite transcript content, then add a safety-wrapped default cleanup-punctuation Asset.

**Architecture:** Add a pure `wispergo-core` safety gate that normalizes raw and candidate text by removing punctuation/whitespace and lowercasing ASCII before comparing preserved content. Wire only Punctuation-only recording output through that gate; unsafe suggestions fall back to raw ASR. Then add the Qwen2.5-0.5B cleanup-punctuation Asset and update the manual eval format to record suggestion, safety decision, and final output.

**Tech Stack:** Rust, Tauri v2, `wispergo-core` provider traits, `InferenceManager`, manifest-driven Assets, Cargo tests, Vitest/TypeScript only if UI types are touched.

---

## File structure

### New files

- `crates/wispergo-core/src/cleanup_safety.rs`
  - Pure punctuation safety gate.
  - Exposes `is_safe_punctuation_cleanup(raw: &str, candidate: &str) -> bool`.
  - Contains no provider, filesystem, network, or desktop dependencies.

- `crates/wispergo-core/tests/cleanup_safety_tests.rs`
  - Black-box tests for accepted punctuation/capitalization changes and rejected rewrites.

### Modified files

- `crates/wispergo-core/src/lib.rs`
  - Expose `cleanup_safety` module.

- `apps/desktop/src-tauri/src/commands/recording.rs`
  - Apply the safety gate to Punctuation-only output from both Ollama dev override and local `InferenceManager` cleanup.
  - Keep Full cleanup behavior unchanged.

- `apps/desktop/src-tauri/resources/models.manifest.json`
  - Add one `cleanup_punctuation` default Asset after safety gate is implemented.

- `apps/desktop/src-tauri/src/commands/settings.rs`
  - Resolve cleanup-punctuation model path from verified app-support Asset when manifest contains a default cleanup-punctuation Asset.
  - Preserve existing bundled cleanup path only as the empty-manifest/dev bridge until Phase 6.

- `docs/manual/offline-cleanup-eval.md`
  - Update fixture columns to record model suggestion, safety decision, final output, and punctuation-quality notes.

- `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
  - Mark 5.2a/5.2b progress after implementation.

- `HANDOFF.md`
  - Update only after implementation/verification state changes.

---

## Task 1: Add pure punctuation safety gate

**Files:**
- Create: `crates/wispergo-core/src/cleanup_safety.rs`
- Modify: `crates/wispergo-core/src/lib.rs`
- Test: `crates/wispergo-core/tests/cleanup_safety_tests.rs`

- [ ] **Step 1: Write failing safety tests**

Create `crates/wispergo-core/tests/cleanup_safety_tests.rs`:

```rust
use wispergo_core::cleanup_safety::is_safe_punctuation_cleanup;

#[test]
fn accepts_english_punctuation_and_capitalization_only() {
    assert!(is_safe_punctuation_cleanup(
        "can you send the updated notes before the meeting starts",
        "Can you send the updated notes before the meeting starts?",
    ));
}

#[test]
fn accepts_chinese_punctuation_only() {
    assert!(is_safe_punctuation_cleanup(
        "你明天可以帮我检查这个离线版本吗",
        "你明天可以帮我检查这个离线版本吗？",
    ));
}

#[test]
fn accepts_mixed_language_when_content_is_preserved() {
    assert!(is_safe_punctuation_cleanup(
        "please remind 小王 to review the offline build tonight",
        "Please remind 小王 to review the offline build tonight.",
    ));
}

#[test]
fn rejects_chinese_translation_to_english() {
    assert!(!is_safe_punctuation_cleanup(
        "你明天可以帮我检查这个离线版本吗",
        "Can you check this offline version for me tomorrow?",
    ));
}

#[test]
fn rejects_removed_cjk_character_in_mixed_text() {
    assert!(!is_safe_punctuation_cleanup(
        "please remind 小王 to review the offline build tonight",
        "Please remind 王 to review the offline build tonight.",
    ));
}

#[test]
fn rejects_romanized_cjk_name() {
    assert!(!is_safe_punctuation_cleanup(
        "please remind 小王 to review the offline build tonight",
        "Please remind Xiao Wang to review the offline build tonight.",
    ));
}

#[test]
fn rejects_added_latin_word() {
    assert!(!is_safe_punctuation_cleanup(
        "today we reviewed the release checklist",
        "Today, we reviewed our release checklist.",
    ));
}

#[test]
fn rejects_removed_latin_word() {
    assert!(!is_safe_punctuation_cleanup(
        "today we reviewed the release checklist",
        "Today, we reviewed the checklist.",
    ));
}

#[test]
fn rejects_empty_candidate() {
    assert!(!is_safe_punctuation_cleanup("hello world", ""));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p wispergo-core --test cleanup_safety_tests
```

Expected: FAIL with unresolved import `wispergo_core::cleanup_safety`.

- [ ] **Step 3: Expose the new module**

In `crates/wispergo-core/src/lib.rs`, add this line next to `cleanup_prompt`:

```rust
pub mod cleanup_safety;
```

- [ ] **Step 4: Implement the safety gate**

Create `crates/wispergo-core/src/cleanup_safety.rs`:

```rust
/// Return true when a punctuation-cleanup candidate preserves all non-punctuation
/// transcript content.
///
/// The gate intentionally prefers false negatives over false positives:
/// rejecting useful punctuation falls back to raw ASR, while accepting a rewrite
/// silently corrupts user dictation.
pub fn is_safe_punctuation_cleanup(raw: &str, candidate: &str) -> bool {
    if raw.trim().is_empty() || candidate.trim().is_empty() {
        return false;
    }

    normalized_content(raw) == normalized_content(candidate)
}

fn normalized_content(text: &str) -> String {
    text.chars()
        .filter_map(normalized_content_char)
        .collect::<String>()
}

fn normalized_content_char(ch: char) -> Option<char> {
    if is_ignored_punctuation_or_spacing(ch) {
        return None;
    }

    Some(ch.to_ascii_lowercase())
}

fn is_ignored_punctuation_or_spacing(ch: char) -> bool {
    ch.is_whitespace()
        || ch.is_ascii_punctuation()
        || matches!(
            ch,
            '。' | '？' | '！' | '，' | '、' | '；' | '：' | '「' | '」' | '『' | '』'
                | '“' | '”' | '‘' | '’' | '（' | '）' | '《' | '》' | '〈' | '〉'
                | '…' | '—' | '～' | '·' | '￥'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_content_removes_common_punctuation_and_spaces() {
        assert_eq!(
            normalized_content(" Hello, 小王！ "),
            "hello小王".to_string()
        );
    }
}
```

- [ ] **Step 5: Run focused safety tests**

Run:

```bash
cargo test -p wispergo-core --test cleanup_safety_tests
cargo test -p wispergo-core cleanup_safety
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

```bash
git add crates/wispergo-core/src/lib.rs crates/wispergo-core/src/cleanup_safety.rs crates/wispergo-core/tests/cleanup_safety_tests.rs
git commit -m "feat(core): add punctuation cleanup safety gate"
```

---

## Task 2: Apply safety gate to Punctuation-only recording output

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/recording.rs`

- [ ] **Step 1: Add failing recording tests for unsafe fallback**

In `apps/desktop/src-tauri/src/commands/recording.rs`, inside the existing `#[cfg(test)] mod tests`, add these tests near the existing punctuation cleanup tests:

```rust
#[tokio::test]
async fn punctuation_cleanup_rejects_rewritten_provider_output() {
    let provider = FakeCleanupProvider::new(Ok("Please remind 王 to review the offline build tonight.".to_string()));
    let manager = manager_for_cleanup_result(PipelineResult::InsertText {
        text: "unused manager output".to_string(),
        source: ProviderSource::Local,
        confidence: None,
    });

    let result = super::apply_cleanup_mode(
        AsrOutput {
            transcript: "please remind 小王 to review the offline build tonight".to_string(),
            source: ProviderSource::Local,
            confidence: Some(0.92),
        },
        CleanupMode::PunctuationOnly,
        Some(&provider),
        &manager,
    )
    .await;

    assert_eq!(
        result,
        PipelineResult::InsertText {
            text: "please remind 小王 to review the offline build tonight".to_string(),
            source: ProviderSource::Local,
            confidence: Some(0.92),
        }
    );
}

#[tokio::test]
async fn punctuation_cleanup_accepts_safe_provider_output() {
    let provider = FakeCleanupProvider::new(Ok("Please remind 小王 to review the offline build tonight.".to_string()));
    let manager = manager_for_cleanup_result(PipelineResult::InsertText {
        text: "unused manager output".to_string(),
        source: ProviderSource::Local,
        confidence: None,
    });

    let result = super::apply_cleanup_mode(
        AsrOutput {
            transcript: "please remind 小王 to review the offline build tonight".to_string(),
            source: ProviderSource::Local,
            confidence: Some(0.92),
        },
        CleanupMode::PunctuationOnly,
        Some(&provider),
        &manager,
    )
    .await;

    assert_eq!(
        result,
        PipelineResult::InsertText {
            text: "Please remind 小王 to review the offline build tonight.".to_string(),
            source: ProviderSource::Local,
            confidence: Some(0.92),
        }
    );
}

#[tokio::test]
async fn punctuation_cleanup_rejects_rewritten_manager_output() {
    let manager = manager_for_cleanup_result(PipelineResult::InsertText {
        text: "Please remind 王 to review the offline build tonight.".to_string(),
        source: ProviderSource::Local,
        confidence: None,
    });

    let result = super::apply_cleanup_mode(
        AsrOutput {
            transcript: "please remind 小王 to review the offline build tonight".to_string(),
            source: ProviderSource::Local,
            confidence: Some(0.92),
        },
        CleanupMode::PunctuationOnly,
        None,
        &manager,
    )
    .await;

    assert_eq!(
        result,
        PipelineResult::InsertText {
            text: "please remind 小王 to review the offline build tonight".to_string(),
            source: ProviderSource::Local,
            confidence: Some(0.92),
        }
    );
}
```

Use the existing `FakeCleanupProvider` constructor shape already present in `recording.rs`; if its name differs, keep the existing fake provider and set its punctuation response to the strings shown above.

- [ ] **Step 2: Run focused tests to verify failure**

Run:

```bash
cargo test -p wispergo-desktop --lib punctuation_cleanup_rejects_rewritten_provider_output punctuation_cleanup_accepts_safe_provider_output punctuation_cleanup_rejects_rewritten_manager_output
```

Expected: at least the two reject tests FAIL because rewritten output is currently inserted.

- [ ] **Step 3: Import the safety gate**

At the top of `apps/desktop/src-tauri/src/commands/recording.rs`, extend imports from `wispergo_core` to include:

```rust
use wispergo_core::cleanup_safety::is_safe_punctuation_cleanup;
```

- [ ] **Step 4: Add a small helper for safe punctuation results**

Near `apply_cleanup_mode`, add:

```rust
fn safe_punctuation_result(asr: &AsrOutput, candidate: String) -> Option<PipelineResult> {
    if !is_safe_punctuation_cleanup(&asr.transcript, &candidate) {
        return None;
    }

    Some(PipelineResult::InsertText {
        text: candidate,
        source: asr.source,
        confidence: asr.confidence,
    })
}
```

- [ ] **Step 5: Gate provider Punctuation-only output**

In `apply_cleanup_mode`, replace the Punctuation-only provider branch:

```rust
return cleanup
    .clean_punctuation_only(CleanupInput {
        transcript: asr.transcript,
        selected_text: None,
        timeout: PUNCTUATION_CLEANUP_TIMEOUT,
    })
    .await
    .map(|text| PipelineResult::InsertText {
        text,
        source: asr.source,
        confidence: asr.confidence,
    })
    .unwrap_or(raw_result);
```

with:

```rust
return cleanup
    .clean_punctuation_only(CleanupInput {
        transcript: asr.transcript.clone(),
        selected_text: None,
        timeout: PUNCTUATION_CLEANUP_TIMEOUT,
    })
    .await
    .ok()
    .and_then(|text| safe_punctuation_result(&asr, text))
    .unwrap_or(raw_result);
```

- [ ] **Step 6: Gate manager Punctuation-only output**

In `apply_cleanup_mode`, replace the Punctuation-only manager branch:

```rust
inference_manager
    .cleanup()
    .request(CleanupInferenceRequest {
        transcript: asr.transcript,
    })
    .map(|output| output.result)
    .unwrap_or(raw_result)
```

with:

```rust
inference_manager
    .cleanup()
    .request(CleanupInferenceRequest {
        transcript: asr.transcript.clone(),
    })
    .ok()
    .and_then(|output| match output.result {
        PipelineResult::InsertText { text, .. } => safe_punctuation_result(&asr, text),
        _ => None,
    })
    .unwrap_or(raw_result)
```

Do not change the Full cleanup branch.

- [ ] **Step 7: Run focused recording tests**

Run:

```bash
cargo test -p wispergo-desktop --lib punctuation_cleanup
```

Expected: PASS, including the new safety tests and existing fallback tests.

- [ ] **Step 8: Commit Task 2**

```bash
git add apps/desktop/src-tauri/src/commands/recording.rs
git commit -m "feat(desktop): gate punctuation cleanup suggestions"
```

---

## Task 3: Add cleanup-punctuation Asset resolution and manifest default

**Files:**
- Modify: `apps/desktop/src-tauri/resources/models.manifest.json`
- Modify: `apps/desktop/src-tauri/src/commands/settings.rs`
- Test: existing desktop settings tests in `apps/desktop/src-tauri/src/commands/settings.rs`

- [ ] **Step 1: Add failing tests for cleanup Asset path resolution**

In `apps/desktop/src-tauri/src/commands/settings.rs`, inside the existing tests module, add tests matching the local helper style used for ASR path resolution:

```rust
#[test]
fn cleanup_uses_verified_app_support_punctuation_asset_when_manifest_populated() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage = AssetStorage::new(tempdir.path().join("models"));
    let manifest = AssetManifest::from_json(&format!(
        r#"{{
            "schemaVersion": 1,
            "assets": [{{
                "id": "qwen2.5-0.5b-instruct",
                "role": "cleanup_punctuation",
                "displayName": "Qwen2.5 0.5B Punctuation",
                "url": "https://example.invalid/qwen.gguf",
                "size": 491400032,
                "sha256": "{}",
                "default": true
            }}]
        }}"#,
        "a".repeat(64)
    ))
    .expect("manifest");
    let asset_path = storage.asset_path("qwen2.5-0.5b-instruct", AssetRole::CleanupPunctuation);
    std::fs::create_dir_all(asset_path.parent().expect("asset parent")).expect("create parent");
    std::fs::write(&asset_path, b"placeholder").expect("write asset");

    let path = resolve_cleanup_model_path_for_settings(
        &LocalModelSettings {
            cleanup_mode: CleanupMode::PunctuationOnly,
            ..LocalModelSettings::default()
        },
        None,
        Some(&manifest),
        Some(&storage),
    )
    .expect("cleanup path");

    assert_eq!(path, asset_path);
}

#[test]
fn cleanup_punctuation_missing_asset_reports_unavailable_path_error() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage = AssetStorage::new(tempdir.path().join("models"));
    let manifest = AssetManifest::from_json(&format!(
        r#"{{
            "schemaVersion": 1,
            "assets": [{{
                "id": "qwen2.5-0.5b-instruct",
                "role": "cleanup_punctuation",
                "displayName": "Qwen2.5 0.5B Punctuation",
                "url": "https://example.invalid/qwen.gguf",
                "size": 491400032,
                "sha256": "{}",
                "default": true
            }}]
        }}"#,
        "a".repeat(64)
    ))
    .expect("manifest");

    let error = resolve_cleanup_model_path_for_settings(
        &LocalModelSettings {
            cleanup_mode: CleanupMode::PunctuationOnly,
            ..LocalModelSettings::default()
        },
        None,
        Some(&manifest),
        Some(&storage),
    )
    .expect_err("missing cleanup asset should report unavailable");

    assert!(error.contains("cleanup punctuation asset 'qwen2.5-0.5b-instruct' is not downloaded yet"));
}
```

Adjust imports in the test module as needed:

```rust
use wispergo_core::asset_manifest::{AssetManifest, AssetRole};
use wispergo_core::asset_storage::AssetStorage;
```

- [ ] **Step 2: Run focused settings tests to verify failure**

Run:

```bash
cargo test -p wispergo-desktop --lib cleanup_uses_verified_app_support_punctuation_asset_when_manifest_populated cleanup_punctuation_missing_asset_reports_unavailable_path_error
```

Expected: FAIL because `resolve_cleanup_model_path_for_settings` does not exist yet.

- [ ] **Step 3: Implement cleanup path resolution helper**

In `apps/desktop/src-tauri/src/commands/settings.rs`, add this helper near `resolve_asr_model_path_for_settings`:

```rust
fn resolve_cleanup_model_path_for_settings(
    settings: &LocalModelSettings,
    resources: Option<&InferenceResourcePaths>,
    manifest: &AssetManifest,
    storage: Option<&AssetStorage>,
) -> Result<PathBuf, String> {
    let role = match settings.cleanup_mode {
        CleanupMode::Off => return Err("cleanup is disabled".to_string()),
        CleanupMode::PunctuationOnly => AssetRole::CleanupPunctuation,
        CleanupMode::FullCleanup => AssetRole::CleanupFull,
    };

    if !manifest.assets.is_empty() {
        let storage = storage.ok_or_else(|| {
            "Local cleanup asset storage is unavailable. Reopen Wispergo and try again."
                .to_string()
        })?;
        if let Some(asset) = manifest.by_role(role).find(|asset| asset.default) {
            let path = storage.asset_path(&asset.id, asset.role);
            return match verify_asset(asset, storage) {
                AssetIntegrity::Valid => Ok(path),
                AssetIntegrity::Missing => Err(match role {
                    AssetRole::CleanupPunctuation => format!(
                        "cleanup punctuation asset '{}' is not downloaded yet",
                        asset.id
                    ),
                    AssetRole::CleanupFull => format!(
                        "full cleanup asset '{}' is not downloaded yet",
                        asset.id
                    ),
                    AssetRole::Asr => unreachable!("cleanup role cannot be ASR"),
                }),
                AssetIntegrity::Corrupt => Err(match role {
                    AssetRole::CleanupPunctuation => format!(
                        "cleanup punctuation asset '{}' is corrupt",
                        asset.id
                    ),
                    AssetRole::CleanupFull => format!(
                        "full cleanup asset '{}' is corrupt",
                        asset.id
                    ),
                    AssetRole::Asr => unreachable!("cleanup role cannot be ASR"),
                }),
            };
        }
    }

    resources
        .filter(|resources| resources.cleanup_model_path.exists())
        .map(|resources| resources.cleanup_model_path.clone())
        .ok_or_else(|| {
            "Local cleanup is not configured. Download cleanup models or reinstall Wispergo."
                .to_string()
        })
}
```


- [ ] **Step 4: Wire settings sync through the helper**

First, change `sync_inference_manager_for_settings` to pass manifest/storage into cleanup sync:

```rust
sync_cleanup_for_settings(
    inference_manager,
    settings,
    resources.as_ref(),
    &manifest,
    storage.as_ref(),
);
```

Then change the `sync_cleanup_for_settings` signature to:

```rust
fn sync_cleanup_for_settings(
    inference_manager: &InferenceManager,
    settings: &LocalModelSettings,
    resources: Option<&InferenceResourcePaths>,
    manifest: &AssetManifest,
    storage: Option<&AssetStorage>,
)
```

In `sync_cleanup_for_settings`, replace direct bundled resource use:

```rust
if !resources.cleanup_model_path.exists() {
    if let Err(err) = inference_manager
        .cleanup()
        .mark_unavailable("Offline punctuation assets are missing.")
    {
        eprintln!("cleanup inference manager unavailable sync failed: {err}");
    }
    return;
}

if let Err(err) = inference_manager.cleanup().arm(CleanupEngineConfig {
    model_path: resources.cleanup_model_path.clone(),
    mode: match settings.cleanup_mode {
        CleanupMode::Off => unreachable!("cleanup off handled above"),
        CleanupMode::PunctuationOnly => CleanupInferenceMode::PunctuationOnly,
        CleanupMode::FullCleanup => CleanupInferenceMode::FullCleanup,
    },
}) {
    eprintln!("cleanup inference manager arm failed: {err}");
}
```

with:

```rust
let cleanup_model_path = match resolve_cleanup_model_path_for_settings(
    settings,
    Some(resources),
    manifest,
    storage,
) {
    Ok(path) => path,
    Err(message) => {
        if let Err(err) = inference_manager.cleanup().mark_unavailable(message) {
            eprintln!("cleanup inference manager unavailable sync failed: {err}");
        }
        return;
    }
};

if let Err(err) = inference_manager.cleanup().arm(CleanupEngineConfig {
    model_path: cleanup_model_path,
    mode: match settings.cleanup_mode {
        CleanupMode::Off => unreachable!("cleanup off handled above"),
        CleanupMode::PunctuationOnly => CleanupInferenceMode::PunctuationOnly,
        CleanupMode::FullCleanup => CleanupInferenceMode::FullCleanup,
    },
}) {
    eprintln!("cleanup inference manager arm failed: {err}");
}
```

If `load_asset_manifest()` or `app_support_asset_storage(app)` already has a different local helper shape, follow the ASR sync code's exact pattern and keep the behavior identical.

- [ ] **Step 5: Add the cleanup-punctuation default Asset**

Modify `apps/desktop/src-tauri/resources/models.manifest.json` by appending this entry after the ASR entries:

```json
{
  "id": "qwen2.5-0.5b-instruct",
  "role": "cleanup_punctuation",
  "displayName": "Qwen2.5 0.5B Punctuation",
  "url": "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf",
  "size": 491400032,
  "sha256": "74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db",
  "default": true
}
```

Keep the JSON valid and preserve existing ASR entries unchanged.

- [ ] **Step 6: Run focused settings and manifest tests**

Run:

```bash
cargo test -p wispergo-desktop --lib cleanup_uses_verified_app_support_punctuation_asset_when_manifest_populated cleanup_punctuation_missing_asset_reports_unavailable_path_error
cargo test -p wispergo-core asset_manifest
```

Expected: PASS.

- [ ] **Step 7: Commit Task 3**

```bash
git add apps/desktop/src-tauri/src/commands/settings.rs apps/desktop/src-tauri/resources/models.manifest.json
git commit -m "feat(desktop): resolve cleanup punctuation asset"
```

---

## Task 4: Update manual eval fixture for safety-gated punctuation

**Files:**
- Modify: `docs/manual/offline-cleanup-eval.md`

- [ ] **Step 1: Rewrite fixture columns**

Replace the existing evaluation table with:

```markdown
| Case | Spoken content | Expected behavior | Raw ASR | Model suggestion | Safety decision | Final inserted output | ASR ms | Cleanup ms | Safety notes | Quality notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| English sentence | today we reviewed the release checklist and fixed the last offline inference issue | Final output must preserve all words; accepted suggestion may add sentence capitalization/punctuation. |  |  |  |  |  |  |  |  |
| English question | can you send the updated notes before the meeting starts | Final output must preserve all words; accepted suggestion may add capitalization and a question mark. |  |  |  |  |  |  |  |  |
| Chinese sentence | 今天我们完成了离线语音识别和标点清理测试 | Final output must preserve every Chinese character in order; accepted suggestion may add appropriate Chinese punctuation. |  |  |  |  |  |  |  |  |
| Chinese question | 你明天可以帮我检查这个离线版本吗 | Final output must preserve every Chinese character in order; accepted suggestion may add appropriate Chinese question punctuation. |  |  |  |  |  |  |  |  |
| Mixed English Chinese | please remind 小王 to review the offline build tonight | Final output must preserve English words and Chinese characters exactly; unsafe suggestions fall back to raw ASR. |  |  |  |  |  |  |  |  |
| Already punctuated | Wispergo already works offline, and cleanup should not rewrite this sentence. | Final output should preserve existing words and punctuation unless a safe punctuation-only change is clearly beneficial. |  |  |  |  |  |  |  |  |
```

Replace the pass criteria section with:

```markdown
## Pass criteria

- Final inserted output does not translate between languages or scripts.
- Final inserted output does not meaningfully add, remove, or rewrite words.
- Safe raw-ASR fallback counts as a safety pass when the model suggestion is unsafe.
- Punctuation quality is recorded separately from safety; lack of punctuation improvement is not a safety failure.
- Apple Silicon latency is acceptable for normal dictation use.
- Intel Macs may fall back to raw ASR when cleanup is unavailable or too slow.
```

- [ ] **Step 2: Commit Task 4**

```bash
git add docs/manual/offline-cleanup-eval.md
git commit -m "docs: update offline cleanup safety eval"
```

---

## Task 5: Run safety-gated eval and record evidence

**Files:**
- Modify: `docs/manual/offline-cleanup-eval.md`
- Optional local-only helper: `crates/wispergo-core/examples/cleanup_eval.rs` (remove before commit unless explicitly approved)

- [ ] **Step 1: Add temporary eval helper locally**

If no reusable harness exists, create a temporary `crates/wispergo-core/examples/cleanup_eval.rs` that prints raw transcript, model suggestion, safety decision, final output, and latency. Do not commit this helper unless explicitly approved.

Use this output shape:

```text
CASE	English sentence	<cleanup_ms>	<raw>	<suggestion>	accepted	<final>
CASE	Mixed English Chinese	<cleanup_ms>	<raw>	<suggestion>	fallback_raw	<final>
```

- [ ] **Step 2: Run eval with 0.5B candidate**

Run:

```bash
cargo run -p wispergo-core --features llama-cpp --example cleanup_eval -- /tmp/wispergo-model-eval/qwen2.5-0.5b-instruct-q4_k_m.gguf | tee /tmp/wispergo-model-eval/qwen-0.5b-safety-eval.tsv
```

Expected: command completes. Unsafe cases should show `fallback_raw`; safe cases may show `accepted`.

- [ ] **Step 3: Fill `docs/manual/offline-cleanup-eval.md`**

For each case, copy:

- Raw ASR = fixture spoken content used for eval.
- Model suggestion = raw model suggestion.
- Safety decision = `accepted` or `fallback_raw`.
- Final inserted output = accepted suggestion or raw ASR.
- Cleanup ms = measured latency.
- Safety notes = why accepted/rejected.
- Quality notes = punctuation improvement or raw fallback.

- [ ] **Step 4: Remove temporary eval helper**

Run:

```bash
rm -rf crates/wispergo-core/examples
```

Expected: `git status --short` does not show `crates/wispergo-core/examples`.

- [ ] **Step 5: Commit Task 5**

```bash
git add docs/manual/offline-cleanup-eval.md
git commit -m "docs: record safety-gated cleanup eval"
```

---

## Task 6: Update roadmap/handoff and run full verification gate

**Files:**
- Modify: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- Modify: `HANDOFF.md`
- Possibly revert: `package.json` if `pnpm test:ts` adds `packageManager`

- [ ] **Step 1: Update roadmap status**

In `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`, update Phase 5.2 to reflect implementation status:

```markdown
- **5.2 Cleanup punctuation safety redesign** ✅
  - Raw-model eval failed for Qwen2.5 0.5B, 1.5B, and 3B, so Punctuation-only now treats LLM output as an untrusted suggestion.
  - Added a deterministic safety gate that accepts only punctuation/capitalization-only changes and falls back to raw ASR for unsafe suggestions.
  - Added the safety-wrapped Qwen2.5-0.5B cleanup-punctuation default Asset.
  - Manual eval records model suggestion, safety decision, final inserted output, safety notes, quality notes, and latency.
```

- [ ] **Step 2: Update `HANDOFF.md`**

Add a concise Phase 5.2 entry with:

```markdown
- **Phase 5.2 is complete locally** on branch `phase-5-2-cleanup-punctuation-default`: Punctuation-only suggestions are safety-gated, unsafe suggestions fall back to raw ASR, the cleanup-punctuation default Asset is Qwen2.5-0.5B Q4_K_M, and `docs/manual/offline-cleanup-eval.md` records safety-gated eval evidence.
```

- [ ] **Step 3: Run full verification gate**

Run:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy -p wispergo-core --all-targets -- -D warnings
cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings
cargo clippy -p wispergo-desktop --all-targets -- -D warnings
pnpm test:ts
```

Expected: all commands PASS.

- [ ] **Step 4: Revert Corepack packageManager edit if present**

Run:

```bash
git diff -- package.json
```

If the only change is an out-of-scope `packageManager` field added by Corepack, run:

```bash
git checkout -- package.json
```

- [ ] **Step 5: Commit docs/status updates**

```bash
git add docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md HANDOFF.md
git commit -m "docs: update phase 5.2 status"
```

---

## Task 7: Open PR and wait for merge

**Files:**
- Create: `/tmp/wispergo-phase-5-2-pr.md`

- [ ] **Step 1: Inspect final diff**

Run:

```bash
git status --short
git diff --stat main...HEAD
```

Expected: only Phase 5.2 safety, manifest, eval docs, roadmap/handoff changes.

- [ ] **Step 2: Push branch**

Run:

```bash
git push -u origin phase-5-2-cleanup-punctuation-default
```

- [ ] **Step 3: Write PR body**

Create `/tmp/wispergo-phase-5-2-pr.md`:

```markdown
## Summary
- add deterministic safety validation for Punctuation-only cleanup suggestions
- fall back to raw ASR when punctuation output rewrites transcript content
- add safety-wrapped Qwen2.5-0.5B cleanup-punctuation default Asset
- update offline cleanup eval to record model suggestion, safety decision, final output, and quality notes

## Verification
- [ ] cargo build --workspace
- [ ] cargo test --workspace
- [ ] cargo clippy -p wispergo-core --all-targets -- -D warnings
- [ ] cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings
- [ ] cargo clippy -p wispergo-desktop --all-targets -- -D warnings
- [ ] pnpm test:ts

## Manual eval
- docs/manual/offline-cleanup-eval.md records safety-gated Qwen2.5-0.5B results
```

Fill the checkboxes with actual PASS notes from Task 6 before creating the PR.

- [ ] **Step 4: Open PR**

Run:

```bash
gh pr create --title "feat(desktop): add safe punctuation cleanup default" --body-file /tmp/wispergo-phase-5-2-pr.md
```

- [ ] **Step 5: Stop and wait for user merge**

Do not merge the PR yourself unless the user explicitly asks. Report PR URL and verification evidence.

---

## Self-review checklist

- Spec coverage:
  - Pure deterministic safety gate: Task 1.
  - Punctuation-only gate in recording path: Task 2.
  - Full cleanup unchanged: Task 2 explicitly avoids Full cleanup branch changes.
  - Safety-wrapped default Asset: Task 3.
  - Manual eval semantics and evidence: Tasks 4 and 5.
  - Roadmap/handoff updates and full verification: Task 6.
  - PR flow and wait-for-merge gate: Task 7.
- Placeholder scan: no unresolved placeholder markers or unspecified test commands.
- Type consistency: plan uses existing domain names `CleanupMode`, `PipelineResult`, `ProviderSource`, `AssetRole::CleanupPunctuation`, `AssetStorage`, and `InferenceManager`.
