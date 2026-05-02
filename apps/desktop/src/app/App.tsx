import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { FloatingRecorder } from "../features/recorder/FloatingRecorder";
import { LanguageToggle } from "../features/recorder/LanguageToggle";
import { SettingsPanel } from "../features/settings/SettingsPanel";
import {
  accessibilityStatus,
  cleanupRuntimeStatus,
  fallbackPolicyLabel,
  listMicrophones,
  localModelSettings,
  microphoneStatus,
  recognitionLanguage,
  recordingStatus,
  requestMicrophoneAccess,
  requestAccessibility,
  selectedMicrophoneId,
  setLanguageMenuOpen,
  setFloatingChromeReason,
  setMicrophoneDevice,
  setLocalModelSettings,
  setRecognitionLanguage,
  startRecording,
  stopRecording,
} from "../lib/tauriApi";
import type {
  AccessibilityStatus,
  AudioInputDevice,
  CleanupRuntimeStatus,
  LocalModelSettings,
  MicrophoneStatus,
  RecognitionLanguage,
  StopRecordingOutput,
} from "../types/pipeline";

type RecordingStatus = "idle" | "recording";
type PermissionRequest = "microphone" | "accessibility";
const MICROPHONE_REFRESH_MS = 2000;
const ACCESSIBILITY_REFRESH_MS = 2000;
const CLEANUP_RUNTIME_REFRESH_MS = 2000;
const POST_INSERT_EXPANDED_MS = 1500;
const RECOGNITION_LANGUAGES = [
  { value: "auto", label: "Auto" },
  { value: "en", label: "English" },
  { value: "zh", label: "Chinese" },
] as const;

export function App() {
  const surface = appSurface();
  const isRecorderSurface = surface === "recorder";
  const isLanguageSurface = surface === "language";
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
    cleanupMode: "punctuation_only",
  });
  const [modelSettingsLoaded, setModelSettingsLoaded] = useState(false);
  const [cleanupRuntime, setCleanupRuntime] = useState<CleanupRuntimeStatus | null>(null);
  const [languageMenuOpen, setLanguageMenuOpenState] = useState(false);
  const [languageNativeHovered, setLanguageNativeHovered] = useState(false);
  const [floatingChromeExpanded, setFloatingChromeExpanded] = useState(false);
  const [lastInsert, setLastInsert] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [requestingPermission, setRequestingPermission] = useState<PermissionRequest | null>(null);
  const [error, setError] = useState<string | null>(null);
  const operationId = useRef(0);
  const statusRef = useRef<RecordingStatus>("idle");
  const pendingRef = useRef(false);
  const microphoneRef = useRef<MicrophoneStatus>(initialMicrophoneStatus);
  const microphoneDowngradeGraceUntilRef = useRef(0);
  const languageMenuOpenRef = useRef(false);
  const postInsertTimerRef = useRef<number | null>(null);
  const postInsertGraceActiveRef = useRef(false);
  const holdDownRef = useRef(false);
  const queuedStopAfterStartRef = useRef(false);

  useEffect(() => {
    if (isRecorderSurface || isLanguageSurface || !modelSettingsLoaded) {
      return;
    }

    if (modelSettings.cleanupMode === "off") {
      setCleanupRuntime(null);
      return;
    }

    let mounted = true;
    const refreshCleanupRuntime = () => {
      void cleanupRuntimeStatus()
        .then((status) => {
          if (mounted) {
            setCleanupRuntime(status);
          }
        })
        .catch(() => {
          if (mounted) {
            setCleanupRuntime({
              state: "unavailable",
              message: "Offline punctuation is unavailable.",
            });
          }
        });
    };

    refreshCleanupRuntime();
    const cleanupRuntimeRefresh = window.setInterval(
      refreshCleanupRuntime,
      CLEANUP_RUNTIME_REFRESH_MS,
    );

    return () => {
      mounted = false;
      window.clearInterval(cleanupRuntimeRefresh);
    };
  }, [isRecorderSurface, isLanguageSurface, modelSettingsLoaded, modelSettings.cleanupMode]);

  useEffect(() => {
    document.documentElement.dataset.surface = surface;
    document.body.dataset.surface = surface;
    return () => {
      delete document.documentElement.dataset.surface;
      delete document.body.dataset.surface;
    };
  }, [surface]);

  useEffect(() => {
    languageMenuOpenRef.current = languageMenuOpen;
  }, [languageMenuOpen]);

  useEffect(() => {
    return () => {
      if (postInsertTimerRef.current !== null) {
        window.clearTimeout(postInsertTimerRef.current);
        postInsertTimerRef.current = null;
      }
      if (postInsertGraceActiveRef.current) {
        postInsertGraceActiveRef.current = false;
        void setFloatingChromeReason("post_insert", false).catch(() => undefined);
      }
    };
  }, []);

  useEffect(() => {
    if (!isRecorderSurface && !isLanguageSurface) {
      return;
    }

    let mounted = true;
    const unlisten = listen<boolean>("wispergo://floating-chrome-expanded-changed", (event) => {
      if (mounted) {
        setFloatingChromeExpanded(event.payload);
      }
    });

    return () => {
      mounted = false;
      void unlisten.then((unsubscribe) => unsubscribe());
    };
  }, [isRecorderSurface, isLanguageSurface]);

  useEffect(() => {
    if (!isRecorderSurface) {
      return;
    }

    void setFloatingChromeReason("recording", status === "recording").catch((err: unknown) => {
      setError(errorMessage(err));
    });
  }, [isRecorderSurface, status]);

  useEffect(() => {
    if (!isRecorderSurface) {
      return;
    }

    void setFloatingChromeReason("processing", pending).catch((err: unknown) => {
      setError(errorMessage(err));
    });
  }, [isRecorderSurface, pending]);

  useEffect(() => {
    let mounted = true;

    void recognitionLanguage()
      .then((language) => {
        if (mounted) {
          applyRecognitionLanguage(language);
        }
      })
      .catch(() => {
        if (mounted) {
          applyRecognitionLanguage("auto");
        }
      });

    const unlisten = listen<RecognitionLanguage>("wispergo://recognition-language-changed", (event) => {
      if (mounted) {
        applyRecognitionLanguage(event.payload);
      }
    });

    return () => {
      mounted = false;
      void unlisten.then((unsubscribe) => unsubscribe());
    };
  }, []);

  useEffect(() => {
    if (!isLanguageSurface) {
      return;
    }

    let mounted = true;
    const unlisten = listen<boolean>("wispergo://language-hover-changed", (event) => {
      if (!mounted) {
        return;
      }

      setLanguageNativeHovered(event.payload);
      if (!event.payload && languageMenuOpenRef.current) {
        updateLanguageMenuOpen(false);
      }
    });

    return () => {
      mounted = false;
      void unlisten.then((unsubscribe) => unsubscribe());
    };
  }, [isLanguageSurface]);

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
    if (isRecorderSurface || isLanguageSurface) {
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
          setModelSettingsLoaded(true);
        }
      })
      .catch(() => {
        if (mounted) {
          setModelSettings({
            whisperBinaryPath: "",
            whisperModelPath: "",
            recognitionLanguage: "auto",
            cleanupMode: "punctuation_only",
          });
          setModelSettingsLoaded(true);
        }
      });

    return () => {
      mounted = false;
      window.clearInterval(microphoneRefresh);
      window.clearInterval(microphoneStatusRefresh);
      window.clearInterval(accessibilityRefresh);
    };
  }, [isRecorderSurface, isLanguageSurface]);

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

  function startPostInsertExpandedGrace() {
    if (!isRecorderSurface) {
      return;
    }
    if (postInsertTimerRef.current !== null) {
      window.clearTimeout(postInsertTimerRef.current);
    }

    postInsertGraceActiveRef.current = true;
    void setFloatingChromeReason("post_insert", true).catch((err: unknown) => {
      setError(errorMessage(err));
    });
    postInsertTimerRef.current = window.setTimeout(() => {
      postInsertTimerRef.current = null;
      postInsertGraceActiveRef.current = false;
      void setFloatingChromeReason("post_insert", false).catch((err: unknown) => {
        setError(errorMessage(err));
      });
    }, POST_INSERT_EXPANDED_MS);
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
      { errorStatus: "idle", onSettled: startPostInsertExpandedGrace },
    );
  }

  function runRecordingCommand<T>(
    command: () => Promise<T>,
    nextStatus: RecordingStatus,
    onSuccess?: (result: T) => void,
    options: {
      errorStatus?: RecordingStatus;
      onSettledSuccess?: () => void;
      onSettled?: () => void;
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
          options.onSettled?.();
        }
      })
      .catch((err: unknown) => {
        if (operationId.current === currentOperation) {
          if (options.errorStatus) {
            applyStatus(options.errorStatus);
          }
          setError(errorMessage(err));
          applyPending(false);
          options.onSettled?.();
        }
      });
  }

  function nextRecognitionLanguage(language: RecognitionLanguage): RecognitionLanguage {
    if (language === "auto") {
      return "en";
    }
    if (language === "en") {
      return "zh";
    }
    return "auto";
  }

  function applyRecognitionLanguage(language: RecognitionLanguage) {
    setModelSettings((current) => ({ ...current, recognitionLanguage: language }));
  }

  function updateRecognitionLanguage(language: RecognitionLanguage) {
    applyRecognitionLanguage(language);
    void setRecognitionLanguage(language).catch((err: unknown) => {
      setError(errorMessage(err));
    });
  }

  function updateLanguageMenuOpen(open: boolean) {
    setLanguageMenuOpenState(open);
    void setLanguageMenuOpen(open).catch((err: unknown) => {
      setError(errorMessage(err));
    });
  }

  return (
    <main
      className={
        isRecorderSurface
          ? "app-shell recorder-surface"
          : isLanguageSurface
            ? "app-shell language-surface"
            : "app-shell"
      }
    >
      {isRecorderSurface ? (
        <FloatingRecorder status={status} busy={pending} expanded={floatingChromeExpanded} />
      ) : null}
      {isLanguageSurface ? (
        <LanguageToggle
          language={modelSettings.recognitionLanguage}
          languages={RECOGNITION_LANGUAGES}
          menuOpen={languageMenuOpen}
          nativeHovered={languageNativeHovered}
          onCycle={() => updateRecognitionLanguage(nextRecognitionLanguage(modelSettings.recognitionLanguage))}
          onSelect={(language) => {
            updateRecognitionLanguage(language);
            updateLanguageMenuOpen(false);
          }}
          onMenuOpenChange={updateLanguageMenuOpen}
          onNativeHoverEnd={() => {
            setLanguageNativeHovered(false);
            void setFloatingChromeReason("language_hover", false).catch((err: unknown) => {
              setError(errorMessage(err));
            });
          }}
        />
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
      {!isRecorderSurface && !isLanguageSurface ? (
        <SettingsPanel
          fallbackPolicy={fallbackPolicy}
          microphones={microphones}
          selectedMicrophoneId={selectedMic}
          microphone={microphone}
          accessibility={accessibility}
          modelSettings={modelSettings}
          cleanupRuntime={cleanupRuntime}
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

function appSurface(): "settings" | "recorder" | "language" {
  const params = new URLSearchParams(window.location.search);
  const surface = params.get("surface");
  if (surface === "recorder" || surface === "language") {
    return surface;
  }
  return "settings";
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

function errorMessage(_err: unknown): string {
  return "Wispergo could not complete that action. Check permissions and try again.";
}
