import { invoke } from "@tauri-apps/api/core";
import type {
  AccessibilityStatus,
  AudioInputDevice,
  CleanupRuntimeStatus,
  LocalModelSettings,
  MicrophoneStatus,
  OllamaSetupStatus,
  RecognitionLanguage,
  RecordingMode,
  StopRecordingOutput,
} from "../types/pipeline";

export async function appHealth(): Promise<string> {
  return invoke<string>("app_health");
}

export async function startRecording(mode: RecordingMode): Promise<void> {
  await invoke("start_recording", { mode });
}

export async function stopRecording(reason: string): Promise<StopRecordingOutput> {
  return invoke<StopRecordingOutput>("stop_recording", { reason });
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

export async function ensureOllamaSetup(): Promise<OllamaSetupStatus> {
  return invoke<OllamaSetupStatus>("ensure_ollama_setup");
}

export function cleanupRuntimeStatus(): Promise<CleanupRuntimeStatus> {
  return invoke<CleanupRuntimeStatus>("cleanup_runtime_status");
}

export async function listMicrophones(): Promise<AudioInputDevice[]> {
  return invoke<AudioInputDevice[]>("list_microphones");
}

export async function selectedMicrophoneId(): Promise<string | null> {
  return invoke<string | null>("selected_microphone_id");
}

export async function setMicrophoneDevice(deviceId: string | null): Promise<void> {
  await invoke("set_microphone_device", { deviceId });
}

export async function microphoneStatus(): Promise<MicrophoneStatus> {
  return invoke<MicrophoneStatus>("microphone_status");
}

export async function requestMicrophoneAccess(): Promise<MicrophoneStatus> {
  return invoke<MicrophoneStatus>("request_microphone_access");
}

export async function accessibilityStatus(): Promise<AccessibilityStatus> {
  return invoke<AccessibilityStatus>("accessibility_status");
}

export async function requestAccessibility(): Promise<AccessibilityStatus> {
  return invoke<AccessibilityStatus>("request_accessibility");
}

export async function localModelSettings(): Promise<LocalModelSettings> {
  return invoke<LocalModelSettings>("local_model_settings");
}

export async function setLocalModelSettings(
  settings: LocalModelSettings,
): Promise<LocalModelSettings> {
  return invoke<LocalModelSettings>("set_local_model_settings", { settings });
}

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
