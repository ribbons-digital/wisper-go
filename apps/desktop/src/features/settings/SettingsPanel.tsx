import { useEffect, useState } from "react";
import type {
  AccessibilityStatus,
  AudioInputDevice,
  CleanupRuntimeStatus,
  LocalModelSettings,
  MicrophoneStatus,
} from "../../types/pipeline";

type PermissionRequest = "microphone" | "accessibility";

type Props = {
  fallbackPolicy: string;
  microphones: AudioInputDevice[];
  selectedMicrophoneId: string | null;
  microphone: MicrophoneStatus;
  accessibility: AccessibilityStatus;
  modelSettings: LocalModelSettings;
  cleanupRuntime?: CleanupRuntimeStatus | null;
  requestingPermission?: PermissionRequest | null;
  onMicrophoneChange: (deviceId: string) => void;
  onRefreshMicrophones: () => void;
  onRefreshAccessibility: () => void;
  onRequestMicrophoneAccess: () => void;
  onRequestAccessibility: () => void;
  onModelSettingsSave: (settings: LocalModelSettings) => void;
};

export function SettingsPanel({
  fallbackPolicy,
  microphones,
  selectedMicrophoneId,
  microphone,
  accessibility,
  modelSettings,
  cleanupRuntime = null,
  requestingPermission = null,
  onMicrophoneChange,
  onRefreshMicrophones,
  onRefreshAccessibility,
  onRequestMicrophoneAccess,
  onRequestAccessibility,
  onModelSettingsSave,
}: Props) {
  const [draftModelSettings, setDraftModelSettings] = useState(modelSettings);
  const cleanupEnabled = draftModelSettings.cleanupMode !== "off";

  useEffect(() => {
    setDraftModelSettings(modelSettings);
  }, [modelSettings]);

  return (
    <section className="settings-panel" aria-label="Settings">
      <div className="shortcut-row">
        <span>Shortcut</span>
        <strong>Hold Command + Shift + Space</strong>
      </div>
      <div className="model-settings">
        <label>
          Recognition language
          <select
            value={draftModelSettings.recognitionLanguage}
            onChange={(event) =>
              setDraftModelSettings((current) => ({
                ...current,
                recognitionLanguage: event.target.value as LocalModelSettings["recognitionLanguage"],
              }))
            }
          >
            <option value="auto">Auto</option>
            <option value="en">English</option>
            <option value="zh">Chinese</option>
          </select>
        </label>
        <label>
          Cleanup mode
          <select
            value={draftModelSettings.cleanupMode}
            onChange={(event) =>
              setDraftModelSettings((current) => ({
                ...current,
                cleanupMode: event.target.value as LocalModelSettings["cleanupMode"],
              }))
            }
          >
            <option value="off">Off (raw transcript)</option>
            <option value="punctuation_only">Punctuation only</option>
            <option value="full_cleanup">Full cleanup and commands</option>
          </select>
        </label>
        <button type="button" onClick={() => onModelSettingsSave(draftModelSettings)}>
          Save model settings
        </button>
      </div>
      {cleanupEnabled && cleanupRuntime ? <CleanupRuntimeNotice status={cleanupRuntime} /> : null}
      <label>
        Microphone input
        <div className="microphone-row">
          <select
            value={selectedMicrophoneId ?? ""}
            onFocus={onRefreshMicrophones}
            onChange={(event) => onMicrophoneChange(event.target.value)}
            disabled={microphones.length === 0}
          >
            {microphones.length === 0 ? <option value="">No microphones found</option> : null}
            {microphones.map((microphone) => (
              <option key={microphone.id} value={microphone.id}>
                {microphone.name}
              </option>
            ))}
          </select>
          <button type="button" onClick={onRefreshMicrophones}>
            Refresh
          </button>
        </div>
      </label>
      <div className="permission-row">
        <span>{microphone.granted ? "Microphone granted" : "Microphone missing"}</span>
        {!microphone.granted ? (
          <button
            type="button"
            onClick={onRequestMicrophoneAccess}
            disabled={requestingPermission === "microphone"}
          >
            {requestingPermission === "microphone" ? "Requesting microphone…" : "Grant microphone"}
          </button>
        ) : null}
      </div>
      <div className="permission-row">
        <span>{accessibility.granted ? "Accessibility granted" : "Accessibility missing"}</span>
        <button type="button" onClick={onRefreshAccessibility}>
          Refresh permissions
        </button>
        {!accessibility.granted ? (
          <button
            type="button"
            onClick={onRequestAccessibility}
            disabled={requestingPermission === "accessibility"}
          >
            {requestingPermission === "accessibility"
              ? "Requesting accessibility…"
              : "Grant accessibility"}
          </button>
        ) : null}
      </div>
      <p>Fallback policy: {fallbackPolicy}</p>
    </section>
  );
}

function CleanupRuntimeNotice({ status }: { status: CleanupRuntimeStatus }) {
  if (status.state === "ready") {
    return (
      <div className="cleanup-runtime" aria-live="polite">
        Offline punctuation ready.
      </div>
    );
  }

  if (status.state === "starting") {
    return (
      <div className="cleanup-runtime" aria-live="polite">
        Preparing offline punctuation. Wispergo will use raw transcripts until it is ready.
      </div>
    );
  }

  return (
    <div className="cleanup-runtime" aria-live="polite">
      {status.message ?? "Offline punctuation is unavailable."} Wispergo will use raw transcripts.
    </div>
  );
}
