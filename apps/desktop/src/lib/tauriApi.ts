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
