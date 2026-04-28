import { useEffect, useState } from "react";
import { FloatingRecorder } from "../features/recorder/FloatingRecorder";
import { SettingsPanel } from "../features/settings/SettingsPanel";
import {
  cancelRecording,
  fallbackPolicyLabel,
  recordingStatus,
  startRecording,
  stopRecording,
} from "../lib/tauriApi";
import type { RecordingMode } from "../types/pipeline";

type RecordingStatus = "idle" | "recording";

export function App() {
  const [status, setStatus] = useState<RecordingStatus>("idle");
  const [mode, setMode] = useState<RecordingMode>("toggle");
  const [fallbackPolicy, setFallbackPolicy] = useState("prefer_local_ask_before_cloud");

  useEffect(() => {
    void recordingStatus()
      .then(setStatus)
      .catch(() => setStatus("idle"));
    void fallbackPolicyLabel()
      .then(setFallbackPolicy)
      .catch(() => {
        setFallbackPolicy("prefer_local_ask_before_cloud");
      });
  }, []);

  return (
    <main className="app-shell">
      <FloatingRecorder
        status={status}
        mode={mode}
        onStart={(nextMode) => {
          void startRecording(nextMode).then(() => setStatus("recording"));
        }}
        onStop={(reason) => {
          void stopRecording(reason).then(() => setStatus("idle"));
        }}
        onCancel={(reason) => {
          void cancelRecording(reason).then(() => setStatus("idle"));
        }}
      />
      <SettingsPanel mode={mode} fallbackPolicy={fallbackPolicy} onModeChange={setMode} />
    </main>
  );
}
