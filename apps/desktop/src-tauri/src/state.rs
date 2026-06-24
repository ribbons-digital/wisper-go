use std::sync::Mutex;

use crate::audio::AudioInputSession;
use crate::shortcut::ShortcutSettings;

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
        match code
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
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
        let value = <Option<String> as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self::from_code(value.as_deref()))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupMode {
    Off,
    #[default]
    PunctuationOnly,
    FullCleanup,
}

impl CleanupMode {
    pub fn from_code(code: Option<&str>) -> Self {
        match code
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "off" => Self::Off,
            "full_cleanup" => Self::FullCleanup,
            "punctuation_only" => Self::PunctuationOnly,
            _ => Self::PunctuationOnly,
        }
    }
}

impl<'de> serde::Deserialize<'de> for CleanupMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <Option<String> as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self::from_code(value.as_deref()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelSettings {
    #[serde(default = "default_asr_model_id")]
    pub asr_model_id: String,
    #[serde(default)]
    pub recognition_language: RecognitionLanguage,
    #[serde(default)]
    pub cleanup_mode: CleanupMode,
}

impl Default for LocalModelSettings {
    fn default() -> Self {
        Self {
            asr_model_id: default_asr_model_id(),
            recognition_language: RecognitionLanguage::Auto,
            cleanup_mode: CleanupMode::PunctuationOnly,
        }
    }
}

impl LocalModelSettings {
    pub fn normalized(self) -> Self {
        Self {
            asr_model_id: normalize_asr_model_id(self.asr_model_id),
            recognition_language: self.recognition_language,
            cleanup_mode: self.cleanup_mode,
        }
    }

    pub fn to_frontend(&self) -> Self {
        Self {
            asr_model_id: self.asr_model_id.clone(),
            recognition_language: self.recognition_language,
            cleanup_mode: self.cleanup_mode,
        }
    }
}

fn default_asr_model_id() -> String {
    "medium".to_string()
}

fn normalize_asr_model_id(id: String) -> String {
    let id = id.trim();
    if id.is_empty() {
        default_asr_model_id()
    } else {
        id.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingStatus {
    Idle,
    Recording,
}

pub enum RecordingSession {
    Live(AudioInputSession),
    #[cfg(test)]
    Buffered(Vec<f32>),
}

impl RecordingSession {
    #[cfg(test)]
    pub fn buffered(audio: Vec<f32>) -> Self {
        Self::Buffered(audio)
    }

    pub fn stop(self) -> Vec<f32> {
        match self {
            Self::Live(session) => session.stop(),
            #[cfg(test)]
            Self::Buffered(audio) => audio,
        }
    }
}

pub struct AppState {
    recording: Mutex<Option<RecordingSession>>,
    selected_microphone_id: Mutex<Option<String>>,
    local_model_settings: Mutex<LocalModelSettings>,
    shortcut_settings: Mutex<ShortcutSettings>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            recording: Mutex::new(None),
            selected_microphone_id: Mutex::new(None),
            local_model_settings: Mutex::new(LocalModelSettings::default()),
            shortcut_settings: Mutex::new(ShortcutSettings::default()),
        }
    }
}

impl AppState {
    pub fn recording_status(&self) -> RecordingStatus {
        if self
            .recording
            .lock()
            .expect("recording status lock")
            .is_some()
        {
            RecordingStatus::Recording
        } else {
            RecordingStatus::Idle
        }
    }

    pub fn selected_microphone_id(&self) -> Option<String> {
        self.selected_microphone_id
            .lock()
            .expect("selected microphone lock")
            .clone()
    }

    pub fn set_selected_microphone_id(&self, device_id: Option<String>) {
        *self
            .selected_microphone_id
            .lock()
            .expect("selected microphone lock") = device_id;
    }

    pub fn local_model_settings(&self) -> LocalModelSettings {
        self.local_model_settings
            .lock()
            .expect("local model settings lock")
            .clone()
    }

    pub fn set_local_model_settings(&self, settings: LocalModelSettings) {
        *self
            .local_model_settings
            .lock()
            .expect("local model settings lock") = settings;
    }

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

    pub fn start_recording(&self, _mode: &str) -> Result<(), String> {
        let device_id = self.selected_microphone_id();
        let session = crate::audio::start_input_session(device_id.as_deref())?;
        self.start_recording_with_session(RecordingSession::Live(session))
    }

    pub fn start_recording_with_session(&self, session: RecordingSession) -> Result<(), String> {
        let mut recording = self.recording.lock().map_err(|err| err.to_string())?;
        if recording.is_some() {
            return Err("recording already active".to_string());
        }
        *recording = Some(session);
        Ok(())
    }

    pub fn stop_recording(&self, _reason: &str) -> Result<Vec<f32>, String> {
        let mut recording = self.recording.lock().map_err(|err| err.to_string())?;
        let session = recording
            .take()
            .ok_or_else(|| "recording is not active".to_string())?;
        Ok(session.stop())
    }

    pub fn cancel_recording(&self, _reason: &str) -> Result<(), String> {
        let mut recording = self.recording.lock().map_err(|err| err.to_string())?;
        *recording = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AppState, CleanupMode, RecognitionLanguage, RecordingSession, RecordingStatus};
    use crate::shortcut::{
        ModifierHoldSettings, ShortcutCombo, ShortcutKey, ShortcutMode, ShortcutModifiers,
        ShortcutSettings,
    };

    #[test]
    fn selected_microphone_round_trips() {
        let state = AppState::default();

        state.set_selected_microphone_id(Some("studio-mic".to_string()));

        assert_eq!(
            state.selected_microphone_id(),
            Some("studio-mic".to_string())
        );
    }

    #[test]
    fn local_model_settings_round_trip() {
        let state = AppState::default();

        state.set_local_model_settings(super::LocalModelSettings {
            asr_model_id: "medium".to_string(),
            recognition_language: RecognitionLanguage::Auto,
            cleanup_mode: CleanupMode::PunctuationOnly,
        });

        assert_eq!(
            state.local_model_settings(),
            super::LocalModelSettings {
                asr_model_id: "medium".to_string(),
                recognition_language: RecognitionLanguage::Auto,
                cleanup_mode: CleanupMode::PunctuationOnly,
            }
        );
    }

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
            modifier_hold: ModifierHoldSettings::default(),
        };

        state.set_shortcut_settings(settings.clone());

        assert_eq!(state.shortcut_settings(), settings);
    }

    #[test]
    fn local_model_settings_default_to_auto_language() {
        let state = AppState::default();

        assert_eq!(
            state.local_model_settings().recognition_language,
            RecognitionLanguage::Auto
        );
    }

    #[test]
    fn local_model_settings_default_to_punctuation_only_cleanup() {
        let state = AppState::default();

        assert_eq!(
            state.local_model_settings().cleanup_mode,
            CleanupMode::PunctuationOnly
        );
    }

    #[test]
    fn local_model_settings_default_to_medium_asr_model() {
        let state = AppState::default();

        assert_eq!(state.local_model_settings().asr_model_id, "medium");
    }

    #[test]
    fn local_model_settings_language_round_trip() {
        let state = AppState::default();

        state.set_local_model_settings(super::LocalModelSettings {
            asr_model_id: "large-v3-turbo".to_string(),
            recognition_language: RecognitionLanguage::Zh,
            cleanup_mode: CleanupMode::FullCleanup,
        });

        assert_eq!(
            state.local_model_settings(),
            super::LocalModelSettings {
                asr_model_id: "large-v3-turbo".to_string(),
                recognition_language: RecognitionLanguage::Zh,
                cleanup_mode: CleanupMode::FullCleanup,
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

    #[test]
    fn missing_cleanup_mode_deserializes_to_punctuation_only() {
        let settings: super::LocalModelSettings = serde_json::from_str(
            r#"{"whisperBinaryPath":"/bin/whisper-cli","whisperModelPath":"/models/model.bin"}"#,
        )
        .expect("settings deserialize");

        assert_eq!(settings.cleanup_mode, CleanupMode::PunctuationOnly);
        assert_eq!(settings.asr_model_id, "medium");
    }

    #[test]
    fn invalid_cleanup_mode_deserializes_to_punctuation_only() {
        let settings: super::LocalModelSettings = serde_json::from_str(
            r#"{"whisperBinaryPath":"/bin/whisper-cli","whisperModelPath":"/models/model.bin","cleanupMode":"translate_everything"}"#,
        )
        .expect("settings deserialize");

        assert_eq!(settings.cleanup_mode, CleanupMode::PunctuationOnly);
    }

    #[test]
    fn cleanup_mode_serializes_as_snake_case() {
        let json = serde_json::to_value(super::LocalModelSettings {
            asr_model_id: "medium".to_string(),
            recognition_language: RecognitionLanguage::Auto,
            cleanup_mode: CleanupMode::FullCleanup,
        })
        .expect("settings serialize");

        assert_eq!(json["cleanupMode"], "full_cleanup");
        assert_eq!(json["asrModelId"], "medium");
    }

    #[test]
    fn legacy_sidecar_path_keys_deserialize_but_are_not_serialized() {
        let settings: super::LocalModelSettings = serde_json::from_str(
            r#"{"whisperBinaryPath":"/bin/whisper-cli","whisperModelPath":"/models/model.bin","asrModelId":"medium","recognitionLanguage":"en","cleanupMode":"punctuation_only"}"#,
        )
        .expect("legacy settings keys should not break deserialization");

        assert_eq!(settings.asr_model_id, "medium");
        assert_eq!(settings.recognition_language, RecognitionLanguage::En);
        assert_eq!(settings.cleanup_mode, CleanupMode::PunctuationOnly);

        let json = serde_json::to_value(settings).expect("settings serialize");
        assert!(json.get("whisperBinaryPath").is_none());
        assert!(json.get("whisperModelPath").is_none());
    }

    #[test]
    fn stop_recording_returns_audio_and_resets_status() {
        let state = AppState::default();
        state
            .start_recording_with_session(RecordingSession::buffered(vec![0.1, 0.2]))
            .expect("start");

        let audio = state.stop_recording("floating_button").expect("stop");

        assert_eq!(audio, vec![0.1, 0.2]);
        assert_eq!(state.recording_status(), RecordingStatus::Idle);
    }

    #[test]
    fn cancel_recording_discards_audio() {
        let state = AppState::default();
        state
            .start_recording_with_session(RecordingSession::buffered(vec![0.1]))
            .expect("start");

        state.cancel_recording("user_cancelled").expect("cancel");

        assert_eq!(state.recording_status(), RecordingStatus::Idle);
    }
}
