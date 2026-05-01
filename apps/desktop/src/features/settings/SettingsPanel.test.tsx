import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";

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
  it("saves local model paths", async () => {
    const user = userEvent.setup();
    const onModelSettingsSave = vi.fn();

    renderSettingsPanel({ onModelSettingsSave });

    await user.type(screen.getByLabelText("Whisper binary path"), "/opt/homebrew/bin/whisper-cli");
    await user.type(screen.getByLabelText("Whisper model path"), "/models/ggml-base.en.bin");
    await user.click(screen.getByRole("button", { name: "Save model settings" }));

    expect(onModelSettingsSave).toHaveBeenCalledWith({
      whisperBinaryPath: "/opt/homebrew/bin/whisper-cli",
      whisperModelPath: "/models/ggml-base.en.bin",
      recognitionLanguage: "auto",
      cleanupMode: "punctuation_only",
    });
  });

  it("saves recognition language with local model settings", async () => {
    const user = userEvent.setup();
    const onModelSettingsSave = vi.fn();
    renderSettingsPanel({ onModelSettingsSave });

    await user.selectOptions(screen.getByLabelText("Recognition language"), "zh");
    await user.click(screen.getByRole("button", { name: "Save model settings" }));

    expect(onModelSettingsSave).toHaveBeenCalledWith({
      whisperBinaryPath: "",
      whisperModelPath: "",
      recognitionLanguage: "zh",
      cleanupMode: "punctuation_only",
    });
  });

  it("saves cleanup mode with local model settings", async () => {
    const user = userEvent.setup();
    const onModelSettingsSave = vi.fn();
    renderSettingsPanel({ onModelSettingsSave });

    await user.selectOptions(screen.getByLabelText("Cleanup mode"), "full_cleanup");
    await user.click(screen.getByRole("button", { name: "Save model settings" }));

    expect(onModelSettingsSave).toHaveBeenCalledWith({
      whisperBinaryPath: "",
      whisperModelPath: "",
      recognitionLanguage: "auto",
      cleanupMode: "full_cleanup",
    });
  });

  it("shows Ollama install instructions when the CLI is missing", () => {
    renderSettingsPanel({
      ollamaSetup: {
        cliInstalled: false,
        serverRunning: false,
        modelInstalled: false,
        model: "qwen2.5:0.5b",
        status: "cli_missing",
        message: "Install Ollama",
      },
    });

    expect(screen.getByText(/Install Ollama to enable local punctuation cleanup/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "https://ollama.com/download" })).toHaveAttribute(
      "href",
      "https://ollama.com/download",
    );
  });

  it("shows ready Ollama status", () => {
    renderSettingsPanel({
      ollamaSetup: {
        cliInstalled: true,
        serverRunning: true,
        modelInstalled: true,
        model: "qwen2.5:0.5b",
        status: "ready",
        message: null,
      },
    });

    expect(screen.getByText("Ollama ready for local cleanup: qwen2.5:0.5b")).toBeInTheDocument();
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
