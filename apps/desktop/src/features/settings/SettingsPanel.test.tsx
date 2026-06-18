import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("../../lib/tauriApi", () => ({
  ASSET_DOWNLOAD_EVENT: "wispergo://asset-download",
  assetReadiness: vi.fn().mockResolvedValue({ state: "ready" }),
  ensureModelAssets: vi.fn().mockResolvedValue({ state: "ready" }),
}));

type SettingsPanelProps = Parameters<typeof SettingsPanel>[0];

function renderSettingsPanel(overrides: Partial<SettingsPanelProps> = {}) {
  const props: SettingsPanelProps = {
    fallbackPolicy: "prefer_local_ask_before_cloud",
    microphones: [],
    selectedMicrophoneId: null,
    microphone: { granted: true, canPrompt: true },
    accessibility: { granted: true, canPrompt: true },
    modelSettings: {
      whisperBinaryPath: "",
      whisperModelPath: "",
      recognitionLanguage: "auto",
      cleanupMode: "punctuation_only",
    },
    requestingPermission: null,
    onMicrophoneChange: vi.fn(),
    onRefreshMicrophones: vi.fn(),
    onRefreshAccessibility: vi.fn(),
    onRequestMicrophoneAccess: vi.fn(),
    onRequestAccessibility: vi.fn(),
    onModelSettingsSave: vi.fn(),
    ...overrides,
  };

  return {
    ...render(<SettingsPanel {...props} />),
    props,
  };
}

describe("SettingsPanel", () => {
  it("hides local model path fields", () => {
    renderSettingsPanel();

    expect(screen.queryByLabelText(/Whisper binary path/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/Whisper model path/i)).not.toBeInTheDocument();
  });

  it("saves recognition language with local model settings", async () => {
    const user = userEvent.setup();
    const onModelSettingsSave = vi.fn();
    renderSettingsPanel({
      onModelSettingsSave,
      modelSettings: {
        whisperBinaryPath: "/usr/local/bin/whisper-cli",
        whisperModelPath: "/models/base.bin",
        recognitionLanguage: "auto",
        cleanupMode: "punctuation_only",
      },
    });

    await user.selectOptions(screen.getByLabelText("Recognition language"), "zh");
    await user.click(screen.getByRole("button", { name: "Save model settings" }));

    expect(onModelSettingsSave).toHaveBeenCalledWith({
      whisperBinaryPath: "/usr/local/bin/whisper-cli",
      whisperModelPath: "/models/base.bin",
      recognitionLanguage: "zh",
      cleanupMode: "punctuation_only",
    });
  });

  it("saves cleanup mode with local model settings", async () => {
    const user = userEvent.setup();
    const onModelSettingsSave = vi.fn();
    renderSettingsPanel({
      onModelSettingsSave,
      modelSettings: {
        whisperBinaryPath: "/usr/local/bin/whisper-cli",
        whisperModelPath: "/models/base.bin",
        recognitionLanguage: "auto",
        cleanupMode: "punctuation_only",
      },
    });

    await user.selectOptions(screen.getByLabelText("Cleanup mode"), "full_cleanup");
    await user.click(screen.getByRole("button", { name: "Save model settings" }));

    expect(onModelSettingsSave).toHaveBeenCalledWith({
      whisperBinaryPath: "/usr/local/bin/whisper-cli",
      whisperModelPath: "/models/base.bin",
      recognitionLanguage: "auto",
      cleanupMode: "full_cleanup",
    });
  });

  it("shows ready offline punctuation status", () => {
    renderSettingsPanel({
      cleanupRuntime: { state: "ready", message: null },
    });

    expect(screen.getByText("Offline punctuation ready.")).toBeInTheDocument();
  });

  it("shows unavailable offline punctuation status and raw transcripts fallback", () => {
    renderSettingsPanel({
      cleanupRuntime: { state: "unavailable", message: "Offline punctuation is unavailable." },
    });

    expect(screen.getByText(/Offline punctuation is unavailable/)).toBeInTheDocument();
    expect(screen.getByText(/raw transcripts/)).toBeInTheDocument();
  });

  it("hides cleanup runtime status when cleanup mode is off", () => {
    renderSettingsPanel({
      modelSettings: {
        whisperBinaryPath: "",
        whisperModelPath: "",
        recognitionLanguage: "auto",
        cleanupMode: "off",
      },
      cleanupRuntime: { state: "ready", message: null },
    });

    expect(screen.queryByText(/Offline punctuation ready/)).not.toBeInTheDocument();
  });

  it("does not render Ollama install links or model text", () => {
    renderSettingsPanel({
      cleanupRuntime: { state: "ready", message: null },
    });

    expect(screen.queryByText(/Install Ollama/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/ollama\.com/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/qwen2\.5/i)).not.toBeInTheDocument();
  });

  it("changes microphone input", async () => {
    const user = userEvent.setup();
    const onMicrophoneChange = vi.fn();

    renderSettingsPanel({
      microphones: [
        { id: "default", name: "System Default", isDefault: true },
        { id: "2", name: "USB Mic", isDefault: false },
      ],
      selectedMicrophoneId: "default",
      accessibility: { granted: false, canPrompt: true },
      onMicrophoneChange,
    });

    await user.selectOptions(screen.getByLabelText("Microphone input"), "2");
    expect(onMicrophoneChange).toHaveBeenCalledWith("2");
  });

  it("requests accessibility permission when missing", async () => {
    const user = userEvent.setup();
    const onRequestAccessibility = vi.fn();

    renderSettingsPanel({
      accessibility: { granted: false, canPrompt: true },
      onRequestAccessibility,
    });

    await user.click(screen.getByRole("button", { name: "Grant accessibility" }));
    expect(onRequestAccessibility).toHaveBeenCalled();
  });

  it("refreshes microphone input devices on demand", async () => {
    const user = userEvent.setup();
    const onRefreshMicrophones = vi.fn();

    renderSettingsPanel({ onRefreshMicrophones });

    await user.click(screen.getByRole("button", { name: "Refresh" }));
    expect(onRefreshMicrophones).toHaveBeenCalled();
  });

  it("shows the reliable global shortcut", () => {
    renderSettingsPanel();

    expect(screen.getByText("Hold Command + Shift + Space")).toBeInTheDocument();
  });

  it("refreshes accessibility permission on demand", async () => {
    const user = userEvent.setup();
    const onRefreshAccessibility = vi.fn();

    renderSettingsPanel({
      accessibility: { granted: false, canPrompt: true },
      onRefreshAccessibility,
    });

    await user.click(screen.getByRole("button", { name: "Refresh permissions" }));
    expect(onRefreshAccessibility).toHaveBeenCalled();
  });

  it("requests microphone permission on demand", async () => {
    const user = userEvent.setup();
    const onRequestMicrophoneAccess = vi.fn();

    renderSettingsPanel({
      microphone: { granted: false, canPrompt: true },
      onRequestMicrophoneAccess,
    });

    await user.click(screen.getByRole("button", { name: "Grant microphone" }));
    expect(onRequestMicrophoneAccess).toHaveBeenCalled();
  });

  it("shows microphone permission request progress", () => {
    renderSettingsPanel({
      microphone: { granted: false, canPrompt: true },
      requestingPermission: "microphone",
    });

    const button = screen.getByRole("button", { name: "Requesting microphone…" });
    expect(button).toBeDisabled();
  });

  it("hides microphone grant action when microphone access is already granted", () => {
    renderSettingsPanel();

    expect(screen.getByText("Microphone granted")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Grant microphone" })).not.toBeInTheDocument();
  });
});

describe("SettingsPanel asset download", () => {
  it("renders nothing when assets are ready", async () => {
    const { assetReadiness } = await import("../../lib/tauriApi");
    vi.mocked(assetReadiness).mockResolvedValueOnce({ state: "ready" });
    renderSettingsPanel();
    expect(screen.queryByText(/Downloading models/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Retry download/i })).not.toBeInTheDocument();
  });

  it("shows retry control when a download failed", async () => {
    const { assetReadiness } = await import("../../lib/tauriApi");
    vi.mocked(assetReadiness).mockResolvedValueOnce({
      state: "failed",
      message: "failed to download assets: medium",
    });
    renderSettingsPanel();
    expect(await screen.findByRole("button", { name: "Retry download" })).toBeInTheDocument();
  });
});
