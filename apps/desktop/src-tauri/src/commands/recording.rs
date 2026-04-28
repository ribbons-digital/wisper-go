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
