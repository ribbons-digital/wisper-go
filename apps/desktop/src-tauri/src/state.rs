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
