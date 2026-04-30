import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { FloatingRecorder } from "../features/recorder/FloatingRecorder";
import { SettingsPanel } from "../features/settings/SettingsPanel";
import {
  accessibilityStatus,
  fallbackPolicyLabel,
  listMicrophones,
  localModelSettings,
  microphoneStatus,
  recordingStatus,
  requestMicrophoneAccess,
  requestAccessibility,
  selectedMicrophoneId,
  setMicrophoneDevice,
  setLocalModelSettings,
  startRecording,
  stopRecording,
} from "../lib/tauriApi";
import type {
  AccessibilityStatus,
  AudioInputDevice,
  LocalModelSettings,
  MicrophoneStatus,
  StopRecordingOutput,
} from "../types/pipeline";

type RecordingStatus = "idle" | "recording";
type PermissionRequest = "microphone" | "accessibility";
const MICROPHONE_REFRESH_MS = 2000;
const ACCESSIBILITY_REFRESH_MS = 2000;

export function App() {
  const surface = appSurface();
  const isRecorderSurface = surface === "recorder";
  const [status, setStatus] = useState<RecordingStatus>("idle");
  const [fallbackPolicy, setFallbackPolicy] = useState("prefer_local_ask_before_cloud");
  const [microphones, setMicrophones] = useState<AudioInputDevice[]>([]);
  const [selectedMic, setSelectedMic] = useState<string | null>(null);
  const initialMicrophoneStatus: MicrophoneStatus = {
    granted: false,
    canPrompt: true,
  };
  const [microphone, setMicrophone] = useState<MicrophoneStatus>(initialMicrophoneStatus);
  const [accessibility, setAccessibility] = useState<AccessibilityStatus>({
    granted: false,
    canPrompt: true,
  });
  const [modelSettings, setModelSettings] = useState<LocalModelSettings>({
    whisperBinaryPath: "",
    whisperModelPath: "",
    recognitionLanguage: "auto",
  });
  const [lastInsert, setLastInsert] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [requestingPermission, setRequestingPermission] = useState<PermissionRequest | null>(null);
  const [error, setError] = useState<string | null>(null);
  const operationId = useRef(0);
  const statusRef = useRef<RecordingStatus>("idle");
  const pendingRef = useRef(false);
  const microphoneRef = useRef<MicrophoneStatus>(initialMicrophoneStatus);
  const microphoneDowngradeGraceUntilRef = useRef(0);
  const holdDownRef = useRef(false);
  const queuedStopAfterStartRef = useRef(false);

  useEffect(() => {
    document.documentElement.dataset.surface = surface;
    document.body.dataset.surface = surface;
    return () => {
      delete document.documentElement.dataset.surface;
      delete document.body.dataset.surface;
    };
  }, [surface]);

  useEffect(() => {
    let mounted = true;
    const initialOperation = operationId.current;

    void recordingStatus()
      .then((nextStatus) => {
        if (mounted && operationId.current === initialOperation) {
          applyStatus(nextStatus);
        }
      })
      .catch(() => {
        if (mounted && operationId.current === initialOperation) {
          applyStatus("idle");
        }
      });
    if (isRecorderSurface) {
      return () => {
        mounted = false;
      };
    }

    void fallbackPolicyLabel()
      .then(setFallbackPolicy)
      .catch(() => {
        setFallbackPolicy("prefer_local_ask_before_cloud");
      });
    const refreshMountedMicrophones = () =>
      listMicrophones()
        .then((devices) => {
          if (mounted) {
            setMicrophones(devices);
          }
        })
        .catch(() => {
          if (mounted) {
            setMicrophones([]);
          }
        });

    void refreshMountedMicrophones();
    const microphoneRefresh = window.setInterval(() => {
      void refreshMountedMicrophones();
    }, MICROPHONE_REFRESH_MS);
    void selectedMicrophoneId()
      .then((deviceId) => {
        if (mounted) {
          setSelectedMic(deviceId);
        }
      })
      .catch(() => {
        if (mounted) {
          setSelectedMic(null);
        }
      });
    const refreshMountedMicrophoneStatus = () =>
      microphoneStatus()
        .then((nextStatus) => {
          if (mounted) {
            applyMicrophoneStatus(nextStatus);
          }
        })
        .catch(() => {
          if (mounted) {
            applyMicrophoneStatus({ granted: false, canPrompt: true });
          }
        });

    void refreshMountedMicrophoneStatus();
    const microphoneStatusRefresh = window.setInterval(() => {
      void refreshMountedMicrophoneStatus();
    }, MICROPHONE_REFRESH_MS);
    const refreshMountedAccessibility = () =>
      accessibilityStatus()
        .then((nextStatus) => {
          if (mounted) {
            setAccessibility(nextStatus);
          }
        })
        .catch(() => {
          if (mounted) {
            setAccessibility({ granted: false, canPrompt: true });
          }
        });

    void refreshMountedAccessibility();
    const accessibilityRefresh = window.setInterval(() => {
      void refreshMountedAccessibility();
    }, ACCESSIBILITY_REFRESH_MS);
    void localModelSettings()
      .then((nextSettings) => {
        if (mounted) {
          setModelSettings(nextSettings);
        }
      })
      .catch(() => {
        if (mounted) {
          setModelSettings({
            whisperBinaryPath: "",
            whisperModelPath: "",
            recognitionLanguage: "auto",
          });
        }
      });

    return () => {
      mounted = false;
      window.clearInterval(microphoneRefresh);
      window.clearInterval(microphoneStatusRefresh);
      window.clearInterval(accessibilityRefresh);
    };
  }, [isRecorderSurface]);

  useEffect(() => {
    if (!isRecorderSurface) {
      return;
    }

    let disposed = false;
    const unlisten = listen<string>("wispergo://record-shortcut", (event) => {
      if (disposed) {
        return;
      }

      if (event.payload === "Pressed") {
        holdDownRef.current = true;
        startShortcutRecording();
        return;
      }

      if (event.payload === "Released") {
        holdDownRef.current = false;
        stopShortcutRecording();
      }
    });

    return () => {
      disposed = true;
      void unlisten.then((unsubscribe) => unsubscribe());
    };
  }, [isRecorderSurface]);

  function applyStatus(nextStatus: RecordingStatus) {
    statusRef.current = nextStatus;
    setStatus(nextStatus);
  }

  function applyPending(nextPending: boolean) {
    pendingRef.current = nextPending;
    setPending(nextPending);
  }

  function refreshMicrophones() {
    return listMicrophones()
      .then((devices) => {
        setMicrophones(devices);
      })
      .catch(() => {
        setMicrophones([]);
      });
  }

  function refreshAccessibility() {
    return accessibilityStatus()
      .then((nextStatus) => {
        setAccessibility(nextStatus);
        return nextStatus;
      })
      .catch(() => {
        const fallback = { granted: false, canPrompt: true };
        setAccessibility(fallback);
        return fallback;
      });
  }

  function applyMicrophoneStatus(
    nextStatus: MicrophoneStatus,
    options: { allowDowngrade?: boolean } = {},
  ) {
    if (
      microphoneRef.current.granted &&
      !nextStatus.granted &&
      !options.allowDowngrade &&
      Date.now() < microphoneDowngradeGraceUntilRef.current
    ) {
      return microphoneRef.current;
    }

    microphoneRef.current = nextStatus;
    setMicrophone(nextStatus);
    return nextStatus;
  }

  function refreshMicrophoneStatus(options: { allowDowngrade?: boolean } = {}) {
    return microphoneStatus()
      .then((nextStatus) => applyMicrophoneStatus(nextStatus, options))
      .catch(() => {
        const fallback = { granted: false, canPrompt: true };
        return applyMicrophoneStatus(fallback, options);
      });
  }

  function refreshPermissions() {
    return Promise.all([
      refreshAccessibility(),
      refreshMicrophoneStatus({ allowDowngrade: true }),
    ]);
  }

  function explainMissingPermission(kind: PermissionRequest, status: MicrophoneStatus | AccessibilityStatus) {
    if (status.granted) {
      return;
    }

    if (kind === "microphone" && !status.canPrompt) {
      setError("Enable microphone access in macOS Settings, then click Refresh permissions.");
      return;
    }

    if (kind === "accessibility") {
      setError("Enable Accessibility access in macOS Settings, then click Refresh permissions.");
    }
  }

  function startShortcutRecording() {
    if (pendingRef.current || statusRef.current !== "idle") {
      return;
    }

    setLastInsert(null);
    queuedStopAfterStartRef.current = false;
    runRecordingCommand(() => startRecording("press_and_hold"), "recording", undefined, {
      onSettledSuccess: () => {
        if (!holdDownRef.current || queuedStopAfterStartRef.current) {
          queuedStopAfterStartRef.current = false;
          stopShortcutRecording();
        }
      },
    });
  }

  function stopShortcutRecording() {
    if (pendingRef.current) {
      queuedStopAfterStartRef.current = true;
      return;
    }

    stopActiveRecording("global_shortcut");
  }

  function stopActiveRecording(reason: string) {
    if (statusRef.current !== "recording") {
      return;
    }

    runRecordingCommand(
      () => stopRecording(reason),
      "idle",
      (result) => {
        setLastInsert(insertSummary(result));
      },
      { errorStatus: "idle" },
    );
  }

  function runRecordingCommand<T>(
    command: () => Promise<T>,
    nextStatus: RecordingStatus,
    onSuccess?: (result: T) => void,
    options: {
      errorStatus?: RecordingStatus;
      onSettledSuccess?: () => void;
    } = {},
  ) {
    const currentOperation = operationId.current + 1;
    operationId.current = currentOperation;
    applyPending(true);
    setError(null);

    void command()
      .then((result) => {
        if (operationId.current === currentOperation) {
          applyStatus(nextStatus);
          applyPending(false);
          onSuccess?.(result);
          options.onSettledSuccess?.();
        }
      })
      .catch((err: unknown) => {
        if (operationId.current === currentOperation) {
          if (options.errorStatus) {
            applyStatus(options.errorStatus);
          }
          setError(errorMessage(err));
          applyPending(false);
        }
      });
  }

  return (
    <main className={isRecorderSurface ? "app-shell recorder-surface" : "app-shell"}>
      {isRecorderSurface ? (
        <FloatingRecorder status={status} busy={pending} />
      ) : null}
      {lastInsert ? (
        <p className="insert-status" role="status">
          {lastInsert}
        </p>
      ) : null}
      {error ? (
        <p className="command-error" role="status">
          {error}
        </p>
      ) : null}
      {!isRecorderSurface ? (
        <SettingsPanel
          fallbackPolicy={fallbackPolicy}
          microphones={microphones}
          selectedMicrophoneId={selectedMic}
          microphone={microphone}
          accessibility={accessibility}
          modelSettings={modelSettings}
          requestingPermission={requestingPermission}
          onMicrophoneChange={(deviceId) => {
            setSelectedMic(deviceId);
            void setMicrophoneDevice(deviceId).catch((err: unknown) => {
              setError(errorMessage(err));
            });
          }}
          onRefreshMicrophones={() => {
            void refreshMicrophones();
          }}
          onRefreshAccessibility={() => {
            void refreshPermissions();
          }}
          onRequestMicrophoneAccess={() => {
            setRequestingPermission("microphone");
            void requestMicrophoneAccess()
              .then((nextStatus) => {
                if (nextStatus.granted) {
                  microphoneDowngradeGraceUntilRef.current = Date.now() + 5_000;
                }
                applyMicrophoneStatus(nextStatus, { allowDowngrade: true });
                if (nextStatus.granted) {
                  setError(null);
                  return;
                }

                return refreshMicrophoneStatus().then((refreshedStatus) => {
                  explainMissingPermission("microphone", refreshedStatus);
                });
              })
              .catch((err: unknown) => {
                setError(errorMessage(err));
              })
              .finally(() => {
                setRequestingPermission(null);
              });
          }}
          onRequestAccessibility={() => {
            setRequestingPermission("accessibility");
            void requestAccessibility()
              .then((nextStatus) => {
                setAccessibility(nextStatus);
                return refreshAccessibility().then((refreshedStatus) => {
                  explainMissingPermission("accessibility", refreshedStatus);
                });
              })
              .catch((err: unknown) => {
                setError(errorMessage(err));
              })
              .finally(() => {
                setRequestingPermission(null);
              });
          }}
          onModelSettingsSave={(settings) => {
            void setLocalModelSettings(settings)
              .then(setModelSettings)
              .catch((err: unknown) => {
                setError(errorMessage(err));
              });
          }}
        />
      ) : null}
    </main>
  );
}

function appSurface(): "settings" | "recorder" {
  const params = new URLSearchParams(window.location.search);
  return params.get("surface") === "recorder" ? "recorder" : "settings";
}

function insertSummary(output: StopRecordingOutput): string {
  if (output.result.kind !== "insert_text") {
    return "Recording processed";
  }
  if (output.insertion === "inserted") {
    return `Inserted: ${output.result.text}`;
  }
  if (output.insertion === "no_editable_target") {
    return `No editable text field detected; copied to clipboard: ${output.result.text}`;
  }
  if (output.insertion === "accessibility_denied") {
    return `Accessibility permission unavailable; copied to clipboard: ${output.result.text}`;
  }
  if (output.insertion === "secure_field") {
    return "Secure text field detected; not inserted or copied.";
  }

  return `Copied to clipboard; auto-paste failed. Check Accessibility permission: ${output.result.text}`;
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
