use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

pub const RECORD_SHORTCUT_EVENT: &str = "wispergo://record-shortcut";
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
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

impl<'de> serde::Deserialize<'de> for ShortcutKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <Option<String> as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self::from_code(value.as_deref()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutSettingsView {
    pub settings: ShortcutSettings,
    pub display_label: String,
}

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

    pub fn to_frontend(&self) -> ShortcutSettingsView {
        let normalized = self.clone().normalized();
        ShortcutSettingsView {
            display_label: normalized.display_label(),
            settings: normalized,
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

impl ShortcutKey {
    fn from_code(code: Option<&str>) -> Self {
        match code.unwrap_or_default() {
            "space" => Self::Space,
            "enter" => Self::Enter,
            "escape" => Self::Escape,
            "tab" => Self::Tab,
            "backquote" => Self::Backquote,
            "minus" => Self::Minus,
            "equal" => Self::Equal,
            "bracketLeft" => Self::BracketLeft,
            "bracketRight" => Self::BracketRight,
            "backslash" => Self::Backslash,
            "semicolon" => Self::Semicolon,
            "quote" => Self::Quote,
            "comma" => Self::Comma,
            "period" => Self::Period,
            "slash" => Self::Slash,
            "arrowUp" => Self::ArrowUp,
            "arrowDown" => Self::ArrowDown,
            "arrowLeft" => Self::ArrowLeft,
            "arrowRight" => Self::ArrowRight,
            "digit0" => Self::Digit0,
            "digit1" => Self::Digit1,
            "digit2" => Self::Digit2,
            "digit3" => Self::Digit3,
            "digit4" => Self::Digit4,
            "digit5" => Self::Digit5,
            "digit6" => Self::Digit6,
            "digit7" => Self::Digit7,
            "digit8" => Self::Digit8,
            "digit9" => Self::Digit9,
            "keyA" => Self::KeyA,
            "keyB" => Self::KeyB,
            "keyC" => Self::KeyC,
            "keyD" => Self::KeyD,
            "keyE" => Self::KeyE,
            "keyF" => Self::KeyF,
            "keyG" => Self::KeyG,
            "keyH" => Self::KeyH,
            "keyI" => Self::KeyI,
            "keyJ" => Self::KeyJ,
            "keyK" => Self::KeyK,
            "keyL" => Self::KeyL,
            "keyM" => Self::KeyM,
            "keyN" => Self::KeyN,
            "keyO" => Self::KeyO,
            "keyP" => Self::KeyP,
            "keyQ" => Self::KeyQ,
            "keyR" => Self::KeyR,
            "keyS" => Self::KeyS,
            "keyT" => Self::KeyT,
            "keyU" => Self::KeyU,
            "keyV" => Self::KeyV,
            "keyW" => Self::KeyW,
            "keyX" => Self::KeyX,
            "keyY" => Self::KeyY,
            "keyZ" => Self::KeyZ,
            _ => Self::Space,
        }
    }

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

pub fn shortcut_event_payload(state: ShortcutState) -> &'static str {
    match state {
        ShortcutState::Pressed => "Pressed",
        ShortcutState::Released => "Released",
    }
}

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
                ModifierHoldInput::ThresholdElapsed {
                    generation: elapsed_generation,
                },
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
                ModifierHoldInput::WatchdogElapsed {
                    generation: elapsed_generation,
                },
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

pub trait ShortcutRegistry {
    fn register_combo(&mut self, settings: &ShortcutSettings) -> Result<(), String>;
    fn unregister_combo(&mut self, settings: &ShortcutSettings) -> Result<(), String>;
    fn start_modifier_hold(&mut self, settings: &ModifierHoldSettings) -> Result<(), String>;
    fn stop_modifier_hold(&mut self, settings: &ModifierHoldSettings) -> Result<(), String>;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shortcut_is_command_shift_space() {
        let settings = ShortcutSettings::default();

        assert!(settings.combo.modifiers.command);
        assert!(settings.combo.modifiers.shift);
        assert!(!settings.combo.modifiers.option);
        assert!(!settings.combo.modifiers.control);
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
            modifier_hold: ModifierHoldSettings::default(),
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
            modifier_hold: ModifierHoldSettings::default(),
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
            modifier_hold: ModifierHoldSettings::default(),
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
            vec![ModifierHoldAction::ScheduleThreshold {
                generation: 1,
                delay_ms: 200
            }]
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
            vec![ModifierHoldAction::ScheduleThreshold {
                generation: 1,
                delay_ms: 200
            }]
        );
        assert_eq!(
            machine.handle_event(ModifierHoldInput::ThresholdElapsed { generation: 1 }),
            vec![
                ModifierHoldAction::EmitPressed,
                ModifierHoldAction::ScheduleWatchdog {
                    generation: 1,
                    delay_ms: 30_000
                },
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
                ModifierHoldAction::ScheduleWatchdog {
                    generation: 2,
                    delay_ms: 30_000
                },
            ]
        );
    }

    #[derive(Default)]
    struct FakeShortcutRegistry {
        calls: Vec<String>,
        fail_next_combo_register: Option<String>,
        fail_next_modifier_hold_start: Option<String>,
    }

    impl ShortcutRegistry for FakeShortcutRegistry {
        fn register_combo(&mut self, settings: &ShortcutSettings) -> Result<(), String> {
            self.calls
                .push(format!("register_combo:{}", settings.display_label()));
            if let Some(error) = self.fail_next_combo_register.take() {
                return Err(error);
            }
            Ok(())
        }

        fn unregister_combo(&mut self, settings: &ShortcutSettings) -> Result<(), String> {
            self.calls
                .push(format!("unregister_combo:{}", settings.display_label()));
            Ok(())
        }

        fn start_modifier_hold(&mut self, settings: &ModifierHoldSettings) -> Result<(), String> {
            self.calls
                .push(format!("start_modifier_hold:{}", settings.display_label()));
            if let Some(error) = self.fail_next_modifier_hold_start.take() {
                return Err(error);
            }
            Ok(())
        }

        fn stop_modifier_hold(&mut self, settings: &ModifierHoldSettings) -> Result<(), String> {
            self.calls
                .push(format!("stop_modifier_hold:{}", settings.display_label()));
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

        assert_eq!(active, Some(settings));
        assert_eq!(registry.calls, vec!["register_combo:⌘ ⇧ Space".to_string()]);
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
            modifier_hold: ModifierHoldSettings::default(),
        };
        let mut registry = FakeShortcutRegistry::default();
        let mut active = Some(previous.clone());

        let view = apply_shortcut_settings(&mut registry, &mut active, next.clone())
            .expect("apply shortcut");

        assert_eq!(active, Some(next));
        assert_eq!(
            registry.calls,
            vec![
                "unregister_combo:⌘ ⇧ Space".to_string(),
                "register_combo:⌘ ⌥ K".to_string(),
            ]
        );
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
            modifier_hold: ModifierHoldSettings::default(),
        };
        let mut registry = FakeShortcutRegistry {
            fail_next_combo_register: Some("shortcut is already registered".to_string()),
            ..FakeShortcutRegistry::default()
        };
        let mut active = Some(previous.clone());

        let error = apply_shortcut_settings(&mut registry, &mut active, next)
            .expect_err("conflict should fail");

        assert!(error.contains("shortcut is already registered"));
        assert_eq!(active, Some(previous));
        assert_eq!(
            registry.calls,
            vec![
                "unregister_combo:⌘ ⇧ Space".to_string(),
                "register_combo:⌘ ⌥ K".to_string(),
                "register_combo:⌘ ⇧ Space".to_string(),
            ]
        );
    }

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

        assert_eq!(active, Some(next));
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

    #[test]
    fn shortcut_event_payload_matches_frontend_contract() {
        assert_eq!(RECORD_SHORTCUT_EVENT, "wispergo://record-shortcut");
        assert_eq!(shortcut_event_payload(ShortcutState::Pressed), "Pressed");
        assert_eq!(shortcut_event_payload(ShortcutState::Released), "Released");
    }
}
