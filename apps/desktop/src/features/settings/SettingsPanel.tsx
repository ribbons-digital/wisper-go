import { useEffect, useState } from "react";
import type {
  AccessibilityStatus,
  AudioInputDevice,
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
  requestingPermission = null,
  onMicrophoneChange,
  onRefreshMicrophones,
  onRefreshAccessibility,
  onRequestMicrophoneAccess,
  onRequestAccessibility,
  onModelSettingsSave,
}: Props) {
  const [draftModelSettings, setDraftModelSettings] = useState(modelSettings);

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
          Whisper binary path
          <input
            type="text"
            value={draftModelSettings.whisperBinaryPath}
            onChange={(event) =>
              setDraftModelSettings((current) => ({
                ...current,
                whisperBinaryPath: event.target.value,
              }))
            }
          />
        </label>
        <label>
          Whisper model path
          <input
            type="text"
            value={draftModelSettings.whisperModelPath}
            onChange={(event) =>
              setDraftModelSettings((current) => ({
                ...current,
                whisperModelPath: event.target.value,
              }))
            }
          />
        </label>
        <button type="button" onClick={() => onModelSettingsSave(draftModelSettings)}>
          Save model settings
        </button>
      </div>
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
