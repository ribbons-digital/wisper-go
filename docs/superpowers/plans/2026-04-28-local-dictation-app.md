# Local Dictation App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a macOS-first, cross-platform-ready desktop dictation app with local-first ASR, voice commands, Ollama cleanup, explicit cloud fallback policy, clipboard insertion, and a thin Tauri UI.

**Architecture:** Use a Tauri v2 desktop app for the shell and UI, with most behavior in a separate Rust core crate. The core crate owns pipeline results, privacy policy, command parsing, provider contracts, fallback decisions, and orchestration tests. Tauri owns app state, global shortcuts, audio/session adapters, clipboard insertion, and React settings/recorder UI.

**Tech Stack:** Tauri v2, Rust, TypeScript, React, Vite, SQLite via `rusqlite`, HTTP via `reqwest`, async Rust via `tokio`, local cleanup via Ollama, local ASR through a configurable whisper.cpp sidecar adapter.

---

## Scope And Sequencing

This plan is intentionally split into independently reviewable tasks. The safest subagent execution order is:

1. Scaffold app and core crate.
2. Build the pure Rust core behavior with tests.
3. Add provider adapters and persistence.
4. Wire Tauri commands, triggers, insertion, and UI.
5. Verify end-to-end with fake providers, then configure real local providers.

The first implementation should not depend on a real microphone, a real Whisper model, or a running Ollama instance. Those integrations are added behind provider interfaces and tested with mocks or local HTTP test servers.

## Planned File Structure

```text
package.json
pnpm-workspace.yaml
Cargo.toml
docs/superpowers/specs/2026-04-28-local-dictation-app-design.md
docs/superpowers/plans/2026-04-28-local-dictation-app.md

apps/desktop/
  package.json
  vite.config.ts
  tsconfig.json
  index.html
  src/
    main.tsx
    app/App.tsx
    app/App.test.tsx
    lib/tauriApi.ts
    types/pipeline.ts
    features/recorder/FloatingRecorder.tsx
    features/recorder/FloatingRecorder.test.tsx
    features/settings/SettingsPanel.tsx
    features/settings/SettingsPanel.test.tsx
  src-tauri/
    Cargo.toml
    tauri.conf.json
    capabilities/default.json
    migrations/0001_initial.sql
    src/
      main.rs
      lib.rs
      state.rs
      commands/mod.rs
      commands/recording.rs
      commands/settings.rs
      trigger/mod.rs
      trigger/manager.rs
      platform/mod.rs
      platform/macos.rs
      insertion/mod.rs
      insertion/clipboard.rs

crates/wispergo-core/
  Cargo.toml
  src/
    lib.rs
    domain.rs
    privacy.rs
    intent.rs
    providers.rs
    fallback.rs
    audio.rs
    store.rs
    ollama.rs
    whisper_sidecar.rs
    pipeline.rs
  tests/
    domain_tests.rs
    privacy_tests.rs
    intent_tests.rs
    provider_tests.rs
    fallback_tests.rs
    audio_tests.rs
    store_tests.rs
    ollama_tests.rs
    whisper_sidecar_tests.rs
    pipeline_tests.rs
    fixtures/
      cleanup_insert_text.json
      cleanup_command_rewrite.json
      cleanup_invalid.json
```

## Subagent Workstream Guidance

- Task 1 is the shared foundation and should be done first.
- Tasks 2, 3, 4, and 5 can be dispatched to separate workers after Task 1 because their write sets are mostly disjoint inside `crates/wispergo-core/src`.
- Tasks 6, 7, and 8 depend on the core domain and provider traits.
- Tasks 9, 10, 11, and 12 depend on the core crate and Tauri scaffold.
- Task 13 is final integration and should be done after the other tasks land.

Workers are not alone in the codebase. Do not revert edits made by others; adapt to existing names and interfaces when integrating.

---

### Task 1: Scaffold Tauri Workspace

**Files:**
- Create: `package.json`
- Create: `pnpm-workspace.yaml`
- Create: `Cargo.toml`
- Create: `apps/desktop/package.json`
- Create: `apps/desktop/vite.config.ts`
- Create: `apps/desktop/tsconfig.json`
- Create: `apps/desktop/index.html`
- Create: `apps/desktop/src/main.tsx`
- Create: `apps/desktop/src/app/App.tsx`
- Create: `apps/desktop/src/styles.css`
- Create: `apps/desktop/src-tauri/Cargo.toml`
- Create: `apps/desktop/src-tauri/tauri.conf.json`
- Create: `apps/desktop/src-tauri/capabilities/default.json`
- Create: `apps/desktop/src-tauri/src/main.rs`
- Create: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Create the frontend and Tauri scaffold files**

Use the Tauri creator if network access is available:

```bash
pnpm create tauri-app apps/desktop --template react-ts --manager pnpm
```

If the creator asks for an app name, use `Wispergo`. If it asks for an identifier, use `com.ribbonsdigital.wispergo`.

Expected: `apps/desktop` contains a React/TypeScript frontend and `apps/desktop/src-tauri`.

- [ ] **Step 2: Normalize the root package workspace**

Set `package.json` to:

```json
{
  "name": "wispergo",
  "private": true,
  "version": "0.1.0",
  "scripts": {
    "desktop:dev": "pnpm --dir apps/desktop tauri dev",
    "desktop:build": "pnpm --dir apps/desktop tauri build",
    "test": "cargo test --workspace",
    "test:ts": "pnpm --dir apps/desktop test",
    "test:rust": "cargo test --workspace"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0"
  }
}
```

Set `pnpm-workspace.yaml` to:

```yaml
packages:
  - "apps/*"
```

Set root `Cargo.toml` to:

```toml
[workspace]
members = ["apps/desktop/src-tauri"]
resolver = "2"
```

- [ ] **Step 3: Install scaffold dependencies**

Run:

```bash
pnpm install
```

Expected: dependencies install and `pnpm-lock.yaml` is created.

- [ ] **Step 4: Add a minimal React app**

Set `apps/desktop/src/app/App.tsx` to:

```tsx
export function App() {
  return (
    <main className="app-shell">
      <section className="recorder-surface">
        <h1>Wispergo</h1>
        <p>Local-first dictation is ready to configure.</p>
      </section>
    </main>
  );
}
```

Set `apps/desktop/src/main.tsx` to:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./app/App";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

Create `apps/desktop/src/styles.css`:

```css
:root {
  color: #1b1f24;
  background: #f5f7fa;
  font-family:
    Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI",
    sans-serif;
}

body {
  margin: 0;
}

.app-shell {
  min-height: 100vh;
  display: grid;
  place-items: center;
}

.recorder-surface {
  width: min(520px, calc(100vw - 32px));
  padding: 24px;
  border: 1px solid #d7dce2;
  border-radius: 8px;
  background: #ffffff;
}
```

- [ ] **Step 5: Add a Rust smoke command**

Ensure `apps/desktop/src-tauri/Cargo.toml` uses these package and library names while preserving generated dependency sections:

```toml
[package]
name = "wispergo-desktop"

[lib]
name = "wispergo_desktop_lib"
crate-type = ["staticlib", "cdylib", "rlib"]
```

Set `apps/desktop/src-tauri/src/lib.rs` to:

```rust
#[tauri::command]
fn app_health() -> &'static str {
    "ok"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_health])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Set `apps/desktop/src-tauri/src/main.rs` to:

```rust
fn main() {
    wispergo_desktop_lib::run();
}
```

- [ ] **Step 6: Run scaffold verification**

Run:

```bash
pnpm --dir apps/desktop build
cargo test --workspace
```

Expected: the frontend builds, and Rust compiles with no test failures.

- [ ] **Step 7: Commit**

```bash
git add package.json pnpm-workspace.yaml Cargo.toml pnpm-lock.yaml apps/desktop
git commit -m "chore: scaffold tauri desktop app"
```

---

### Task 2: Core Domain Contracts

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/wispergo-core/Cargo.toml`
- Create: `crates/wispergo-core/src/lib.rs`
- Create: `crates/wispergo-core/src/domain.rs`
- Create: `crates/wispergo-core/tests/domain_tests.rs`
- Create: `apps/desktop/src/types/pipeline.ts`
- Modify: `apps/desktop/src-tauri/Cargo.toml`

- [ ] **Step 1: Write failing domain serialization tests**

Create `crates/wispergo-core/tests/domain_tests.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p wispergo-core --test domain_tests
```

Expected: FAIL because `wispergo-core` does not exist yet.

- [ ] **Step 3: Create the core crate and domain types**

Update root `Cargo.toml`:

```toml
[workspace]
members = ["apps/desktop/src-tauri", "crates/wispergo-core"]
resolver = "2"
```

Create `crates/wispergo-core/Cargo.toml`:

```toml
[package]
name = "wispergo-core"
version = "0.1.0"
edition = "2021"

[dependencies]
async-trait = "0.1"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tempfile = "3"
thiserror = "2"
tokio = { version = "1", features = ["macros", "process", "rt-multi-thread", "time"] }

[dev-dependencies]
httpmock = "0.7"
```

Create `crates/wispergo-core/src/lib.rs`:

```rust
pub mod domain;
```

Create `crates/wispergo-core/src/domain.rs`:

```rust
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
```

Update `apps/desktop/src-tauri/Cargo.toml` to depend on the core crate. Preserve generated Tauri package metadata, the `[lib]` section, and any existing `[build-dependencies]`; add only the missing dependency entries:

```toml
[dependencies]
wispergo-core = { path = "../../../crates/wispergo-core" }
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 4: Add TypeScript mirror types**

Create `apps/desktop/src/types/pipeline.ts`:

```ts
export type ProviderSource = "local" | "cloud";
export type CommandSource = "rules" | "local_llm" | "cloud_llm";
export type RecordingMode = "press_and_hold" | "toggle" | "floating_button";
export type RewriteStyle = "casual" | "professional" | "shorter" | "longer";

export type CommandAction =
  | { kind: "new_line" }
  | { kind: "new_paragraph" }
  | { kind: "cancel" }
  | { kind: "literal_dictation"; text: string }
  | { kind: "delete_previous_phrase" }
  | { kind: "replace_selection"; text: string }
  | { kind: "rewrite_selection"; style: RewriteStyle }
  | { kind: "format_selection_bullets" }
  | { kind: "format_selection_numbered" }
  | { kind: "unsupported"; reason: string };

export type PipelineResult =
  | {
      kind: "insert_text";
      text: string;
      source: ProviderSource;
      confidence?: number | null;
    }
  | {
      kind: "command";
      command: CommandAction;
      requires_confirmation: boolean;
      source: CommandSource;
    }
  | { kind: "cancelled"; reason: string }
  | { kind: "error"; recoverable: boolean; message: string };
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p wispergo-core --test domain_tests
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/wispergo-core apps/desktop/src/types/pipeline.ts apps/desktop/src-tauri/Cargo.toml
git commit -m "feat: add core domain contracts"
```

---

### Task 3: Privacy Policy Engine

**Files:**
- Create: `crates/wispergo-core/src/privacy.rs`
- Modify: `crates/wispergo-core/src/lib.rs`
- Create: `crates/wispergo-core/tests/privacy_tests.rs`

- [ ] **Step 1: Write failing privacy tests**

Create `crates/wispergo-core/tests/privacy_tests.rs`:

```rust
use wispergo_core::privacy::{
    CloudFallbackMode, ContextKind, PrivacyPolicy, PrivacyPolicyEngine, ProviderKind,
};

#[test]
fn local_only_never_allows_cloud_asr_or_cleanup() {
    let policy = PrivacyPolicy {
        fallback_mode: CloudFallbackMode::LocalOnly,
        ..PrivacyPolicy::default()
    };
    let engine = PrivacyPolicyEngine::new(policy);

    assert!(!engine.can_use_cloud("com.apple.Notes", ProviderKind::Asr));
    assert!(!engine.can_use_cloud("com.apple.Notes", ProviderKind::Cleanup));
}

#[test]
fn app_cloud_deny_list_overrides_automatic_fallback() {
    let policy = PrivacyPolicy {
        fallback_mode: CloudFallbackMode::PreferLocalAutomaticCloud,
        cloud_disabled_apps: vec!["com.apple.Terminal".to_string()],
        ..PrivacyPolicy::default()
    };
    let engine = PrivacyPolicyEngine::new(policy);

    assert!(!engine.can_use_cloud("com.apple.Terminal", ProviderKind::Cleanup));
    assert!(engine.can_use_cloud("com.apple.Notes", ProviderKind::Cleanup));
}

#[test]
fn context_disabled_for_app_blocks_selected_and_nearby_text() {
    let policy = PrivacyPolicy {
        context_disabled_apps: vec!["com.company.SecretApp".to_string()],
        ..PrivacyPolicy::default()
    };
    let engine = PrivacyPolicyEngine::new(policy);

    assert!(!engine.can_collect_context(
        "com.company.SecretApp",
        ContextKind::SelectedText
    ));
    assert!(!engine.can_collect_context(
        "com.company.SecretApp",
        ContextKind::NearbyText
    ));
    assert!(engine.can_collect_context("com.apple.Notes", ContextKind::ActiveApp));
}

#[test]
fn history_and_audio_defaults_are_private() {
    let engine = PrivacyPolicyEngine::default();

    assert!(engine.can_store_history());
    assert!(!engine.can_store_audio());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p wispergo-core --test privacy_tests
```

Expected: FAIL because `privacy` is not implemented.

- [ ] **Step 3: Implement the policy engine**

Update `crates/wispergo-core/src/lib.rs`:

```rust
pub mod domain;
pub mod privacy;
```

Create `crates/wispergo-core/src/privacy.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudFallbackMode {
    LocalOnly,
    PreferLocalAskBeforeCloud,
    PreferLocalAutomaticCloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Asr,
    Cleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextKind {
    ActiveApp,
    WindowTitle,
    SelectedText,
    NearbyText,
    Dictionary,
    StyleProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyPolicy {
    pub fallback_mode: CloudFallbackMode,
    pub cloud_disabled_apps: Vec<String>,
    pub context_disabled_apps: Vec<String>,
    pub history_enabled: bool,
    pub store_audio: bool,
}

impl Default for PrivacyPolicy {
    fn default() -> Self {
        Self {
            fallback_mode: CloudFallbackMode::PreferLocalAskBeforeCloud,
            cloud_disabled_apps: Vec::new(),
            context_disabled_apps: Vec::new(),
            history_enabled: true,
            store_audio: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PrivacyPolicyEngine {
    policy: PrivacyPolicy,
}

impl Default for PrivacyPolicyEngine {
    fn default() -> Self {
        Self::new(PrivacyPolicy::default())
    }
}

impl PrivacyPolicyEngine {
    pub fn new(policy: PrivacyPolicy) -> Self {
        Self { policy }
    }

    pub fn policy(&self) -> &PrivacyPolicy {
        &self.policy
    }

    pub fn cloud_fallback_mode(&self) -> CloudFallbackMode {
        self.policy.fallback_mode
    }

    pub fn can_use_cloud(&self, app_id: &str, _provider: ProviderKind) -> bool {
        if self.policy.cloud_disabled_apps.iter().any(|id| id == app_id) {
            return false;
        }

        !matches!(self.policy.fallback_mode, CloudFallbackMode::LocalOnly)
    }

    pub fn can_collect_context(&self, app_id: &str, kind: ContextKind) -> bool {
        if matches!(
            kind,
            ContextKind::SelectedText | ContextKind::NearbyText | ContextKind::WindowTitle
        ) && self
            .policy
            .context_disabled_apps
            .iter()
            .any(|id| id == app_id)
        {
            return false;
        }

        true
    }

    pub fn can_store_history(&self) -> bool {
        self.policy.history_enabled
    }

    pub fn can_store_audio(&self) -> bool {
        self.policy.store_audio
    }
}
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p wispergo-core --test privacy_tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wispergo-core/src/lib.rs crates/wispergo-core/src/privacy.rs crates/wispergo-core/tests/privacy_tests.rs
git commit -m "feat: add privacy policy engine"
```

---

### Task 4: Rule-Based Intent Engine

**Files:**
- Create: `crates/wispergo-core/src/intent.rs`
- Modify: `crates/wispergo-core/src/lib.rs`
- Create: `crates/wispergo-core/tests/intent_tests.rs`

- [ ] **Step 1: Write failing intent tests**

Create `crates/wispergo-core/tests/intent_tests.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p wispergo-core --test intent_tests
```

Expected: FAIL because `intent` is not implemented.

- [ ] **Step 3: Implement the rule parser**

Update `crates/wispergo-core/src/lib.rs`:

```rust
pub mod domain;
pub mod intent;
pub mod privacy;
```

Create `crates/wispergo-core/src/intent.rs`:

```rust
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
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p wispergo-core --test intent_tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wispergo-core/src/lib.rs crates/wispergo-core/src/intent.rs crates/wispergo-core/tests/intent_tests.rs
git commit -m "feat: add rule based intent engine"
```

---

### Task 5: Provider Contracts And Mocks

**Files:**
- Create: `crates/wispergo-core/src/providers.rs`
- Modify: `crates/wispergo-core/src/lib.rs`
- Create: `crates/wispergo-core/tests/provider_tests.rs`

- [ ] **Step 1: Write failing provider contract tests**

Create `crates/wispergo-core/tests/provider_tests.rs`:

```rust
use std::sync::{Arc, Mutex};
use std::time::Duration;

use wispergo_core::domain::{CommandAction, CommandSource, PipelineResult, ProviderSource};
use wispergo_core::providers::{
    AsrOutput, AsrProvider, CleanupInput, CleanupOutput, CleanupProvider, FakeAsrProvider,
    FakeCleanupProvider, ProviderError,
};

#[tokio::test]
async fn fake_asr_returns_configured_transcript() {
    let provider = FakeAsrProvider::new(Ok(AsrOutput {
        transcript: "hello world".to_string(),
        confidence: Some(0.8),
        source: ProviderSource::Local,
    }));

    let result = provider.transcribe(vec![0.0, 0.1]).await.expect("asr output");

    assert_eq!(result.transcript, "hello world");
    assert_eq!(result.source, ProviderSource::Local);
}

#[tokio::test]
async fn fake_cleanup_returns_structured_result() {
    let provider = FakeCleanupProvider::new(Ok(CleanupOutput {
        result: PipelineResult::Command {
            command: CommandAction::NewParagraph,
            requires_confirmation: false,
            source: CommandSource::LocalLlm,
        },
    }));

    let result = provider
        .clean(CleanupInput {
            transcript: "new paragraph".to_string(),
            selected_text: None,
            timeout: Duration::from_millis(500),
        })
        .await
        .expect("cleanup output");

    assert!(matches!(result.result, PipelineResult::Command { .. }));
}

#[tokio::test]
async fn provider_errors_distinguish_timeout_and_unavailable() {
    let timeout = ProviderError::Timeout {
        provider: "local_asr".to_string(),
    };
    let unavailable = ProviderError::Unavailable {
        provider: "ollama".to_string(),
    };

    assert!(timeout.is_recoverable());
    assert!(unavailable.is_recoverable());
}

#[tokio::test]
async fn fake_providers_record_call_counts() {
    let calls = Arc::new(Mutex::new(0));
    let provider = FakeAsrProvider::with_counter(
        Ok(AsrOutput {
            transcript: "hi".to_string(),
            confidence: None,
            source: ProviderSource::Local,
        }),
        calls.clone(),
    );

    let _ = provider.transcribe(vec![0.2]).await;

    assert_eq!(*calls.lock().expect("counter lock"), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p wispergo-core --test provider_tests
```

Expected: FAIL because provider contracts are not implemented.

- [ ] **Step 3: Implement provider traits and fakes**

Update `crates/wispergo-core/src/lib.rs`:

```rust
pub mod domain;
pub mod intent;
pub mod privacy;
pub mod providers;
```

Create `crates/wispergo-core/src/providers.rs`:

```rust
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{PipelineResult, ProviderSource};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AsrOutput {
    pub transcript: String,
    pub confidence: Option<f32>,
    pub source: ProviderSource,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CleanupInput {
    pub transcript: String,
    pub selected_text: Option<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CleanupOutput {
    pub result: PipelineResult,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProviderError {
    #[error("{provider} is unavailable")]
    Unavailable { provider: String },
    #[error("{provider} timed out")]
    Timeout { provider: String },
    #[error("{provider} returned invalid output: {message}")]
    InvalidOutput { provider: String, message: String },
    #[error("{provider} failed: {message}")]
    Failed { provider: String, message: String },
}

impl ProviderError {
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Unavailable { .. } | Self::Timeout { .. } | Self::InvalidOutput { .. }
        )
    }
}

#[async_trait]
pub trait AsrProvider: Send + Sync {
    async fn transcribe(&self, audio: Vec<f32>) -> Result<AsrOutput, ProviderError>;
}

#[async_trait]
pub trait CleanupProvider: Send + Sync {
    async fn clean(&self, input: CleanupInput) -> Result<CleanupOutput, ProviderError>;
}

#[derive(Debug, Clone)]
pub struct FakeAsrProvider {
    response: Result<AsrOutput, ProviderError>,
    calls: Option<Arc<Mutex<usize>>>,
}

impl FakeAsrProvider {
    pub fn new(response: Result<AsrOutput, ProviderError>) -> Self {
        Self {
            response,
            calls: None,
        }
    }

    pub fn with_counter(
        response: Result<AsrOutput, ProviderError>,
        calls: Arc<Mutex<usize>>,
    ) -> Self {
        Self {
            response,
            calls: Some(calls),
        }
    }
}

#[async_trait]
impl AsrProvider for FakeAsrProvider {
    async fn transcribe(&self, _audio: Vec<f32>) -> Result<AsrOutput, ProviderError> {
        if let Some(calls) = &self.calls {
            *calls.lock().expect("fake asr counter lock") += 1;
        }
        self.response.clone()
    }
}

#[derive(Debug, Clone)]
pub struct FakeCleanupProvider {
    response: Result<CleanupOutput, ProviderError>,
}

impl FakeCleanupProvider {
    pub fn new(response: Result<CleanupOutput, ProviderError>) -> Self {
        Self { response }
    }
}

#[async_trait]
impl CleanupProvider for FakeCleanupProvider {
    async fn clean(&self, _input: CleanupInput) -> Result<CleanupOutput, ProviderError> {
        self.response.clone()
    }
}
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p wispergo-core --test provider_tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wispergo-core/src/lib.rs crates/wispergo-core/src/providers.rs crates/wispergo-core/tests/provider_tests.rs
git commit -m "feat: add provider contracts"
```

---

### Task 6: Fallback Decisions

**Files:**
- Create: `crates/wispergo-core/src/fallback.rs`
- Modify: `crates/wispergo-core/src/lib.rs`
- Create: `crates/wispergo-core/tests/fallback_tests.rs`

- [ ] **Step 1: Write failing fallback tests**

Create `crates/wispergo-core/tests/fallback_tests.rs`:

```rust
use wispergo_core::fallback::{FallbackDecision, FallbackEngine, FallbackRequest};
use wispergo_core::privacy::{CloudFallbackMode, PrivacyPolicy, ProviderKind};
use wispergo_core::providers::ProviderError;

#[test]
fn local_only_fails_closed_on_provider_timeout() {
    let engine = FallbackEngine::new(PrivacyPolicy {
        fallback_mode: CloudFallbackMode::LocalOnly,
        ..PrivacyPolicy::default()
    });

    let decision = engine.decide(FallbackRequest {
        app_id: "com.apple.Notes".to_string(),
        provider_kind: ProviderKind::Asr,
        error: ProviderError::Timeout {
            provider: "local_asr".to_string(),
        },
    });

    assert_eq!(decision, FallbackDecision::FailLocalOnly);
}

#[test]
fn ask_before_cloud_returns_confirmation_decision() {
    let engine = FallbackEngine::new(PrivacyPolicy {
        fallback_mode: CloudFallbackMode::PreferLocalAskBeforeCloud,
        ..PrivacyPolicy::default()
    });

    let decision = engine.decide(FallbackRequest {
        app_id: "com.apple.Notes".to_string(),
        provider_kind: ProviderKind::Cleanup,
        error: ProviderError::Unavailable {
            provider: "ollama".to_string(),
        },
    });

    assert_eq!(decision, FallbackDecision::AskBeforeCloud);
}

#[test]
fn automatic_cloud_respects_app_deny_list() {
    let engine = FallbackEngine::new(PrivacyPolicy {
        fallback_mode: CloudFallbackMode::PreferLocalAutomaticCloud,
        cloud_disabled_apps: vec!["com.apple.Terminal".to_string()],
        ..PrivacyPolicy::default()
    });

    let decision = engine.decide(FallbackRequest {
        app_id: "com.apple.Terminal".to_string(),
        provider_kind: ProviderKind::Cleanup,
        error: ProviderError::Timeout {
            provider: "ollama".to_string(),
        },
    });

    assert_eq!(decision, FallbackDecision::CloudBlockedForApp);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p wispergo-core --test fallback_tests
```

Expected: FAIL because fallback decisions are not implemented.

- [ ] **Step 3: Implement fallback engine**

Update `crates/wispergo-core/src/lib.rs`:

```rust
pub mod domain;
pub mod fallback;
pub mod intent;
pub mod privacy;
pub mod providers;
```

Create `crates/wispergo-core/src/fallback.rs`:

```rust
use crate::privacy::{CloudFallbackMode, PrivacyPolicy, PrivacyPolicyEngine, ProviderKind};
use crate::providers::ProviderError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackRequest {
    pub app_id: String,
    pub provider_kind: ProviderKind,
    pub error: ProviderError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackDecision {
    FailLocalOnly,
    AskBeforeCloud,
    UseCloudAutomatically,
    CloudBlockedForApp,
    FailUnrecoverable,
}

#[derive(Debug, Clone)]
pub struct FallbackEngine {
    privacy: PrivacyPolicyEngine,
}

impl FallbackEngine {
    pub fn new(policy: PrivacyPolicy) -> Self {
        Self {
            privacy: PrivacyPolicyEngine::new(policy),
        }
    }

    pub fn decide(&self, request: FallbackRequest) -> FallbackDecision {
        if !request.error.is_recoverable() {
            return FallbackDecision::FailUnrecoverable;
        }

        if !self
            .privacy
            .can_use_cloud(&request.app_id, request.provider_kind)
        {
            return if matches!(
                self.privacy.cloud_fallback_mode(),
                CloudFallbackMode::LocalOnly
            ) {
                FallbackDecision::FailLocalOnly
            } else {
                FallbackDecision::CloudBlockedForApp
            };
        }

        match self.privacy.cloud_fallback_mode() {
            CloudFallbackMode::LocalOnly => FallbackDecision::FailLocalOnly,
            CloudFallbackMode::PreferLocalAskBeforeCloud => FallbackDecision::AskBeforeCloud,
            CloudFallbackMode::PreferLocalAutomaticCloud => {
                FallbackDecision::UseCloudAutomatically
            }
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p wispergo-core --test fallback_tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wispergo-core/src/lib.rs crates/wispergo-core/src/fallback.rs crates/wispergo-core/tests/fallback_tests.rs
git commit -m "feat: add fallback decision engine"
```

---

### Task 7: Audio Formatting And VAD

**Files:**
- Create: `crates/wispergo-core/src/audio.rs`
- Modify: `crates/wispergo-core/src/lib.rs`
- Create: `crates/wispergo-core/tests/audio_tests.rs`

- [ ] **Step 1: Write failing audio tests**

Create `crates/wispergo-core/tests/audio_tests.rs`:

```rust
use wispergo_core::audio::{trim_silence, VadConfig};

#[test]
fn trims_leading_and_trailing_silence() {
    let input = vec![0.0, 0.001, 0.08, 0.12, 0.002, 0.0];
    let output = trim_silence(&input, VadConfig::default());

    assert_eq!(output, vec![0.08, 0.12]);
}

#[test]
fn keeps_audio_when_every_sample_is_below_threshold() {
    let input = vec![0.0, 0.001, 0.002];
    let output = trim_silence(&input, VadConfig::default());

    assert_eq!(output, input);
}

#[test]
fn custom_threshold_changes_trim_boundary() {
    let input = vec![0.03, 0.06, 0.02];
    let output = trim_silence(
        &input,
        VadConfig {
            silence_threshold: 0.05,
        },
    );

    assert_eq!(output, vec![0.06]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p wispergo-core --test audio_tests
```

Expected: FAIL because audio helpers are not implemented.

- [ ] **Step 3: Implement VAD trimming helpers**

Update `crates/wispergo-core/src/lib.rs`:

```rust
pub mod audio;
pub mod domain;
pub mod fallback;
pub mod intent;
pub mod privacy;
pub mod providers;
```

Create `crates/wispergo-core/src/audio.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VadConfig {
    pub silence_threshold: f32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            silence_threshold: 0.02,
        }
    }
}

pub fn trim_silence(samples: &[f32], config: VadConfig) -> Vec<f32> {
    let first = samples
        .iter()
        .position(|sample| sample.abs() >= config.silence_threshold);
    let last = samples
        .iter()
        .rposition(|sample| sample.abs() >= config.silence_threshold);

    match (first, last) {
        (Some(start), Some(end)) if start <= end => samples[start..=end].to_vec(),
        _ => samples.to_vec(),
    }
}
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p wispergo-core --test audio_tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wispergo-core/src/lib.rs crates/wispergo-core/src/audio.rs crates/wispergo-core/tests/audio_tests.rs
git commit -m "feat: add vad audio trimming"
```

---

### Task 8: SQLite Local Store

**Files:**
- Create: `apps/desktop/src-tauri/migrations/0001_initial.sql`
- Create: `crates/wispergo-core/src/store.rs`
- Modify: `crates/wispergo-core/src/lib.rs`
- Create: `crates/wispergo-core/tests/store_tests.rs`

- [ ] **Step 1: Write failing store tests**

Create `crates/wispergo-core/tests/store_tests.rs`:

```rust
use wispergo_core::privacy::{CloudFallbackMode, PrivacyPolicy};
use wispergo_core::store::LocalStore;

#[test]
fn saves_and_loads_privacy_policy() {
    let store = LocalStore::open_in_memory().expect("open store");
    store.migrate().expect("migrate");

    let policy = PrivacyPolicy {
        fallback_mode: CloudFallbackMode::PreferLocalAutomaticCloud,
        cloud_disabled_apps: vec!["com.apple.Terminal".to_string()],
        context_disabled_apps: vec!["com.company.SecretApp".to_string()],
        history_enabled: false,
        store_audio: false,
    };

    store.save_privacy_policy(&policy).expect("save policy");
    let loaded = store.load_privacy_policy().expect("load policy");

    assert_eq!(loaded, policy);
}

#[test]
fn history_respects_enabled_flag_at_call_site() {
    let store = LocalStore::open_in_memory().expect("open store");
    store.migrate().expect("migrate");

    store
        .insert_history("hello world", "local")
        .expect("insert history");

    let rows = store.history_count().expect("history count");
    assert_eq!(rows, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p wispergo-core --test store_tests
```

Expected: FAIL because `store` is not implemented.

- [ ] **Step 3: Add migration SQL**

Create `apps/desktop/src-tauri/migrations/0001_initial.sql`:

```sql
CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY NOT NULL,
  value TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  text TEXT NOT NULL,
  source TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS dictionary_terms (
  term TEXT PRIMARY KEY NOT NULL,
  replacement TEXT,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS style_profiles (
  name TEXT PRIMARY KEY NOT NULL,
  instructions TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS provider_telemetry (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  provider TEXT NOT NULL,
  latency_ms INTEGER NOT NULL,
  success INTEGER NOT NULL,
  created_at INTEGER NOT NULL
);
```

- [ ] **Step 4: Implement LocalStore**

Update `crates/wispergo-core/src/lib.rs`:

```rust
pub mod audio;
pub mod domain;
pub mod fallback;
pub mod intent;
pub mod privacy;
pub mod providers;
pub mod store;
```

Create `crates/wispergo-core/src/store.rs`:

```rust
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use thiserror::Error;

use crate::privacy::PrivacyPolicy;

const PRIVACY_POLICY_KEY: &str = "privacy_policy";
const MIGRATION: &str = include_str!("../../../apps/desktop/src-tauri/migrations/0001_initial.sql");

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("system clock is before unix epoch")]
    Clock,
}

pub struct LocalStore {
    conn: Connection,
}

impl LocalStore {
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Ok(Self {
            conn: Connection::open_in_memory()?,
        })
    }

    pub fn migrate(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(MIGRATION)?;
        Ok(())
    }

    pub fn save_privacy_policy(&self, policy: &PrivacyPolicy) -> Result<(), StoreError> {
        let value = serde_json::to_string(policy)?;
        self.conn.execute(
            "INSERT INTO settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![PRIVACY_POLICY_KEY, value, now()?],
        )?;
        Ok(())
    }

    pub fn load_privacy_policy(&self) -> Result<PrivacyPolicy, StoreError> {
        let value: String = self.conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![PRIVACY_POLICY_KEY],
            |row| row.get(0),
        )?;
        Ok(serde_json::from_str(&value)?)
    }

    pub fn insert_history(&self, text: &str, source: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO history (text, source, created_at) VALUES (?1, ?2, ?3)",
            params![text, source, now()?],
        )?;
        Ok(())
    }

    pub fn history_count(&self) -> Result<i64, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?)
    }
}

fn now() -> Result<i64, StoreError> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreError::Clock)?
        .as_secs() as i64)
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p wispergo-core --test store_tests
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/migrations/0001_initial.sql crates/wispergo-core/src/lib.rs crates/wispergo-core/src/store.rs crates/wispergo-core/tests/store_tests.rs
git commit -m "feat: add sqlite local store"
```

---

### Task 9: Ollama Cleanup Provider

**Files:**
- Create: `crates/wispergo-core/src/ollama.rs`
- Modify: `crates/wispergo-core/src/lib.rs`
- Create: `crates/wispergo-core/tests/fixtures/cleanup_insert_text.json`
- Create: `crates/wispergo-core/tests/fixtures/cleanup_command_rewrite.json`
- Create: `crates/wispergo-core/tests/fixtures/cleanup_invalid.json`
- Create: `crates/wispergo-core/tests/ollama_tests.rs`

- [ ] **Step 1: Add cleanup fixture files**

Create `crates/wispergo-core/tests/fixtures/cleanup_insert_text.json`:

```json
{
  "result": {
    "kind": "insert_text",
    "text": "Hello, world.",
    "source": "local",
    "confidence": 0.91
  }
}
```

Create `crates/wispergo-core/tests/fixtures/cleanup_command_rewrite.json`:

```json
{
  "result": {
    "kind": "command",
    "command": {
      "kind": "rewrite_selection",
      "style": "professional"
    },
    "requires_confirmation": false,
    "source": "local_llm"
  }
}
```

Create `crates/wispergo-core/tests/fixtures/cleanup_invalid.json`:

```json
{
  "result": {
    "kind": "command",
    "command": {
      "kind": "send_message"
    },
    "requires_confirmation": false,
    "source": "local_llm"
  }
}
```

- [ ] **Step 2: Write failing Ollama tests**

Create `crates/wispergo-core/tests/ollama_tests.rs`:

```rust
use std::time::Duration;

use httpmock::prelude::*;
use wispergo_core::domain::{CommandAction, PipelineResult};
use wispergo_core::ollama::{parse_cleanup_json, OllamaCleanupProvider};
use wispergo_core::providers::{CleanupInput, CleanupProvider, ProviderError};

#[test]
fn parses_valid_insert_text_fixture() {
    let fixture = include_str!("fixtures/cleanup_insert_text.json");
    let output = parse_cleanup_json(fixture).expect("parse fixture");

    assert!(matches!(output.result, PipelineResult::InsertText { .. }));
}

#[test]
fn parses_valid_command_fixture() {
    let fixture = include_str!("fixtures/cleanup_command_rewrite.json");
    let output = parse_cleanup_json(fixture).expect("parse fixture");

    assert!(matches!(
        output.result,
        PipelineResult::Command {
            command: CommandAction::RewriteSelection { .. },
            ..
        }
    ));
}

#[test]
fn rejects_unknown_command_fixture() {
    let fixture = include_str!("fixtures/cleanup_invalid.json");
    let error = parse_cleanup_json(fixture).expect_err("invalid command should fail");

    assert!(matches!(error, ProviderError::InvalidOutput { .. }));
}

#[tokio::test]
async fn calls_ollama_chat_api_and_parses_json_content() {
    let server = MockServer::start();
    let body = serde_json::json!({
        "message": {
            "content": include_str!("fixtures/cleanup_insert_text.json")
        }
    });

    let mock = server.mock(|when, then| {
        when.method(POST).path("/api/chat");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(body);
    });

    let provider = OllamaCleanupProvider::new(server.base_url(), "llama3.2:3b".to_string());
    let output = provider
        .clean(CleanupInput {
            transcript: "hello world".to_string(),
            selected_text: None,
            timeout: Duration::from_secs(2),
        })
        .await
        .expect("cleanup output");

    mock.assert();
    assert!(matches!(output.result, PipelineResult::InsertText { .. }));
}
```

- [ ] **Step 3: Run test to verify it fails**

Run:

```bash
cargo test -p wispergo-core --test ollama_tests
```

Expected: FAIL because Ollama provider is not implemented.

- [ ] **Step 4: Implement Ollama cleanup provider**

Update `crates/wispergo-core/src/lib.rs`:

```rust
pub mod audio;
pub mod domain;
pub mod fallback;
pub mod intent;
pub mod ollama;
pub mod privacy;
pub mod providers;
pub mod store;
```

Create `crates/wispergo-core/src/ollama.rs`:

```rust
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::providers::{CleanupInput, CleanupOutput, CleanupProvider, ProviderError};

#[derive(Debug, Clone)]
pub struct OllamaCleanupProvider {
    base_url: String,
    model: String,
    client: Client,
}

impl OllamaCleanupProvider {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl CleanupProvider for OllamaCleanupProvider {
    async fn clean(&self, input: CleanupInput) -> Result<CleanupOutput, ProviderError> {
        let request = OllamaChatRequest {
            model: self.model.clone(),
            stream: false,
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: cleanup_system_prompt(),
                },
                OllamaMessage {
                    role: "user".to_string(),
                    content: cleanup_user_prompt(&input),
                },
            ],
        };

        let url = format!("{}/api/chat", self.base_url);
        let response = tokio::time::timeout(
            input.timeout,
            self.client.post(url).json(&request).send(),
        )
        .await
        .map_err(|_| ProviderError::Timeout {
            provider: "ollama".to_string(),
        })?
        .map_err(|err| ProviderError::Unavailable {
            provider: format!("ollama: {err}"),
        })?;

        let body: OllamaChatResponse =
            response.json().await.map_err(|err| ProviderError::InvalidOutput {
                provider: "ollama".to_string(),
                message: err.to_string(),
            })?;

        parse_cleanup_json(&body.message.content)
    }
}

pub fn parse_cleanup_json(input: &str) -> Result<CleanupOutput, ProviderError> {
    serde_json::from_str::<CleanupOutput>(input).map_err(|err| ProviderError::InvalidOutput {
        provider: "ollama".to_string(),
        message: err.to_string(),
    })
}

fn cleanup_system_prompt() -> String {
    "Return only JSON matching the CleanupOutput schema. Do not execute commands. Classify user intent into insert_text, command, cancelled, or error results.".to_string()
}

fn cleanup_user_prompt(input: &CleanupInput) -> String {
    format!(
        "Transcript: {}\nSelected text: {}",
        input.transcript,
        input.selected_text.as_deref().unwrap_or("")
    )
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    stream: bool,
    messages: Vec<OllamaMessage>,
}

#[derive(Debug, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    content: String,
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p wispergo-core --test ollama_tests
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/wispergo-core/src/lib.rs crates/wispergo-core/src/ollama.rs crates/wispergo-core/tests/ollama_tests.rs crates/wispergo-core/tests/fixtures
git commit -m "feat: add ollama cleanup provider"
```

---

### Task 10: Whisper Sidecar ASR Adapter

**Files:**
- Create: `crates/wispergo-core/src/whisper_sidecar.rs`
- Modify: `crates/wispergo-core/src/lib.rs`
- Create: `crates/wispergo-core/tests/whisper_sidecar_tests.rs`

- [ ] **Step 1: Write failing sidecar tests**

Create `crates/wispergo-core/tests/whisper_sidecar_tests.rs`:

```rust
use std::fs;

use tempfile::tempdir;
use wispergo_core::domain::ProviderSource;
use wispergo_core::providers::AsrProvider;
use wispergo_core::whisper_sidecar::{parse_whisper_output, WhisperSidecarProvider};

#[test]
fn parses_plain_whisper_output() {
    let transcript = parse_whisper_output(" hello world \n").expect("parse");
    assert_eq!(transcript, "hello world");
}

#[tokio::test]
async fn sidecar_provider_invokes_configured_binary() {
    let dir = tempdir().expect("tempdir");
    let script = dir.path().join("fake-whisper.sh");
    fs::write(
        &script,
        "#!/bin/sh\nprintf 'sidecar transcript\\n'\n",
    )
    .expect("write script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("chmod");
    }

    let provider = WhisperSidecarProvider::new(script, None);
    let output = provider.transcribe(vec![0.1, 0.2]).await.expect("transcribe");

    assert_eq!(output.transcript, "sidecar transcript");
    assert_eq!(output.source, ProviderSource::Local);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p wispergo-core --test whisper_sidecar_tests
```

Expected: FAIL because the sidecar adapter is not implemented.

- [ ] **Step 3: Implement configurable whisper sidecar adapter**

Update `crates/wispergo-core/src/lib.rs`:

```rust
pub mod audio;
pub mod domain;
pub mod fallback;
pub mod intent;
pub mod ollama;
pub mod privacy;
pub mod providers;
pub mod store;
pub mod whisper_sidecar;
```

Create `crates/wispergo-core/src/whisper_sidecar.rs`:

```rust
use std::path::PathBuf;

use async_trait::async_trait;
use tokio::process::Command;

use crate::domain::ProviderSource;
use crate::providers::{AsrOutput, AsrProvider, ProviderError};

#[derive(Debug, Clone)]
pub struct WhisperSidecarProvider {
    binary_path: PathBuf,
    model_path: Option<PathBuf>,
}

impl WhisperSidecarProvider {
    pub fn new(binary_path: PathBuf, model_path: Option<PathBuf>) -> Self {
        Self {
            binary_path,
            model_path,
        }
    }
}

#[async_trait]
impl AsrProvider for WhisperSidecarProvider {
    async fn transcribe(&self, _audio: Vec<f32>) -> Result<AsrOutput, ProviderError> {
        let mut command = Command::new(&self.binary_path);

        if let Some(model_path) = &self.model_path {
            command.arg("--model").arg(model_path);
        }

        let output = command.output().await.map_err(|err| ProviderError::Unavailable {
            provider: format!("whisper_sidecar: {err}"),
        })?;

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
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p wispergo-core --test whisper_sidecar_tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wispergo-core/src/lib.rs crates/wispergo-core/src/whisper_sidecar.rs crates/wispergo-core/tests/whisper_sidecar_tests.rs
git commit -m "feat: add whisper sidecar provider"
```

---

### Task 11: Pipeline Orchestration

**Files:**
- Create: `crates/wispergo-core/src/pipeline.rs`
- Modify: `crates/wispergo-core/src/lib.rs`
- Create: `crates/wispergo-core/tests/pipeline_tests.rs`

- [ ] **Step 1: Write failing pipeline tests**

Create `crates/wispergo-core/tests/pipeline_tests.rs`:

```rust
use std::time::Duration;

use wispergo_core::domain::{ActiveContext, CommandAction, PipelineResult, ProviderSource};
use wispergo_core::pipeline::{Pipeline, PipelineInput};
use wispergo_core::privacy::PrivacyPolicy;
use wispergo_core::providers::{
    AsrOutput, CleanupOutput, FakeAsrProvider, FakeCleanupProvider, ProviderError,
};

fn context() -> ActiveContext {
    ActiveContext {
        app_id: "com.apple.Notes".to_string(),
        app_name: "Notes".to_string(),
        window_title: None,
        selected_text: None,
        style_profile: None,
    }
}

#[tokio::test]
async fn rule_command_skips_cleanup_provider() {
    let pipeline = Pipeline::new(
        FakeAsrProvider::new(Ok(AsrOutput {
            transcript: "new paragraph".to_string(),
            confidence: Some(0.9),
            source: ProviderSource::Local,
        })),
        FakeCleanupProvider::new(Err(ProviderError::Failed {
            provider: "cleanup".to_string(),
            message: "should not be called".to_string(),
        })),
        PrivacyPolicy::default(),
    );

    let result = pipeline
        .run(PipelineInput {
            audio: vec![0.1],
            context: context(),
            cleanup_timeout: Duration::from_millis(500),
        })
        .await;

    assert!(matches!(
        result,
        PipelineResult::Command {
            command: CommandAction::NewParagraph,
            ..
        }
    ));
}

#[tokio::test]
async fn dictation_flows_through_cleanup_provider() {
    let pipeline = Pipeline::new(
        FakeAsrProvider::new(Ok(AsrOutput {
            transcript: "hello world".to_string(),
            confidence: Some(0.9),
            source: ProviderSource::Local,
        })),
        FakeCleanupProvider::new(Ok(CleanupOutput {
            result: PipelineResult::InsertText {
                text: "Hello, world.".to_string(),
                source: ProviderSource::Local,
                confidence: Some(0.9),
            },
        })),
        PrivacyPolicy::default(),
    );

    let result = pipeline
        .run(PipelineInput {
            audio: vec![0.1],
            context: context(),
            cleanup_timeout: Duration::from_millis(500),
        })
        .await;

    assert_eq!(
        result,
        PipelineResult::InsertText {
            text: "Hello, world.".to_string(),
            source: ProviderSource::Local,
            confidence: Some(0.9)
        }
    );
}

#[tokio::test]
async fn cleanup_timeout_inserts_raw_asr_for_plain_dictation() {
    let pipeline = Pipeline::new(
        FakeAsrProvider::new(Ok(AsrOutput {
            transcript: "plain dictation".to_string(),
            confidence: Some(0.7),
            source: ProviderSource::Local,
        })),
        FakeCleanupProvider::new(Err(ProviderError::Timeout {
            provider: "ollama".to_string(),
        })),
        PrivacyPolicy::default(),
    );

    let result = pipeline
        .run(PipelineInput {
            audio: vec![0.1],
            context: context(),
            cleanup_timeout: Duration::from_millis(100),
        })
        .await;

    assert_eq!(
        result,
        PipelineResult::InsertText {
            text: "plain dictation".to_string(),
            source: ProviderSource::Local,
            confidence: Some(0.7)
        }
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p wispergo-core --test pipeline_tests
```

Expected: FAIL because pipeline orchestration is not implemented.

- [ ] **Step 3: Implement pipeline orchestration**

Update `crates/wispergo-core/src/lib.rs`:

```rust
pub mod audio;
pub mod domain;
pub mod fallback;
pub mod intent;
pub mod ollama;
pub mod pipeline;
pub mod privacy;
pub mod providers;
pub mod store;
pub mod whisper_sidecar;
```

Create `crates/wispergo-core/src/pipeline.rs`:

```rust
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
            intent: IntentEngine::default(),
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
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p wispergo-core --test pipeline_tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wispergo-core/src/lib.rs crates/wispergo-core/src/pipeline.rs crates/wispergo-core/tests/pipeline_tests.rs
git commit -m "feat: add dictation pipeline orchestration"
```

---

### Task 12: Tauri State, Commands, And Clipboard Insertion

**Files:**
- Create: `apps/desktop/src-tauri/src/state.rs`
- Create: `apps/desktop/src-tauri/src/commands/mod.rs`
- Create: `apps/desktop/src-tauri/src/commands/recording.rs`
- Create: `apps/desktop/src-tauri/src/commands/settings.rs`
- Create: `apps/desktop/src-tauri/src/insertion/mod.rs`
- Create: `apps/desktop/src-tauri/src/insertion/clipboard.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src/lib/tauriApi.ts`

- [ ] **Step 1: Write failing Rust command tests**

Create `apps/desktop/src-tauri/src/commands/recording.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use crate::state::{AppState, RecordingStatus};

    #[test]
    fn start_and_cancel_recording_update_state() {
        let state = AppState::default();

        state.start_recording("toggle").expect("start");
        assert_eq!(state.recording_status(), RecordingStatus::Recording);

        state.cancel_recording("user_cancelled").expect("cancel");
        assert_eq!(state.recording_status(), RecordingStatus::Idle);
    }
}
```

Run:

```bash
cargo test -p wispergo-desktop --lib start_and_cancel_recording_update_state
```

Expected: FAIL because `AppState` does not exist.

- [ ] **Step 2: Implement state and command modules**

Create `apps/desktop/src-tauri/src/state.rs`:

```rust
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingStatus {
    Idle,
    Recording,
}

pub struct AppState {
    recording: Mutex<RecordingStatus>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            recording: Mutex::new(RecordingStatus::Idle),
        }
    }
}

impl AppState {
    pub fn recording_status(&self) -> RecordingStatus {
        *self.recording.lock().expect("recording status lock")
    }

    pub fn start_recording(&self, _mode: &str) -> Result<(), String> {
        let mut recording = self.recording.lock().map_err(|err| err.to_string())?;
        *recording = RecordingStatus::Recording;
        Ok(())
    }

    pub fn stop_recording(&self, _reason: &str) -> Result<(), String> {
        let mut recording = self.recording.lock().map_err(|err| err.to_string())?;
        *recording = RecordingStatus::Idle;
        Ok(())
    }

    pub fn cancel_recording(&self, reason: &str) -> Result<(), String> {
        self.stop_recording(reason)
    }
}
```

Create `apps/desktop/src-tauri/src/commands/mod.rs`:

```rust
pub mod recording;
pub mod settings;
```

Set `apps/desktop/src-tauri/src/commands/recording.rs` to:

```rust
use tauri::State;

use crate::state::{AppState, RecordingStatus};

#[tauri::command]
pub fn start_recording(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    state.start_recording(&mode)
}

#[tauri::command]
pub fn stop_recording(state: State<'_, AppState>, reason: String) -> Result<(), String> {
    state.stop_recording(&reason)
}

#[tauri::command]
pub fn cancel_recording(state: State<'_, AppState>, reason: String) -> Result<(), String> {
    state.cancel_recording(&reason)
}

#[tauri::command]
pub fn recording_status(state: State<'_, AppState>) -> &'static str {
    match state.recording_status() {
        RecordingStatus::Idle => "idle",
        RecordingStatus::Recording => "recording",
    }
}

#[cfg(test)]
mod tests {
    use crate::state::{AppState, RecordingStatus};

    #[test]
    fn start_and_cancel_recording_update_state() {
        let state = AppState::default();

        state.start_recording("toggle").expect("start");
        assert_eq!(state.recording_status(), RecordingStatus::Recording);

        state.cancel_recording("user_cancelled").expect("cancel");
        assert_eq!(state.recording_status(), RecordingStatus::Idle);
    }
}
```

Create `apps/desktop/src-tauri/src/commands/settings.rs`:

```rust
#[tauri::command]
pub fn fallback_policy_label() -> &'static str {
    "prefer_local_ask_before_cloud"
}
```

- [ ] **Step 3: Add clipboard insertion adapter**

Create `apps/desktop/src-tauri/src/insertion/mod.rs`:

```rust
pub mod clipboard;
```

Create `apps/desktop/src-tauri/src/insertion/clipboard.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertionResult {
    Inserted,
    CopiedOnly,
}

pub trait Clipboard {
    fn set_text(&self, text: &str) -> Result<(), String>;
}

pub fn insert_via_clipboard<C: Clipboard>(
    clipboard: &C,
    text: &str,
) -> Result<InsertionResult, String> {
    clipboard.set_text(text)?;
    Ok(InsertionResult::CopiedOnly)
}

#[cfg(test)]
mod tests {
    use super::{insert_via_clipboard, Clipboard, InsertionResult};

    struct FakeClipboard;

    impl Clipboard for FakeClipboard {
        fn set_text(&self, _text: &str) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn failed_native_paste_still_copies_text() {
        let result = insert_via_clipboard(&FakeClipboard, "hello").expect("insert");

        assert_eq!(result, InsertionResult::CopiedOnly);
    }
}
```

- [ ] **Step 4: Wire commands into Tauri**

Update `apps/desktop/src-tauri/src/lib.rs`:

```rust
mod commands;
mod insertion;
mod state;

use commands::recording::{cancel_recording, recording_status, start_recording, stop_recording};
use commands::settings::fallback_policy_label;
use state::AppState;

#[tauri::command]
fn app_health() -> &'static str {
    "ok"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            app_health,
            start_recording,
            stop_recording,
            cancel_recording,
            recording_status,
            fallback_policy_label
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 5: Add typed frontend API wrappers**

Create `apps/desktop/src/lib/tauriApi.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { RecordingMode } from "../types/pipeline";

export async function appHealth(): Promise<string> {
  return invoke<string>("app_health");
}

export async function startRecording(mode: RecordingMode): Promise<void> {
  await invoke("start_recording", { mode });
}

export async function stopRecording(reason: string): Promise<void> {
  await invoke("stop_recording", { reason });
}

export async function cancelRecording(reason: string): Promise<void> {
  await invoke("cancel_recording", { reason });
}

export async function recordingStatus(): Promise<"idle" | "recording"> {
  return invoke<"idle" | "recording">("recording_status");
}

export async function fallbackPolicyLabel(): Promise<string> {
  return invoke<string>("fallback_policy_label");
}
```

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p wispergo-desktop --lib
pnpm --dir apps/desktop test -- --run
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src-tauri/src apps/desktop/src/lib/tauriApi.ts
git commit -m "feat: add tauri recording commands"
```

---

### Task 13: Trigger Manager And Thin UI

**Files:**
- Create: `apps/desktop/src/features/recorder/FloatingRecorder.tsx`
- Create: `apps/desktop/src/features/recorder/FloatingRecorder.test.tsx`
- Create: `apps/desktop/src/features/settings/SettingsPanel.tsx`
- Create: `apps/desktop/src/features/settings/SettingsPanel.test.tsx`
- Modify: `apps/desktop/src/app/App.tsx`
- Create: `apps/desktop/src/app/App.test.tsx`
- Modify: `apps/desktop/vite.config.ts`
- Create: `apps/desktop/vitest.setup.ts`
- Modify: `package.json`
- Create: `apps/desktop/src-tauri/src/trigger/mod.rs`
- Create: `apps/desktop/src-tauri/src/trigger/manager.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/Cargo.toml`

- [ ] **Step 1: Add frontend test dependencies if missing**

Update `apps/desktop/package.json` dev dependencies to include:

```json
{
  "devDependencies": {
    "@testing-library/jest-dom": "^6.4.8",
    "@testing-library/react": "^16.0.1",
    "@testing-library/user-event": "^14.5.2",
    "jsdom": "^24.1.1",
    "vitest": "^2.0.5"
  }
}
```

Ensure the script exists:

```json
{
  "scripts": {
    "test": "vitest"
  }
}
```

Update root `package.json` after the frontend test script exists:

```json
{
  "scripts": {
    "test": "pnpm --dir apps/desktop test -- --run && cargo test --workspace",
    "test:ts": "pnpm --dir apps/desktop test -- --run",
    "test:rust": "cargo test --workspace"
  }
}
```

Ensure `apps/desktop/vite.config.ts` includes a jsdom test environment:

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: "./vitest.setup.ts",
  },
});
```

Create `apps/desktop/vitest.setup.ts`:

```ts
import "@testing-library/jest-dom/vitest";
```

- [ ] **Step 2: Write failing recorder UI tests**

Create `apps/desktop/src/features/recorder/FloatingRecorder.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { FloatingRecorder } from "./FloatingRecorder";

describe("FloatingRecorder", () => {
  it("starts and stops recording in toggle mode", async () => {
    const user = userEvent.setup();
    const onStart = vi.fn();
    const onStop = vi.fn();

    const { rerender } = render(
      <FloatingRecorder
        status="idle"
        mode="toggle"
        onStart={onStart}
        onStop={onStop}
        onCancel={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Start recording" }));
    expect(onStart).toHaveBeenCalledWith("toggle");

    rerender(
      <FloatingRecorder
        status="recording"
        mode="toggle"
        onStart={onStart}
        onStop={onStop}
        onCancel={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Stop recording" }));
    expect(onStop).toHaveBeenCalledWith("floating_button");
  });
});
```

- [ ] **Step 3: Implement FloatingRecorder**

Create `apps/desktop/src/features/recorder/FloatingRecorder.tsx`:

```tsx
import type { RecordingMode } from "../../types/pipeline";

type RecordingStatus = "idle" | "recording";

type Props = {
  status: RecordingStatus;
  mode: RecordingMode;
  onStart: (mode: RecordingMode) => void;
  onStop: (reason: string) => void;
  onCancel: (reason: string) => void;
};

export function FloatingRecorder({ status, mode, onStart, onStop, onCancel }: Props) {
  const isRecording = status === "recording";

  return (
    <section className="floating-recorder" aria-label="Recorder">
      <div className="recording-status">{isRecording ? "Recording" : "Ready"}</div>
      <button
        type="button"
        className="record-button"
        aria-label={isRecording ? "Stop recording" : "Start recording"}
        onClick={() => {
          if (isRecording) {
            onStop("floating_button");
          } else {
            onStart(mode);
          }
        }}
      >
        {isRecording ? "Stop" : "Record"}
      </button>
      <button type="button" onClick={() => onCancel("user_cancelled")}>
        Cancel
      </button>
    </section>
  );
}
```

- [ ] **Step 4: Add settings UI and app wiring**

Create `apps/desktop/src/features/settings/SettingsPanel.tsx`:

```tsx
import type { RecordingMode } from "../../types/pipeline";

type Props = {
  mode: RecordingMode;
  fallbackPolicy: string;
  onModeChange: (mode: RecordingMode) => void;
};

export function SettingsPanel({ mode, fallbackPolicy, onModeChange }: Props) {
  return (
    <section className="settings-panel" aria-label="Settings">
      <label>
        Recording mode
        <select
          value={mode}
          onChange={(event) => onModeChange(event.target.value as RecordingMode)}
        >
          <option value="press_and_hold">Press and hold</option>
          <option value="toggle">Toggle</option>
          <option value="floating_button">Floating button</option>
        </select>
      </label>
      <p>Fallback policy: {fallbackPolicy}</p>
    </section>
  );
}
```

Create `apps/desktop/src/features/settings/SettingsPanel.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";

describe("SettingsPanel", () => {
  it("changes recording mode", async () => {
    const user = userEvent.setup();
    const onModeChange = vi.fn();

    render(
      <SettingsPanel
        mode="toggle"
        fallbackPolicy="prefer_local_ask_before_cloud"
        onModeChange={onModeChange}
      />,
    );

    await user.selectOptions(screen.getByLabelText("Recording mode"), "press_and_hold");
    expect(onModeChange).toHaveBeenCalledWith("press_and_hold");
  });
});
```

Set `apps/desktop/src/app/App.tsx` to:

```tsx
import { useEffect, useState } from "react";
import { FloatingRecorder } from "../features/recorder/FloatingRecorder";
import { SettingsPanel } from "../features/settings/SettingsPanel";
import {
  cancelRecording,
  fallbackPolicyLabel,
  recordingStatus,
  startRecording,
  stopRecording,
} from "../lib/tauriApi";
import type { RecordingMode } from "../types/pipeline";

type RecordingStatus = "idle" | "recording";

export function App() {
  const [status, setStatus] = useState<RecordingStatus>("idle");
  const [mode, setMode] = useState<RecordingMode>("toggle");
  const [fallbackPolicy, setFallbackPolicy] = useState("prefer_local_ask_before_cloud");

  useEffect(() => {
    void recordingStatus().then(setStatus).catch(() => setStatus("idle"));
    void fallbackPolicyLabel().then(setFallbackPolicy).catch(() => {
      setFallbackPolicy("prefer_local_ask_before_cloud");
    });
  }, []);

  return (
    <main className="app-shell">
      <FloatingRecorder
        status={status}
        mode={mode}
        onStart={(nextMode) => {
          void startRecording(nextMode).then(() => setStatus("recording"));
        }}
        onStop={(reason) => {
          void stopRecording(reason).then(() => setStatus("idle"));
        }}
        onCancel={(reason) => {
          void cancelRecording(reason).then(() => setStatus("idle"));
        }}
      />
      <SettingsPanel
        mode={mode}
        fallbackPolicy={fallbackPolicy}
        onModeChange={setMode}
      />
    </main>
  );
}
```

Create `apps/desktop/src/app/App.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";

vi.mock("../lib/tauriApi", () => ({
  cancelRecording: vi.fn().mockResolvedValue(undefined),
  fallbackPolicyLabel: vi.fn().mockResolvedValue("prefer_local_ask_before_cloud"),
  recordingStatus: vi.fn().mockResolvedValue("idle"),
  startRecording: vi.fn().mockResolvedValue(undefined),
  stopRecording: vi.fn().mockResolvedValue(undefined),
}));

describe("App", () => {
  it("renders recorder and settings surfaces", async () => {
    render(<App />);

    expect(screen.getByRole("region", { name: "Recorder" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Settings" })).toBeInTheDocument();
  });
});
```

- [ ] **Step 5: Add trigger manager skeleton**

Update `apps/desktop/src-tauri/Cargo.toml` dependencies:

```toml
tauri-plugin-global-shortcut = "2"
```

Create `apps/desktop/src-tauri/src/trigger/mod.rs`:

```rust
pub mod manager;
```

Create `apps/desktop/src-tauri/src/trigger/manager.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent {
    PressAndHoldStart,
    PressAndHoldStop,
    Toggle,
}

#[derive(Debug, Default)]
pub struct TriggerManager {
    toggle_active: bool,
}

impl TriggerManager {
    pub fn handle(&mut self, event: TriggerEvent) -> &'static str {
        match event {
            TriggerEvent::PressAndHoldStart => "start",
            TriggerEvent::PressAndHoldStop => "stop",
            TriggerEvent::Toggle => {
                self.toggle_active = !self.toggle_active;
                if self.toggle_active {
                    "start"
                } else {
                    "stop"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TriggerEvent, TriggerManager};

    #[test]
    fn toggle_alternates_start_and_stop() {
        let mut manager = TriggerManager::default();

        assert_eq!(manager.handle(TriggerEvent::Toggle), "start");
        assert_eq!(manager.handle(TriggerEvent::Toggle), "stop");
    }
}
```

Update `apps/desktop/src-tauri/src/lib.rs` to include the module and plugin:

```rust
mod commands;
mod insertion;
mod state;
mod trigger;

use commands::recording::{cancel_recording, recording_status, start_recording, stop_recording};
use commands::settings::fallback_policy_label;
use state::AppState;

#[tauri::command]
fn app_health() -> &'static str {
    "ok"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            app_health,
            start_recording,
            stop_recording,
            cancel_recording,
            recording_status,
            fallback_policy_label
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: Run UI and trigger tests**

Run:

```bash
pnpm --dir apps/desktop test -- --run
cargo test -p wispergo-desktop --lib trigger
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/package.json apps/desktop/src apps/desktop/src-tauri
git commit -m "feat: add recorder ui and trigger manager"
```

---

### Task 14: End-To-End Verification With Fake Providers

**Files:**
- Modify: `docs/superpowers/specs/2026-04-28-local-dictation-app-design.md`
- Modify: `docs/superpowers/plans/2026-04-28-local-dictation-app.md`

- [ ] **Step 1: Run full verification**

Run:

```bash
pnpm test
```

Expected: frontend tests and Rust workspace tests pass.

Run:

```bash
pnpm --dir apps/desktop tauri build
```

Expected: Tauri desktop app builds successfully for macOS.

- [ ] **Step 2: Check the app manually**

Run:

```bash
pnpm desktop:dev
```

Expected:

- The app opens.
- Floating recorder shows `Ready`.
- Clicking `Record` changes the status to `Recording`.
- Clicking `Stop` changes the status to `Ready`.
- Settings allow switching recording mode.
- Fallback policy displays `prefer_local_ask_before_cloud`.

- [ ] **Step 3: Record known limitations in the design spec**

Append this section to `docs/superpowers/specs/2026-04-28-local-dictation-app-design.md`:

```markdown
## Implementation Notes

The first implementation uses fake providers for the UI path and contract tests for local providers. Real microphone capture, packaged whisper.cpp binaries/models, deeper macOS context collection, and native paste automation are integration milestones after the foundation is verified.
```

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/specs/2026-04-28-local-dictation-app-design.md docs/superpowers/plans/2026-04-28-local-dictation-app.md
git commit -m "docs: record initial implementation verification notes"
```

---

## Final Verification Checklist

- [ ] `pnpm test` passes.
- [ ] `cargo test --workspace` passes.
- [ ] `pnpm --dir apps/desktop test -- --run` passes.
- [ ] `pnpm --dir apps/desktop tauri build` succeeds.
- [ ] No command path silently calls a cloud provider.
- [ ] Dictated audio is not persisted.
- [ ] Destructive commands require confirmation.
- [ ] Clipboard insertion leaves text available if native paste is unavailable.
- [ ] The app can be launched with `pnpm desktop:dev`.

## Known Risks To Watch During Implementation

- Tauri generated file names and package names can differ slightly by CLI version. Preserve generated metadata where it exists, but keep the interfaces in this plan.
- Global shortcut press/release behavior differs by platform; Task 13 only adds a tested manager skeleton and plugin wiring.
- The whisper sidecar adapter proves the contract but does not package a model or Metal-accelerated binary.
- Ollama is an external local service; tests must use `httpmock` and should not require a real Ollama daemon.
- macOS clipboard paste may need Accessibility permissions. The fallback requirement is that final text remains copied.
- Nearby text and selected text collection are privacy-sensitive and should stay behind the policy engine.
