import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type {
  AccessibilityStatus,
  AssetDownloadStatus,
  AudioInputDevice,
  CleanupRuntimeStatus,
  LocalModelSettings,
  MicrophoneStatus,
} from "../../types/pipeline";
import {
  ASSET_DOWNLOAD_EVENT,
  assetReadiness,
  ensureModelAssets,
} from "../../lib/tauriApi";

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
  const [assetStatus, setAssetStatus] = useState<AssetDownloadStatus | null>(null);
  const [downloadingAssets, setDownloadingAssets] = useState(false);
  const cleanupEnabled = draftModelSettings.cleanupMode !== "off";
  const setup = setupSummary(microphone, accessibility, assetStatus);

  useEffect(() => {
    setDraftModelSettings(modelSettings);
  }, [modelSettings]);

  useEffect(() => {
    let cancelled = false;
    const unlisten = listen<AssetDownloadStatus>(ASSET_DOWNLOAD_EVENT, (event) => {
      if (!cancelled) {
        setAssetStatus(event.payload);
        if (event.payload.state !== "downloading") {
          setDownloadingAssets(false);
        }
      }
    });
    void assetReadiness().then((status) => {
      if (!cancelled) setAssetStatus(status);
    });
    return () => {
      cancelled = true;
      void unlisten.then((unsubscribe) => unsubscribe());
    };
  }, []);

  const handleDownloadAssets = () => {
    setDownloadingAssets(true);
    void ensureModelAssets().then((status) => {
      setAssetStatus(status);
      setDownloadingAssets(false);
    });
  };

  useEffect(() => {
    if (assetStatus?.state === "missing" && !downloadingAssets) {
      handleDownloadAssets();
    }
  }, [assetStatus, downloadingAssets]);

  return (
    <section className="settings-panel" aria-label="Settings">
      <header className="settings-hero" aria-label="Setup status">
        <div className="settings-hero-copy">
          <p className="settings-kicker">Wispergo</p>
          <h2>{setup.ready ? "Ready for dictation" : "Finish setup"}</h2>
          <p>
            {setup.ready
              ? "Hold the shortcut and speak to begin"
              : "Grant permissions and download the required model to start dictating"}
          </p>
          <div className="settings-hero-facts" aria-label="Setup summary">
            <span><SettingsIcon name="microphone" />{microphone.granted ? "Microphone granted" : "Microphone missing"}</span>
            <span><SettingsIcon name="accessibility" />{accessibility.granted ? "Accessibility granted" : "Accessibility missing"}</span>
            <span><SettingsIcon name="keyboard" />⌘ ⇧ Space</span>
          </div>
        </div>
        <strong className={setup.ready ? "settings-status is-ready" : "settings-status needs-setup"}>
          <span aria-hidden="true" />
          {setup.ready ? "Ready" : "Setup needed"}
        </strong>
      </header>

      <div className="settings-dashboard-grid">
        <section className="settings-card setup-card" aria-label="Setup checklist">
          <div className="settings-card-heading">
            <h3>Setup</h3>
            <span>Required</span>
          </div>
          <ul className="setup-checklist" aria-label="Setup checklist">
            <SetupChecklistItem icon="microphone" label="Microphone" status={setup.microphone} />
            <SetupChecklistItem icon="accessibility" label="Accessibility" status={setup.accessibility} />
            <SetupChecklistItem icon="chip" label="Local models" status={setup.models} />
          </ul>
          <div className="permission-actions settings-card-actions">
            {!microphone.granted ? (
              <button
                type="button"
                onClick={onRequestMicrophoneAccess}
                disabled={requestingPermission === "microphone"}
              >
                <SettingsIcon name="microphone" />
                {requestingPermission === "microphone" ? "Requesting microphone…" : "Grant microphone"}
              </button>
            ) : null}
            <button type="button" onClick={onRefreshAccessibility}>
              <SettingsIcon name="refresh" />
              Refresh permissions
            </button>
            {!accessibility.granted ? (
              <button
                type="button"
                onClick={onRequestAccessibility}
                disabled={requestingPermission === "accessibility"}
              >
                <SettingsIcon name="accessibility" />
                {requestingPermission === "accessibility" ? "Requesting accessibility…" : "Grant accessibility"}
              </button>
            ) : null}
          </div>
          <AssetDownloadNotice
            status={assetStatus}
            downloading={downloadingAssets}
            onDownload={handleDownloadAssets}
          />
        </section>

        <section className="settings-card input-card" aria-label="Input preferences">
          <div className="settings-card-heading">
            <h3>Input</h3>
            <span>Microphone</span>
          </div>
          <label className="settings-field microphone-field">
            <span>Source</span>
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
          </label>
          <button type="button" onClick={onRefreshMicrophones}>
            <SettingsIcon name="refresh" />
            Refresh devices
          </button>
        </section>

        <section className="settings-card model-settings" aria-label="Dictation preferences">
          <div className="settings-card-heading">
            <h3>Dictation</h3>
            <span>Local-first</span>
          </div>
          <div className="dictation-fields">
            <label className="settings-field">
              <span>Language</span>
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
                <option value="zh">Chinese / Mixed</option>
              </select>
            </label>
            <label className="settings-field">
              <span>ASR model</span>
              <select
                value={draftModelSettings.asrModelId}
                onChange={(event) =>
                  setDraftModelSettings((current) => ({
                    ...current,
                    asrModelId: event.target.value as LocalModelSettings["asrModelId"],
                  }))
                }
              >
                <option value="medium">Medium</option>
                <option value="large-v3-turbo">Accuracy Pack</option>
              </select>
            </label>
            <label className="settings-field">
              <span>Cleanup</span>
              <select
                value={draftModelSettings.cleanupMode}
                onChange={(event) =>
                  setDraftModelSettings((current) => ({
                    ...current,
                    cleanupMode: event.target.value as LocalModelSettings["cleanupMode"],
                  }))
                }
              >
                <option value="off">Off</option>
                <option value="punctuation_only">Punctuation</option>
                <option value="full_cleanup">Full cleanup</option>
              </select>
            </label>
          </div>
          {cleanupEnabled && cleanupRuntime ? <CleanupRuntimeNotice status={cleanupRuntime} /> : null}
          <div className="settings-notes" aria-label="Dictation notes">
            <p className="settings-note">
              Use Chinese / Mixed for Chinese-English dictation. Full cleanup downloads the optional 3B pack before activation.
            </p>
          </div>
          <button className="settings-primary-action" type="button" onClick={() => onModelSettingsSave(draftModelSettings)}>
            Save changes
          </button>
        </section>
      </div>
    </section>
  );
}

type SetupItemStatus = "Ready" | "Checking" | "Needs permission" | "Needs download" | "Downloading" | "Failed";

type SetupSummary = {
  ready: boolean;
  microphone: SetupItemStatus;
  accessibility: SetupItemStatus;
  models: SetupItemStatus;
};

function setupSummary(
  microphone: MicrophoneStatus,
  accessibility: AccessibilityStatus,
  assetStatus: AssetDownloadStatus | null,
): SetupSummary {
  const microphoneReady = microphone.granted;
  const accessibilityReady = accessibility.granted;
  const models = modelSetupStatus(assetStatus);
  const modelsReady = models === "Ready";

  return {
    ready: microphoneReady && accessibilityReady && modelsReady,
    microphone: microphoneReady ? "Ready" : "Needs permission",
    accessibility: accessibilityReady ? "Ready" : "Needs permission",
    models,
  };
}

function modelSetupStatus(status: AssetDownloadStatus | null): SetupItemStatus {
  if (!status) return "Checking";
  if (status.state === "ready") return "Ready";
  if (status.state === "missing") return "Needs download";
  if (status.state === "downloading") return "Downloading";
  return "Failed";
}

type SettingsIconName = "accessibility" | "check" | "chevron" | "chip" | "keyboard" | "microphone" | "refresh";

function SetupChecklistItem({ icon, label, status }: { icon: SettingsIconName; label: string; status: SetupItemStatus }) {
  const ready = status === "Ready";
  return (
    <li>
      <SettingsIcon name={icon} />
      <span>{label}</span>
      <strong className={ready ? "is-ready" : "needs-setup"}>{status}</strong>
    </li>
  );
}

function SettingsIcon({ name }: { name: SettingsIconName }) {
  return (
    <svg className={`settings-icon settings-icon-${name}`} aria-hidden="true" viewBox="0 0 20 20" focusable="false">
      {settingsIconPath(name)}
    </svg>
  );
}

function settingsIconPath(name: SettingsIconName) {
  switch (name) {
    case "accessibility":
      return (
        <>
          <circle cx="10" cy="10" r="7" />
          <circle cx="10" cy="6.8" r="1.2" />
          <path d="M6.8 9.2h6.4M10 8.9v3.1M8.1 15l1.9-3 1.9 3" />
        </>
      );
    case "check":
      return <path d="M4.5 10.5 8.2 14 15.5 6.5" />;
    case "chevron":
      return <path d="m5.5 7.5 4.5 4.5 4.5-4.5" />;
    case "chip":
      return (
        <>
          <rect x="5.8" y="5.8" width="8.4" height="8.4" rx="1.2" />
          <path d="M8 3.5v2.3M12 3.5v2.3M8 14.2v2.3M12 14.2v2.3M3.5 8h2.3M3.5 12h2.3M14.2 8h2.3M14.2 12h2.3" />
        </>
      );
    case "keyboard":
      return (
        <>
          <rect x="3.5" y="6" width="13" height="8" rx="1.6" />
          <path d="M6 8.6h.1M8.5 8.6h.1M11 8.6h.1M13.5 8.6h.1M6 11.4h4.3M12.6 11.4h1.4" />
        </>
      );
    case "microphone":
      return (
        <>
          <rect x="7" y="3.5" width="6" height="9" rx="3" />
          <path d="M4.8 9.5a5.2 5.2 0 0 0 10.4 0M10 14.7v2.2M7.5 16.9h5" />
        </>
      );
    case "refresh":
      return <path d="M14.5 6.2A5.8 5.8 0 0 0 4.2 9.8M14.5 6.2V3.8M14.5 6.2h-2.7M5.5 13.8a5.8 5.8 0 0 0 10.3-3.6M5.5 13.8v2.4M5.5 13.8h2.7" />;
  }
}

function CleanupRuntimeNotice({ status }: { status: CleanupRuntimeStatus }) {
  if (status.state === "ready") {
    return (
      <div className="cleanup-runtime" aria-live="polite">
        <SettingsIcon name="check" />
        Offline punctuation ready
      </div>
    );
  }

  if (status.state === "starting") {
    return (
      <div className="cleanup-runtime" aria-live="polite">
        <SettingsIcon name="refresh" />
        Preparing offline punctuation. Wispergo will use raw transcripts until it is ready.
      </div>
    );
  }

  return (
    <div className="cleanup-runtime" aria-live="polite">
      <SettingsIcon name="refresh" />
      {status.message ?? "Offline punctuation is unavailable."} Wispergo will use raw transcripts.
    </div>
  );
}

function AssetDownloadNotice({
  status,
  downloading,
  onDownload,
}: {
  status: AssetDownloadStatus | null;
  downloading: boolean;
  onDownload: () => void;
}) {
  // Until the manifest is populated (Phase 5), readiness reports Ready and
  // there is nothing to download. Show nothing in that case to avoid a
  // confusing empty affordance.
  if (!status || status.state === "ready") {
    return null;
  }

  if (status.state === "missing") {
    return (
      <div className="asset-download" aria-live="polite">
        <SettingsIcon name="refresh" />
        Model download needed: {status.displayName}. Starting download…
      </div>
    );
  }

  if (status.state === "downloading") {
    return (
      <div className="asset-download" aria-live="polite">
        <SettingsIcon name="refresh" />
        Downloading models: {status.displayName}…
      </div>
    );
  }

  return (
    <div className="asset-download" aria-live="polite">
      <SettingsIcon name="refresh" />
      {status.message ?? "Model download failed."}{" "}
      <button type="button" onClick={onDownload} disabled={downloading}>
        {downloading ? "Retrying…" : "Retry download"}
      </button>
    </div>
  );
}
