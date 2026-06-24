import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
      asrModelId: "medium",
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
        asrModelId: "medium",
        recognitionLanguage: "auto",
        cleanupMode: "punctuation_only",
      },
    });

    await user.selectOptions(screen.getByLabelText("Language"), "zh");
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onModelSettingsSave).toHaveBeenCalledWith({
      asrModelId: "medium",
      recognitionLanguage: "zh",
      cleanupMode: "punctuation_only",
    });
  });

  it("saves ASR model tier with local model settings", async () => {
    const user = userEvent.setup();
    const onModelSettingsSave = vi.fn();
    renderSettingsPanel({
      onModelSettingsSave,
      modelSettings: {
        asrModelId: "medium",
        recognitionLanguage: "auto",
        cleanupMode: "punctuation_only",
      },
    });

    await user.selectOptions(screen.getByLabelText("ASR model"), "large-v3-turbo");
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onModelSettingsSave).toHaveBeenCalledWith({
      asrModelId: "large-v3-turbo",
      recognitionLanguage: "auto",
      cleanupMode: "punctuation_only",
    });
  });

  it("saves cleanup mode with local model settings", async () => {
    const user = userEvent.setup();
    const onModelSettingsSave = vi.fn();
    renderSettingsPanel({
      onModelSettingsSave,
      modelSettings: {
        asrModelId: "medium",
        recognitionLanguage: "auto",
        cleanupMode: "punctuation_only",
      },
    });

    await user.selectOptions(screen.getByLabelText("Cleanup"), "full_cleanup");
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onModelSettingsSave).toHaveBeenCalledWith({
      asrModelId: "medium",
      recognitionLanguage: "auto",
      cleanupMode: "full_cleanup",
    });
  });

  it("shows setup needed when microphone, accessibility, or models are missing", async () => {
    const { assetReadiness } = await import("../../lib/tauriApi");
    vi.mocked(assetReadiness).mockResolvedValueOnce({
      state: "missing",
      assetId: "medium",
      displayName: "Whisper medium",
    });

    renderSettingsPanel({
      microphone: { granted: false, canPrompt: true },
      accessibility: { granted: false, canPrompt: true },
    });

    expect(await screen.findByText("Setup needed")).toBeInTheDocument();
    expect(screen.getAllByText("Microphone").length).toBeGreaterThan(0);
    expect(screen.getByText("Accessibility")).toBeInTheDocument();
    expect(screen.getByText("Local models")).toBeInTheDocument();
  });

  it("shows ready when permissions and required models are ready", async () => {
    const { assetReadiness } = await import("../../lib/tauriApi");
    vi.mocked(assetReadiness).mockResolvedValueOnce({ state: "ready" });

    renderSettingsPanel();

    expect(await screen.findByText("Ready for dictation")).toBeInTheDocument();
  });

  it("presents settings as product dashboard instead of engineering diagnostics", async () => {
    const { assetReadiness } = await import("../../lib/tauriApi");
    vi.mocked(assetReadiness).mockResolvedValueOnce({ state: "ready" });

    renderSettingsPanel({ fallbackPolicy: "prefer_local_ask_before_cloud" });

    expect(await screen.findByText("Ready for dictation")).toBeInTheDocument();
    expect(screen.getByText("Dictation"));
    expect(screen.getByText("Input"));
    expect(screen.getByRole("button", { name: "Save changes" })).toBeInTheDocument();
    expect(screen.queryByText(/Fallback policy/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/prefer_local_ask_before_cloud/i)).not.toBeInTheDocument();
  });

  it("explains Chinese mixed-language recognition mode", () => {
    renderSettingsPanel();

    expect(
      screen.getByText(
        "Use Chinese / Mixed for Chinese-English dictation. Full cleanup downloads the optional 3B pack before activation.",
      ),
    ).toBeInTheDocument();
  });

  it("explains full cleanup pack activation", () => {
    renderSettingsPanel();

    expect(
      screen.getByText(/Full cleanup downloads the optional 3B pack before activation/),
    ).toBeInTheDocument();
  });

  it("shows ready offline punctuation status", () => {
    renderSettingsPanel({
      cleanupRuntime: { state: "ready", message: null },
    });

    expect(screen.getByText("Offline punctuation ready")).toBeInTheDocument();
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
        asrModelId: "medium",
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

    await user.selectOptions(screen.getByLabelText("Source"), "2");
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

    await user.click(screen.getByRole("button", { name: "Refresh devices" }));
    expect(onRefreshMicrophones).toHaveBeenCalled();
  });

  it("shows the reliable global shortcut", () => {
    renderSettingsPanel();

    expect(screen.getAllByText("⌘ ⇧ Space").length).toBeGreaterThan(0);
    expect(screen.getByRole("region", { name: "Shortcut preferences" })).toBeInTheDocument();
    expect(document.querySelectorAll(".settings-icon").length).toBeGreaterThan(0);
  });

  it("saves shortcut combo settings", async () => {
    const user = userEvent.setup();
    const onShortcutSettingsSave = vi.fn();
    renderSettingsPanel({ onShortcutSettingsSave });

    await user.click(screen.getByLabelText("⇧ Shift"));
    await user.click(screen.getByLabelText("⌥ Option"));
    await user.selectOptions(screen.getByLabelText("Key"), "keyK");
    await user.click(screen.getByRole("button", { name: "Save shortcut" }));

    expect(onShortcutSettingsSave).toHaveBeenCalledWith({
      mode: "combo",
      combo: {
        modifiers: { command: true, shift: false, option: true, control: false },
        key: "keyK",
      },
      modifierHold: { key: "right_command", holdThresholdMs: 200 },
    });
  });

  it("records and saves shortcut combo settings", async () => {
    const user = userEvent.setup();
    const onShortcutSettingsSave = vi.fn();
    renderSettingsPanel({ onShortcutSettingsSave });

    const recordButton = screen.getByRole("button", { name: "Record shortcut" });
    await user.click(recordButton);
    fireEvent.keyDown(recordButton, { code: "KeyK", metaKey: true, altKey: true });
    await user.click(screen.getByRole("button", { name: "Save shortcut" }));

    expect(onShortcutSettingsSave).toHaveBeenCalledWith({
      mode: "combo",
      combo: {
        modifiers: { command: true, shift: false, option: true, control: false },
        key: "keyK",
      },
      modifierHold: { key: "right_command", holdThresholdMs: 200 },
    });
  });

  it("records the default Command Shift Space combo", async () => {
    const user = userEvent.setup();
    const onShortcutSettingsSave = vi.fn();
    renderSettingsPanel({ onShortcutSettingsSave });

    const recordButton = screen.getByRole("button", { name: "Record shortcut" });
    await user.click(recordButton);
    fireEvent.keyDown(recordButton, { code: "Space", metaKey: true, shiftKey: true });
    await user.click(screen.getByRole("button", { name: "Save shortcut" }));

    expect(onShortcutSettingsSave).toHaveBeenCalledWith({
      mode: "combo",
      combo: {
        modifiers: { command: true, shift: true, option: false, control: false },
        key: "space",
      },
      modifierHold: { key: "right_command", holdThresholdMs: 200 },
    });
  });

  it("rejects recorded shortcut combos without a modifier", async () => {
    const user = userEvent.setup();
    renderSettingsPanel();

    const recordButton = screen.getByRole("button", { name: "Record shortcut" });
    await user.click(recordButton);
    fireEvent.keyDown(recordButton, { code: "KeyK" });

    expect(screen.getByRole("status")).toHaveTextContent("at least one modifier");
  });

  it("resets shortcut combo settings to default", async () => {
    const user = userEvent.setup();
    const onShortcutSettingsSave = vi.fn();
    renderSettingsPanel({
      onShortcutSettingsSave,
      shortcutView: {
        settings: {
          mode: "combo",
          combo: {
            modifiers: { command: true, shift: false, option: true, control: false },
            key: "keyK",
          },
          modifierHold: { key: "right_command", holdThresholdMs: 200 },
        },
        displayLabel: "⌘ ⌥ K",
      },
    });

    await user.click(screen.getByRole("button", { name: "Reset to default" }));

    expect(onShortcutSettingsSave).toHaveBeenCalledWith({
      mode: "combo",
      combo: {
        modifiers: { command: true, shift: true, option: false, control: false },
        key: "space",
      },
      modifierHold: { key: "right_command", holdThresholdMs: 200 },
    });
  });

  it("renders shortcut save errors", () => {
    renderSettingsPanel({ shortcutError: "Shortcut is already registered" });

    expect(screen.getByRole("status")).toHaveTextContent("Shortcut is already registered");
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

  it("starts default asset download when readiness reports missing", async () => {
    const { assetReadiness, ensureModelAssets } = await import("../../lib/tauriApi");
    vi.mocked(assetReadiness).mockResolvedValueOnce({
      state: "missing",
      assetId: "medium",
      displayName: "Whisper medium",
    });
    vi.mocked(ensureModelAssets).mockResolvedValueOnce({ state: "ready" });

    renderSettingsPanel();

    await waitFor(() => expect(ensureModelAssets).toHaveBeenCalled());
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
