# Language Toggle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Wisper Flow-inspired language selector for Auto, English, and Chinese that controls Whisper recognition language from both the floating UI and Settings.

**Architecture:** Keep the recorder pill status-only and create a separate interactive `language` Tauri window next to it. Persist recognition language in existing local model settings, expose focused Tauri commands for floating-language updates, and pass the selected language to the Whisper sidecar only when it is not Auto.

**Tech Stack:** Tauri v2, Rust, React 18, TypeScript, Vitest, Cargo tests, whisper.cpp `whisper-cli`.

---

## File Structure

- Modify `apps/desktop/src-tauri/src/state.rs`
  - Owns `RecognitionLanguage`, `LocalModelSettings`, normalization/defaults, and state tests.
- Modify `apps/desktop/src-tauri/src/commands/settings.rs`
  - Adds `recognition_language` and `set_recognition_language` commands and emits language-change events.
- Modify `apps/desktop/src-tauri/src/commands/recording.rs`
  - Maps persisted language to Whisper sidecar language arguments.
- Modify `apps/desktop/src-tauri/src/lib.rs`
  - Registers new commands, positions the new floating language window, and tests window config.
- Modify `apps/desktop/src-tauri/tauri.conf.json`
  - Adds a separate `language` floating window.
- Modify `apps/desktop/src-tauri/capabilities/default.json`
  - Grants capabilities to the new `language` window.
- Modify `crates/wispergo-core/src/whisper_sidecar.rs`
  - Adds optional `--language <code>` sidecar argument.
- Modify `crates/wispergo-core/src/ollama.rs`
  - Updates cleanup prompt to preserve the transcript language.
- Modify `crates/wispergo-core/tests/whisper_sidecar_tests.rs`
  - Verifies Whisper receives language args when configured.
- Modify `apps/desktop/src/types/pipeline.ts`
  - Adds frontend language union and extends local model settings.
- Modify `apps/desktop/src/lib/tauriApi.ts`
  - Adds recognition language APIs and language menu geometry API.
- Modify `apps/desktop/src/features/settings/SettingsPanel.tsx`
  - Adds Recognition language select.
- Modify `apps/desktop/src/features/settings/SettingsPanel.test.tsx`
  - Verifies settings save includes language.
- Create `apps/desktop/src/features/recorder/LanguageToggle.tsx`
  - Renders the floating language button, hover chevron, and popover menu.
- Create `apps/desktop/src/features/recorder/LanguageToggle.test.tsx`
  - Tests cycle and menu interactions.
- Modify `apps/desktop/src/features/recorder/FloatingRecorder.tsx`
  - Leave status pill behavior intact; no language behavior belongs here.
- Modify `apps/desktop/src/features/recorder/FloatingRecorder.test.tsx`
  - Keeps recorder pill status-only coverage.
- Modify `apps/desktop/src/app/App.tsx`
  - Adds `language` surface, loads/syncs recognition language, and wires floating control handlers.
- Modify `apps/desktop/src/app/App.test.tsx`
  - Mocks new APIs and verifies language surface behavior.
- Modify `apps/desktop/src/styles.css`
  - Adds styling for the language control surface, button, chevron, and popover.

---

## Task 1: Add RecognitionLanguage to Rust settings state

**Files:**
- Modify: `apps/desktop/src-tauri/src/state.rs`

- [ ] **Step 1: Write failing state tests**

Add these tests inside `#[cfg(test)] mod tests` in `apps/desktop/src-tauri/src/state.rs`:

```rust
use super::{AppState, RecognitionLanguage, RecordingSession, RecordingStatus};

#[test]
fn local_model_settings_default_to_auto_language() {
    let state = AppState::default();

    assert_eq!(
        state.local_model_settings().recognition_language,
        RecognitionLanguage::Auto
    );
}

#[test]
fn local_model_settings_language_round_trip() {
    let state = AppState::default();

    state.set_local_model_settings(super::LocalModelSettings {
        whisper_binary_path: Some("/opt/homebrew/bin/whisper-cli".to_string()),
        whisper_model_path: Some("/models/ggml-large-v3-turbo.bin".to_string()),
        recognition_language: RecognitionLanguage::Zh,
    });

    assert_eq!(
        state.local_model_settings(),
        super::LocalModelSettings {
            whisper_binary_path: Some("/opt/homebrew/bin/whisper-cli".to_string()),
            whisper_model_path: Some("/models/ggml-large-v3-turbo.bin".to_string()),
            recognition_language: RecognitionLanguage::Zh,
        }
    );
}

#[test]
fn invalid_recognition_language_deserializes_to_auto() {
    let settings: super::LocalModelSettings = serde_json::from_str(
        r#"{"whisperBinaryPath":"/bin/whisper-cli","whisperModelPath":"/models/model.bin","recognitionLanguage":"fr"}"#,
    )
    .expect("settings deserialize");

    assert_eq!(settings.recognition_language, RecognitionLanguage::Auto);
}

#[test]
fn recognition_language_maps_to_whisper_codes() {
    assert_eq!(RecognitionLanguage::Auto.whisper_code(), None);
    assert_eq!(RecognitionLanguage::En.whisper_code(), Some("en"));
    assert_eq!(RecognitionLanguage::Zh.whisper_code(), Some("zh"));
}
```

Also update the existing `local_model_settings_round_trip` expected value to include:

```rust
recognition_language: RecognitionLanguage::Auto,
```

- [ ] **Step 2: Run state tests and verify they fail**

Run:

```bash
cargo test -p wispergo-desktop local_model_settings --lib
```

Expected: FAIL because `RecognitionLanguage` and `recognition_language` do not exist yet.

- [ ] **Step 3: Implement RecognitionLanguage and extend LocalModelSettings**

At the top of `apps/desktop/src-tauri/src/state.rs`, keep existing imports and add the enum before `LocalModelSettings`:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RecognitionLanguage {
    #[default]
    Auto,
    En,
    Zh,
}

impl RecognitionLanguage {
    pub fn from_code(code: Option<&str>) -> Self {
        match code.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
            "en" => Self::En,
            "zh" => Self::Zh,
            _ => Self::Auto,
        }
    }

    pub fn whisper_code(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::En => Some("en"),
            Self::Zh => Some("zh"),
        }
    }
}

impl<'de> serde::Deserialize<'de> for RecognitionLanguage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Option::<String>::deserialize(deserializer)?;
        Ok(Self::from_code(value.as_deref()))
    }
}
```

Change `LocalModelSettings` to include the new field:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelSettings {
    pub whisper_binary_path: Option<String>,
    pub whisper_model_path: Option<String>,
    #[serde(default)]
    pub recognition_language: RecognitionLanguage,
}
```

Update `LocalModelSettings::normalized`:

```rust
pub fn normalized(self) -> Self {
    Self {
        whisper_binary_path: normalize_optional_path(self.whisper_binary_path),
        whisper_model_path: normalize_optional_path(self.whisper_model_path),
        recognition_language: self.recognition_language,
    }
}
```

Update `LocalModelSettings::to_frontend`:

```rust
pub fn to_frontend(&self) -> Self {
    Self {
        whisper_binary_path: Some(self.whisper_binary_path.clone().unwrap_or_default()),
        whisper_model_path: Some(self.whisper_model_path.clone().unwrap_or_default()),
        recognition_language: self.recognition_language,
    }
}
```

- [ ] **Step 4: Run state tests and verify they pass**

Run:

```bash
cargo test -p wispergo-desktop local_model_settings --lib
```

Expected: PASS for the local model settings tests.

- [ ] **Step 5: Commit state model changes**

```bash
git add apps/desktop/src-tauri/src/state.rs
git commit -m "feat: add recognition language setting"
```

---

## Task 2: Add Rust commands for recognition language persistence and sync

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/settings.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing command registration tests**

In `apps/desktop/src-tauri/src/lib.rs`, extend the existing test module with:

```rust
#[test]
fn app_registers_recognition_language_commands() {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
        .expect("lib source");

    assert!(source.contains("recognition_language"));
    assert!(source.contains("set_recognition_language"));
}
```

- [ ] **Step 2: Run the registration test and verify it fails**

Run:

```bash
cargo test -p wispergo-desktop app_registers_recognition_language_commands --lib
```

Expected: FAIL because commands are not imported or registered yet.

- [ ] **Step 3: Add commands in settings.rs**

Change the imports in `apps/desktop/src-tauri/src/commands/settings.rs`:

```rust
use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::{AppState, LocalModelSettings, RecognitionLanguage};
```

Add this constant near `SETTINGS_FILE_NAME`:

```rust
pub const RECOGNITION_LANGUAGE_CHANGED_EVENT: &str = "wispergo://recognition-language-changed";
```

Add these commands after `set_local_model_settings`:

```rust
#[tauri::command]
pub fn recognition_language(state: State<'_, AppState>) -> RecognitionLanguage {
    state.local_model_settings().recognition_language
}

#[tauri::command]
pub fn set_recognition_language(
    app: AppHandle,
    state: State<'_, AppState>,
    language: RecognitionLanguage,
) -> Result<RecognitionLanguage, String> {
    let mut settings = state.local_model_settings();
    settings.recognition_language = language;
    state.set_local_model_settings(settings.clone());
    save_persisted_settings(&app, &settings)?;
    app.emit(RECOGNITION_LANGUAGE_CHANGED_EVENT, language)
        .map_err(|err| err.to_string())?;
    Ok(language)
}
```

Update `set_local_model_settings` to emit after saving:

```rust
#[tauri::command]
pub fn set_local_model_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: LocalModelSettings,
) -> Result<LocalModelSettings, String> {
    let settings = settings.normalized();
    state.set_local_model_settings(settings.clone());
    save_persisted_settings(&app, &settings)?;
    app.emit(RECOGNITION_LANGUAGE_CHANGED_EVENT, settings.recognition_language)
        .map_err(|err| err.to_string())?;
    Ok(settings.to_frontend())
}
```

- [ ] **Step 4: Register commands in lib.rs**

Update the `commands::settings` import in `apps/desktop/src-tauri/src/lib.rs`:

```rust
use commands::settings::{
    accessibility_status, fallback_policy_label, list_microphones, load_persisted_settings,
    local_model_settings, microphone_status, recognition_language, request_accessibility,
    request_microphone_access, selected_microphone_id, set_local_model_settings,
    set_microphone_device, set_recognition_language,
};
```

Add the commands to `tauri::generate_handler![...]`:

```rust
recognition_language,
set_recognition_language
```

- [ ] **Step 5: Run registration test and verify it passes**

Run:

```bash
cargo test -p wispergo-desktop app_registers_recognition_language_commands --lib
```

Expected: PASS.

- [ ] **Step 6: Commit command changes**

```bash
git add apps/desktop/src-tauri/src/commands/settings.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat: add recognition language commands"
```

---

## Task 3: Pass selected language to Whisper and preserve transcript language in cleanup

**Files:**
- Modify: `crates/wispergo-core/src/whisper_sidecar.rs`
- Modify: `crates/wispergo-core/tests/whisper_sidecar_tests.rs`
- Modify: `apps/desktop/src-tauri/src/commands/recording.rs`
- Modify: `crates/wispergo-core/src/ollama.rs`

- [ ] **Step 1: Write failing Whisper sidecar language test**

Add this test to `crates/wispergo-core/tests/whisper_sidecar_tests.rs`:

```rust
#[tokio::test]
async fn sidecar_receives_configured_language_code() {
    let dir = tempdir().expect("tempdir");
    let script = dir.path().join("fake-whisper-language.sh");
    let marker = dir.path().join("args.txt");
    let model = dir.path().join("model.bin");
    fs::write(&model, "fake model").expect("write model");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\n\
             while [ \"$#\" -gt 0 ]; do\n\
             printf 'arg=%s\\n' \"$1\" >> \"{}\"\n\
             case \"$1\" in\n\
             --file|--model|--language)\n\
             shift\n\
             printf 'value=%s\\n' \"$1\" >> \"{}\"\n\
             ;;\n\
             esac\n\
             shift\n\
             done\n\
             printf '你好世界\\n'\n",
            marker.display(),
            marker.display()
        ),
    )
    .expect("write script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("chmod");
    }

    let provider = WhisperSidecarProvider::new(script, Some(model)).with_language(Some("zh".to_string()));
    let output = provider
        .transcribe(vec![0.1, 0.2])
        .await
        .expect("transcribe");

    assert_eq!(output.transcript, "你好世界");
    let args = fs::read_to_string(&marker).expect("read sidecar marker");
    assert!(args.contains("arg=--language"));
    assert!(args.contains("value=zh"));
}
```

- [ ] **Step 2: Run the sidecar language test and verify it fails**

Run:

```bash
cargo test -p wispergo-core sidecar_receives_configured_language_code
```

Expected: FAIL because `with_language` does not exist.

- [ ] **Step 3: Implement optional language args in WhisperSidecarProvider**

Update `WhisperSidecarProvider` in `crates/wispergo-core/src/whisper_sidecar.rs`:

```rust
#[derive(Debug, Clone)]
pub struct WhisperSidecarProvider {
    binary_path: PathBuf,
    model_path: Option<PathBuf>,
    language_code: Option<String>,
    timeout: Duration,
}
```

Update `new`:

```rust
pub fn new(binary_path: PathBuf, model_path: Option<PathBuf>) -> Self {
    Self {
        binary_path,
        model_path,
        language_code: None,
        timeout: DEFAULT_TIMEOUT,
    }
}
```

Add this builder method:

```rust
pub fn with_language(mut self, language_code: Option<String>) -> Self {
    self.language_code = language_code.and_then(|code| {
        let code = code.trim().to_string();
        if code.is_empty() {
            None
        } else {
            Some(code)
        }
    });
    self
}
```

In `transcribe`, after the `--model` block and before `--no-timestamps`, add:

```rust
if let Some(language_code) = &self.language_code {
    command.arg("--language").arg(language_code);
}
```

- [ ] **Step 4: Wire desktop recording settings to the provider**

In `apps/desktop/src-tauri/src/commands/recording.rs`, update `local_asr_provider`:

```rust
fn local_asr_provider(settings: &LocalModelSettings) -> Result<WhisperSidecarProvider, String> {
    let paths = resolve_asr_paths(settings)?;

    Ok(WhisperSidecarProvider::new(paths.binary_path, Some(paths.model_path))
        .with_language(settings.recognition_language.whisper_code().map(str::to_string))
        .with_timeout(Duration::from_secs(30)))
}
```

Update the existing `configured_asr_paths_take_precedence` test's `LocalModelSettings` literal to include:

```rust
recognition_language: crate::state::RecognitionLanguage::Auto,
```

Add a focused test near it:

```rust
#[test]
fn chinese_language_maps_to_whisper_code() {
    assert_eq!(crate::state::RecognitionLanguage::Zh.whisper_code(), Some("zh"));
}
```

- [ ] **Step 5: Update cleanup prompt to preserve language**

In `crates/wispergo-core/src/ollama.rs`, change `cleanup_system_prompt()` to:

```rust
fn cleanup_system_prompt() -> String {
    "Return only JSON matching the CleanupOutput schema. Do not execute commands. Classify user intent into insert_text, command, cancelled, or error results. Preserve the transcript's original language and script; do not translate between languages.".to_string()
}
```

- [ ] **Step 6: Run Whisper and desktop recording tests**

Run:

```bash
cargo test -p wispergo-core sidecar_receives_configured_language_code
cargo test -p wispergo-desktop chinese_language_maps_to_whisper_code --lib
```

Expected: both PASS.

- [ ] **Step 7: Commit Whisper language changes**

```bash
git add crates/wispergo-core/src/whisper_sidecar.rs crates/wispergo-core/tests/whisper_sidecar_tests.rs apps/desktop/src-tauri/src/commands/recording.rs crates/wispergo-core/src/ollama.rs
git commit -m "feat: pass recognition language to whisper"
```

---

## Task 4: Add frontend types, API methods, and Settings language select

**Files:**
- Modify: `apps/desktop/src/types/pipeline.ts`
- Modify: `apps/desktop/src/lib/tauriApi.ts`
- Modify: `apps/desktop/src/features/settings/SettingsPanel.tsx`
- Modify: `apps/desktop/src/features/settings/SettingsPanel.test.tsx`
- Modify: `apps/desktop/src/app/App.test.tsx`

- [ ] **Step 1: Write failing SettingsPanel language test**

Refactor the top of `apps/desktop/src/features/settings/SettingsPanel.test.tsx` to add a render helper:

```ts
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";

type SettingsPanelProps = Parameters<typeof SettingsPanel>[0];

function renderSettingsPanel(overrides: Partial<SettingsPanelProps> = {}) {
  const props: SettingsPanelProps = {
    fallbackPolicy: "prefer_local_ask_before_cloud",
    microphones: [],
    selectedMicrophoneId: null,
    microphone: { granted: true, canPrompt: true },
    accessibility: { granted: true, canPrompt: true },
    modelSettings: {
      whisperBinaryPath: "",
      whisperModelPath: "",
      recognitionLanguage: "auto",
    },
    requestingPermission: null,
    onMicrophoneChange: vi.fn(),
    onRefreshMicrophones: vi.fn(),
    onRefreshAccessibility: vi.fn(),
    onRequestMicrophoneAccess: vi.fn(),
    onRequestAccessibility: vi.fn(),
    onModelSettingsSave: vi.fn(),
    ...overrides,
  };

  return {
    ...render(<SettingsPanel {...props} />),
    props,
  };
}
```

Add this test:

```ts
it("saves recognition language with local model settings", async () => {
  const user = userEvent.setup();
  const onModelSettingsSave = vi.fn();
  renderSettingsPanel({ onModelSettingsSave });

  await user.selectOptions(screen.getByLabelText("Recognition language"), "zh");
  await user.click(screen.getByRole("button", { name: "Save model settings" }));

  expect(onModelSettingsSave).toHaveBeenCalledWith({
    whisperBinaryPath: "",
    whisperModelPath: "",
    recognitionLanguage: "zh",
  });
});
```

- [ ] **Step 2: Run SettingsPanel tests and verify they fail**

Run:

```bash
pnpm --dir apps/desktop test -- src/features/settings/SettingsPanel.test.tsx
```

Expected: FAIL because `recognitionLanguage` is not in the type/UI.

- [ ] **Step 3: Add frontend recognition language type**

In `apps/desktop/src/types/pipeline.ts`, add:

```ts
export type RecognitionLanguage = "auto" | "en" | "zh";
```

Change `LocalModelSettings` to:

```ts
export type LocalModelSettings = {
  whisperBinaryPath: string;
  whisperModelPath: string;
  recognitionLanguage: RecognitionLanguage;
};
```

- [ ] **Step 4: Add Tauri API methods**

In `apps/desktop/src/lib/tauriApi.ts`, import `RecognitionLanguage` and add:

```ts
export async function recognitionLanguage(): Promise<RecognitionLanguage> {
  return invoke<RecognitionLanguage>("recognition_language");
}

export async function setRecognitionLanguage(
  language: RecognitionLanguage,
): Promise<RecognitionLanguage> {
  return invoke<RecognitionLanguage>("set_recognition_language", { language });
}

export async function setLanguageMenuOpen(open: boolean): Promise<void> {
  await invoke("set_language_menu_open", { open });
}
```

- [ ] **Step 5: Add Recognition language select to SettingsPanel**

In `SettingsPanel.tsx`, add this label inside `.model-settings`, after Whisper model path and before the Save button:

```tsx
<label>
  Recognition language
  <select
    value={draftModelSettings.recognitionLanguage}
    onChange={(event) =>
      setDraftModelSettings((current) => ({
        ...current,
        recognitionLanguage: event.target.value as LocalModelSettings["recognitionLanguage"],
      }))
    }
  >
    <option value="auto">Auto</option>
    <option value="en">English</option>
    <option value="zh">Chinese</option>
  </select>
</label>
```

- [ ] **Step 6: Update existing frontend test model settings literals**

Every `modelSettings={{ whisperBinaryPath: "", whisperModelPath: "" }}` literal in `SettingsPanel.test.tsx` and `App.test.tsx` must become:

```ts
modelSettings={{ whisperBinaryPath: "", whisperModelPath: "", recognitionLanguage: "auto" }}
```

Every mocked `localModelSettings` response in `App.test.tsx` must include:

```ts
recognitionLanguage: "auto",
```

- [ ] **Step 7: Run SettingsPanel tests and verify they pass**

Run:

```bash
pnpm --dir apps/desktop test -- src/features/settings/SettingsPanel.test.tsx
```

Expected: PASS.

- [ ] **Step 8: Commit frontend settings changes**

```bash
git add apps/desktop/src/types/pipeline.ts apps/desktop/src/lib/tauriApi.ts apps/desktop/src/features/settings/SettingsPanel.tsx apps/desktop/src/features/settings/SettingsPanel.test.tsx apps/desktop/src/app/App.test.tsx
git commit -m "feat: add recognition language settings UI"
```

---

## Task 5: Create floating LanguageToggle component

**Files:**
- Create: `apps/desktop/src/features/recorder/LanguageToggle.tsx`
- Create: `apps/desktop/src/features/recorder/LanguageToggle.test.tsx`
- Modify: `apps/desktop/src/styles.css`

- [ ] **Step 1: Write failing LanguageToggle tests**

Create `apps/desktop/src/features/recorder/LanguageToggle.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { LanguageToggle } from "./LanguageToggle";

const languages = [
  { value: "auto", label: "Auto" },
  { value: "en", label: "English" },
  { value: "zh", label: "Chinese" },
] as const;

describe("LanguageToggle", () => {
  it("shows a globe for automatic language detection", () => {
    render(
      <LanguageToggle
        language="auto"
        languages={languages}
        menuOpen={false}
        onCycle={vi.fn()}
        onSelect={vi.fn()}
        onMenuOpenChange={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Recognition language: Auto" })).toHaveTextContent("🌐");
  });

  it("shows two-letter language codes for explicit languages", () => {
    render(
      <LanguageToggle
        language="zh"
        languages={languages}
        menuOpen={false}
        onCycle={vi.fn()}
        onSelect={vi.fn()}
        onMenuOpenChange={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Recognition language: Chinese" })).toHaveTextContent("ZH");
  });

  it("cycles when the primary language button is clicked", async () => {
    const user = userEvent.setup();
    const onCycle = vi.fn();
    render(
      <LanguageToggle
        language="auto"
        languages={languages}
        menuOpen={false}
        onCycle={onCycle}
        onSelect={vi.fn()}
        onMenuOpenChange={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Recognition language: Auto" }));

    expect(onCycle).toHaveBeenCalled();
  });

  it("opens menu from chevron and selects a single language", async () => {
    const user = userEvent.setup();
    const onMenuOpenChange = vi.fn();
    const onSelect = vi.fn();
    render(
      <LanguageToggle
        language="en"
        languages={languages}
        menuOpen
        onCycle={vi.fn()}
        onSelect={onSelect}
        onMenuOpenChange={onMenuOpenChange}
      />,
    );

    expect(screen.getByRole("menuitemradio", { name: "English" })).toHaveAttribute("aria-checked", "true");
    await user.click(screen.getByRole("menuitemradio", { name: "Chinese" }));

    expect(onSelect).toHaveBeenCalledWith("zh");
  });
});
```

- [ ] **Step 2: Run LanguageToggle tests and verify they fail**

Run:

```bash
pnpm --dir apps/desktop test -- src/features/recorder/LanguageToggle.test.tsx
```

Expected: FAIL because `LanguageToggle.tsx` does not exist.

- [ ] **Step 3: Implement LanguageToggle component**

Create `apps/desktop/src/features/recorder/LanguageToggle.tsx`:

```tsx
import type { RecognitionLanguage } from "../../types/pipeline";

type LanguageOption = {
  value: RecognitionLanguage;
  label: string;
};

type Props = {
  language: RecognitionLanguage;
  languages: readonly LanguageOption[];
  menuOpen: boolean;
  onCycle: () => void;
  onSelect: (language: RecognitionLanguage) => void;
  onMenuOpenChange: (open: boolean) => void;
};

export function LanguageToggle({
  language,
  languages,
  menuOpen,
  onCycle,
  onSelect,
  onMenuOpenChange,
}: Props) {
  const current = languages.find((option) => option.value === language) ?? languages[0];

  return (
    <div className={menuOpen ? "language-toggle is-open" : "language-toggle"}>
      {menuOpen ? (
        <div className="language-menu" role="menu" aria-label="Recognition language">
          {languages.map((option) => {
            const selected = option.value === language;
            return (
              <button
                key={option.value}
                type="button"
                role="menuitemradio"
                aria-checked={selected}
                className="language-menu-item"
                onClick={() => onSelect(option.value)}
              >
                <span>{option.label}</span>
                {selected ? <span aria-hidden="true">✓</span> : null}
              </button>
            );
          })}
        </div>
      ) : null}
      <div className="language-toggle-bar">
        <button
          type="button"
          className="language-chevron"
          aria-label="Choose recognition language"
          aria-expanded={menuOpen}
          onClick={() => onMenuOpenChange(!menuOpen)}
        >
          ⌃
        </button>
        <button
          type="button"
          className="language-current"
          aria-label={`Recognition language: ${current.label}`}
          onClick={onCycle}
        >
          {languageIndicator(language)}
        </button>
      </div>
    </div>
  );
}

function languageIndicator(language: RecognitionLanguage) {
  if (language === "auto") {
    return "🌐";
  }
  return language.toUpperCase();
}
```

- [ ] **Step 4: Add CSS for the floating language control**

Append these rules to `apps/desktop/src/styles.css` before the media query:

```css
.language-surface {
  width: 100vw;
  min-height: 100vh;
  margin: 0;
  padding: 0;
  display: grid;
  align-content: end;
  justify-content: end;
  overflow: hidden;
}

.language-toggle {
  display: grid;
  justify-items: end;
  gap: 6px;
  color: #ffffff;
}

.language-toggle-bar {
  display: inline-flex;
  align-items: center;
  height: 40px;
  border: 1px solid rgb(255 255 255 / 16%);
  border-radius: 999px;
  background: #05070a;
  overflow: hidden;
}

.language-current,
.language-chevron,
.language-menu-item {
  border: 0;
  background: transparent;
  color: inherit;
}

.language-current {
  width: 40px;
  height: 40px;
  min-height: 40px;
  padding: 0;
  font-size: 0.76rem;
  font-weight: 700;
}

.language-chevron {
  width: 0;
  min-height: 40px;
  padding: 0;
  opacity: 0;
  overflow: hidden;
  color: #c7d0dc;
  transition:
    width 120ms ease,
    opacity 120ms ease;
}

.language-toggle:hover .language-chevron,
.language-toggle:focus-within .language-chevron,
.language-toggle.is-open .language-chevron {
  width: 32px;
  opacity: 1;
}

.language-menu {
  min-width: 220px;
  padding: 8px;
  border: 1px solid rgb(0 0 0 / 12%);
  border-radius: 10px;
  background: #ffffff;
  color: #18202a;
  box-shadow: 0 18px 48px rgb(0 0 0 / 22%);
}

.language-menu-item {
  width: 100%;
  min-height: 34px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 0 8px;
  border-radius: 6px;
  color: #18202a;
  text-align: left;
}

.language-menu-item:hover {
  background: #f1f4f7;
}
```

- [ ] **Step 5: Run LanguageToggle tests and verify they pass**

Run:

```bash
pnpm --dir apps/desktop test -- src/features/recorder/LanguageToggle.test.tsx
```

Expected: PASS.

- [ ] **Step 6: Commit LanguageToggle component**

```bash
git add apps/desktop/src/features/recorder/LanguageToggle.tsx apps/desktop/src/features/recorder/LanguageToggle.test.tsx apps/desktop/src/styles.css
git commit -m "feat: add floating language toggle component"
```

---

## Task 6: Add separate Tauri language window and positioning

**Files:**
- Modify: `apps/desktop/src-tauri/tauri.conf.json`
- Modify: `apps/desktop/src-tauri/capabilities/default.json`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing window config tests**

Add these tests to `apps/desktop/src-tauri/src/lib.rs` test module:

```rust
#[test]
fn language_window_is_configured_as_separate_interactive_surface() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config = fs::read_to_string(manifest_dir.join("tauri.conf.json")).expect("tauri config");
    let config: Value = serde_json::from_str(&config).expect("valid tauri config json");
    let language = config["app"]["windows"]
        .as_array()
        .expect("windows array")
        .iter()
        .find(|window| window["label"].as_str() == Some("language"))
        .expect("language window configured");

    assert_eq!(language["url"].as_str(), Some("/?surface=language"));
    assert_eq!(language["transparent"].as_bool(), Some(true));
    assert_eq!(language["backgroundColor"].as_str(), Some("#00000000"));
    assert_eq!(language["decorations"].as_bool(), Some(false));
    assert_eq!(language["alwaysOnTop"].as_bool(), Some(true));
    assert_eq!(language["focus"].as_bool(), Some(false));
    assert_eq!(language["focusable"].as_bool(), Some(false));
}

#[test]
fn default_capability_includes_language_window() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let capability = fs::read_to_string(manifest_dir.join("capabilities/default.json"))
        .expect("default capability");

    assert!(capability.contains("\"language\""));
}
```

- [ ] **Step 2: Run window tests and verify they fail**

Run:

```bash
cargo test -p wispergo-desktop language_window_is_configured_as_separate_interactive_surface --lib
cargo test -p wispergo-desktop default_capability_includes_language_window --lib
```

Expected: both commands FAIL because the `language` window is not configured.

- [ ] **Step 3: Add language window to tauri.conf.json**

Add this window object after the existing `recorder` window in `apps/desktop/src-tauri/tauri.conf.json`:

```json
{
  "label": "language",
  "title": "Wispergo Language",
  "url": "/?surface=language",
  "width": 74,
  "height": 52,
  "decorations": false,
  "alwaysOnTop": true,
  "transparent": true,
  "backgroundColor": "#00000000",
  "shadow": false,
  "resizable": false,
  "skipTaskbar": true,
  "focus": false,
  "focusable": false,
  "visible": true,
  "visibleOnAllWorkspaces": true,
  "acceptFirstMouse": true
}
```

- [ ] **Step 4: Add language window to capabilities**

Change `apps/desktop/src-tauri/capabilities/default.json` windows to:

```json
"windows": ["main", "recorder", "language"]
```

- [ ] **Step 5: Implement positioning and menu resize command**

In `apps/desktop/src-tauri/src/lib.rs`, add this command function before `run()`:

```rust
#[tauri::command]
fn set_language_menu_open(app: tauri::AppHandle, open: bool) -> Result<(), String> {
    position_language_window(&app, open).map_err(|err| err.to_string())
}
```

Add `set_language_menu_open` to `tauri::generate_handler![...]`:

```rust
set_language_menu_open
```

Add constants near the helper functions:

```rust
const FLOATING_BOTTOM_MARGIN: i32 = 88;
const FLOATING_GAP: i32 = 8;
const RECORDER_WINDOW_WIDTH: u32 = 320;
const LANGUAGE_CLOSED_WIDTH: u32 = 74;
const LANGUAGE_CLOSED_HEIGHT: u32 = 52;
const LANGUAGE_OPEN_WIDTH: u32 = 260;
const LANGUAGE_OPEN_HEIGHT: u32 = 190;
```

Replace the `position_recorder_window(app.handle());` call in setup with:

```rust
position_recorder_window(app.handle());
position_language_window(app.handle(), false)?;
```

Replace the `y` calculation inside `position_recorder_window` with:

```rust
let y = monitor_position.y
    + monitor_size.height as i32
    - window_size.height as i32
    - FLOATING_BOTTOM_MARGIN;
```

Add this function below `position_recorder_window`:

```rust
fn position_language_window(app: &tauri::AppHandle, open: bool) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("language") else {
        return Ok(());
    };
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(());
    };

    let (width, height) = if open {
        (LANGUAGE_OPEN_WIDTH, LANGUAGE_OPEN_HEIGHT)
    } else {
        (LANGUAGE_CLOSED_WIDTH, LANGUAGE_CLOSED_HEIGHT)
    };

    window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(width, height)))?;

    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let recorder_x = monitor_position.x + (monitor_size.width as i32 - RECORDER_WINDOW_WIDTH as i32) / 2;
    let x = recorder_x - FLOATING_GAP - width as i32;
    let y = monitor_position.y + monitor_size.height as i32 - height as i32 - FLOATING_BOTTOM_MARGIN;

    window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(x, y)))?;
    Ok(())
}
```

- [ ] **Step 6: Run window tests and verify they pass**

Run:

```bash
cargo test -p wispergo-desktop language_window_is_configured_as_separate_interactive_surface --lib
cargo test -p wispergo-desktop default_capability_includes_language_window --lib
```

Expected: PASS.

- [ ] **Step 7: Commit Tauri window changes**

```bash
git add apps/desktop/src-tauri/tauri.conf.json apps/desktop/src-tauri/capabilities/default.json apps/desktop/src-tauri/src/lib.rs
git commit -m "feat: add floating language window"
```

---

## Task 7: Integrate language surface in App

**Files:**
- Modify: `apps/desktop/src/app/App.tsx`
- Modify: `apps/desktop/src/app/App.test.tsx`

- [ ] **Step 1: Write failing App language surface tests**

Update the `../lib/tauriApi` mock in `App.test.tsx` to include functions before writing tests:

```ts
recognitionLanguage: vi.fn().mockResolvedValue("auto"),
setRecognitionLanguage: vi.fn().mockResolvedValue("en"),
setLanguageMenuOpen: vi.fn().mockResolvedValue(undefined),
```

Import them at the top:

```ts
recognitionLanguage,
setLanguageMenuOpen,
setRecognitionLanguage,
```

Add resets/defaults in `beforeEach`:

```ts
vi.mocked(recognitionLanguage).mockReset();
vi.mocked(setRecognitionLanguage).mockReset();
vi.mocked(setLanguageMenuOpen).mockReset();
vi.mocked(recognitionLanguage).mockResolvedValue("auto");
vi.mocked(setRecognitionLanguage).mockImplementation(async (language) => language);
vi.mocked(setLanguageMenuOpen).mockResolvedValue(undefined);
```

Add tests:

```ts
it("renders only language controls on the language surface", async () => {
  window.history.pushState({}, "", "/?surface=language");

  render(<App />);

  expect(await screen.findByRole("button", { name: "Recognition language: Auto" })).toBeInTheDocument();
  expect(screen.queryByRole("region", { name: "Recorder" })).not.toBeInTheDocument();
  expect(screen.queryByRole("region", { name: "Settings" })).not.toBeInTheDocument();
});

it("cycles recognition language from the language surface", async () => {
  const user = userEvent.setup();
  window.history.pushState({}, "", "/?surface=language");

  render(<App />);
  await user.click(await screen.findByRole("button", { name: "Recognition language: Auto" }));

  expect(setRecognitionLanguage).toHaveBeenCalledWith("en");
});

it("opens the language menu from the chevron", async () => {
  const user = userEvent.setup();
  window.history.pushState({}, "", "/?surface=language");

  render(<App />);
  await user.click(await screen.findByRole("button", { name: "Choose recognition language" }));

  expect(setLanguageMenuOpen).toHaveBeenCalledWith(true);
  expect(await screen.findByRole("menuitemradio", { name: "Auto ✓" })).toBeInTheDocument();
});
```

- [ ] **Step 2: Run App tests and verify they fail**

Run:

```bash
pnpm --dir apps/desktop test -- src/app/App.test.tsx
```

Expected: FAIL because `language` surface and handlers are not implemented.

- [ ] **Step 3: Implement App language state and helpers**

In `apps/desktop/src/app/App.tsx`, import `LanguageToggle` and new APIs:

```ts
import { LanguageToggle } from "../features/recorder/LanguageToggle";
```

Add API imports:

```ts
recognitionLanguage,
setLanguageMenuOpen,
setRecognitionLanguage,
```

Add type import:

```ts
RecognitionLanguage,
```

Add constants near refresh constants:

```ts
const RECOGNITION_LANGUAGES = [
  { value: "auto", label: "Auto" },
  { value: "en", label: "English" },
  { value: "zh", label: "Chinese" },
] as const;
```

Add state:

```ts
const [languageMenuOpen, setLanguageMenuOpenState] = useState(false);
```

Update initial `modelSettings`:

```ts
const [modelSettings, setModelSettings] = useState<LocalModelSettings>({
  whisperBinaryPath: "",
  whisperModelPath: "",
  recognitionLanguage: "auto",
});
```

Add helper functions before `return`:

```ts
function nextRecognitionLanguage(language: RecognitionLanguage): RecognitionLanguage {
  if (language === "auto") {
    return "en";
  }
  if (language === "en") {
    return "zh";
  }
  return "auto";
}

function applyRecognitionLanguage(language: RecognitionLanguage) {
  setModelSettings((current) => ({ ...current, recognitionLanguage: language }));
}

function updateRecognitionLanguage(language: RecognitionLanguage) {
  applyRecognitionLanguage(language);
  void setRecognitionLanguage(language).catch((err: unknown) => {
    setError(errorMessage(err));
  });
}

function updateLanguageMenuOpen(open: boolean) {
  setLanguageMenuOpenState(open);
  void setLanguageMenuOpen(open).catch((err: unknown) => {
    setError(errorMessage(err));
  });
}
```

- [ ] **Step 4: Load and sync recognition language**

Add an effect after the surface dataset effect:

```ts
useEffect(() => {
  let mounted = true;

  void recognitionLanguage()
    .then((language) => {
      if (mounted) {
        applyRecognitionLanguage(language);
      }
    })
    .catch(() => {
      if (mounted) {
        applyRecognitionLanguage("auto");
      }
    });

  const unlisten = listen<RecognitionLanguage>("wispergo://recognition-language-changed", (event) => {
    if (mounted) {
      applyRecognitionLanguage(event.payload);
    }
  });

  return () => {
    mounted = false;
    void unlisten.then((unsubscribe) => unsubscribe());
  };
}, []);
```

Update all fallback model settings in `App.tsx` to include `recognitionLanguage: "auto"`.

- [ ] **Step 5: Render language surface**

Change `appSurface()` return type and implementation:

```ts
function appSurface(): "settings" | "recorder" | "language" {
  const params = new URLSearchParams(window.location.search);
  const surface = params.get("surface");
  if (surface === "recorder" || surface === "language") {
    return surface;
  }
  return "settings";
}
```

Add:

```ts
const isLanguageSurface = surface === "language";
```

Change the `<main>` className expression to:

```tsx
<main
  className={
    isRecorderSurface
      ? "app-shell recorder-surface"
      : isLanguageSurface
        ? "app-shell language-surface"
        : "app-shell"
  }
>
```

Render the language control before the `lastInsert` block:

```tsx
{isLanguageSurface ? (
  <LanguageToggle
    language={modelSettings.recognitionLanguage}
    languages={RECOGNITION_LANGUAGES}
    menuOpen={languageMenuOpen}
    onCycle={() => updateRecognitionLanguage(nextRecognitionLanguage(modelSettings.recognitionLanguage))}
    onSelect={(language) => {
      updateRecognitionLanguage(language);
      updateLanguageMenuOpen(false);
    }}
    onMenuOpenChange={updateLanguageMenuOpen}
  />
) : null}
```

Change `!isRecorderSurface ? (` for settings to:

```tsx
{!isRecorderSurface && !isLanguageSurface ? (
```

Update `onModelSettingsSave` success handler:

```ts
.then((settings) => {
  setModelSettings(settings);
})
```

- [ ] **Step 6: Run App tests and verify they pass**

Run:

```bash
pnpm --dir apps/desktop test -- src/app/App.test.tsx
```

Expected: PASS.

- [ ] **Step 7: Commit App integration**

```bash
git add apps/desktop/src/app/App.tsx apps/desktop/src/app/App.test.tsx
git commit -m "feat: wire language toggle surface"
```

---

## Task 8: Full verification and build

**Files:**
- No new files.

- [ ] **Step 1: Run all frontend tests**

Run:

```bash
pnpm --dir apps/desktop test
```

Expected: all Vitest suites PASS.

- [ ] **Step 2: Run all Rust tests**

Run:

```bash
cargo test --workspace
```

Expected: all Rust tests PASS.

- [ ] **Step 3: Run production frontend build**

Run:

```bash
pnpm --dir apps/desktop build
```

Expected: TypeScript and Vite build PASS.

- [ ] **Step 4: Run packaged desktop build**

Run:

```bash
pnpm desktop:build
```

Expected: Tauri build PASS and output includes:

```text
Finished 1 bundle at:
    /Users/shiang/projects/ribbons-digital/wispergo/target/release/bundle/macos/Wispergo.app
```

- [ ] **Step 5: Manual validation checklist**

Launch:

```bash
open target/release/bundle/macos/Wispergo.app
```

Validate:

- Recorder pill remains status-only.
- Separate language button appears next to the recorder pill.
- Auto shows globe icon.
- Clicking globe changes to `EN`.
- Clicking `EN` changes to `ZH`.
- Clicking `ZH` changes to globe/Auto.
- Hover reveals the chevron.
- Clicking chevron opens menu above the button.
- Selecting Auto, English, or Chinese updates the indicator and closes the menu.
- Settings window shows Recognition language.
- Saving Settings persists the selected language.
- Chinese mode invokes Whisper with `--language zh` according to the sidecar test.

- [ ] **Step 6: Stop if verification exposes a defect**

If any verification command or manual validation item fails, return to the task that introduced that behavior, add or update the failing test there, and repeat that task's red-green verification before running Task 8 again.

---

## Self-Review

- Spec coverage:
  - Default Auto: Task 1 and Task 4.
  - Settings language config: Task 4.
  - Floating globe/EN/ZH control: Task 5 and Task 7.
  - Click to cycle Auto -> EN -> ZH -> Auto: Task 7.
  - Hover chevron and popover: Task 5.
  - Single-select menu: Task 5.
  - Whisper `--language en` / `--language zh`: Task 3.
  - Preserve original language during cleanup: Task 3.
  - Recorder pill remains status-only: Task 5 and Task 7.
  - Separate clickable control: Task 6.
- Placeholder scan: no TBD, TODO, or unspecified implementation steps remain.
- Type consistency:
  - Rust uses `RecognitionLanguage::{Auto, En, Zh}`.
  - TypeScript uses `RecognitionLanguage = "auto" | "en" | "zh"`.
  - Serialized settings use `recognitionLanguage`.
  - Tauri commands use `recognition_language`, `set_recognition_language`, and `set_language_menu_open`.
