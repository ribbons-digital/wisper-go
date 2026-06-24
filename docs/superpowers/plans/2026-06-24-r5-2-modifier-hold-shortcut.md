# R5.2 Modifier-Hold Shortcut Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add single physical modifier-key hold-to-dictate as a second shortcut mode while preserving R5.1 combo shortcuts, the default `⌘ ⇧ Space`, and the existing frontend `Pressed` / `Released` event contract.

**Architecture:** Extend the existing `ShortcutSettings` model with a `modifier_hold` mode and a focused modifier-hold state machine that is unit-testable without macOS APIs. On macOS, install a listen-only `CGEventTap` for `flagsChanged` and `keyDown` events only while modifier-hold mode is active; modifier release is derived from `flagsChanged`, and normal key chords are detected from `keyDown`. It emits the same `wispergo://record-shortcut` payloads as combo mode. Keep combo registration unchanged and use the same apply/rollback path so saving a broken modifier-hold mode restores the previous active trigger.

**Tech Stack:** Rust/Tauri v2, `tauri-plugin-global-shortcut`, `core-graphics` 0.25 event taps, CoreFoundation run loop, React/TypeScript, Vitest.

---

## Scope

Implement only R5.2 from `docs/superpowers/specs/2026-06-24-r5-shortcut-customization-design.md`:

- Add shortcut mode `modifier_hold` alongside existing `combo`.
- Add selectable physical modifier keys:
  - Left Command
  - Right Command
  - Left Option
  - Right Option
  - Left Control
  - Right Control
  - Left Shift
  - Right Shift
- Default app behavior remains combo `⌘ ⇧ Space` on fresh/missing settings.
- When user chooses modifier hold, default hold key is `right_command`, threshold `200ms`.
- Holding the selected modifier alone past the threshold emits `Pressed`.
- Releasing after active emits `Released`.
- Pressing another key or adding another modifier before threshold cancels until the selected modifier is released.
- Pressing another key or adding another modifier while active emits `Released` and cancels until release.
- Add watchdog release to avoid stuck recording if a modifier-release `flagsChanged` event is missed.
- Settings and recorder labels show `Hold Right ⌘`, etc.
- Modifier-hold mode may be saved without Accessibility, but Settings must explain it requires Accessibility to work.

## Out of scope

- No arbitrary single-letter/key hold.
- No Fn key support.
- No active event tap, suppression, swallowing, or rewriting keystrokes.
- No ASR, cleanup, Pipecat, release workflow, or asset manifest changes.
- No changes to the frontend recording event contract: keep `wispergo://record-shortcut` payloads exactly `Pressed` and `Released`.

## File responsibility map

- `apps/desktop/src-tauri/Cargo.toml`
  - Add direct `core-graphics = "0.25"` dependency for event tap types already present transitively.
- `apps/desktop/src-tauri/src/shortcut.rs`
  - Extend persistent model with `ShortcutMode::ModifierHold`, `ModifierHoldSettings`, `ModifierHoldKey`.
  - Own display labels and normalization.
  - Add pure `ModifierHoldStateMachine` and unit tests.
  - Extend fakeable shortcut backend/apply rollback helper to support combo and modifier-hold modes.
- `apps/desktop/src-tauri/src/modifier_hold.rs`
  - New macOS runtime monitor using listen-only CoreGraphics event tap.
  - New non-macOS stub that returns an explanatory error if accidentally activated.
  - Translate CoreGraphics event flags into physical modifier state and drive the pure state machine.
- `apps/desktop/src-tauri/src/lib.rs`
  - Manage an optional modifier monitor handle in `ActiveShortcutState`.
  - Extend `TauriShortcutRegistry` backend implementation to start/stop modifier-hold monitor.
  - Stop monitor on app exit.
- `apps/desktop/src-tauri/src/commands/settings.rs`
  - Persist extended shortcut settings with backward compatibility.
  - Existing commands should continue to work with the extended model.
- `apps/desktop/src/types/pipeline.ts`
  - Add `modifier_hold` mode and modifier-hold types.
- `apps/desktop/src/features/settings/SettingsPanel.tsx`
  - Add mode selector and modifier-hold key selector.
  - Show Accessibility helper/warning for modifier-hold mode.
  - Keep combo UI unchanged when combo mode is selected.
- `apps/desktop/src/features/settings/SettingsPanel.test.tsx`
  - Add mode selection, save payload, label, and Accessibility warning coverage.
- `apps/desktop/src/app/App.tsx` / `apps/desktop/src/app/App.test.tsx`
  - Mostly unchanged except type defaults; add/update tests for loaded modifier-hold label in Settings/recorder.
- `apps/desktop/src/features/recorder/FloatingRecorder.tsx`
  - No behavior change expected; existing `shortcutLabel` prop should work.
- `apps/desktop/src/styles.css`
  - Compact styles for segmented mode selector and modifier-hold controls.
- `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
  - Mark R5.2 implemented after verification, not before.
- `HANDOFF.md`
  - Record branch, implementation status, verification, and next gate.

---

## Task 1: Extend shortcut settings model for modifier-hold mode

**Files:**
- Modify: `apps/desktop/src-tauri/src/shortcut.rs`

- [ ] **Step 1: Add failing model tests**

Append these tests inside the existing `#[cfg(test)] mod tests` in `shortcut.rs`:

```rust
#[test]
fn modifier_hold_settings_label_uses_physical_key_name() {
    let settings = ShortcutSettings {
        mode: ShortcutMode::ModifierHold,
        combo: ShortcutCombo::default(),
        modifier_hold: ModifierHoldSettings {
            key: ModifierHoldKey::RightCommand,
            hold_threshold_ms: DEFAULT_MODIFIER_HOLD_THRESHOLD_MS,
        },
    };

    assert_eq!(settings.display_label(), "Hold Right ⌘");
    assert_eq!(settings.to_frontend().display_label, "Hold Right ⌘");
}

#[test]
fn modifier_hold_settings_serialize_as_snake_case_mode_and_key() {
    let settings = ShortcutSettings {
        mode: ShortcutMode::ModifierHold,
        combo: ShortcutCombo::default(),
        modifier_hold: ModifierHoldSettings {
            key: ModifierHoldKey::LeftOption,
            hold_threshold_ms: 200,
        },
    };

    let json = serde_json::to_string(&settings).expect("serialize shortcut settings");
    assert!(json.contains("\"mode\":\"modifier_hold\""));
    assert!(json.contains("\"key\":\"left_option\""));
    assert!(json.contains("\"holdThresholdMs\":200"));

    let parsed = serde_json::from_str::<ShortcutSettings>(&json).expect("deserialize settings");
    assert_eq!(parsed, settings);
}

#[test]
fn missing_modifier_hold_fields_default_to_right_command_threshold() {
    let settings = serde_json::from_str::<ShortcutSettings>(
        r#"{"mode":"modifier_hold","modifierHold":{}}"#,
    )
    .expect("deserialize modifier hold defaults");

    assert_eq!(settings.mode, ShortcutMode::ModifierHold);
    assert_eq!(settings.modifier_hold.key, ModifierHoldKey::RightCommand);
    assert_eq!(
        settings.modifier_hold.hold_threshold_ms,
        DEFAULT_MODIFIER_HOLD_THRESHOLD_MS
    );
}

#[test]
fn invalid_modifier_hold_threshold_normalizes_to_default_threshold() {
    let settings = ShortcutSettings {
        mode: ShortcutMode::ModifierHold,
        combo: ShortcutCombo::default(),
        modifier_hold: ModifierHoldSettings {
            key: ModifierHoldKey::LeftCommand,
            hold_threshold_ms: 0,
        },
    }
    .normalized();

    assert_eq!(settings.mode, ShortcutMode::ModifierHold);
    assert_eq!(settings.modifier_hold.key, ModifierHoldKey::LeftCommand);
    assert_eq!(
        settings.modifier_hold.hold_threshold_ms,
        DEFAULT_MODIFIER_HOLD_THRESHOLD_MS
    );
}

#[test]
fn unknown_shortcut_mode_deserializes_to_default_combo() {
    let settings = serde_json::from_str::<ShortcutSettings>(
        r#"{"mode":"future_mode","modifierHold":{"key":"right_command"}}"#,
    )
    .expect("deserialize unknown mode");

    assert_eq!(settings.normalized(), ShortcutSettings::default());
}
```

- [ ] **Step 2: Run model tests and verify failure**

Run:

```bash
cargo test -p wispergo-desktop shortcut::tests::modifier_hold -- --nocapture
```

Expected: compile failures for missing `ModifierHoldSettings`, `ModifierHoldKey`, `DEFAULT_MODIFIER_HOLD_THRESHOLD_MS`, and `ShortcutMode::ModifierHold`.

- [ ] **Step 3: Implement the model extension**

Update the top of `shortcut.rs` as follows. Preserve existing combo structs and key mappings; add only the new fields/types and update the shown impls.

```rust
pub const DEFAULT_MODIFIER_HOLD_THRESHOLD_MS: u64 = 200;

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutSettings {
    #[serde(default)]
    pub mode: ShortcutMode,
    #[serde(default)]
    pub combo: ShortcutCombo,
    #[serde(default)]
    pub modifier_hold: ModifierHoldSettings,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutMode {
    #[default]
    Combo,
    ModifierHold,
}

impl<'de> serde::Deserialize<'de> for ShortcutMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <Option<String> as serde::Deserialize>::deserialize(deserializer)?;
        Ok(match value.unwrap_or_default().as_str() {
            "combo" => Self::Combo,
            "modifier_hold" => Self::ModifierHold,
            _ => Self::Combo,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifierHoldSettings {
    #[serde(default)]
    pub key: ModifierHoldKey,
    #[serde(default = "default_modifier_hold_threshold_ms")]
    pub hold_threshold_ms: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModifierHoldKey {
    LeftCommand,
    #[default]
    RightCommand,
    LeftOption,
    RightOption,
    LeftControl,
    RightControl,
    LeftShift,
    RightShift,
}

impl<'de> serde::Deserialize<'de> for ModifierHoldKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <Option<String> as serde::Deserialize>::deserialize(deserializer)?;
        Ok(match value.unwrap_or_default().as_str() {
            "left_command" => Self::LeftCommand,
            "right_command" => Self::RightCommand,
            "left_option" => Self::LeftOption,
            "right_option" => Self::RightOption,
            "left_control" => Self::LeftControl,
            "right_control" => Self::RightControl,
            "left_shift" => Self::LeftShift,
            "right_shift" => Self::RightShift,
            _ => Self::RightCommand,
        })
    }
}

fn default_modifier_hold_threshold_ms() -> u64 {
    DEFAULT_MODIFIER_HOLD_THRESHOLD_MS
}
```

Add defaults and display labels:

```rust
impl Default for ModifierHoldSettings {
    fn default() -> Self {
        Self {
            key: ModifierHoldKey::RightCommand,
            hold_threshold_ms: DEFAULT_MODIFIER_HOLD_THRESHOLD_MS,
        }
    }
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            mode: ShortcutMode::Combo,
            combo: ShortcutCombo::default(),
            modifier_hold: ModifierHoldSettings::default(),
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
            ShortcutMode::ModifierHold => {
                let mut settings = self;
                if settings.modifier_hold.hold_threshold_ms == 0 {
                    settings.modifier_hold.hold_threshold_ms = DEFAULT_MODIFIER_HOLD_THRESHOLD_MS;
                }
                settings
            }
        }
    }

    pub fn display_label(&self) -> String {
        match self.mode {
            ShortcutMode::Combo => self.combo.display_label(),
            ShortcutMode::ModifierHold => self.modifier_hold.display_label(),
        }
    }

    pub fn to_tauri_shortcut(&self) -> Result<Shortcut, String> {
        match self.mode {
            ShortcutMode::Combo => self.combo.to_tauri_shortcut(),
            ShortcutMode::ModifierHold => Err(
                "Modifier-hold shortcuts are monitored instead of registered as key combinations."
                    .to_string(),
            ),
        }
    }
}

impl ModifierHoldSettings {
    pub fn display_label(&self) -> String {
        format!("Hold {}", self.key.label())
    }
}

impl ModifierHoldKey {
    pub fn label(self) -> &'static str {
        match self {
            Self::LeftCommand => "Left ⌘",
            Self::RightCommand => "Right ⌘",
            Self::LeftOption => "Left ⌥",
            Self::RightOption => "Right ⌥",
            Self::LeftControl => "Left ⌃",
            Self::RightControl => "Right ⌃",
            Self::LeftShift => "Left ⇧",
            Self::RightShift => "Right ⇧",
        }
    }
}
```

- [ ] **Step 4: Update existing struct literals in tests**

Every test-created `ShortcutSettings { mode, combo }` must include:

```rust
modifier_hold: ModifierHoldSettings::default(),
```

Do not change app defaults. Missing persisted settings still default to combo `⌘ ⇧ Space`.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p wispergo-desktop shortcut::tests -- --nocapture
```

Expected: all shortcut tests pass.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/src/shortcut.rs
git commit -m "feat(desktop): add modifier hold shortcut model"
```

---

## Task 2: Add pure modifier-hold state machine

**Files:**
- Modify: `apps/desktop/src-tauri/src/shortcut.rs`

- [ ] **Step 1: Add failing state-machine tests**

Append these tests to `shortcut.rs`:

```rust
fn right_command_hold_settings() -> ModifierHoldSettings {
    ModifierHoldSettings {
        key: ModifierHoldKey::RightCommand,
        hold_threshold_ms: 200,
    }
}

#[test]
fn modifier_hold_tap_does_not_start() {
    let mut machine = ModifierHoldStateMachine::new(right_command_hold_settings());

    assert_eq!(
        machine.handle_event(ModifierHoldInput::SelectedModifierDown),
        vec![ModifierHoldAction::ScheduleThreshold { generation: 1, delay_ms: 200 }]
    );
    assert_eq!(
        machine.handle_event(ModifierHoldInput::SelectedModifierUp),
        Vec::<ModifierHoldAction>::new()
    );
}

#[test]
fn modifier_hold_threshold_starts_and_release_stops() {
    let mut machine = ModifierHoldStateMachine::new(right_command_hold_settings());

    assert_eq!(
        machine.handle_event(ModifierHoldInput::SelectedModifierDown),
        vec![ModifierHoldAction::ScheduleThreshold { generation: 1, delay_ms: 200 }]
    );
    assert_eq!(
        machine.handle_event(ModifierHoldInput::ThresholdElapsed { generation: 1 }),
        vec![
            ModifierHoldAction::EmitPressed,
            ModifierHoldAction::ScheduleWatchdog { generation: 1, delay_ms: 30_000 },
        ]
    );
    assert_eq!(
        machine.handle_event(ModifierHoldInput::SelectedModifierUp),
        vec![ModifierHoldAction::EmitReleased]
    );
}

#[test]
fn modifier_hold_other_key_before_threshold_cancels_until_release() {
    let mut machine = ModifierHoldStateMachine::new(right_command_hold_settings());

    let _ = machine.handle_event(ModifierHoldInput::SelectedModifierDown);
    assert_eq!(
        machine.handle_event(ModifierHoldInput::OtherKeyDown),
        Vec::<ModifierHoldAction>::new()
    );
    assert_eq!(
        machine.handle_event(ModifierHoldInput::ThresholdElapsed { generation: 1 }),
        Vec::<ModifierHoldAction>::new()
    );
    assert_eq!(
        machine.handle_event(ModifierHoldInput::SelectedModifierUp),
        Vec::<ModifierHoldAction>::new()
    );
}

#[test]
fn modifier_hold_extra_modifier_before_threshold_cancels_until_release() {
    let mut machine = ModifierHoldStateMachine::new(right_command_hold_settings());

    let _ = machine.handle_event(ModifierHoldInput::SelectedModifierDown);
    assert_eq!(
        machine.handle_event(ModifierHoldInput::OtherModifierJoined),
        Vec::<ModifierHoldAction>::new()
    );
    assert_eq!(
        machine.handle_event(ModifierHoldInput::ThresholdElapsed { generation: 1 }),
        Vec::<ModifierHoldAction>::new()
    );
    assert_eq!(
        machine.handle_event(ModifierHoldInput::SelectedModifierUp),
        Vec::<ModifierHoldAction>::new()
    );
}

#[test]
fn modifier_hold_other_key_while_active_releases_and_cancels() {
    let mut machine = ModifierHoldStateMachine::new(right_command_hold_settings());

    let _ = machine.handle_event(ModifierHoldInput::SelectedModifierDown);
    let _ = machine.handle_event(ModifierHoldInput::ThresholdElapsed { generation: 1 });
    assert_eq!(
        machine.handle_event(ModifierHoldInput::OtherKeyDown),
        vec![ModifierHoldAction::EmitReleased]
    );
    assert_eq!(
        machine.handle_event(ModifierHoldInput::SelectedModifierUp),
        Vec::<ModifierHoldAction>::new()
    );
}

#[test]
fn modifier_hold_watchdog_releases_stuck_active_recording() {
    let mut machine = ModifierHoldStateMachine::new(right_command_hold_settings());

    let _ = machine.handle_event(ModifierHoldInput::SelectedModifierDown);
    let _ = machine.handle_event(ModifierHoldInput::ThresholdElapsed { generation: 1 });
    assert_eq!(
        machine.handle_event(ModifierHoldInput::WatchdogElapsed { generation: 1 }),
        vec![ModifierHoldAction::EmitReleased]
    );
}

#[test]
fn modifier_hold_stays_cancelled_when_other_modifier_is_released_first() {
    let mut machine = ModifierHoldStateMachine::new(right_command_hold_settings());

    let _ = machine.handle_event(ModifierHoldInput::SelectedModifierDown);
    let _ = machine.handle_event(ModifierHoldInput::OtherModifierJoined);
    assert_eq!(
        machine.handle_event(ModifierHoldInput::SelectedModifierDown),
        Vec::<ModifierHoldAction>::new()
    );
    assert_eq!(
        machine.handle_event(ModifierHoldInput::ThresholdElapsed { generation: 1 }),
        Vec::<ModifierHoldAction>::new()
    );
    assert_eq!(
        machine.handle_event(ModifierHoldInput::SelectedModifierUp),
        Vec::<ModifierHoldAction>::new()
    );
}

#[test]
fn modifier_hold_ignores_stale_threshold_generation() {
    let mut machine = ModifierHoldStateMachine::new(right_command_hold_settings());

    let _ = machine.handle_event(ModifierHoldInput::SelectedModifierDown);
    let _ = machine.handle_event(ModifierHoldInput::SelectedModifierUp);
    let _ = machine.handle_event(ModifierHoldInput::SelectedModifierDown);

    assert_eq!(
        machine.handle_event(ModifierHoldInput::ThresholdElapsed { generation: 1 }),
        Vec::<ModifierHoldAction>::new()
    );
    assert_eq!(
        machine.handle_event(ModifierHoldInput::ThresholdElapsed { generation: 2 }),
        vec![
            ModifierHoldAction::EmitPressed,
            ModifierHoldAction::ScheduleWatchdog { generation: 2, delay_ms: 30_000 },
        ]
    );
}
```

- [ ] **Step 2: Run state-machine tests and verify failure**

Run:

```bash
cargo test -p wispergo-desktop shortcut::tests::modifier_hold_ -- --nocapture
```

Expected: compile failures for missing `ModifierHoldStateMachine`, `ModifierHoldInput`, and `ModifierHoldAction`.

- [ ] **Step 3: Implement the pure state machine**

Add to `shortcut.rs` outside the test module:

```rust
pub const MODIFIER_HOLD_WATCHDOG_MS: u64 = 30_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierHoldInput {
    SelectedModifierDown,
    SelectedModifierUp,
    OtherModifierJoined,
    OtherKeyDown,
    ThresholdElapsed { generation: u64 },
    WatchdogElapsed { generation: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierHoldAction {
    ScheduleThreshold { generation: u64, delay_ms: u64 },
    ScheduleWatchdog { generation: u64, delay_ms: u64 },
    EmitPressed,
    EmitReleased,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModifierHoldPhase {
    Idle,
    Pending { generation: u64 },
    Active { generation: u64 },
    CancelledUntilRelease,
}

#[derive(Debug)]
pub struct ModifierHoldStateMachine {
    settings: ModifierHoldSettings,
    phase: ModifierHoldPhase,
    generation: u64,
}

impl ModifierHoldStateMachine {
    pub fn new(settings: ModifierHoldSettings) -> Self {
        Self {
            settings,
            phase: ModifierHoldPhase::Idle,
            generation: 0,
        }
    }

    pub fn handle_event(&mut self, input: ModifierHoldInput) -> Vec<ModifierHoldAction> {
        match (self.phase, input) {
            (ModifierHoldPhase::Idle, ModifierHoldInput::SelectedModifierDown) => {
                self.generation = self.generation.saturating_add(1);
                let generation = self.generation;
                self.phase = ModifierHoldPhase::Pending { generation };
                vec![ModifierHoldAction::ScheduleThreshold {
                    generation,
                    delay_ms: self.settings.hold_threshold_ms,
                }]
            }
            (ModifierHoldPhase::Pending { .. }, ModifierHoldInput::SelectedModifierUp) => {
                self.phase = ModifierHoldPhase::Idle;
                Vec::new()
            }
            (
                ModifierHoldPhase::Pending { .. },
                ModifierHoldInput::OtherKeyDown | ModifierHoldInput::OtherModifierJoined,
            ) => {
                self.phase = ModifierHoldPhase::CancelledUntilRelease;
                Vec::new()
            }
            (
                ModifierHoldPhase::Pending { generation },
                ModifierHoldInput::ThresholdElapsed { generation: elapsed_generation },
            ) if generation == elapsed_generation => {
                self.phase = ModifierHoldPhase::Active { generation };
                vec![
                    ModifierHoldAction::EmitPressed,
                    ModifierHoldAction::ScheduleWatchdog {
                        generation,
                        delay_ms: MODIFIER_HOLD_WATCHDOG_MS,
                    },
                ]
            }
            (ModifierHoldPhase::Active { .. }, ModifierHoldInput::SelectedModifierUp) => {
                self.phase = ModifierHoldPhase::Idle;
                vec![ModifierHoldAction::EmitReleased]
            }
            (
                ModifierHoldPhase::Active { .. },
                ModifierHoldInput::OtherKeyDown | ModifierHoldInput::OtherModifierJoined,
            ) => {
                self.phase = ModifierHoldPhase::CancelledUntilRelease;
                vec![ModifierHoldAction::EmitReleased]
            }
            (
                ModifierHoldPhase::Active { generation },
                ModifierHoldInput::WatchdogElapsed { generation: elapsed_generation },
            ) if generation == elapsed_generation => {
                self.phase = ModifierHoldPhase::Idle;
                vec![ModifierHoldAction::EmitReleased]
            }
            (ModifierHoldPhase::CancelledUntilRelease, ModifierHoldInput::SelectedModifierUp) => {
                self.phase = ModifierHoldPhase::Idle;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
}
```

- [ ] **Step 4: Run state-machine tests**

Run:

```bash
cargo test -p wispergo-desktop shortcut::tests::modifier_hold_ -- --nocapture
```

Expected: all modifier-hold state-machine tests pass.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/shortcut.rs
git commit -m "feat(desktop): add modifier hold state machine"
```

---

## Task 3: Extend shortcut apply/rollback backend for both modes

**Files:**
- Modify: `apps/desktop/src-tauri/src/shortcut.rs`
- Later consumed by: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add failing backend tests**

Update the existing fake registry in `shortcut.rs` tests to record combo and modifier actions. Add these tests:

```rust
#[test]
fn apply_shortcut_starts_modifier_hold_when_switching_from_combo() {
    let previous = ShortcutSettings::default();
    let next = ShortcutSettings {
        mode: ShortcutMode::ModifierHold,
        combo: ShortcutCombo::default(),
        modifier_hold: ModifierHoldSettings {
            key: ModifierHoldKey::RightCommand,
            hold_threshold_ms: 200,
        },
    };
    let mut active = Some(previous.clone());
    let mut registry = FakeShortcutRegistry::default();

    let view = apply_shortcut_settings(&mut registry, &mut active, next.clone())
        .expect("apply modifier hold");

    assert_eq!(active, Some(next.clone()));
    assert_eq!(view.display_label, "Hold Right ⌘");
    assert_eq!(
        registry.calls,
        vec![
            "unregister_combo:⌘ ⇧ Space".to_string(),
            "start_modifier_hold:Hold Right ⌘".to_string(),
        ]
    );
}

#[test]
fn apply_shortcut_rolls_back_to_combo_when_modifier_hold_start_fails() {
    let previous = ShortcutSettings::default();
    let next = ShortcutSettings {
        mode: ShortcutMode::ModifierHold,
        combo: ShortcutCombo::default(),
        modifier_hold: ModifierHoldSettings {
            key: ModifierHoldKey::RightCommand,
            hold_threshold_ms: 200,
        },
    };
    let mut active = Some(previous.clone());
    let mut registry = FakeShortcutRegistry {
        fail_next_modifier_hold_start: Some("Accessibility permission is required".to_string()),
        ..FakeShortcutRegistry::default()
    };

    let error = apply_shortcut_settings(&mut registry, &mut active, next)
        .expect_err("modifier-hold start should fail");

    assert!(error.contains("Accessibility permission is required"));
    assert_eq!(active, Some(previous));
    assert_eq!(
        registry.calls,
        vec![
            "unregister_combo:⌘ ⇧ Space".to_string(),
            "start_modifier_hold:Hold Right ⌘".to_string(),
            "register_combo:⌘ ⇧ Space".to_string(),
        ]
    );
}

#[test]
fn apply_shortcut_stops_modifier_hold_when_switching_back_to_combo() {
    let previous = ShortcutSettings {
        mode: ShortcutMode::ModifierHold,
        combo: ShortcutCombo::default(),
        modifier_hold: ModifierHoldSettings {
            key: ModifierHoldKey::RightCommand,
            hold_threshold_ms: 200,
        },
    };
    let next = ShortcutSettings::default();
    let mut active = Some(previous.clone());
    let mut registry = FakeShortcutRegistry::default();

    let view = apply_shortcut_settings(&mut registry, &mut active, next.clone())
        .expect("switch back to combo");

    assert_eq!(active, Some(next));
    assert_eq!(view.display_label, "⌘ ⇧ Space");
    assert_eq!(
        registry.calls,
        vec![
            "stop_modifier_hold:Hold Right ⌘".to_string(),
            "register_combo:⌘ ⇧ Space".to_string(),
        ]
    );
}

#[test]
fn apply_shortcut_rolls_back_to_modifier_hold_when_combo_registration_fails() {
    let previous = ShortcutSettings {
        mode: ShortcutMode::ModifierHold,
        combo: ShortcutCombo::default(),
        modifier_hold: ModifierHoldSettings {
            key: ModifierHoldKey::RightCommand,
            hold_threshold_ms: 200,
        },
    };
    let next = ShortcutSettings::default();
    let mut active = Some(previous.clone());
    let mut registry = FakeShortcutRegistry {
        fail_next_combo_register: Some("shortcut already registered".to_string()),
        ..FakeShortcutRegistry::default()
    };

    let error = apply_shortcut_settings(&mut registry, &mut active, next)
        .expect_err("combo registration should fail");

    assert!(error.contains("shortcut already registered"));
    assert_eq!(active, Some(previous));
    assert_eq!(
        registry.calls,
        vec![
            "stop_modifier_hold:Hold Right ⌘".to_string(),
            "register_combo:⌘ ⇧ Space".to_string(),
            "start_modifier_hold:Hold Right ⌘".to_string(),
        ]
    );
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p wispergo-desktop shortcut::tests::apply_shortcut_ -- --nocapture
```

Expected: compile failures because `ShortcutRegistry` lacks modifier-hold methods.

- [ ] **Step 3: Extend `ShortcutRegistry` trait**

Replace the trait with:

```rust
pub trait ShortcutRegistry {
    fn register_combo(&mut self, settings: &ShortcutSettings) -> Result<(), String>;
    fn unregister_combo(&mut self, settings: &ShortcutSettings) -> Result<(), String>;
    fn start_modifier_hold(&mut self, settings: &ModifierHoldSettings) -> Result<(), String>;
    fn stop_modifier_hold(&mut self, settings: &ModifierHoldSettings) -> Result<(), String>;
}
```

Update all existing call sites from `register` / `unregister` to `register_combo` / `unregister_combo`.

- [ ] **Step 4: Update `apply_shortcut_settings`**

Replace the body with this mode-aware version:

```rust
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
        deactivate_shortcut(registry, previous_settings)?;
    }

    if let Err(activate_error) = activate_shortcut(registry, &next) {
        if let Some(previous_settings) = previous.as_ref() {
            if let Err(rollback_error) = activate_shortcut(registry, previous_settings) {
                return Err(format!(
                    "Shortcut could not be changed: {activate_error}. The previous shortcut could not be restored: {rollback_error}"
                ));
            }
            *active = Some(previous_settings.clone());
        } else {
            *active = None;
        }
        return Err(format!("Shortcut could not be changed: {activate_error}"));
    }

    *active = Some(next.clone());
    Ok(next.to_frontend())
}

fn activate_shortcut<R: ShortcutRegistry>(
    registry: &mut R,
    settings: &ShortcutSettings,
) -> Result<(), String> {
    match settings.mode {
        ShortcutMode::Combo => registry.register_combo(settings),
        ShortcutMode::ModifierHold => registry.start_modifier_hold(&settings.modifier_hold),
    }
}

fn deactivate_shortcut<R: ShortcutRegistry>(
    registry: &mut R,
    settings: &ShortcutSettings,
) -> Result<(), String> {
    match settings.mode {
        ShortcutMode::Combo => registry.unregister_combo(settings),
        ShortcutMode::ModifierHold => registry.stop_modifier_hold(&settings.modifier_hold),
    }
}
```

- [ ] **Step 5: Update fake registry**

In the `FakeShortcutRegistry` test helper, add fields and impl methods:

```rust
#[derive(Default)]
struct FakeShortcutRegistry {
    calls: Vec<String>,
    fail_next_combo_register: Option<String>,
    fail_next_modifier_hold_start: Option<String>,
}

impl ShortcutRegistry for FakeShortcutRegistry {
    fn register_combo(&mut self, settings: &ShortcutSettings) -> Result<(), String> {
        self.calls.push(format!("register_combo:{}", settings.display_label()));
        if let Some(error) = self.fail_next_combo_register.take() {
            return Err(error);
        }
        Ok(())
    }

    fn unregister_combo(&mut self, settings: &ShortcutSettings) -> Result<(), String> {
        self.calls.push(format!("unregister_combo:{}", settings.display_label()));
        Ok(())
    }

    fn start_modifier_hold(&mut self, settings: &ModifierHoldSettings) -> Result<(), String> {
        self.calls.push(format!("start_modifier_hold:{}", settings.display_label()));
        if let Some(error) = self.fail_next_modifier_hold_start.take() {
            return Err(error);
        }
        Ok(())
    }

    fn stop_modifier_hold(&mut self, settings: &ModifierHoldSettings) -> Result<(), String> {
        self.calls.push(format!("stop_modifier_hold:{}", settings.display_label()));
        Ok(())
    }
}
```

Update old expected call strings in existing tests from `register:` / `unregister:` to `register_combo:` / `unregister_combo:`.

- [ ] **Step 6: Run shortcut tests**

Run:

```bash
cargo test -p wispergo-desktop shortcut::tests -- --nocapture
```

Expected: all shortcut tests pass.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src-tauri/src/shortcut.rs
git commit -m "feat(desktop): support modifier hold shortcut apply"
```

---

## Task 4: Add macOS listen-only modifier-hold monitor

**Files:**
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Create: `apps/desktop/src-tauri/src/modifier_hold.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add direct dependency**

In `apps/desktop/src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
core-graphics = "0.25"
```

Do not run `pnpm install`; this is a Rust dependency already present transitively but must be direct before importing it.

- [ ] **Step 2: Verify the installed `core-graphics` API before writing monitor code**

Run:

```bash
rg -n "pub fn new<|pub fn create_runloop_source|pub enum CallbackResult|pub fn get_flags" \
  ~/.cargo/registry/src/index.crates.io-*/core-graphics-0.25.0/src/event.rs \
  ~/.cargo/registry/src/index.crates.io-*/core-foundation-0.10.1/src/mach_port.rs
```

Expected for the current lockfile, verified locally during planning against `core-graphics-0.25.0` and `core-foundation-0.10.1`:

- `CGEventTap::new` accepts `Vec<CGEventType>` and a `Send + 'static` closure returning `CallbackResult`.
- `CallbackResult::Keep` exists.
- `mach_port().create_runloop_source(0)` returns `Result<CFRunLoopSource, ()>`.
- `CGEvent::get_flags()` exists and returns flags with `.bits()`.
- `CFRunLoop::run_in_mode(mode, Duration, bool)` exists and returns `CFRunLoopRunResult::{Finished, Stopped, TimedOut, HandledSource}`.

If the local crate source differs during implementation, update the monitor code to match the installed API while preserving the non-negotiable semantics: listen-only tap, no dropped/replaced events, `FlagsChanged` + `KeyDown` only, physical device flag masks, cooperative bounded shutdown, and a `cargo build -p wispergo-desktop` gate before commit.

- [ ] **Step 3: Create testable flag mapping tests**

Create `apps/desktop/src-tauri/src/modifier_hold.rs` with the tests first:

```rust
use crate::shortcut::ModifierHoldKey;

const NX_DEVICELCTLKEYMASK: u64 = 0x0000_0001;
const NX_DEVICELSHIFTKEYMASK: u64 = 0x0000_0002;
const NX_DEVICERSHIFTKEYMASK: u64 = 0x0000_0004;
const NX_DEVICELCMDKEYMASK: u64 = 0x0000_0008;
const NX_DEVICERCMDKEYMASK: u64 = 0x0000_0010;
const NX_DEVICELALTKEYMASK: u64 = 0x0000_0020;
const NX_DEVICERALTKEYMASK: u64 = 0x0000_0040;
const NX_DEVICERCTLKEYMASK: u64 = 0x0000_2000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_modifier_detects_right_command_from_device_flag() {
        assert!(modifier_is_down(ModifierHoldKey::RightCommand, NX_DEVICERCMDKEYMASK));
        assert!(!modifier_is_down(ModifierHoldKey::RightCommand, NX_DEVICELCMDKEYMASK));
    }

    #[test]
    fn selected_modifier_detects_all_supported_physical_keys() {
        assert!(modifier_is_down(ModifierHoldKey::LeftCommand, NX_DEVICELCMDKEYMASK));
        assert!(modifier_is_down(ModifierHoldKey::RightCommand, NX_DEVICERCMDKEYMASK));
        assert!(modifier_is_down(ModifierHoldKey::LeftOption, NX_DEVICELALTKEYMASK));
        assert!(modifier_is_down(ModifierHoldKey::RightOption, NX_DEVICERALTKEYMASK));
        assert!(modifier_is_down(ModifierHoldKey::LeftControl, NX_DEVICELCTLKEYMASK));
        assert!(modifier_is_down(ModifierHoldKey::RightControl, NX_DEVICERCTLKEYMASK));
        assert!(modifier_is_down(ModifierHoldKey::LeftShift, NX_DEVICELSHIFTKEYMASK));
        assert!(modifier_is_down(ModifierHoldKey::RightShift, NX_DEVICERSHIFTKEYMASK));
    }

    #[test]
    fn only_selected_modifier_detects_no_other_modifier() {
        assert!(!has_other_modifier(ModifierHoldKey::RightCommand, NX_DEVICERCMDKEYMASK));
        assert!(has_other_modifier(
            ModifierHoldKey::RightCommand,
            NX_DEVICERCMDKEYMASK | NX_DEVICELSHIFTKEYMASK,
        ));
    }

    #[test]
    fn selected_modifier_reasserted_without_other_modifier_is_not_other_modifier() {
        let flags = NX_DEVICERCMDKEYMASK;

        assert!(modifier_is_down(ModifierHoldKey::RightCommand, flags));
        assert!(!has_other_modifier(ModifierHoldKey::RightCommand, flags));
    }

    #[test]
    fn monitor_source_stays_listen_only_and_never_drops_events() {
        let source = include_str!("modifier_hold.rs");
        let drop_variant = ["CallbackResult", "::", "Drop"].concat();
        let replace_variant = ["CallbackResult", "::", "Replace"].concat();

        assert!(source.contains("CGEventTapOptions::ListenOnly"));
        assert!(source.contains("CallbackResult::Keep"));
        assert!(!source.contains(&drop_variant));
        assert!(!source.contains(&replace_variant));
    }
}
```

- [ ] **Step 4: Add flag mapping helpers**

Below the constants in `modifier_hold.rs`, add:

```rust
fn modifier_mask(key: ModifierHoldKey) -> u64 {
    match key {
        ModifierHoldKey::LeftCommand => NX_DEVICELCMDKEYMASK,
        ModifierHoldKey::RightCommand => NX_DEVICERCMDKEYMASK,
        ModifierHoldKey::LeftOption => NX_DEVICELALTKEYMASK,
        ModifierHoldKey::RightOption => NX_DEVICERALTKEYMASK,
        ModifierHoldKey::LeftControl => NX_DEVICELCTLKEYMASK,
        ModifierHoldKey::RightControl => NX_DEVICERCTLKEYMASK,
        ModifierHoldKey::LeftShift => NX_DEVICELSHIFTKEYMASK,
        ModifierHoldKey::RightShift => NX_DEVICERSHIFTKEYMASK,
    }
}

fn all_supported_modifier_masks() -> u64 {
    NX_DEVICELCMDKEYMASK
        | NX_DEVICERCMDKEYMASK
        | NX_DEVICELALTKEYMASK
        | NX_DEVICERALTKEYMASK
        | NX_DEVICELCTLKEYMASK
        | NX_DEVICERCTLKEYMASK
        | NX_DEVICELSHIFTKEYMASK
        | NX_DEVICERSHIFTKEYMASK
}

fn modifier_is_down(key: ModifierHoldKey, flags: u64) -> bool {
    flags & modifier_mask(key) != 0
}

fn has_other_modifier(key: ModifierHoldKey, flags: u64) -> bool {
    flags & (all_supported_modifier_masks() & !modifier_mask(key)) != 0
}
```

- [ ] **Step 5: Add a single timer worker, not per-event timer threads**

Still in `modifier_hold.rs`, add a timer command helper used by the monitor runtime. One worker thread owns all sleeps, so rapid modifier taps do not create a long-lived thread per input event.

```rust
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::shortcut::{
    ModifierHoldAction, ModifierHoldInput, ModifierHoldSettings, ModifierHoldStateMachine,
    RECORD_SHORTCUT_EVENT,
};

enum TimerCommand {
    ScheduleThreshold { generation: u64, delay_ms: u64 },
    ScheduleWatchdog { generation: u64, delay_ms: u64 },
    Stop,
}

struct ScheduledTimer {
    deadline: Instant,
    input: ModifierHoldInput,
}

fn spawn_timer_worker(
    app: AppHandle,
    machine: Arc<Mutex<ModifierHoldStateMachine>>,
) -> (mpsc::Sender<TimerCommand>, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<TimerCommand>();
    let join = thread::Builder::new()
        .name("wispergo-modifier-hold-timer".to_string())
        .spawn(move || {
            let mut timers: Vec<ScheduledTimer> = Vec::new();
            loop {
                timers.sort_by_key(|timer| timer.deadline);
                let timeout = timers
                    .first()
                    .map(|timer| timer.deadline.saturating_duration_since(Instant::now()))
                    .unwrap_or(Duration::from_secs(60));

                match rx.recv_timeout(timeout) {
                    Ok(TimerCommand::ScheduleThreshold { generation, delay_ms }) => {
                        timers.push(ScheduledTimer {
                            deadline: Instant::now() + Duration::from_millis(delay_ms),
                            input: ModifierHoldInput::ThresholdElapsed { generation },
                        });
                    }
                    Ok(TimerCommand::ScheduleWatchdog { generation, delay_ms }) => {
                        timers.push(ScheduledTimer {
                            deadline: Instant::now() + Duration::from_millis(delay_ms),
                            input: ModifierHoldInput::WatchdogElapsed { generation },
                        });
                    }
                    Ok(TimerCommand::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }

                let now = Instant::now();
                let mut pending = Vec::new();
                for timer in timers.drain(..) {
                    if timer.deadline <= now {
                        dispatch_input(&app, &machine, timer.input);
                    } else {
                        pending.push(timer);
                    }
                }
                timers = pending;
            }
        })
        .expect("spawn modifier hold timer worker");
    (tx, join)
}
```

- [ ] **Step 6: Add monitor runtime implementation**

Add the runtime implementation. This uses `CGEventTapOptions::ListenOnly`; never return the event-dropping callback variant. The event tap and CoreFoundation run loop stay on the monitor thread. Shutdown is cooperative: the monitor thread runs the run loop in short intervals and checks a stop channel between intervals, so no raw run-loop pointer is sent across threads and `stop()` joins after at most the polling interval in normal conditions.

```rust
pub struct ModifierHoldMonitor {
    stop: mpsc::Sender<()>,
    stopped: mpsc::Receiver<()>,
    join: Option<thread::JoinHandle<()>>,
}

impl ModifierHoldMonitor {
    #[cfg(target_os = "macos")]
    pub fn start(app: AppHandle, settings: ModifierHoldSettings) -> Result<Self, String> {
        use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoop, CFRunLoopRunResult};
        use core_graphics::event::{
            CallbackResult, CGEventTap, CGEventTapLocation, CGEventTapOptions,
            CGEventTapPlacement, CGEventType,
        };

        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (stopped_tx, stopped_rx) = mpsc::channel::<()>();
        let join = thread::Builder::new()
            .name("wispergo-modifier-hold-monitor".to_string())
            .spawn(move || {
                let run_loop = CFRunLoop::get_current();
                let machine = Arc::new(Mutex::new(ModifierHoldStateMachine::new(settings.clone())));
                let (timer_tx, timer_join) = spawn_timer_worker(app.clone(), Arc::clone(&machine));
                let selected_key = settings.key;
                let app_for_callback = app.clone();
                let machine_for_callback = Arc::clone(&machine);
                let timer_for_callback = timer_tx.clone();

                let tap_result = CGEventTap::new(
                    CGEventTapLocation::Session,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::ListenOnly,
                    vec![CGEventType::FlagsChanged, CGEventType::KeyDown],
                    move |_proxy, event_type, event| {
                        handle_cg_event(
                            &app_for_callback,
                            &machine_for_callback,
                            &timer_for_callback,
                            selected_key,
                            event_type,
                            event,
                        );
                        CallbackResult::Keep
                    },
                );

                let Ok(tap) = tap_result else {
                    let _ = timer_tx.send(TimerCommand::Stop);
                    let _ = timer_join.join();
                    let _ = ready_tx.send(Err(
                        "Modifier-hold shortcuts require macOS Accessibility permission.".to_string(),
                    ));
                    return;
                };

                let source = match tap.mach_port().create_runloop_source(0) {
                    Ok(source) => source,
                    Err(()) => {
                        let _ = timer_tx.send(TimerCommand::Stop);
                        let _ = timer_join.join();
                        let _ = ready_tx.send(Err(
                            "Modifier-hold event monitor could not start.".to_string(),
                        ));
                        return;
                    }
                };

                run_loop.add_source(&source, unsafe { kCFRunLoopDefaultMode });
                tap.enable();
                let _ = ready_tx.send(Ok(()));

                loop {
                    match stop_rx.try_recv() {
                        Ok(()) | Err(mpsc::TryRecvError::Disconnected) => break,
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                    match CFRunLoop::run_in_mode(
                        unsafe { kCFRunLoopDefaultMode },
                        Duration::from_millis(100),
                        true,
                    ) {
                        CFRunLoopRunResult::Stopped | CFRunLoopRunResult::Finished => {}
                        CFRunLoopRunResult::TimedOut | CFRunLoopRunResult::HandledSource => {}
                    }
                }

                let _ = timer_tx.send(TimerCommand::Stop);
                let _ = timer_join.join();
                drop(tap);
                let _ = stopped_tx.send(());
            })
            .map_err(|err| err.to_string())?;

        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| "Modifier-hold event monitor did not start in time.".to_string())??;

        Ok(Self {
            stop: stop_tx,
            stopped: stopped_rx,
            join: Some(join),
        })
    }

    #[cfg(not(target_os = "macos"))]
    pub fn start(_app: AppHandle, _settings: ModifierHoldSettings) -> Result<Self, String> {
        Err("Modifier-hold shortcuts are only supported on macOS.".to_string())
    }

    pub fn stop(mut self) {
        let _ = self.stop.send(());
        if self.stopped.recv_timeout(Duration::from_secs(2)).is_ok() {
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }
}

impl Drop for ModifierHoldMonitor {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}

```

- [ ] **Step 7: Add event handling helpers**

Add below the monitor impl:

```rust
#[cfg(target_os = "macos")]
fn handle_cg_event(
    app: &AppHandle,
    machine: &Arc<Mutex<ModifierHoldStateMachine>>,
    timer: &mpsc::Sender<TimerCommand>,
    selected_key: ModifierHoldKey,
    event_type: core_graphics::event::CGEventType,
    event: &core_graphics::event::CGEvent,
) {
    use core_graphics::event::CGEventType;

    match event_type {
        CGEventType::FlagsChanged => {
            let flags = event.get_flags().bits();
            let input = if modifier_is_down(selected_key, flags) {
                if has_other_modifier(selected_key, flags) {
                    ModifierHoldInput::OtherModifierJoined
                } else {
                    ModifierHoldInput::SelectedModifierDown
                }
            } else {
                ModifierHoldInput::SelectedModifierUp
            };
            dispatch_input_with_timer(app, machine, timer, input);
        }
        CGEventType::KeyDown => {
            dispatch_input_with_timer(app, machine, timer, ModifierHoldInput::OtherKeyDown);
        }
        _ => {}
    }
}

#[cfg(target_os = "macos")]
fn dispatch_input_with_timer(
    app: &AppHandle,
    machine: &Arc<Mutex<ModifierHoldStateMachine>>,
    timer: &mpsc::Sender<TimerCommand>,
    input: ModifierHoldInput,
) {
    let actions = machine
        .lock()
        .expect("modifier hold state lock")
        .handle_event(input);
    run_actions(app, timer, actions);
}

#[cfg(target_os = "macos")]
fn dispatch_input(
    app: &AppHandle,
    machine: &Arc<Mutex<ModifierHoldStateMachine>>,
    input: ModifierHoldInput,
) {
    let actions = machine
        .lock()
        .expect("modifier hold state lock")
        .handle_event(input);
    for action in actions {
        match action {
            ModifierHoldAction::EmitPressed => {
                let _ = app.emit(RECORD_SHORTCUT_EVENT, "Pressed");
            }
            ModifierHoldAction::EmitReleased => {
                let _ = app.emit(RECORD_SHORTCUT_EVENT, "Released");
            }
            ModifierHoldAction::ScheduleThreshold { .. }
            | ModifierHoldAction::ScheduleWatchdog { .. } => {}
        }
    }
}

#[cfg(target_os = "macos")]
fn run_actions(
    app: &AppHandle,
    timer: &mpsc::Sender<TimerCommand>,
    actions: Vec<ModifierHoldAction>,
) {
    for action in actions {
        match action {
            ModifierHoldAction::EmitPressed => {
                let _ = app.emit(RECORD_SHORTCUT_EVENT, "Pressed");
            }
            ModifierHoldAction::EmitReleased => {
                let _ = app.emit(RECORD_SHORTCUT_EVENT, "Released");
            }
            ModifierHoldAction::ScheduleThreshold { generation, delay_ms } => {
                let _ = timer.send(TimerCommand::ScheduleThreshold { generation, delay_ms });
            }
            ModifierHoldAction::ScheduleWatchdog { generation, delay_ms } => {
                let _ = timer.send(TimerCommand::ScheduleWatchdog { generation, delay_ms });
            }
        }
    }
}
```

- [ ] **Step 8: Register module in `lib.rs`**

At the top of `apps/desktop/src-tauri/src/lib.rs`, add:

```rust
mod modifier_hold;
```

- [ ] **Step 9: Compile the monitor, not just unit tests**

Run both commands:

```bash
cargo test -p wispergo-desktop modifier_hold::tests -- --nocapture
cargo build -p wispergo-desktop
```

Expected: flag mapping tests pass and the macOS monitor body compiles. If `core_graphics` or `core_foundation` API names differ, adjust the concrete imports/calls until `cargo build -p wispergo-desktop` passes. Preserve all required semantics: `CGEventTapLocation::Session`, `CGEventTapOptions::ListenOnly`, event types `FlagsChanged` and `KeyDown`, and `CallbackResult::Keep`.

- [ ] **Step 10: Commit**

```bash
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/src/modifier_hold.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): add modifier hold monitor"
```

---

## Task 5: Wire modifier-hold monitor into shortcut runtime

**Files:**
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/shortcut.rs` if needed for visibility/imports

- [ ] **Step 1: Extend active runtime state**

In `lib.rs`, replace:

```rust
#[derive(Default)]
struct ActiveShortcutState(Mutex<Option<ShortcutSettings>>);
```

with:

```rust
#[derive(Default)]
struct ActiveShortcutState(Mutex<ActiveShortcutRuntime>);

#[derive(Default)]
struct ActiveShortcutRuntime {
    settings: Option<ShortcutSettings>,
    modifier_monitor: Option<modifier_hold::ModifierHoldMonitor>,
}
```

- [ ] **Step 2: Update `apply_shortcut_settings_for_app`**

Replace the current body:

```rust
let active_state = app.state::<ActiveShortcutState>();
let mut active = active_state.0.lock().map_err(|err| err.to_string())?;
let mut registry = TauriShortcutRegistry { app };
shortcut::apply_shortcut_settings(&mut registry, &mut active, settings)
```

with:

```rust
let active_state = app.state::<ActiveShortcutState>();
let mut active = active_state.0.lock().map_err(|err| err.to_string())?;
let mut registry = TauriShortcutRegistry {
    app,
    modifier_monitor: &mut active.modifier_monitor,
};
shortcut::apply_shortcut_settings(&mut registry, &mut active.settings, settings)
```

- [ ] **Step 3: Update `TauriShortcutRegistry`**

Replace the struct with:

```rust
struct TauriShortcutRegistry<'a> {
    app: &'a tauri::AppHandle,
    modifier_monitor: &'a mut Option<modifier_hold::ModifierHoldMonitor>,
}
```

Replace the impl with:

```rust
impl ShortcutRegistry for TauriShortcutRegistry<'_> {
    fn register_combo(&mut self, settings: &ShortcutSettings) -> Result<(), String> {
        self.app
            .global_shortcut()
            .on_shortcut(settings.to_tauri_shortcut()?, |app, _shortcut, event| {
                let _ = app.emit(RECORD_SHORTCUT_EVENT, shortcut_event_payload(event.state));
            })
            .map_err(|err| err.to_string())
    }

    fn unregister_combo(&mut self, settings: &ShortcutSettings) -> Result<(), String> {
        self.app
            .global_shortcut()
            .unregister(settings.to_tauri_shortcut()?)
            .map_err(|err| err.to_string())
    }

    fn start_modifier_hold(
        &mut self,
        settings: &shortcut::ModifierHoldSettings,
    ) -> Result<(), String> {
        if let Some(monitor) = self.modifier_monitor.take() {
            monitor.stop();
        }

        // Modifier-hold preferences may be saved before Accessibility is granted.
        // In that state the desired mode is persisted, but no monitor is active.
        // Settings already shows the permission warning, and request_accessibility
        // re-applies the saved shortcut after permission is granted.
        if !crate::platform::macos::accessibility_status().granted {
            return Ok(());
        }

        let monitor = modifier_hold::ModifierHoldMonitor::start(self.app.clone(), settings.clone())?;
        *self.modifier_monitor = Some(monitor);
        Ok(())
    }

    fn stop_modifier_hold(
        &mut self,
        _settings: &shortcut::ModifierHoldSettings,
    ) -> Result<(), String> {
        if let Some(monitor) = self.modifier_monitor.take() {
            monitor.stop();
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Stop monitor on app exit**

In the `.run(|app_handle, event| { ... })` exit branch, before/after inference shutdown, add:

```rust
if let Ok(mut active) = app_handle.state::<ActiveShortcutState>().0.lock() {
    if let Some(monitor) = active.modifier_monitor.take() {
        monitor.stop();
    }
}
```

- [ ] **Step 5: Preserve save-without-Accessibility semantics**

The `start_modifier_hold` implementation above intentionally returns `Ok(())` when Accessibility is missing. This is required by the spec: modifier-hold mode may be saved before permission is granted, but Settings must explain it will not work yet. In that state `active.settings` is `modifier_hold`, `modifier_monitor` is `None`, and the previous combo is no longer the advertised trigger.

Rollback invariant: `stop_modifier_hold` must always `take()` the monitor before stopping it, and `start_modifier_hold` must only assign `*modifier_monitor = Some(monitor)` after `ModifierHoldMonitor::start` succeeds. This keeps runtime state consistent during both apply failures and `set_shortcut_settings` save-failure rollbacks:

- combo → modifier-hold apply fails: combo is re-registered; `modifier_monitor` remains `None`.
- modifier-hold → combo apply fails: modifier monitor is started again before `active.settings` is restored.
- modifier-hold → combo succeeds but settings save fails: `set_shortcut_settings` re-applies the previous modifier-hold settings, which stops combo and restarts modifier monitoring when Accessibility is granted.
- combo → modifier-hold succeeds but settings save fails: `set_shortcut_settings` re-applies the previous combo, which stops and clears the modifier monitor before combo registration.

If the event tap fails after Accessibility is granted, keep returning `Err`; `apply_shortcut_settings` should then roll back to the previous active trigger.

- [ ] **Step 6: Add a force-start helper for saved modifier-hold settings**

Do not re-route Accessibility grant through `apply_shortcut_settings_for_app`: that helper returns early when `previous == next`, and after save-without-Accessibility the active desired settings are already `modifier_hold` while the monitor is `None`. Add a separate helper in `apps/desktop/src-tauri/src/lib.rs` that bypasses the equality guard and starts the monitor if needed:

```rust
pub(crate) fn start_saved_modifier_hold_monitor_if_needed(
    app: &tauri::AppHandle,
) -> Result<(), String> {
    let active_state = app.state::<ActiveShortcutState>();
    let mut active = active_state.0.lock().map_err(|err| err.to_string())?;
    let Some(settings) = active.settings.clone() else {
        return Ok(());
    };
    if !matches!(settings.mode, shortcut::ShortcutMode::ModifierHold) {
        return Ok(());
    }
    if active.modifier_monitor.is_some() {
        return Ok(());
    }

    let mut registry = TauriShortcutRegistry {
        app,
        modifier_monitor: &mut active.modifier_monitor,
    };
    registry.start_modifier_hold(&settings.modifier_hold)
}
```

This helper is intentionally not a general shortcut apply function. It only repairs the deferred monitor state after Accessibility is granted.

- [ ] **Step 7: Re-apply saved modifier-hold settings after Accessibility grant**

Change `request_accessibility` in `apps/desktop/src-tauri/src/commands/settings.rs` from:

```rust
#[tauri::command]
pub fn request_accessibility() -> AccessibilityStatus {
    macos::request_accessibility()
}
```

to:

```rust
#[tauri::command]
pub fn request_accessibility(app: AppHandle) -> AccessibilityStatus {
    let status = macos::request_accessibility();
    if status.granted {
        let _ = crate::start_saved_modifier_hold_monitor_if_needed(&app);
    }
    status
}
```

`AppHandle` is already imported in `settings.rs`. Add/update a command registration or source-text test if the existing tests assert command signatures.

- [ ] **Step 8: Run compile/tests**

Run:

```bash
cargo test -p wispergo-desktop shortcut::tests modifier_hold::tests -- --nocapture
```

Expected: all shortcut and modifier-hold tests pass.

- [ ] **Step 9: Commit**

```bash
git add apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/shortcut.rs apps/desktop/src-tauri/src/commands/settings.rs
git commit -m "feat(desktop): wire modifier hold shortcut runtime"
```

---

## Task 6: Persist modifier-hold settings and update frontend types/API contract

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/settings.rs`
- Modify: `apps/desktop/src/types/pipeline.ts`
- Modify: `apps/desktop/src/app/App.test.tsx`

- [ ] **Step 1: Add Rust persisted-settings tests**

In `commands/settings.rs` tests, extend persisted settings coverage:

```rust
#[test]
fn persisted_settings_round_trip_modifier_hold_shortcut() {
    let persisted = PersistedSettings {
        local_model: LocalModelSettings::default(),
        shortcut: ShortcutSettings {
            mode: ShortcutMode::ModifierHold,
            combo: ShortcutCombo::default(),
            modifier_hold: ModifierHoldSettings {
                key: ModifierHoldKey::RightCommand,
                hold_threshold_ms: 200,
            },
        },
    };

    let json = serde_json::to_string(&persisted).expect("persisted settings should serialize");
    assert!(json.contains("modifier_hold"));
    assert!(json.contains("right_command"));

    let parsed = serde_json::from_str::<PersistedSettings>(&json)
        .expect("persisted settings should deserialize");
    assert_eq!(parsed.shortcut.display_label(), "Hold Right ⌘");
}
```

Add imports:

```rust
use crate::shortcut::{
    ModifierHoldKey, ModifierHoldSettings, ShortcutCombo, ShortcutKey, ShortcutMode,
    ShortcutModifiers, ShortcutSettings,
};
```

- [ ] **Step 2: Run Rust persisted settings tests**

Run:

```bash
cargo test -p wispergo-desktop commands::settings::tests::persisted_settings -- --nocapture
```

Expected: tests pass once Task 1 model exists.

- [ ] **Step 3: Extend TypeScript types**

In `apps/desktop/src/types/pipeline.ts`, change:

```ts
export type ShortcutMode = "combo";
```

to:

```ts
export type ShortcutMode = "combo" | "modifier_hold";
```

Add:

```ts
export type ModifierHoldKey =
  | "left_command"
  | "right_command"
  | "left_option"
  | "right_option"
  | "left_control"
  | "right_control"
  | "left_shift"
  | "right_shift";

export type ModifierHoldSettings = {
  key: ModifierHoldKey;
  holdThresholdMs: number;
};
```

Update `ShortcutSettings`:

```ts
export type ShortcutSettings = {
  mode: ShortcutMode;
  combo: ShortcutCombo;
  modifierHold: ModifierHoldSettings;
};
```

- [ ] **Step 4: Update every TS shortcut settings literal**

Find every shortcut settings literal before editing:

```bash
rg -n 'mode: "combo"|mode: "modifier_hold"' apps/desktop/src
```

Every `ShortcutSettings` literal must include:

```ts
modifierHold: { key: "right_command", holdThresholdMs: 200 },
```

Confirmed current locations that must be updated:

- `apps/desktop/src/app/App.tsx` (`DEFAULT_SHORTCUT_VIEW`)
- `apps/desktop/src/app/App.test.tsx` (`defaultShortcutView` and shortcut save expectations/mocks)
- `apps/desktop/src/features/settings/SettingsPanel.tsx` (`DEFAULT_SHORTCUT_SETTINGS`)
- `apps/desktop/src/features/settings/SettingsPanel.test.tsx` (default/reset/save expectations and custom `shortcutView` literals)

After editing, run the grep again and verify every object with `mode: "combo"` or `mode: "modifier_hold"` has a sibling `modifierHold` property unless it is a partial assertion deliberately typed outside `ShortcutSettings`.

- [ ] **Step 5: Update recorder hint copy before adding modifier-hold App tests**

In `apps/desktop/src/features/recorder/FloatingRecorder.tsx`, add:

```ts
function shortcutHint(label: string) {
  if (label.startsWith("Hold ")) {
    return `${label} to dictate`;
  }
  return `Hold ${label} to dictate`;
}
```

Replace the hint with:

```tsx
<div className="recording-hint">
  {setupNeeded ? "open settings to finish" : shortcutHint(shortcutLabel)}
</div>
```

Default prop remains:

```ts
shortcutLabel = "Command + Shift + Space"
```

Update `FloatingRecorder.test.tsx` combo assertions from `hold Command + Shift + Space` to `Hold Command + Shift + Space to dictate`, and add:

```ts
it("renders modifier-hold prompt without duplicate hold wording", () => {
  render(<FloatingRecorder status="idle" expanded shortcutLabel="Hold Right ⌘" />);

  expect(screen.getByText("Hold Right ⌘ to dictate")).toBeInTheDocument();
});
```

- [ ] **Step 6: Add App test for loaded modifier-hold label on recorder surface**

In `App.test.tsx`, add:

```ts
it("renders loaded modifier-hold shortcut label in the recorder hint", async () => {
  vi.mocked(shortcutSettings).mockResolvedValueOnce({
    settings: {
      mode: "modifier_hold",
      combo: {
        modifiers: { command: true, shift: true, option: false, control: false },
        key: "space",
      },
      modifierHold: { key: "right_command", holdThresholdMs: 200 },
    },
    displayLabel: "Hold Right ⌘",
  });
  window.history.pushState({}, "", "/?surface=recorder");

  render(<App />);
  await emitFloatingChromeExpanded(true);

  expect(await screen.findByRole("region", { name: "Recorder" })).toHaveTextContent(
    "Hold Right ⌘ to dictate",
  );
});
```

Update existing combo App tests from `hold ⌘ ⇧ Space` to `Hold ⌘ ⇧ Space to dictate`.

- [ ] **Step 7: Run frontend type/test gate**

Run:

```bash
pnpm test:ts
```

Expected: TypeScript compiles and tests pass. If Corepack adds `packageManager`, run `git checkout -- package.json` before committing.

- [ ] **Step 8: Commit**

```bash
git checkout -- package.json
git add apps/desktop/src-tauri/src/commands/settings.rs apps/desktop/src/types/pipeline.ts apps/desktop/src/app/App.tsx apps/desktop/src/app/App.test.tsx apps/desktop/src/features/recorder/FloatingRecorder.tsx apps/desktop/src/features/recorder/FloatingRecorder.test.tsx apps/desktop/src/features/settings/SettingsPanel.tsx apps/desktop/src/features/settings/SettingsPanel.test.tsx
git commit -m "feat(desktop): persist modifier hold settings"
```

---

## Task 7: Add Settings UI for modifier-hold mode

**Files:**
- Modify: `apps/desktop/src/features/settings/SettingsPanel.tsx`
- Modify: `apps/desktop/src/features/settings/SettingsPanel.test.tsx`
- Modify: `apps/desktop/src/styles.css`

- [ ] **Step 1: Add SettingsPanel tests for modifier-hold UI**

In `SettingsPanel.test.tsx`, add:

```ts
it("shows modifier-hold controls when modifier mode is selected", async () => {
  const user = userEvent.setup();
  renderSettingsPanel();

  await user.click(screen.getByRole("radio", { name: "Hold one modifier" }));

  expect(screen.getByLabelText("Modifier key")).toBeInTheDocument();
  expect(screen.getByText("Starts when held by itself. Normal shortcuts are ignored.")).toBeInTheDocument();
});

it("saves modifier-hold shortcut settings", async () => {
  const user = userEvent.setup();
  const onShortcutSettingsSave = vi.fn();
  renderSettingsPanel({ onShortcutSettingsSave });

  await user.click(screen.getByRole("radio", { name: "Hold one modifier" }));
  await user.selectOptions(screen.getByLabelText("Modifier key"), "right_command");
  await user.click(screen.getByRole("button", { name: "Save shortcut" }));

  expect(onShortcutSettingsSave).toHaveBeenCalledWith({
    mode: "modifier_hold",
    combo: {
      modifiers: { command: true, shift: true, option: false, control: false },
      key: "space",
    },
    modifierHold: { key: "right_command", holdThresholdMs: 200 },
  });
});

it("warns that modifier-hold requires Accessibility when missing", async () => {
  const user = userEvent.setup();
  renderSettingsPanel({ accessibility: { granted: false, canPrompt: true } });

  await user.click(screen.getByRole("radio", { name: "Hold one modifier" }));

  expect(screen.getByRole("status")).toHaveTextContent("Accessibility permission is required");
});

it("renders saved modifier-hold label in shortcut card", () => {
  renderSettingsPanel({
    shortcutView: {
      settings: {
        mode: "modifier_hold",
        combo: {
          modifiers: { command: true, shift: true, option: false, control: false },
          key: "space",
        },
        modifierHold: { key: "right_command", holdThresholdMs: 200 },
      },
      displayLabel: "Hold Right ⌘",
    },
  });

  expect(screen.getAllByText("Hold Right ⌘").length).toBeGreaterThan(0);
});
```

- [ ] **Step 2: Add constants in SettingsPanel**

In `SettingsPanel.tsx`, add imports/types:

```ts
import type { ModifierHoldKey } from "../../types/pipeline";
```

Add constants near shortcut constants:

```ts
const DEFAULT_MODIFIER_HOLD = { key: "right_command", holdThresholdMs: 200 } as const;

const MODIFIER_HOLD_KEY_OPTIONS: Array<{ value: ModifierHoldKey; label: string }> = [
  { value: "left_command", label: "Left Command" },
  { value: "right_command", label: "Right Command" },
  { value: "left_option", label: "Left Option" },
  { value: "right_option", label: "Right Option" },
  { value: "left_control", label: "Left Control" },
  { value: "right_control", label: "Right Control" },
  { value: "left_shift", label: "Left Shift" },
  { value: "right_shift", label: "Right Shift" },
];
```

Ensure `DEFAULT_SHORTCUT_SETTINGS` includes:

```ts
modifierHold: DEFAULT_MODIFIER_HOLD,
```

- [ ] **Step 3: Add mode selector UI**

Inside the Shortcut card, before the current shortcut display, add:

```tsx
<div className="shortcut-mode-toggle" role="radiogroup" aria-label="Shortcut mode">
  <label>
    <input
      type="radio"
      name="shortcut-mode"
      checked={draftShortcutSettings.mode === "combo"}
      onChange={() =>
        setDraftShortcutSettings((current) => ({ ...current, mode: "combo" }))
      }
    />
    <span>Key combination</span>
  </label>
  <label>
    <input
      type="radio"
      name="shortcut-mode"
      checked={draftShortcutSettings.mode === "modifier_hold"}
      onChange={() =>
        setDraftShortcutSettings((current) => ({
          ...current,
          mode: "modifier_hold",
          modifierHold: current.modifierHold ?? DEFAULT_MODIFIER_HOLD,
        }))
      }
    />
    <span>Hold one modifier</span>
  </label>
</div>
```

- [ ] **Step 4: Condition combo controls**

Wrap existing `Record shortcut`, modifiers checkbox group, and key selector in:

```tsx
{draftShortcutSettings.mode === "combo" ? (
  <div className="shortcut-combo-editor">
    {/* existing combo controls */}
  </div>
) : null}
```

Keep the combo draft untouched while the user switches modes, so switching back preserves their combo draft.

- [ ] **Step 5: Add modifier-hold controls**

Below the combo editor, add:

```tsx
{draftShortcutSettings.mode === "modifier_hold" ? (
  <div className="shortcut-hold-editor">
    <label className="settings-field">
      <span>Modifier key</span>
      <select
        value={draftShortcutSettings.modifierHold.key}
        onChange={(event) =>
          setDraftShortcutSettings((current) => ({
            ...current,
            modifierHold: {
              ...current.modifierHold,
              key: event.target.value as ModifierHoldKey,
              holdThresholdMs: current.modifierHold.holdThresholdMs || 200,
            },
          }))
        }
      >
        {MODIFIER_HOLD_KEY_OPTIONS.map((option) => (
          <option key={option.value} value={option.value}>{option.label}</option>
        ))}
      </select>
    </label>
    <p className="settings-note">
      Starts when held by itself. Normal shortcuts are ignored.
    </p>
    {!accessibility.granted ? (
      <p className="shortcut-warning" role="status">
        Accessibility permission is required before modifier-hold shortcuts can work.
      </p>
    ) : null}
  </div>
) : null}
```

- [ ] **Step 6: Update save/reset behavior**

`Save shortcut` should keep calling:

```tsx
onShortcutSettingsSave(draftShortcutSettings)
```

`Reset to default` should continue to pass `DEFAULT_SHORTCUT_SETTINGS`, which resets to combo `⌘ ⇧ Space`, not modifier hold.

- [ ] **Step 7: Add CSS**

In `apps/desktop/src/styles.css`, near existing shortcut styles, add:

```css
.shortcut-mode-toggle {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.settings-panel .shortcut-mode-toggle label {
  min-height: 52px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 14px;
  border: 2px solid #bdbbb4;
  border-radius: 14px;
  background: #f6f5ef;
  color: #42423d;
  letter-spacing: 0;
  text-transform: none;
}

.shortcut-mode-toggle input {
  min-height: auto;
  width: auto;
  padding: 0;
  accent-color: #1d1c31;
}

.shortcut-combo-editor,
.shortcut-hold-editor {
  display: grid;
  gap: 14px;
}

.shortcut-warning {
  color: #8a5b13;
  font-weight: 650;
}
```

- [ ] **Step 8: Run frontend tests**

Run:

```bash
pnpm test:ts
```

Expected: all frontend tests pass. If Corepack adds `packageManager`, run `git checkout -- package.json`.

- [ ] **Step 9: Commit**

```bash
git checkout -- package.json
git add apps/desktop/src/features/settings/SettingsPanel.tsx apps/desktop/src/features/settings/SettingsPanel.test.tsx apps/desktop/src/styles.css
git commit -m "feat(desktop): add modifier hold settings UI"
```

---

## Task 8: Update docs after implementation verification

**Files:**
- Modify: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- Modify: `HANDOFF.md`

- [ ] **Step 1: Update roadmap only after Task 9 verification passes**

In the R5 section, change:

```markdown
- R5.2: single modifier-key hold-to-dictate, including Right Command for
  keyboards without Right Option, remains planned/not started.
```

to:

```markdown
- R5.2: single modifier-key hold-to-dictate, including Right Command for
  keyboards without Right Option ✅
```

Add/adjust R5.2 DoD line:

```markdown
- R5.2 DoD: modifier-hold mode avoids normal shortcut interference with
  threshold/cancel-on-chord behavior, emits the existing Pressed/Released
  event contract, and includes watchdog release protection.
```

- [ ] **Step 2: Update HANDOFF**

Update `HANDOFF.md` to include:

```markdown
- **R5.2 modifier-hold shortcut customization is implemented on branch `r5-2-modifier-hold-shortcut`**: adds `modifier_hold` shortcut mode, physical modifier selection including Right Command, listen-only macOS event monitoring with threshold/cancel-on-chord/watchdog behavior, and Settings/recorder labels. R5.1 combo mode remains available and default.
```

Update current slice:

```markdown
## Current slice: R5.2 modifier-hold shortcut customization

**Implementation status:** Implemented on branch `r5-2-modifier-hold-shortcut`; PR pending. Default remains combo `⌘ ⇧ Space`; modifier hold is opt-in.

**Next step:** Open PR and wait for user merge. After merge, sync `main`, delete branch, then decide whether to move to R6 docs or pause.
```

- [ ] **Step 3: Commit docs**

```bash
git add docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md HANDOFF.md
git commit -m "docs: update r5 modifier hold status"
```

---

## Task 9: Final verification and PR

**Files:**
- No code changes expected unless verification fails.

- [ ] **Step 1: Run Rust formatting**

Run:

```bash
cargo fmt --all
```

If `cargo fmt --all` formats unrelated core files, revert unrelated files before committing:

```bash
git checkout -- crates/wispergo-core
```

- [ ] **Step 2: Run frontend tests**

Run:

```bash
pnpm test:ts
```

Expected: all tests pass. Then run:

```bash
git checkout -- package.json
```

- [ ] **Step 3: Run workspace build/test gates**

Run:

```bash
cargo build --workspace
cargo test --workspace
```

Expected: both pass.

- [ ] **Step 4: Run clippy gates**

Run:

```bash
cargo clippy -p wispergo-core --all-targets -- -D warnings
cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings
cargo clippy -p wispergo-desktop --all-targets -- -D warnings
```

Expected: all pass. The desktop clippy gate is mandatory before every PR.

- [ ] **Step 5: Run desktop build and thin bundle check**

Run:

```bash
pnpm desktop:build
pnpm check:macos-thin-bundle
git checkout -- package.json
```

Expected: app builds and thin-bundle check passes.

- [ ] **Step 6: Run static diff check**

Run:

```bash
git diff --check
git status --short
```

Expected: no whitespace errors; `package.json` must not include a Corepack-added `packageManager` change.

- [ ] **Step 7: Opus 4.8 review**

Run:

```bash
git show --format=fuller --no-ext-diff HEAD | claude -p --model claude-opus-4-8 "Review this Wispergo R5.2 modifier-hold shortcut implementation for correctness, scope control, and regressions. Focus on: listen-only macOS event monitoring, physical left/right modifier detection, threshold/cancel-on-chord/watchdog state machine, preserving combo default and R5.1 combo mode, frontend settings UX, and avoiding arbitrary single-key hold. Return blockers first, then non-blocking notes."
```

Fix blockers before PR. Non-blocking notes may be documented in the PR.

- [ ] **Step 8: Manual smoke checklist**

Run the app locally with a verified ASR Asset available:

```bash
pnpm desktop:dev
```

Manual checks:

- Fresh/default combo `⌘ ⇧ Space` still starts/stops dictation.
- Settings can switch to `Hold one modifier` → `Right Command`.
- Holding Right Command alone starts after threshold and release stops.
- Quick `Command+Tab`, `Command+C`, `Command+V`, and `Command+Space` do not start recording.
- Holding selected modifier then pressing another key stops/cancels recording.
- Switching back to combo stops modifier monitor and combo shortcut works.
- If Accessibility is disabled, Settings communicates that modifier hold requires Accessibility.

Record any not-run manual items in the PR body. Do not claim manual success unless actually run.

- [ ] **Step 9: Push and open PR**

```bash
git push -u origin r5-2-modifier-hold-shortcut
cat > /tmp/wispergo-r5-2-pr.md <<'EOF'
## Summary

- Adds opt-in `modifier_hold` shortcut mode alongside existing combo shortcuts.
- Adds physical modifier selection including Right Command.
- Uses a listen-only macOS event tap with threshold, cancel-on-chord, and watchdog release behavior.
- Keeps default `⌘ ⇧ Space` combo mode unchanged.
- Updates Settings/recorder labels and docs.

## Explicitly out of scope

- Arbitrary single-key hold.
- Fn key support.
- Active key interception/suppression.
- ASR, cleanup, Pipecat, release workflow, or asset changes.

## Verification

- [fill with exact commands/results from Task 9]

## Manual smoke

- [fill with exact manual checks run, or state not run]
EOF
gh pr create --base main --head r5-2-modifier-hold-shortcut --title "feat(desktop): add modifier-hold shortcut" --body-file /tmp/wispergo-r5-2-pr.md
```

- [ ] **Step 10: Stop and wait for user merge**

Do not merge the PR yourself. After the user reports merge:

```bash
git checkout main
git pull --ff-only origin main
git branch -d r5-2-modifier-hold-shortcut
git push origin --delete r5-2-modifier-hold-shortcut
```

Then update `HANDOFF.md` if the merge changed status language.

---

## Self-review

### Spec coverage

- R5.2 `modifier_hold` mode: Tasks 1, 6, 7.
- Physical modifier keys including Right Command: Tasks 1, 4, 7.
- Fn out of scope: Out-of-scope section and no Fn type.
- Listen-only monitoring: Task 4 uses `CGEventTapOptions::ListenOnly` and `CallbackResult::Keep`.
- `flagsChanged` detection: Task 4 monitors `CGEventType::FlagsChanged`.
- Chord cancellation before threshold and while active: Task 2 state-machine tests and implementation; Task 4 routes `KeyDown` and extra modifier flags.
- Existing `Pressed` / `Released` contract: Tasks 4 and 10.
- Watchdog release: Task 2 and Task 4.
- Accessibility messaging: Task 7.
- R5.1 combo default preserved: Tasks 1, 3, 5, 10.

### Placeholder scan

No `TBD`, `TODO`, or unspecified implementation placeholders are intentionally left in executable steps. The only fill-in section is the PR body verification list, which must be populated with actual results after verification.

### Type consistency

Rust names used throughout: `ShortcutSettings`, `ShortcutMode::ModifierHold`, `ModifierHoldSettings`, `ModifierHoldKey`, `ModifierHoldStateMachine`, `ModifierHoldInput`, `ModifierHoldAction`, `ModifierHoldMonitor`, `ShortcutRegistry`.

TypeScript names used throughout: `ShortcutMode`, `ModifierHoldKey`, `ModifierHoldSettings`, `ShortcutSettings`, `ShortcutSettingsView`.
