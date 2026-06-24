# R5.1 Shortcut Combo Customization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add user-configurable key-combination dictation shortcuts while preserving `Command + Shift + Space` as the default and keeping failed/conflicting saves from disabling the previous working shortcut.

**Architecture:** Introduce a focused Rust `shortcut` module for serializable combo settings, label generation, Tauri shortcut conversion, and conflict-safe apply/rollback logic behind a fakeable registry trait. Persist shortcut settings alongside existing local model settings, expose `shortcut_settings` and `set_shortcut_settings` Tauri commands, and pass the selected display label to Settings and the recorder UI. R5.2 single-modifier hold is intentionally not implemented here.

**Tech Stack:** Rust/Tauri v2, `tauri-plugin-global-shortcut`, React/TypeScript, Vitest, existing `settings.json` app-support persistence.

---

## Scope

Implement only the R5.1 slice from `docs/superpowers/specs/2026-06-24-r5-shortcut-customization-design.md`:

- Key-combination customization.
- Dynamic combo registration and conflict-safe rollback.
- Settings UI for recording/saving a combo.
- Selected shortcut label in Settings hero and recorder hint.

Do **not** implement:

- Single modifier-key hold.
- Fn/right-vs-left modifier monitoring.
- Arbitrary single-key hold.
- ASR/Pipecat/engine changes.

## File map

Create:

- `apps/desktop/src-tauri/src/shortcut.rs`
  - Owns `ShortcutSettings`, `ShortcutCombo`, `ShortcutModifiers`, `ShortcutKey`, display labels, default normalization, conversion to `tauri_plugin_global_shortcut::Shortcut`, and a fakeable registry/apply helper.

Modify:

- `apps/desktop/src-tauri/src/lib.rs`
  - Register the global-shortcut plugin without a hardcoded shortcut, manage the active shortcut state, call dynamic setup during app setup, expose shortcut commands.

- `apps/desktop/src-tauri/src/commands/settings.rs`
  - Persist `shortcut` beside `localModel` in the existing settings file, load it on startup, expose `shortcut_settings` and `set_shortcut_settings` commands.

- `apps/desktop/src-tauri/src/state.rs`
  - Store shortcut settings in `AppState` with getters/setters.

- `apps/desktop/src/types/pipeline.ts`
  - Add TypeScript `ShortcutSettings`, `ShortcutCombo`, `ShortcutModifiers`, `ShortcutKey` types.

- `apps/desktop/src/lib/tauriApi.ts`
  - Add `shortcutSettings()` and `setShortcutSettings(settings)` wrappers.

- `apps/desktop/src/app/App.tsx`
  - Load shortcut settings, pass display label into Settings and recorder, update label after save.

- `apps/desktop/src/features/recorder/FloatingRecorder.tsx`
  - Replace hardcoded hint with `shortcutLabel` prop.

- `apps/desktop/src/features/recorder/FloatingRecorder.test.tsx`
  - Update tests for default and custom label.

- `apps/desktop/src/features/settings/SettingsPanel.tsx`
  - Add compact Shortcut card/control for key-combination mode.

- `apps/desktop/src/features/settings/SettingsPanel.test.tsx`
  - Add tests for label rendering, shortcut capture/save, and conflict error.

- `apps/desktop/src/app/App.test.tsx`
  - Mock new Tauri API calls and verify the selected label flows to recorder/settings.

- `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
  - Mark R5.1 implemented after code verification, not before.

- `HANDOFF.md`
  - Record R5.1 implementation status and next gate after PR.

---

## Implementation tasks

### Task 1: Add Rust shortcut settings model and conversion tests

**Files:**
- Create: `apps/desktop/src-tauri/src/shortcut.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Create `shortcut.rs` with model tests first**

Create `apps/desktop/src-tauri/src/shortcut.rs` with tests and minimal type stubs. Start with tests that describe the public contract.

```rust
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutSettings {
    #[serde(default)]
    pub mode: ShortcutMode,
    #[serde(default)]
    pub combo: ShortcutCombo,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutMode {
    #[default]
    Combo,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutCombo {
    #[serde(default)]
    pub modifiers: ShortcutModifiers,
    #[serde(default)]
    pub key: ShortcutKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutModifiers {
    #[serde(default)]
    pub command: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub option: bool,
    #[serde(default)]
    pub control: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutKey {
    #[default]
    Space,
    Enter,
    Escape,
    Tab,
    Backquote,
    Minus,
    Equal,
    BracketLeft,
    BracketRight,
    Backslash,
    Semicolon,
    Quote,
    Comma,
    Period,
    Slash,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutSettingsView {
    pub settings: ShortcutSettings,
    pub display_label: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shortcut_is_command_shift_space() {
        let settings = ShortcutSettings::default();

        assert_eq!(settings.combo.modifiers.command, true);
        assert_eq!(settings.combo.modifiers.shift, true);
        assert_eq!(settings.combo.modifiers.option, false);
        assert_eq!(settings.combo.modifiers.control, false);
        assert_eq!(settings.combo.key, ShortcutKey::Space);
        assert_eq!(settings.display_label(), "⌘ ⇧ Space");
    }

    #[test]
    fn missing_shortcut_fields_deserialize_to_default_combo() {
        let settings: ShortcutSettings = serde_json::from_str("{}").expect("deserialize");

        assert_eq!(settings, ShortcutSettings::default());
    }

    #[test]
    fn invalid_empty_modifier_combo_normalizes_to_default() {
        let settings = ShortcutSettings {
            mode: ShortcutMode::Combo,
            combo: ShortcutCombo {
                modifiers: ShortcutModifiers {
                    command: false,
                    shift: false,
                    option: false,
                    control: false,
                },
                key: ShortcutKey::KeyA,
            },
        };

        assert_eq!(settings.normalized(), ShortcutSettings::default());
    }

    #[test]
    fn custom_combo_labels_use_mac_symbols() {
        let settings = ShortcutSettings {
            mode: ShortcutMode::Combo,
            combo: ShortcutCombo {
                modifiers: ShortcutModifiers {
                    command: true,
                    shift: false,
                    option: true,
                    control: false,
                },
                key: ShortcutKey::KeyK,
            },
        };

        assert_eq!(settings.display_label(), "⌘ ⌥ K");
    }

    #[test]
    fn modifier_label_order_is_stable() {
        let settings = ShortcutSettings {
            mode: ShortcutMode::Combo,
            combo: ShortcutCombo {
                modifiers: ShortcutModifiers {
                    command: true,
                    shift: true,
                    option: true,
                    control: true,
                },
                key: ShortcutKey::KeyK,
            },
        };

        assert_eq!(settings.display_label(), "⌘ ⇧ ⌥ ⌃ K");
    }

    #[test]
    fn combo_converts_to_tauri_shortcut() {
        let shortcut = ShortcutSettings::default()
            .to_tauri_shortcut()
            .expect("shortcut");

        assert!(shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::Space));
    }

    #[test]
    fn frontend_view_includes_settings_and_label() {
        let view = ShortcutSettings::default().to_frontend();

        assert_eq!(view.settings, ShortcutSettings::default());
        assert_eq!(view.display_label, "⌘ ⇧ Space");
    }
}
```

- [ ] **Step 2: Run the new tests and verify they fail because implementations are missing**

Run:

```bash
cargo test -p wispergo-desktop shortcut::tests -- --nocapture
```

Expected: compile failures for missing `Default`, `display_label`, `normalized`, `to_tauri_shortcut`, and `to_frontend` implementations.

- [ ] **Step 3: Add `mod shortcut;` so tests compile in the crate**

Modify `apps/desktop/src-tauri/src/lib.rs` near the existing module declarations:

```rust
mod platform;
mod shortcut;
mod state;
```

- [ ] **Step 4: Implement defaults, normalization, labels, and conversion**

Replace the stubs in `shortcut.rs` with complete implementations:

```rust
impl Default for ShortcutModifiers {
    fn default() -> Self {
        Self {
            command: true,
            shift: true,
            option: false,
            control: false,
        }
    }
}

impl Default for ShortcutCombo {
    fn default() -> Self {
        Self {
            modifiers: ShortcutModifiers::default(),
            key: ShortcutKey::Space,
        }
    }
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            mode: ShortcutMode::Combo,
            combo: ShortcutCombo::default(),
        }
    }
}

impl ShortcutSettings {
    pub fn normalized(self) -> Self {
        match self.mode {
            ShortcutMode::Combo => {
                if !self.combo.modifiers.has_any() {
                    Self::default()
                } else {
                    self
                }
            }
        }
    }

    pub fn display_label(&self) -> String {
        self.combo.display_label()
    }

    pub fn to_frontend(&self) -> ShortcutSettingsView {
        let normalized = self.clone().normalized();
        ShortcutSettingsView {
            display_label: normalized.display_label(),
            settings: normalized,
        }
    }

    pub fn to_tauri_shortcut(&self) -> Result<Shortcut, String> {
        self.combo.to_tauri_shortcut()
    }
}

impl ShortcutModifiers {
    pub fn has_any(&self) -> bool {
        self.command || self.shift || self.option || self.control
    }

    fn to_tauri_modifiers(self) -> Modifiers {
        let mut modifiers = Modifiers::empty();
        if self.command {
            modifiers |= Modifiers::SUPER;
        }
        if self.shift {
            modifiers |= Modifiers::SHIFT;
        }
        if self.option {
            modifiers |= Modifiers::ALT;
        }
        if self.control {
            modifiers |= Modifiers::CONTROL;
        }
        modifiers
    }

    fn label_parts(self) -> Vec<&'static str> {
        let mut parts = Vec::new();
        if self.command {
            parts.push("⌘");
        }
        if self.shift {
            parts.push("⇧");
        }
        if self.option {
            parts.push("⌥");
        }
        if self.control {
            parts.push("⌃");
        }
        parts
    }
}

impl ShortcutCombo {
    pub fn display_label(&self) -> String {
        let mut parts = self.modifiers.label_parts();
        parts.push(self.key.label());
        parts.join(" ")
    }

    pub fn to_tauri_shortcut(&self) -> Result<Shortcut, String> {
        if !self.modifiers.has_any() {
            return Err("Choose at least one modifier key.".to_string());
        }
        Ok(Shortcut::new(
            Some(self.modifiers.to_tauri_modifiers()),
            self.key.to_code(),
        ))
    }
}
```

Add exhaustive `ShortcutKey` helpers:

```rust
impl ShortcutKey {
    fn label(self) -> &'static str {
        match self {
            Self::Space => "Space",
            Self::Enter => "Return",
            Self::Escape => "Esc",
            Self::Tab => "Tab",
            Self::Backquote => "`",
            Self::Minus => "-",
            Self::Equal => "=",
            Self::BracketLeft => "[",
            Self::BracketRight => "]",
            Self::Backslash => "\\",
            Self::Semicolon => ";",
            Self::Quote => "'",
            Self::Comma => ",",
            Self::Period => ".",
            Self::Slash => "/",
            Self::ArrowUp => "↑",
            Self::ArrowDown => "↓",
            Self::ArrowLeft => "←",
            Self::ArrowRight => "→",
            Self::Digit0 => "0",
            Self::Digit1 => "1",
            Self::Digit2 => "2",
            Self::Digit3 => "3",
            Self::Digit4 => "4",
            Self::Digit5 => "5",
            Self::Digit6 => "6",
            Self::Digit7 => "7",
            Self::Digit8 => "8",
            Self::Digit9 => "9",
            Self::KeyA => "A",
            Self::KeyB => "B",
            Self::KeyC => "C",
            Self::KeyD => "D",
            Self::KeyE => "E",
            Self::KeyF => "F",
            Self::KeyG => "G",
            Self::KeyH => "H",
            Self::KeyI => "I",
            Self::KeyJ => "J",
            Self::KeyK => "K",
            Self::KeyL => "L",
            Self::KeyM => "M",
            Self::KeyN => "N",
            Self::KeyO => "O",
            Self::KeyP => "P",
            Self::KeyQ => "Q",
            Self::KeyR => "R",
            Self::KeyS => "S",
            Self::KeyT => "T",
            Self::KeyU => "U",
            Self::KeyV => "V",
            Self::KeyW => "W",
            Self::KeyX => "X",
            Self::KeyY => "Y",
            Self::KeyZ => "Z",
        }
    }

    fn to_code(self) -> Code {
        match self {
            Self::Space => Code::Space,
            Self::Enter => Code::Enter,
            Self::Escape => Code::Escape,
            Self::Tab => Code::Tab,
            Self::Backquote => Code::Backquote,
            Self::Minus => Code::Minus,
            Self::Equal => Code::Equal,
            Self::BracketLeft => Code::BracketLeft,
            Self::BracketRight => Code::BracketRight,
            Self::Backslash => Code::Backslash,
            Self::Semicolon => Code::Semicolon,
            Self::Quote => Code::Quote,
            Self::Comma => Code::Comma,
            Self::Period => Code::Period,
            Self::Slash => Code::Slash,
            Self::ArrowUp => Code::ArrowUp,
            Self::ArrowDown => Code::ArrowDown,
            Self::ArrowLeft => Code::ArrowLeft,
            Self::ArrowRight => Code::ArrowRight,
            Self::Digit0 => Code::Digit0,
            Self::Digit1 => Code::Digit1,
            Self::Digit2 => Code::Digit2,
            Self::Digit3 => Code::Digit3,
            Self::Digit4 => Code::Digit4,
            Self::Digit5 => Code::Digit5,
            Self::Digit6 => Code::Digit6,
            Self::Digit7 => Code::Digit7,
            Self::Digit8 => Code::Digit8,
            Self::Digit9 => Code::Digit9,
            Self::KeyA => Code::KeyA,
            Self::KeyB => Code::KeyB,
            Self::KeyC => Code::KeyC,
            Self::KeyD => Code::KeyD,
            Self::KeyE => Code::KeyE,
            Self::KeyF => Code::KeyF,
            Self::KeyG => Code::KeyG,
            Self::KeyH => Code::KeyH,
            Self::KeyI => Code::KeyI,
            Self::KeyJ => Code::KeyJ,
            Self::KeyK => Code::KeyK,
            Self::KeyL => Code::KeyL,
            Self::KeyM => Code::KeyM,
            Self::KeyN => Code::KeyN,
            Self::KeyO => Code::KeyO,
            Self::KeyP => Code::KeyP,
            Self::KeyQ => Code::KeyQ,
            Self::KeyR => Code::KeyR,
            Self::KeyS => Code::KeyS,
            Self::KeyT => Code::KeyT,
            Self::KeyU => Code::KeyU,
            Self::KeyV => Code::KeyV,
            Self::KeyW => Code::KeyW,
            Self::KeyX => Code::KeyX,
            Self::KeyY => Code::KeyY,
            Self::KeyZ => Code::KeyZ,
        }
    }
}
```

- [ ] **Step 5: Run focused Rust tests**

Run:

```bash
cargo test -p wispergo-desktop shortcut::tests -- --nocapture
```

Expected: all `shortcut::tests` pass.

- [ ] **Step 6: Commit Task 1**

```bash
git add apps/desktop/src-tauri/src/shortcut.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): add shortcut settings model"
```

### Task 2: Add fakeable shortcut registration with rollback

**Files:**
- Modify: `apps/desktop/src-tauri/src/shortcut.rs`

- [ ] **Step 1: Add failing tests for conflict-safe apply/rollback**

Append tests to `shortcut.rs`:

```rust
    #[derive(Default)]
    struct FakeShortcutRegistry {
        active: Option<ShortcutSettings>,
        fail_next_register: Option<String>,
        unregistered: Vec<ShortcutSettings>,
        registered: Vec<ShortcutSettings>,
    }

    impl ShortcutRegistry for FakeShortcutRegistry {
        fn register(&mut self, settings: &ShortcutSettings) -> Result<(), String> {
            if let Some(message) = self.fail_next_register.take() {
                return Err(message);
            }
            self.registered.push(settings.clone());
            self.active = Some(settings.clone());
            Ok(())
        }

        fn unregister(&mut self, settings: &ShortcutSettings) -> Result<(), String> {
            self.unregistered.push(settings.clone());
            if self.active.as_ref() == Some(settings) {
                self.active = None;
            }
            Ok(())
        }
    }

    #[test]
    fn apply_shortcut_registers_new_combo_when_no_previous_active() {
        let mut registry = FakeShortcutRegistry::default();
        let mut active = None;
        let settings = ShortcutSettings::default();

        let view = apply_shortcut_settings(&mut registry, &mut active, settings.clone())
            .expect("apply shortcut");

        assert_eq!(active, Some(settings.clone()));
        assert_eq!(registry.registered, vec![settings]);
        assert_eq!(registry.unregistered.len(), 0);
        assert_eq!(view.display_label, "⌘ ⇧ Space");
    }

    #[test]
    fn apply_shortcut_replaces_previous_combo() {
        let previous = ShortcutSettings::default();
        let next = ShortcutSettings {
            mode: ShortcutMode::Combo,
            combo: ShortcutCombo {
                modifiers: ShortcutModifiers {
                    command: true,
                    shift: false,
                    option: true,
                    control: false,
                },
                key: ShortcutKey::KeyK,
            },
        };
        let mut registry = FakeShortcutRegistry {
            active: Some(previous.clone()),
            ..FakeShortcutRegistry::default()
        };
        let mut active = Some(previous.clone());

        let view = apply_shortcut_settings(&mut registry, &mut active, next.clone())
            .expect("apply shortcut");

        assert_eq!(active, Some(next.clone()));
        assert_eq!(registry.unregistered, vec![previous]);
        assert_eq!(registry.registered, vec![next]);
        assert_eq!(view.display_label, "⌘ ⌥ K");
    }

    #[test]
    fn apply_shortcut_rolls_back_when_new_registration_fails() {
        let previous = ShortcutSettings::default();
        let next = ShortcutSettings {
            mode: ShortcutMode::Combo,
            combo: ShortcutCombo {
                modifiers: ShortcutModifiers {
                    command: true,
                    shift: false,
                    option: true,
                    control: false,
                },
                key: ShortcutKey::KeyK,
            },
        };
        let mut registry = FakeShortcutRegistry {
            active: Some(previous.clone()),
            fail_next_register: Some("shortcut is already registered".to_string()),
            ..FakeShortcutRegistry::default()
        };
        let mut active = Some(previous.clone());

        let error = apply_shortcut_settings(&mut registry, &mut active, next)
            .expect_err("conflict should fail");

        assert!(error.contains("shortcut is already registered"));
        assert_eq!(active, Some(previous.clone()));
        assert_eq!(registry.active, Some(previous.clone()));
        assert_eq!(registry.unregistered, vec![previous.clone()]);
        assert_eq!(registry.registered, vec![previous]);
    }
```

- [ ] **Step 2: Run the tests and verify they fail for missing trait/helper**

Run:

```bash
cargo test -p wispergo-desktop shortcut::tests::apply_shortcut -- --nocapture
```

Expected: compile failures for missing `ShortcutRegistry` and `apply_shortcut_settings`.

- [ ] **Step 3: Implement registry trait and rollback helper**

Add to `shortcut.rs` outside the tests:

```rust
pub trait ShortcutRegistry {
    fn register(&mut self, settings: &ShortcutSettings) -> Result<(), String>;
    fn unregister(&mut self, settings: &ShortcutSettings) -> Result<(), String>;
}

pub fn apply_shortcut_settings<R: ShortcutRegistry>(
    registry: &mut R,
    active: &mut Option<ShortcutSettings>,
    next: ShortcutSettings,
) -> Result<ShortcutSettingsView, String> {
    let next = next.normalized();
    let previous = active.clone();

    if previous.as_ref() == Some(&next) {
        return Ok(next.to_frontend());
    }

    if let Some(previous_settings) = previous.as_ref() {
        registry.unregister(previous_settings)?;
    }

    if let Err(register_error) = registry.register(&next) {
        if let Some(previous_settings) = previous.as_ref() {
            if let Err(rollback_error) = registry.register(previous_settings) {
                return Err(format!(
                    "Shortcut could not be changed: {register_error}. The previous shortcut could not be restored: {rollback_error}"
                ));
            }
            *active = Some(previous_settings.clone());
        } else {
            *active = None;
        }
        return Err(format!("Shortcut could not be changed: {register_error}"));
    }

    *active = Some(next.clone());
    Ok(next.to_frontend())
}
```

- [ ] **Step 4: Run focused rollback tests**

Run:

```bash
cargo test -p wispergo-desktop shortcut::tests::apply_shortcut -- --nocapture
```

Expected: all `apply_shortcut_*` tests pass.

- [ ] **Step 5: Commit Task 2**

```bash
git add apps/desktop/src-tauri/src/shortcut.rs
git commit -m "feat(desktop): add shortcut rollback helper"
```

### Task 3: Persist shortcut settings and expose Tauri commands

**Files:**
- Modify: `apps/desktop/src-tauri/src/state.rs`
- Modify: `apps/desktop/src-tauri/src/commands/settings.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add failing state tests**

In `apps/desktop/src-tauri/src/state.rs`, import shortcut settings in the test module and add tests:

```rust
#[cfg(test)]
mod tests {
    use super::{AppState, CleanupMode, RecognitionLanguage, RecordingSession, RecordingStatus};
    use crate::shortcut::{ShortcutCombo, ShortcutKey, ShortcutMode, ShortcutModifiers, ShortcutSettings};

    #[test]
    fn shortcut_settings_default_to_command_shift_space() {
        let state = AppState::default();

        assert_eq!(state.shortcut_settings(), ShortcutSettings::default());
    }

    #[test]
    fn shortcut_settings_round_trip() {
        let state = AppState::default();
        let settings = ShortcutSettings {
            mode: ShortcutMode::Combo,
            combo: ShortcutCombo {
                modifiers: ShortcutModifiers {
                    command: true,
                    shift: false,
                    option: true,
                    control: false,
                },
                key: ShortcutKey::KeyK,
            },
        };

        state.set_shortcut_settings(settings.clone());

        assert_eq!(state.shortcut_settings(), settings);
    }
```

Keep the existing tests in the same module unchanged.

- [ ] **Step 2: Run state tests and verify they fail**

Run:

```bash
cargo test -p wispergo-desktop state::tests::shortcut_settings -- --nocapture
```

Expected: compile failures for missing `shortcut_settings` field and methods.

- [ ] **Step 3: Add shortcut settings to `AppState`**

Modify `apps/desktop/src-tauri/src/state.rs`:

```rust
use crate::audio::AudioInputSession;
use crate::shortcut::ShortcutSettings;
```

Add field:

```rust
pub struct AppState {
    recording: Mutex<Option<RecordingSession>>,
    selected_microphone_id: Mutex<Option<String>>,
    local_model_settings: Mutex<LocalModelSettings>,
    shortcut_settings: Mutex<ShortcutSettings>,
}
```

Initialize it:

```rust
shortcut_settings: Mutex::new(ShortcutSettings::default()),
```

Add methods:

```rust
pub fn shortcut_settings(&self) -> ShortcutSettings {
    self.shortcut_settings
        .lock()
        .expect("shortcut settings lock")
        .clone()
}

pub fn set_shortcut_settings(&self, settings: ShortcutSettings) {
    *self
        .shortcut_settings
        .lock()
        .expect("shortcut settings lock") = settings.normalized();
}
```

- [ ] **Step 4: Run state tests**

Run:

```bash
cargo test -p wispergo-desktop state::tests::shortcut_settings -- --nocapture
```

Expected: shortcut state tests pass.

- [ ] **Step 5: Add persistence tests for `PersistedSettings`**

In `apps/desktop/src-tauri/src/commands/settings.rs` test module, add tests near existing serialization/default tests:

```rust
    #[test]
    fn persisted_settings_default_shortcut_is_command_shift_space() {
        let persisted: super::PersistedSettings = serde_json::from_str("{}").expect("deserialize");

        assert_eq!(persisted.shortcut, ShortcutSettings::default());
    }

    #[test]
    fn persisted_settings_round_trips_local_model_and_shortcut() {
        let persisted = super::PersistedSettings {
            local_model: LocalModelSettings {
                asr_model_id: "large-v3-turbo".to_string(),
                recognition_language: RecognitionLanguage::Zh,
                cleanup_mode: CleanupMode::FullCleanup,
            },
            shortcut: ShortcutSettings {
                mode: ShortcutMode::Combo,
                combo: ShortcutCombo {
                    modifiers: ShortcutModifiers {
                        command: true,
                        shift: false,
                        option: true,
                        control: false,
                    },
                    key: ShortcutKey::KeyK,
                },
            },
        };

        let json = serde_json::to_string(&persisted).expect("serialize");
        let parsed: super::PersistedSettings = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.local_model.asr_model_id, "large-v3-turbo");
        assert_eq!(parsed.shortcut.display_label(), "⌘ ⌥ K");
    }
```

Add imports in the test module:

```rust
use crate::shortcut::{ShortcutCombo, ShortcutKey, ShortcutMode, ShortcutModifiers, ShortcutSettings};
```

- [ ] **Step 6: Run persistence tests and verify failure**

Run:

```bash
cargo test -p wispergo-desktop commands::settings::tests::persisted_settings -- --nocapture
```

Expected: compile failure because `PersistedSettings` has no `shortcut` field.

- [ ] **Step 7: Extend `PersistedSettings` and load/save paths**

At the top of `settings.rs`, import shortcut types:

```rust
use crate::shortcut::{apply_shortcut_settings, ShortcutSettings, ShortcutSettingsView};
```

Change `PersistedSettings`:

```rust
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedSettings {
    #[serde(default)]
    local_model: LocalModelSettings,
    #[serde(default)]
    shortcut: ShortcutSettings,
}
```

Change `load_persisted_settings` to load both fields:

```rust
let persisted = serde_json::from_str::<PersistedSettings>(&content).map_err(|err| err.to_string())?;
state.set_local_model_settings(persisted.local_model.normalized());
state.set_shortcut_settings(persisted.shortcut.normalized());
```

Change `save_persisted_settings` signature to accept both values:

```rust
fn save_persisted_settings(
    app: &AppHandle,
    local_model: &LocalModelSettings,
    shortcut: &ShortcutSettings,
) -> Result<(), String> {
    let path = settings_file_path(app)?;
    let persisted = PersistedSettings {
        local_model: local_model.clone(),
        shortcut: shortcut.clone().normalized(),
    };
    let content = serde_json::to_string_pretty(&persisted).map_err(|err| err.to_string())?;
    fs::write(path, content).map_err(|err| err.to_string())
}
```

Update both existing call sites explicitly:

In `apply_local_model_settings`, replace the existing save call with:

```rust
save_persisted_settings(&app, &settings, &state.shortcut_settings())?;
```

In `set_recognition_language`, replace the existing save call with:

```rust
save_persisted_settings(&app, &settings, &state.shortcut_settings())?;
```

- [ ] **Step 8: Add Tauri commands for shortcut settings**

Add to `settings.rs`:

```rust
#[tauri::command]
pub fn shortcut_settings(state: State<'_, AppState>) -> ShortcutSettingsView {
    state.shortcut_settings().to_frontend()
}

#[tauri::command]
pub fn set_shortcut_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: ShortcutSettings,
) -> Result<ShortcutSettingsView, String> {
    let previous = state.shortcut_settings();
    let settings = settings.normalized();
    let view = crate::apply_shortcut_settings_for_app(&app, settings.clone())?;
    state.set_shortcut_settings(settings.clone());

    if let Err(save_error) =
        save_persisted_settings(&app, &state.local_model_settings(), &state.shortcut_settings())
    {
        let _ = crate::apply_shortcut_settings_for_app(&app, previous.clone());
        state.set_shortcut_settings(previous);
        return Err(format!("Shortcut could not be saved: {save_error}"));
    }

    Ok(view)
}
```

This calls a crate-level helper added in Task 4. If implementing Task 3 before Task 4, temporarily create the function signature in `lib.rs` and let the test fail until Task 4 completes.

- [ ] **Step 9: Register commands in `lib.rs`**

Modify imports from `commands::settings`:

```rust
shortcut_settings, set_shortcut_settings,
```

Add both to `tauri::generate_handler!`:

```rust
shortcut_settings,
set_shortcut_settings,
```

- [ ] **Step 10: Run focused tests**

Run:

```bash
cargo test -p wispergo-desktop state::tests::shortcut_settings commands::settings::tests::persisted_settings -- --nocapture
```

Expected: tests pass after Task 4 helper exists; if this is run before Task 4, expected compile failure is only the missing `apply_shortcut_settings_for_app` symbol.

- [ ] **Step 11: Commit Task 3 after Task 4 compiles**

```bash
git add apps/desktop/src-tauri/src/state.rs apps/desktop/src-tauri/src/commands/settings.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): persist shortcut settings"
```

### Task 4: Wire dynamic global shortcut registration in Tauri

**Files:**
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/shortcut.rs`

- [ ] **Step 1: Add tests for event payload helper and active-state update**

In `apps/desktop/src-tauri/src/shortcut.rs`, add a pure helper and add the tests to the existing `#[cfg(test)] mod tests` block from Task 1. Do not create a second `mod tests` block.

```rust
pub const RECORD_SHORTCUT_EVENT: &str = "wispergo://record-shortcut";

pub fn shortcut_event_payload(state: ShortcutState) -> &'static str {
    match state {
        ShortcutState::Pressed => "Pressed",
        ShortcutState::Released => "Released",
    }
}
```

Add tests inside the existing test module:

```rust
    #[test]
    fn shortcut_event_payload_matches_frontend_contract() {
        assert_eq!(RECORD_SHORTCUT_EVENT, "wispergo://record-shortcut");
        assert_eq!(shortcut_event_payload(ShortcutState::Pressed), "Pressed");
        assert_eq!(shortcut_event_payload(ShortcutState::Released), "Released");
    }
```

Import `ShortcutState` at the top:

```rust
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};
```

- [ ] **Step 2: Run helper test and verify failure before helper exists**

Run:

```bash
cargo test -p wispergo-desktop shortcut::tests::shortcut_event_payload_matches_frontend_contract -- --nocapture
```

Expected: compile failure before helper implementation, then pass after Step 1 implementation.

- [ ] **Step 3: Replace hardcoded plugin registration with dynamic plugin setup**

In `apps/desktop/src-tauri/src/lib.rs`, replace imports:

```rust
use shortcut::{
    shortcut_event_payload, RECORD_SHORTCUT_EVENT, ShortcutRegistry, ShortcutSettings,
    ShortcutSettingsView,
};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
```

Remove direct imports of `Code`, `Modifiers`, and `ShortcutState` from `lib.rs`.

Add managed state:

```rust
#[derive(Default)]
struct ActiveShortcutState(Mutex<Option<ShortcutSettings>>);
```

Add `.manage(ActiveShortcutState::default())` to the builder.

Replace `setup_global_shortcut(app.handle())?;` with:

```rust
setup_global_shortcut_plugin(app)?;
setup_active_shortcut(app.handle())?;
```

Add helper functions. Use only the per-shortcut `on_shortcut` handler in `TauriShortcutRegistry::register`; the plugin builder must not also install a builder-level `with_handler`, otherwise each shortcut event can be emitted twice.

```rust
fn setup_global_shortcut_plugin(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.plugin(tauri_plugin_global_shortcut::Builder::new().build())?;
    Ok(())
}

fn setup_active_shortcut(app: &tauri::AppHandle) -> Result<(), String> {
    let settings = app.state::<AppState>().shortcut_settings();
    apply_shortcut_settings_for_app(app, settings).map(|_| ())
}

pub(crate) fn apply_shortcut_settings_for_app(
    app: &tauri::AppHandle,
    settings: ShortcutSettings,
) -> Result<ShortcutSettingsView, String> {
    let active_state = app.state::<ActiveShortcutState>();
    let mut active = active_state.0.lock().map_err(|err| err.to_string())?;
    let mut registry = TauriShortcutRegistry { app };
    shortcut::apply_shortcut_settings(&mut registry, &mut active, settings)
}

struct TauriShortcutRegistry<'a> {
    app: &'a tauri::AppHandle,
}

impl ShortcutRegistry for TauriShortcutRegistry<'_> {
    fn register(&mut self, settings: &ShortcutSettings) -> Result<(), String> {
        self.app
            .global_shortcut()
            .on_shortcut(settings.to_tauri_shortcut()?, |app, _shortcut, event| {
                let _ = app.emit(RECORD_SHORTCUT_EVENT, shortcut_event_payload(event.state));
            })
            .map_err(|err| err.to_string())
    }

    fn unregister(&mut self, settings: &ShortcutSettings) -> Result<(), String> {
        self.app
            .global_shortcut()
            .unregister(settings.to_tauri_shortcut()?)
            .map_err(|err| err.to_string())
    }
}
```

- [ ] **Step 4: Remove old `setup_global_shortcut` function**

Delete the old function that hardcoded:

```rust
Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Space)
```

- [ ] **Step 5: Run Rust compile/tests for shortcut/settings/lib**

Run:

```bash
cargo test -p wispergo-desktop shortcut::tests state::tests::shortcut_settings commands::settings::tests::persisted_settings -- --nocapture
```

Expected: focused tests pass.

- [ ] **Step 6: Run existing lib tests that exercise app command registration and build-script behavior**

Run:

```bash
cargo test -p wispergo-desktop tests::app_registers_recognition_language_and_ollama_setup_commands tests::desktop_build_runs_stable_macos_signing_script -- --nocapture
```

Expected: tests still pass.

- [ ] **Step 7: Commit Task 4**

```bash
git add apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/shortcut.rs apps/desktop/src-tauri/src/commands/settings.rs apps/desktop/src-tauri/src/state.rs
git commit -m "feat(desktop): register shortcut dynamically"
```

### Task 5: Add frontend shortcut types and API wrappers

**Files:**
- Modify: `apps/desktop/src/types/pipeline.ts`
- Modify: `apps/desktop/src/lib/tauriApi.ts`

- [ ] **Step 1: Add TypeScript types**

In `apps/desktop/src/types/pipeline.ts`, add after local model types:

```ts
export type ShortcutMode = "combo";

export type ShortcutKey =
  | "space"
  | "enter"
  | "escape"
  | "tab"
  | "backquote"
  | "minus"
  | "equal"
  | "bracketLeft"
  | "bracketRight"
  | "backslash"
  | "semicolon"
  | "quote"
  | "comma"
  | "period"
  | "slash"
  | "arrowUp"
  | "arrowDown"
  | "arrowLeft"
  | "arrowRight"
  | "digit0"
  | "digit1"
  | "digit2"
  | "digit3"
  | "digit4"
  | "digit5"
  | "digit6"
  | "digit7"
  | "digit8"
  | "digit9"
  | "keyA"
  | "keyB"
  | "keyC"
  | "keyD"
  | "keyE"
  | "keyF"
  | "keyG"
  | "keyH"
  | "keyI"
  | "keyJ"
  | "keyK"
  | "keyL"
  | "keyM"
  | "keyN"
  | "keyO"
  | "keyP"
  | "keyQ"
  | "keyR"
  | "keyS"
  | "keyT"
  | "keyU"
  | "keyV"
  | "keyW"
  | "keyX"
  | "keyY"
  | "keyZ";

export type ShortcutModifiers = {
  command: boolean;
  shift: boolean;
  option: boolean;
  control: boolean;
};

export type ShortcutCombo = {
  modifiers: ShortcutModifiers;
  key: ShortcutKey;
};

export type ShortcutSettings = {
  mode: ShortcutMode;
  combo: ShortcutCombo;
};

export type ShortcutSettingsView = {
  settings: ShortcutSettings;
  displayLabel: string;
};
```

- [ ] **Step 2: Add Tauri wrappers**

In `apps/desktop/src/lib/tauriApi.ts`, import `ShortcutSettings` and `ShortcutSettingsView` and add:

```ts
export async function shortcutSettings(): Promise<ShortcutSettingsView> {
  return invoke<ShortcutSettingsView>("shortcut_settings");
}

export async function setShortcutSettings(
  settings: ShortcutSettings,
): Promise<ShortcutSettingsView> {
  return invoke<ShortcutSettingsView>("set_shortcut_settings", { settings });
}
```

- [ ] **Step 3: Run TypeScript tests to catch type errors**

Run:

```bash
pnpm test:ts
```

Expected: tests pass or fail only because mocks do not include the new functions. If mocks fail, Task 7 updates them.

- [ ] **Step 4: Revert Corepack package manager mutation**

Run:

```bash
python3 - <<'PY'
import json
from pathlib import Path
p=Path('package.json')
data=json.loads(p.read_text())
data.pop('packageManager', None)
p.write_text(json.dumps(data, indent=2)+"\n")
PY
```

- [ ] **Step 5: Commit Task 5**

```bash
git add apps/desktop/src/types/pipeline.ts apps/desktop/src/lib/tauriApi.ts package.json
git commit -m "feat(desktop): expose shortcut settings API"
```

### Task 6: Pass dynamic shortcut label to recorder

**Files:**
- Modify: `apps/desktop/src/features/recorder/FloatingRecorder.tsx`
- Modify: `apps/desktop/src/features/recorder/FloatingRecorder.test.tsx`
- Modify: `apps/desktop/src/app/App.tsx`
- Modify: `apps/desktop/src/app/App.test.tsx`

- [ ] **Step 1: Write failing recorder component tests**

Modify `FloatingRecorder.test.tsx`:

```tsx
it("renders a keyboard-only shortcut prompt while expanded and idle", () => {
  render(<FloatingRecorder status="idle" expanded shortcutLabel="⌘ ⇧ Space" />);

  expect(screen.getByRole("region", { name: "Recorder" })).toHaveTextContent("Ready");
  expect(screen.getByText("hold ⌘ ⇧ Space")).toBeInTheDocument();
  expect(screen.queryByRole("button")).not.toBeInTheDocument();
});

it("renders a custom shortcut prompt while expanded and idle", () => {
  render(<FloatingRecorder status="idle" expanded shortcutLabel="⌘ ⌥ K" />);

  expect(screen.getByText("hold ⌘ ⌥ K")).toBeInTheDocument();
  expect(screen.queryByText("hold Command + Shift + Space")).not.toBeInTheDocument();
});
```

Update collapsed test to assert no `hold ⌘ ⇧ Space` text:

```tsx
expect(screen.queryByText("hold ⌘ ⇧ Space")).not.toBeInTheDocument();
```

- [ ] **Step 2: Run recorder test and verify failure**

Run:

```bash
pnpm --dir apps/desktop test src/features/recorder/FloatingRecorder.test.tsx
```

Expected: failure because `FloatingRecorder` does not accept `shortcutLabel` and still renders old copy.

- [ ] **Step 3: Implement `shortcutLabel` prop**

Modify `FloatingRecorder.tsx`:

```tsx
type Props = {
  status: RecordingStatus;
  busy?: boolean;
  expanded?: boolean;
  setupNeeded?: boolean;
  shortcutLabel?: string;
};

export function FloatingRecorder({
  status,
  busy = false,
  expanded = true,
  setupNeeded = false,
  shortcutLabel = "⌘ ⇧ Space",
}: Props) {
```

Replace hardcoded hint:

```tsx
<div className="recording-hint">{setupNeeded ? "open settings to finish" : `hold ${shortcutLabel}`}</div>
```

- [ ] **Step 4: Run recorder test and verify pass**

Run:

```bash
pnpm --dir apps/desktop test src/features/recorder/FloatingRecorder.test.tsx
```

Expected: recorder tests pass.

- [ ] **Step 5: Load shortcut label in `App.tsx`**

Update imports from `tauriApi.ts`:

```ts
shortcutSettings,
setShortcutSettings,
```

Update types import:

```ts
ShortcutSettings,
ShortcutSettingsView,
```

Add state near model settings state:

```tsx
const DEFAULT_SHORTCUT_VIEW: ShortcutSettingsView = {
  settings: {
    mode: "combo",
    combo: {
      modifiers: { command: true, shift: true, option: false, control: false },
      key: "space",
    },
  },
  displayLabel: "⌘ ⇧ Space",
};

const [shortcutView, setShortcutView] = useState<ShortcutSettingsView>(DEFAULT_SHORTCUT_VIEW);
const shortcutViewRef = useRef(DEFAULT_SHORTCUT_VIEW);
```

Load shortcut settings in a startup `useEffect` that runs for every surface, including `/?surface=recorder`. Do not put this call only in the Settings-surface-only effect, because the recorder hint needs the selected label too.

```tsx
useEffect(() => {
  let mounted = true;
  shortcutSettings()
    .then((view) => {
      if (mounted) {
        shortcutViewRef.current = view;
        setShortcutView(view);
      }
    })
    .catch(() => {
      if (mounted) {
        shortcutViewRef.current = DEFAULT_SHORTCUT_VIEW;
        setShortcutView(DEFAULT_SHORTCUT_VIEW);
      }
    });
  return () => {
    mounted = false;
  };
}, []);
```

Pass label to recorder:

```tsx
<FloatingRecorder
  status={status}
  busy={pending}
  expanded={floatingChromeExpanded}
  setupNeeded={setupNeeded}
  shortcutLabel={shortcutView.displayLabel}
/>
```

- [ ] **Step 6: Update App tests/mocks**

In `App.test.tsx`, mock new API functions:

```ts
shortcutSettings: vi.fn().mockResolvedValue({
  settings: {
    mode: "combo",
    combo: {
      modifiers: { command: true, shift: true, option: false, control: false },
      key: "space",
    },
  },
  displayLabel: "⌘ ⇧ Space",
}),
setShortcutSettings: vi.fn(async (settings) => ({ settings, displayLabel: "⌘ ⌥ K" })),
```

Update assertions that still expect old copy:

```tsx
expect(screen.getByRole("region", { name: "Recorder" })).toHaveTextContent("hold ⌘ ⇧ Space");
```

Add an App test for loaded custom label:

```tsx
it("renders loaded shortcut label in the recorder hint", async () => {
  vi.mocked(shortcutSettings).mockResolvedValueOnce({
    settings: {
      mode: "combo",
      combo: {
        modifiers: { command: true, shift: false, option: true, control: false },
        key: "keyK",
      },
    },
    displayLabel: "⌘ ⌥ K",
  });
  window.history.pushState({}, "", "/?surface=recorder");
  render(<App />);
  await emitFloatingChromeExpanded(true);

  expect(await screen.findByText("hold ⌘ ⌥ K")).toBeInTheDocument();
});
```

- [ ] **Step 7: Run App/recorder tests**

Run:

```bash
pnpm --dir apps/desktop test src/features/recorder/FloatingRecorder.test.tsx src/app/App.test.tsx
```

Expected: tests pass.

- [ ] **Step 8: Revert Corepack package manager mutation**

Run:

```bash
python3 - <<'PY'
import json
from pathlib import Path
p=Path('package.json')
data=json.loads(p.read_text())
data.pop('packageManager', None)
p.write_text(json.dumps(data, indent=2)+"\n")
PY
```

- [ ] **Step 9: Commit Task 6**

```bash
git add apps/desktop/src/features/recorder/FloatingRecorder.tsx apps/desktop/src/features/recorder/FloatingRecorder.test.tsx apps/desktop/src/app/App.tsx apps/desktop/src/app/App.test.tsx package.json
git commit -m "feat(desktop): show selected shortcut label"
```

### Task 7: Add Settings shortcut combo UI and conflict errors

**Files:**
- Modify: `apps/desktop/src/features/settings/SettingsPanel.tsx`
- Modify: `apps/desktop/src/features/settings/SettingsPanel.test.tsx`
- Modify: `apps/desktop/src/app/App.tsx`
- Modify: `apps/desktop/src/styles.css`

- [ ] **Step 1: Add failing SettingsPanel tests**

Update `SettingsPanel.test.tsx` mock props with new properties:

```ts
shortcutView: {
  settings: {
    mode: "combo",
    combo: {
      modifiers: { command: true, shift: true, option: false, control: false },
      key: "space",
    },
  },
  displayLabel: "⌘ ⇧ Space",
},
shortcutError: null,
onShortcutSettingsSave: vi.fn(),
```

Replace the old global shortcut test:

```tsx
it("shows the selected shortcut label in the hero and shortcut card", () => {
  renderSettingsPanel();

  expect(screen.getByText("Shortcut")).toBeInTheDocument();
  expect(screen.getAllByText("⌘ ⇧ Space").length).toBeGreaterThanOrEqual(2);
});
```

Add a capture/save test:

```tsx
it("records and saves a key-combination shortcut", async () => {
  const user = userEvent.setup();
  const onShortcutSettingsSave = vi.fn();
  renderSettingsPanel({ onShortcutSettingsSave });

  await user.click(screen.getByRole("button", { name: "Record shortcut" }));
  await user.keyboard("{Meta>}{Alt>}k{/Alt}{/Meta}");
  await user.click(screen.getByRole("button", { name: "Save changes" }));

  expect(onShortcutSettingsSave).toHaveBeenCalledWith({
    mode: "combo",
    combo: {
      modifiers: { command: true, shift: false, option: true, control: false },
      key: "keyK",
    },
  });
});
```

Add an error rendering test:

```tsx
it("shows shortcut save errors inline", () => {
  renderSettingsPanel({ shortcutError: "Shortcut could not be changed: already registered" });

  expect(screen.getByText("Shortcut could not be changed: already registered")).toBeInTheDocument();
});
```

Add validation test:

```tsx
it("rejects shortcut recording without a modifier", async () => {
  const user = userEvent.setup();
  const onShortcutSettingsSave = vi.fn();
  renderSettingsPanel({ onShortcutSettingsSave });

  await user.click(screen.getByRole("button", { name: "Record shortcut" }));
  await user.keyboard("k");

  expect(screen.getByText("Use at least one modifier key."));
  expect(onShortcutSettingsSave).not.toHaveBeenCalled();
});
```

- [ ] **Step 2: Run SettingsPanel test and verify failure**

Run:

```bash
pnpm --dir apps/desktop test src/features/settings/SettingsPanel.test.tsx
```

Expected: compile/render failures because props/UI do not exist.

- [ ] **Step 3: Add props and local draft state**

In `SettingsPanel.tsx`, extend the React import and add shortcut types:

```ts
import { useEffect, useRef, useState } from "react";
import type { ShortcutKey, ShortcutSettings, ShortcutSettingsView } from "../../types/pipeline";
```

Add props:

```ts
shortcutView: ShortcutSettingsView;
shortcutError?: string | null;
onShortcutSettingsSave: (settings: ShortcutSettings) => void;
```

Add state and a focus ref:

```tsx
const [draftShortcutSettings, setDraftShortcutSettings] = useState(shortcutView.settings);
const [recordingShortcut, setRecordingShortcut] = useState(false);
const [localShortcutError, setLocalShortcutError] = useState<string | null>(null);
const shortcutRecordButtonRef = useRef<HTMLButtonElement | null>(null);

useEffect(() => {
  setDraftShortcutSettings(shortcutView.settings);
}, [shortcutView]);

useEffect(() => {
  if (recordingShortcut) {
    shortcutRecordButtonRef.current?.focus();
  }
}, [recordingShortcut]);
```

- [ ] **Step 4: Add key mapping helpers**

Add inside `SettingsPanel.tsx` below the component or in a small local helper section:

```ts
const KEY_CODE_TO_SHORTCUT_KEY: Record<string, ShortcutKey> = {
  Space: "space",
  Enter: "enter",
  Escape: "escape",
  Tab: "tab",
  Backquote: "backquote",
  Minus: "minus",
  Equal: "equal",
  BracketLeft: "bracketLeft",
  BracketRight: "bracketRight",
  Backslash: "backslash",
  Semicolon: "semicolon",
  Quote: "quote",
  Comma: "comma",
  Period: "period",
  Slash: "slash",
  ArrowUp: "arrowUp",
  ArrowDown: "arrowDown",
  ArrowLeft: "arrowLeft",
  ArrowRight: "arrowRight",
};

for (let index = 0; index <= 9; index += 1) {
  KEY_CODE_TO_SHORTCUT_KEY[`Digit${index}`] = `digit${index}` as ShortcutKey;
}
for (const letter of "ABCDEFGHIJKLMNOPQRSTUVWXYZ") {
  KEY_CODE_TO_SHORTCUT_KEY[`Key${letter}`] = `key${letter}` as ShortcutKey;
}

function shortcutKeyFromKeyboardEvent(event: React.KeyboardEvent): ShortcutKey | null {
  return KEY_CODE_TO_SHORTCUT_KEY[event.code] ?? null;
}

function shortcutHasModifier(event: React.KeyboardEvent) {
  return event.metaKey || event.shiftKey || event.altKey || event.ctrlKey;
}

function labelForDraftShortcut(settings: ShortcutSettings) {
  const parts: string[] = [];
  if (settings.combo.modifiers.command) parts.push("⌘");
  if (settings.combo.modifiers.shift) parts.push("⇧");
  if (settings.combo.modifiers.option) parts.push("⌥");
  if (settings.combo.modifiers.control) parts.push("⌃");
  parts.push(shortcutKeyLabel(settings.combo.key));
  return parts.join(" ");
}

function shortcutKeyLabel(key: ShortcutKey) {
  if (key === "space") return "Space";
  if (key === "enter") return "Return";
  if (key === "escape") return "Esc";
  if (key === "tab") return "Tab";
  if (key.startsWith("key")) return key.slice(3).toUpperCase();
  if (key.startsWith("digit")) return key.slice(5);
  const special: Partial<Record<ShortcutKey, string>> = {
    backquote: "`",
    minus: "-",
    equal: "=",
    bracketLeft: "[",
    bracketRight: "]",
    backslash: "\\",
    semicolon: ";",
    quote: "'",
    comma: ",",
    period: ".",
    slash: "/",
    arrowUp: "↑",
    arrowDown: "↓",
    arrowLeft: "←",
    arrowRight: "→",
  };
  return special[key] ?? key;
}
```

- [ ] **Step 5: Add Shortcut card UI**

First, replace the Settings hero's hardcoded shortcut fact with the dynamic label:

```tsx
<span><SettingsIcon name="keyboard" />{shortcutView.displayLabel}</span>
```

Then insert a compact card after the Input card and before Dictation, or inside the input area if the layout fits:

```tsx
<section className="settings-card shortcut-card" aria-label="Shortcut preferences">
  <div className="settings-card-heading">
    <h3>Shortcut</h3>
    <span>Hold to dictate</span>
  </div>
  <div className="shortcut-current">
    <span>Current</span>
    <strong>{shortcutView.displayLabel}</strong>
  </div>
  <button
    ref={shortcutRecordButtonRef}
    type="button"
    className={recordingShortcut ? "is-recording-shortcut" : undefined}
    onClick={() => {
      setRecordingShortcut(true);
      setLocalShortcutError(null);
    }}
    onKeyDown={(event) => {
      if (!recordingShortcut) return;
      event.preventDefault();
      event.stopPropagation();
      const key = shortcutKeyFromKeyboardEvent(event);
      if (!key) {
        setLocalShortcutError("That key is not supported for shortcuts yet.");
        return;
      }
      if (!shortcutHasModifier(event)) {
        setLocalShortcutError("Use at least one modifier key.");
        return;
      }
      const next: ShortcutSettings = {
        mode: "combo",
        combo: {
          modifiers: {
            command: event.metaKey,
            shift: event.shiftKey,
            option: event.altKey,
            control: event.ctrlKey,
          },
          key,
        },
      };
      setDraftShortcutSettings(next);
      setRecordingShortcut(false);
      setLocalShortcutError(null);
    }}
  >
    {recordingShortcut ? "Press shortcut…" : "Record shortcut"}
  </button>
  <p className="settings-help">Selected: {labelForDraftShortcut(draftShortcutSettings)}</p>
  {(localShortcutError || shortcutError) ? (
    <p className="settings-error" role="alert">{localShortcutError ?? shortcutError}</p>
  ) : null}
</section>
```

The `shortcutRecordButtonRef` focus effect is required so the next keydown reaches the recording button in both jsdom tests and the real Settings window.

- [ ] **Step 6: Save shortcut together with existing Save changes**

In the existing Save changes button handler, call both saves:

```tsx
onModelSettingsSave(draftModelSettings);
onShortcutSettingsSave(draftShortcutSettings);
```

This is acceptable for R5.1 because the app-level handlers keep each command separate and errors from shortcut save are shown inline. If product behavior feels noisy, change the app-level handler to skip shortcut save when unchanged.

- [ ] **Step 7: Add styles**

In `apps/desktop/src/styles.css`, add minimal styles using existing settings vocabulary:

```css
.shortcut-card {
  gap: 14px;
}

.shortcut-current {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.shortcut-current span,
.settings-help {
  color: var(--text-muted);
  font-size: 0.86rem;
}

.shortcut-current strong {
  font-size: 1rem;
  font-weight: 700;
}

.settings-error {
  margin: 0;
  color: var(--danger, #b42318);
  font-size: 0.86rem;
}

button.is-recording-shortcut {
  outline: 2px solid var(--accent, #7c3aed);
  outline-offset: 2px;
}
```

If existing CSS variables use different names, use the nearest existing token rather than creating a new visual system.

- [ ] **Step 8: Wire App save handler and error state**

In `App.tsx`, add state:

```tsx
const [shortcutError, setShortcutError] = useState<string | null>(null);
```

Add handler:

```tsx
function handleShortcutSettingsSave(settings: ShortcutSettings) {
  setShortcutError(null);
  setShortcutSettings(settings)
    .then((view) => {
      shortcutViewRef.current = view;
      setShortcutView(view);
    })
    .catch((err: unknown) => {
      setShortcutError(errorMessage(err));
    });
}
```

Pass to SettingsPanel:

```tsx
shortcutView={shortcutView}
shortcutError={shortcutError}
onShortcutSettingsSave={handleShortcutSettingsSave}
```

- [ ] **Step 9: Run SettingsPanel and App tests**

Run:

```bash
pnpm --dir apps/desktop test src/features/settings/SettingsPanel.test.tsx src/app/App.test.tsx
```

Expected: tests pass.

- [ ] **Step 10: Revert Corepack package manager mutation**

Run:

```bash
python3 - <<'PY'
import json
from pathlib import Path
p=Path('package.json')
data=json.loads(p.read_text())
data.pop('packageManager', None)
p.write_text(json.dumps(data, indent=2)+"\n")
PY
```

- [ ] **Step 11: Commit Task 7**

```bash
git add apps/desktop/src/features/settings/SettingsPanel.tsx apps/desktop/src/features/settings/SettingsPanel.test.tsx apps/desktop/src/app/App.tsx apps/desktop/src/app/App.test.tsx apps/desktop/src/styles.css package.json
git commit -m "feat(desktop): add shortcut combo settings UI"
```

### Task 8: Update roadmap and handoff for R5.1 implementation

**Files:**
- Modify: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- Modify: `HANDOFF.md`

- [ ] **Step 1: Update roadmap R5 status after implementation is verified**

In the R5 section, update R5.1 line from planned to implemented only after Task 9 verification passes:

```markdown
- R5.1: key-combination customization with conflict-safe save/rollback. ✅
```

Keep R5.2 as planned/not started.

- [ ] **Step 2: Update `HANDOFF.md` current slice**

Record:

- branch name `r5-1-shortcut-combo-customization`;
- R5.1 implemented, PR pending;
- R5.2 single modifier hold remains next planned slice after user merge/approval;
- verification commands that passed.

Use concise text such as:

```markdown
## Current slice: R5.1 shortcut combo customization

**Implementation status:** Implemented on branch `r5-1-shortcut-combo-customization`. Adds persisted combo shortcut settings, dynamic global shortcut registration with rollback, Settings shortcut recording UI, and dynamic recorder/Settings labels. R5.2 single modifier hold remains not implemented.

**Next step:** Open PR and wait for user merge. After merge, sync `main`, delete the branch, then decide whether to proceed to R5.2.
```

- [ ] **Step 3: Commit docs**

```bash
git add docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md HANDOFF.md
git commit -m "docs: update r5 shortcut combo status"
```

### Task 9: Full verification and PR

**Files:**
- No source edits unless verification exposes a bug.

- [ ] **Step 1: Run full frontend tests**

```bash
pnpm test:ts
```

Expected: all Vitest tests pass.

- [ ] **Step 2: Revert Corepack package manager mutation**

```bash
python3 - <<'PY'
import json
from pathlib import Path
p=Path('package.json')
data=json.loads(p.read_text())
data.pop('packageManager', None)
p.write_text(json.dumps(data, indent=2)+"\n")
PY
```

- [ ] **Step 3: Run desktop Rust tests**

```bash
cargo test -p wispergo-desktop
```

Expected: all desktop tests pass.

- [ ] **Step 4: Run desktop clippy gate**

```bash
cargo clippy -p wispergo-desktop --all-targets -- -D warnings
```

Expected: no warnings or errors.

- [ ] **Step 5: Run build and thin-bundle check**

```bash
pnpm desktop:build
pnpm check:macos-thin-bundle
```

Expected: build succeeds and thin-bundle check passes.

- [ ] **Step 6: Revert Corepack package manager mutation again if needed**

```bash
python3 - <<'PY'
import json
from pathlib import Path
p=Path('package.json')
data=json.loads(p.read_text())
data.pop('packageManager', None)
p.write_text(json.dumps(data, indent=2)+"\n")
PY
```

- [ ] **Step 7: Check final diff**

```bash
git status --short
git diff --stat main...HEAD
```

Expected: only intended source/test/docs files changed; no `packageManager` field in `package.json`.

- [ ] **Step 8: Push and open PR**

```bash
git push -u origin r5-1-shortcut-combo-customization
cat > /tmp/wispergo-r5-1-pr.md <<'EOF'
## Summary
- add persisted key-combination shortcut settings with default `⌘ ⇧ Space`
- register dictation shortcut dynamically with conflict-safe rollback
- add Settings UI to record/save a shortcut combo and show inline errors
- update recorder and Settings copy to show the selected shortcut label

## Out of scope
- single modifier-key hold-to-dictate; planned for R5.2
- arbitrary single-key hold
- ASR/engine changes

## Verification
- `pnpm test:ts`
- `cargo test -p wispergo-desktop`
- `cargo clippy -p wispergo-desktop --all-targets -- -D warnings`
- `pnpm desktop:build`
- `pnpm check:macos-thin-bundle`

## Manual follow-up
- Fresh profile defaults to `⌘ ⇧ Space`
- Custom combo updates Settings and recorder hint
- Conflicting combo save keeps the previous working shortcut active
EOF
gh pr create --base main --head r5-1-shortcut-combo-customization --title "feat(desktop): customize shortcut combo" --body-file /tmp/wispergo-r5-1-pr.md
```

---

## Plan self-review

### Spec coverage

- Default remains `Command + Shift + Space`: Task 1 default tests and Task 4 dynamic startup registration.
- Key-combination customization: Tasks 5-7.
- Conflict-safe save/rollback: Tasks 2 and 4.
- Persist settings separately from `LocalModelSettings`: Task 3.
- Settings and recorder copy show selected label: Tasks 6 and 7.
- R5.2 single modifier hold not implemented: Scope and PR text explicitly exclude it.

### Placeholder scan

This plan intentionally avoids vague markers and unspecified test steps. Every task includes concrete files, commands, and expected outcomes.

### Type consistency

Rust names used throughout: `ShortcutSettings`, `ShortcutCombo`, `ShortcutModifiers`, `ShortcutKey`, `ShortcutSettingsView`, `ShortcutRegistry`, `apply_shortcut_settings`.

TypeScript names used throughout: `ShortcutSettings`, `ShortcutSettingsView`, `ShortcutKey`, `shortcutSettings`, `setShortcutSettings`.
