import { useEffect, useRef, useState } from "react";
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
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const operationId = useRef(0);

  useEffect(() => {
    let mounted = true;
    const initialOperation = operationId.current;

    void recordingStatus()
      .then((nextStatus) => {
        if (mounted && operationId.current === initialOperation) {
          setStatus(nextStatus);
        }
      })
      .catch(() => {
        if (mounted && operationId.current === initialOperation) {
          setStatus("idle");
        }
      });
    void fallbackPolicyLabel()
      .then(setFallbackPolicy)
      .catch(() => {
        setFallbackPolicy("prefer_local_ask_before_cloud");
      });

    return () => {
      mounted = false;
    };
  }, []);

  function runRecordingCommand(command: () => Promise<void>, nextStatus: RecordingStatus) {
    const currentOperation = operationId.current + 1;
    operationId.current = currentOperation;
    setPending(true);
    setError(null);

    void command()
      .then(() => {
        if (operationId.current === currentOperation) {
          setStatus(nextStatus);
        }
      })
      .catch((err: unknown) => {
        if (operationId.current === currentOperation) {
          setError(errorMessage(err));
        }
      })
      .finally(() => {
        if (operationId.current === currentOperation) {
          setPending(false);
        }
      });
  }

  return (
    <main className="app-shell">
      <FloatingRecorder
        status={status}
        mode={mode}
        disabled={pending}
        onStart={(nextMode) => {
          runRecordingCommand(() => startRecording(nextMode), "recording");
        }}
        onStop={(reason) => {
          runRecordingCommand(() => stopRecording(reason), "idle");
        }}
        onCancel={(reason) => {
          runRecordingCommand(() => cancelRecording(reason), "idle");
        }}
      />
      {error ? (
        <p className="command-error" role="status">
          {error}
        </p>
      ) : null}
      <SettingsPanel mode={mode} fallbackPolicy={fallbackPolicy} onModeChange={setMode} />
    </main>
  );
}

function errorMessage(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  if (typeof err === "string") {
    return err;
  }
  return "Command failed";
}
