import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";

describe("SettingsPanel", () => {
  it("saves local model paths", async () => {
    const user = userEvent.setup();
    const onModelSettingsSave = vi.fn();

    render(
      <SettingsPanel
        fallbackPolicy="prefer_local_ask_before_cloud"
        microphones={[]}
        selectedMicrophoneId={null}
        microphone={{ granted: true, canPrompt: true }}
        accessibility={{ granted: true, canPrompt: true }}
        modelSettings={{ whisperBinaryPath: "", whisperModelPath: "" }}
        onMicrophoneChange={vi.fn()}
        onRefreshMicrophones={vi.fn()}
        onRefreshAccessibility={vi.fn()}
        onRequestMicrophoneAccess={vi.fn()}
        onRequestAccessibility={vi.fn()}
        onModelSettingsSave={onModelSettingsSave}
      />,
    );

    await user.type(screen.getByLabelText("Whisper binary path"), "/opt/homebrew/bin/whisper-cli");
    await user.type(screen.getByLabelText("Whisper model path"), "/models/ggml-base.en.bin");
    await user.click(screen.getByRole("button", { name: "Save model settings" }));

    expect(onModelSettingsSave).toHaveBeenCalledWith({
      whisperBinaryPath: "/opt/homebrew/bin/whisper-cli",
      whisperModelPath: "/models/ggml-base.en.bin",
    });
  });

  it("changes microphone input", async () => {
    const user = userEvent.setup();
    const onMicrophoneChange = vi.fn();

    render(
      <SettingsPanel
        fallbackPolicy="prefer_local_ask_before_cloud"
        microphones={[
          { id: "default", name: "System Default", isDefault: true },
          { id: "2", name: "USB Mic", isDefault: false },
        ]}
        selectedMicrophoneId="default"
        microphone={{ granted: true, canPrompt: true }}
        accessibility={{ granted: false, canPrompt: true }}
        modelSettings={{ whisperBinaryPath: "", whisperModelPath: "" }}
        onMicrophoneChange={onMicrophoneChange}
        onRefreshMicrophones={vi.fn()}
        onRefreshAccessibility={vi.fn()}
        onRequestMicrophoneAccess={vi.fn()}
        onRequestAccessibility={vi.fn()}
        onModelSettingsSave={vi.fn()}
      />,
    );

    await user.selectOptions(screen.getByLabelText("Microphone input"), "2");
    expect(onMicrophoneChange).toHaveBeenCalledWith("2");
  });

  it("requests accessibility permission when missing", async () => {
    const user = userEvent.setup();
    const onRequestAccessibility = vi.fn();

    render(
      <SettingsPanel
        fallbackPolicy="prefer_local_ask_before_cloud"
        microphones={[]}
        selectedMicrophoneId={null}
        microphone={{ granted: true, canPrompt: true }}
        accessibility={{ granted: false, canPrompt: true }}
        modelSettings={{ whisperBinaryPath: "", whisperModelPath: "" }}
        onMicrophoneChange={vi.fn()}
        onRefreshMicrophones={vi.fn()}
        onRefreshAccessibility={vi.fn()}
        onRequestMicrophoneAccess={vi.fn()}
        onRequestAccessibility={onRequestAccessibility}
        onModelSettingsSave={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Grant accessibility" }));
    expect(onRequestAccessibility).toHaveBeenCalled();
  });

  it("refreshes microphone input devices on demand", async () => {
    const user = userEvent.setup();
    const onRefreshMicrophones = vi.fn();

    render(
      <SettingsPanel
        fallbackPolicy="prefer_local_ask_before_cloud"
        microphones={[]}
        selectedMicrophoneId={null}
        microphone={{ granted: true, canPrompt: true }}
        accessibility={{ granted: true, canPrompt: true }}
        modelSettings={{ whisperBinaryPath: "", whisperModelPath: "" }}
        onMicrophoneChange={vi.fn()}
        onRefreshMicrophones={onRefreshMicrophones}
        onRefreshAccessibility={vi.fn()}
        onRequestMicrophoneAccess={vi.fn()}
        onRequestAccessibility={vi.fn()}
        onModelSettingsSave={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Refresh" }));
    expect(onRefreshMicrophones).toHaveBeenCalled();
  });

  it("shows the reliable global shortcut", () => {
    render(
      <SettingsPanel
        fallbackPolicy="prefer_local_ask_before_cloud"
        microphones={[]}
        selectedMicrophoneId={null}
        microphone={{ granted: true, canPrompt: true }}
        accessibility={{ granted: true, canPrompt: true }}
        modelSettings={{ whisperBinaryPath: "", whisperModelPath: "" }}
        onMicrophoneChange={vi.fn()}
        onRefreshMicrophones={vi.fn()}
        onRefreshAccessibility={vi.fn()}
        onRequestMicrophoneAccess={vi.fn()}
        onRequestAccessibility={vi.fn()}
        onModelSettingsSave={vi.fn()}
      />,
    );

    expect(screen.getByText("Hold Command + Shift + Space")).toBeInTheDocument();
  });

  it("refreshes accessibility permission on demand", async () => {
    const user = userEvent.setup();
    const onRefreshAccessibility = vi.fn();

    render(
      <SettingsPanel
        fallbackPolicy="prefer_local_ask_before_cloud"
        microphones={[]}
        selectedMicrophoneId={null}
        microphone={{ granted: true, canPrompt: true }}
        accessibility={{ granted: false, canPrompt: true }}
        modelSettings={{ whisperBinaryPath: "", whisperModelPath: "" }}
        onMicrophoneChange={vi.fn()}
        onRefreshMicrophones={vi.fn()}
        onRefreshAccessibility={onRefreshAccessibility}
        onRequestMicrophoneAccess={vi.fn()}
        onRequestAccessibility={vi.fn()}
        onModelSettingsSave={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Refresh permissions" }));
    expect(onRefreshAccessibility).toHaveBeenCalled();
  });

  it("requests microphone permission on demand", async () => {
    const user = userEvent.setup();
    const onRequestMicrophoneAccess = vi.fn();

    render(
      <SettingsPanel
        fallbackPolicy="prefer_local_ask_before_cloud"
        microphones={[]}
        selectedMicrophoneId={null}
        microphone={{ granted: false, canPrompt: true }}
        accessibility={{ granted: true, canPrompt: true }}
        modelSettings={{ whisperBinaryPath: "", whisperModelPath: "" }}
        onMicrophoneChange={vi.fn()}
        onRefreshMicrophones={vi.fn()}
        onRefreshAccessibility={vi.fn()}
        onRequestMicrophoneAccess={onRequestMicrophoneAccess}
        onRequestAccessibility={vi.fn()}
        onModelSettingsSave={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Grant microphone" }));
    expect(onRequestMicrophoneAccess).toHaveBeenCalled();
  });

  it("shows microphone permission request progress", () => {
    render(
      <SettingsPanel
        fallbackPolicy="prefer_local_ask_before_cloud"
        microphones={[]}
        selectedMicrophoneId={null}
        microphone={{ granted: false, canPrompt: true }}
        accessibility={{ granted: true, canPrompt: true }}
        modelSettings={{ whisperBinaryPath: "", whisperModelPath: "" }}
        requestingPermission="microphone"
        onMicrophoneChange={vi.fn()}
        onRefreshMicrophones={vi.fn()}
        onRefreshAccessibility={vi.fn()}
        onRequestMicrophoneAccess={vi.fn()}
        onRequestAccessibility={vi.fn()}
        onModelSettingsSave={vi.fn()}
      />,
    );

    const button = screen.getByRole("button", { name: "Requesting microphone…" });
    expect(button).toBeDisabled();
  });

  it("hides microphone grant action when microphone access is already granted", () => {
    render(
      <SettingsPanel
        fallbackPolicy="prefer_local_ask_before_cloud"
        microphones={[]}
        selectedMicrophoneId={null}
        microphone={{ granted: true, canPrompt: true }}
        accessibility={{ granted: true, canPrompt: true }}
        modelSettings={{ whisperBinaryPath: "", whisperModelPath: "" }}
        onMicrophoneChange={vi.fn()}
        onRefreshMicrophones={vi.fn()}
        onRefreshAccessibility={vi.fn()}
        onRequestMicrophoneAccess={vi.fn()}
        onRequestAccessibility={vi.fn()}
        onModelSettingsSave={vi.fn()}
      />,
    );

    expect(screen.getByText("Microphone granted")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Grant microphone" })).not.toBeInTheDocument();
  });
});
