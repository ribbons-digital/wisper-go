use std::sync::Mutex;

use crate::audio::AudioInputSession;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelSettings {
    pub whisper_binary_path: Option<String>,
    pub whisper_model_path: Option<String>,
}

impl LocalModelSettings {
    pub fn normalized(self) -> Self {
        Self {
            whisper_binary_path: normalize_optional_path(self.whisper_binary_path),
            whisper_model_path: normalize_optional_path(self.whisper_model_path),
        }
    }

    pub fn to_frontend(&self) -> Self {
        Self {
            whisper_binary_path: Some(self.whisper_binary_path.clone().unwrap_or_default()),
            whisper_model_path: Some(self.whisper_model_path.clone().unwrap_or_default()),
        }
    }
}

fn normalize_optional_path(path: Option<String>) -> Option<String> {
    let path = path?.trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
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
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            recording: Mutex::new(None),
            selected_microphone_id: Mutex::new(None),
            local_model_settings: Mutex::new(LocalModelSettings::default()),
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
    use super::{AppState, RecordingSession, RecordingStatus};

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
            whisper_binary_path: Some("/opt/homebrew/bin/whisper-cli".to_string()),
            whisper_model_path: Some("/models/base.bin".to_string()),
        });

        assert_eq!(
            state.local_model_settings(),
            super::LocalModelSettings {
                whisper_binary_path: Some("/opt/homebrew/bin/whisper-cli".to_string()),
                whisper_model_path: Some("/models/base.bin".to_string()),
            }
        );
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
