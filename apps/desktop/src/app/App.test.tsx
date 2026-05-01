import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import {
  accessibilityStatus,
  ensureOllamaSetup,
  listMicrophones,
  localModelSettings,
  microphoneStatus,
  recognitionLanguage,
  requestMicrophoneAccess,
  requestAccessibility,
  selectedMicrophoneId,
  setLanguageMenuOpen,
  setMicrophoneDevice,
  setLocalModelSettings,
  setRecognitionLanguage,
  startRecording,
  stopRecording,
} from "../lib/tauriApi";

const eventListeners = new Map<string, (event: { payload: unknown }) => void>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((eventName: string, callback: (event: { payload: unknown }) => void) => {
    eventListeners.set(eventName, callback);
    return Promise.resolve(() => {
      eventListeners.delete(eventName);
    });
  }),
}));

vi.mock("../lib/tauriApi", () => ({
  accessibilityStatus: vi.fn().mockResolvedValue({ granted: false, canPrompt: true }),
  cancelRecording: vi.fn().mockResolvedValue(undefined),
  ensureOllamaSetup: vi.fn().mockResolvedValue({
    cliInstalled: true,
    serverRunning: true,
    modelInstalled: true,
    model: "qwen2.5:0.5b",
    status: "ready",
    message: null,
  }),
  fallbackPolicyLabel: vi.fn().mockResolvedValue("prefer_local_ask_before_cloud"),
  localModelSettings: vi.fn().mockResolvedValue({
    whisperBinaryPath: "/usr/local/bin/whisper-cli",
    whisperModelPath: "/models/base.bin",
    recognitionLanguage: "auto",
    cleanupMode: "punctuation_only",
  }),
  listMicrophones: vi.fn().mockResolvedValue([
    { id: "default", name: "System Default", isDefault: true },
    { id: "1", name: "Studio Mic", isDefault: false },
  ]),
  microphoneStatus: vi.fn().mockResolvedValue({ granted: true, canPrompt: true }),
  recognitionLanguage: vi.fn().mockResolvedValue("auto"),
  requestAccessibility: vi.fn().mockResolvedValue({ granted: true, canPrompt: true }),
  requestMicrophoneAccess: vi.fn().mockResolvedValue({ granted: true, canPrompt: true }),
  recordingStatus: vi.fn().mockResolvedValue("idle"),
  selectedMicrophoneId: vi.fn().mockResolvedValue("default"),
  setLanguageMenuOpen: vi.fn().mockResolvedValue(undefined),
  setMicrophoneDevice: vi.fn().mockResolvedValue(undefined),
  setLocalModelSettings: vi.fn().mockImplementation((settings) => Promise.resolve(settings)),
  setRecognitionLanguage: vi.fn().mockResolvedValue("en"),
  startRecording: vi.fn().mockResolvedValue(undefined),
  stopRecording: vi.fn().mockResolvedValue({
    result: { kind: "insert_text", text: "hello from voice", source: "local", confidence: null },
    insertion: "inserted",
  }),
}));

describe("App", () => {
  beforeEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
    vi.mocked(accessibilityStatus).mockReset();
    vi.mocked(ensureOllamaSetup).mockReset();
    vi.mocked(localModelSettings).mockReset();
    vi.mocked(listMicrophones).mockReset();
    vi.mocked(microphoneStatus).mockReset();
    vi.mocked(recognitionLanguage).mockReset();
    vi.mocked(requestAccessibility).mockReset();
    vi.mocked(requestMicrophoneAccess).mockReset();
    vi.mocked(selectedMicrophoneId).mockReset();
    vi.mocked(setLanguageMenuOpen).mockReset();
    vi.mocked(setMicrophoneDevice).mockReset();
    vi.mocked(setLocalModelSettings).mockReset();
    vi.mocked(setRecognitionLanguage).mockReset();
    vi.mocked(startRecording).mockReset();
    vi.mocked(stopRecording).mockReset();
    eventListeners.clear();
    window.history.pushState({}, "", "/");
    vi.mocked(accessibilityStatus).mockResolvedValue({ granted: false, canPrompt: true });
    vi.mocked(ensureOllamaSetup).mockResolvedValue({
      cliInstalled: true,
      serverRunning: true,
      modelInstalled: true,
      model: "qwen2.5:0.5b",
      status: "ready",
      message: null,
    });
    vi.mocked(localModelSettings).mockResolvedValue({
      whisperBinaryPath: "/usr/local/bin/whisper-cli",
      whisperModelPath: "/models/base.bin",
      recognitionLanguage: "auto",
      cleanupMode: "punctuation_only",
    });
    vi.mocked(listMicrophones).mockResolvedValue([
      { id: "default", name: "System Default", isDefault: true },
      { id: "1", name: "Studio Mic", isDefault: false },
    ]);
    vi.mocked(microphoneStatus).mockResolvedValue({ granted: true, canPrompt: true });
    vi.mocked(recognitionLanguage).mockResolvedValue("auto");
    vi.mocked(requestAccessibility).mockResolvedValue({ granted: true, canPrompt: true });
    vi.mocked(requestMicrophoneAccess).mockResolvedValue({ granted: true, canPrompt: true });
    vi.mocked(selectedMicrophoneId).mockResolvedValue("default");
    vi.mocked(setLanguageMenuOpen).mockResolvedValue(undefined);
    vi.mocked(setMicrophoneDevice).mockResolvedValue(undefined);
    vi.mocked(setLocalModelSettings).mockImplementation((settings) => Promise.resolve(settings));
    vi.mocked(setRecognitionLanguage).mockImplementation(async (language) => language);
    vi.mocked(startRecording).mockResolvedValue(undefined);
    vi.mocked(stopRecording).mockResolvedValue({
      result: { kind: "insert_text", text: "hello from voice", source: "local", confidence: null },
      insertion: "inserted",
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders settings by default", async () => {
    render(<App />);

    expect(screen.getByRole("region", { name: "Settings" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Recorder" })).not.toBeInTheDocument();
  });

  it("does not expose recorder control buttons", () => {
    render(<App />);

    expect(screen.queryByRole("button", { name: "Start recording" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Stop recording" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Cancel recording" })).not.toBeInTheDocument();
  });

  it("starts on shortcut press and stops on shortcut release", async () => {
    window.history.pushState({}, "", "/?surface=recorder");
    render(<App />);

    await emitRecordShortcut("Pressed");
    expect(startRecording).toHaveBeenCalledWith("press_and_hold");
    expect(await screen.findByText("Recording")).toBeInTheDocument();

    await emitRecordShortcut("Released");
    expect(stopRecording).toHaveBeenCalledWith("global_shortcut");
    expect(await screen.findByText("Ready")).toBeInTheDocument();
    expect(await screen.findByText("Inserted: hello from voice")).toBeInTheDocument();
  });

  it("does not expose floating recorder mouse controls", () => {
    window.history.pushState({}, "", "/?surface=recorder");
    render(<App />);

    expect(screen.getByRole("region", { name: "Recorder" })).toHaveTextContent(
      "hold Command + Shift + Space",
    );
    expect(screen.queryByRole("button", { name: /dictation/i })).not.toBeInTheDocument();
  });

  it("lets settings change the selected microphone", async () => {
    const user = userEvent.setup();
    render(<App />);

    await screen.findByDisplayValue("System Default");
    await user.selectOptions(screen.getByLabelText("Microphone input"), "1");

    expect(setMicrophoneDevice).toHaveBeenCalledWith("1");
  });

  it("loads and saves local model settings", async () => {
    const user = userEvent.setup();
    render(<App />);

    expect(await screen.findByDisplayValue("/usr/local/bin/whisper-cli")).toBeInTheDocument();
    expect(screen.getByDisplayValue("/models/base.bin")).toBeInTheDocument();

    await user.clear(screen.getByLabelText("Whisper binary path"));
    await user.type(screen.getByLabelText("Whisper binary path"), "/opt/homebrew/bin/whisper-cli");
    await user.clear(screen.getByLabelText("Whisper model path"));
    await user.type(screen.getByLabelText("Whisper model path"), "/models/small.bin");
    await user.click(screen.getByRole("button", { name: "Save model settings" }));

    expect(setLocalModelSettings).toHaveBeenCalledWith({
      whisperBinaryPath: "/opt/homebrew/bin/whisper-cli",
      whisperModelPath: "/models/small.bin",
      recognitionLanguage: "auto",
      cleanupMode: "punctuation_only",
    });
  });

  it("checks Ollama setup after cleanup-enabled settings load and renders status", async () => {
    render(<App />);

    expect(await screen.findByText("Ollama ready for local cleanup: qwen2.5:0.5b")).toBeInTheDocument();
    expect(ensureOllamaSetup).toHaveBeenCalledTimes(1);
  });

  it("does not check Ollama setup when saved cleanup mode is off", async () => {
    vi.mocked(localModelSettings).mockResolvedValueOnce({
      whisperBinaryPath: "/usr/local/bin/whisper-cli",
      whisperModelPath: "/models/base.bin",
      recognitionLanguage: "auto",
      cleanupMode: "off",
    });

    render(<App />);
    expect(await screen.findByDisplayValue("/usr/local/bin/whisper-cli")).toBeInTheDocument();
    await act(async () => {
      await Promise.resolve();
    });

    expect(ensureOllamaSetup).not.toHaveBeenCalled();
    expect(screen.queryByText(/Ollama ready for local cleanup/)).not.toBeInTheDocument();
  });

  it("renders Ollama install prompt when the CLI is missing", async () => {
    vi.mocked(ensureOllamaSetup).mockResolvedValueOnce({
      cliInstalled: false,
      serverRunning: false,
      modelInstalled: false,
      model: "qwen2.5:0.5b",
      status: "cli_missing",
      message: "Install Ollama",
    });

    render(<App />);

    expect(await screen.findByText(/Install Ollama to enable local punctuation cleanup/)).toBeInTheDocument();
  });

  it("requests accessibility permission from the setup panel", async () => {
    const user = userEvent.setup();
    vi.mocked(requestAccessibility).mockResolvedValueOnce({ granted: false, canPrompt: true });
    vi.mocked(accessibilityStatus)
      .mockResolvedValueOnce({ granted: false, canPrompt: true })
      .mockResolvedValueOnce({ granted: true, canPrompt: true });
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Grant accessibility" }));

    expect(requestAccessibility).toHaveBeenCalled();
    expect(await screen.findByText("Accessibility granted")).toBeInTheDocument();
  });

  it("refreshes permission statuses from the settings panel", async () => {
    const user = userEvent.setup();
    vi.mocked(accessibilityStatus)
      .mockResolvedValueOnce({ granted: false, canPrompt: true })
      .mockResolvedValueOnce({ granted: true, canPrompt: true });
    vi.mocked(microphoneStatus)
      .mockResolvedValueOnce({ granted: false, canPrompt: true })
      .mockResolvedValueOnce({ granted: true, canPrompt: false });

    render(<App />);
    expect(await screen.findByText("Accessibility missing")).toBeInTheDocument();
    expect(await screen.findByText("Microphone missing")).toBeInTheDocument();

    const accessibilityCallsBeforeRefresh = vi.mocked(accessibilityStatus).mock.calls.length;
    const microphoneCallsBeforeRefresh = vi.mocked(microphoneStatus).mock.calls.length;
    await user.click(screen.getByRole("button", { name: "Refresh permissions" }));

    expect(vi.mocked(accessibilityStatus).mock.calls.length).toBeGreaterThan(
      accessibilityCallsBeforeRefresh,
    );
    expect(vi.mocked(microphoneStatus).mock.calls.length).toBeGreaterThan(
      microphoneCallsBeforeRefresh,
    );
    expect(await screen.findByText("Accessibility granted")).toBeInTheDocument();
    expect(await screen.findByText("Microphone granted")).toBeInTheDocument();
  });

  it("requests microphone permission from the settings panel", async () => {
    const user = userEvent.setup();
    vi.mocked(microphoneStatus).mockResolvedValueOnce({ granted: false, canPrompt: true });

    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Grant microphone" }));

    expect(requestMicrophoneAccess).toHaveBeenCalled();
    expect(await screen.findByText("Microphone granted")).toBeInTheDocument();
  });

  it("trusts a granted microphone request when the immediate status refresh is stale", async () => {
    const user = userEvent.setup();
    vi.mocked(microphoneStatus)
      .mockResolvedValueOnce({ granted: false, canPrompt: true })
      .mockResolvedValueOnce({ granted: false, canPrompt: false });
    vi.mocked(requestMicrophoneAccess).mockResolvedValueOnce({ granted: true, canPrompt: false });

    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Grant microphone" }));

    expect(requestMicrophoneAccess).toHaveBeenCalled();
    expect(await screen.findByText("Microphone granted")).toBeInTheDocument();
  });

  it("ignores stale in-flight microphone refreshes after a grant succeeds", async () => {
    const user = userEvent.setup();
    const staleStatus = deferred<{ granted: boolean; canPrompt: boolean }>();
    vi.mocked(microphoneStatus).mockReturnValueOnce(staleStatus.promise);
    vi.mocked(requestMicrophoneAccess).mockResolvedValueOnce({ granted: true, canPrompt: false });

    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Grant microphone" }));
    expect(await screen.findByText("Microphone granted")).toBeInTheDocument();

    await act(async () => {
      staleStatus.resolve({ granted: false, canPrompt: false });
      await staleStatus.promise;
    });

    expect(screen.getByText("Microphone granted")).toBeInTheDocument();
  });

  it("allows automatic microphone refreshes to detect later revocation", async () => {
    vi.useFakeTimers();
    vi.mocked(microphoneStatus)
      .mockResolvedValueOnce({ granted: true, canPrompt: false })
      .mockResolvedValueOnce({ granted: false, canPrompt: false });

    render(<App />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(screen.getByText("Microphone granted")).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });

    expect(screen.getByText("Microphone missing")).toBeInTheDocument();
  });

  it("explains when microphone permission must be enabled in system settings", async () => {
    const user = userEvent.setup();
    vi.mocked(microphoneStatus).mockResolvedValue({ granted: false, canPrompt: false });
    vi.mocked(requestMicrophoneAccess).mockResolvedValueOnce({ granted: false, canPrompt: false });

    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Grant microphone" }));

    expect(await screen.findByRole("status")).toHaveTextContent(
      "Enable microphone access in macOS Settings, then click Refresh permissions.",
    );
  });

  it("keeps status stable and reports failures when commands reject", async () => {
    vi.mocked(startRecording).mockRejectedValueOnce(new Error("microphone denied"));
    window.history.pushState({}, "", "/?surface=recorder");
    render(<App />);

    await emitRecordShortcut("Pressed");

    expect(await screen.findByText("Ready")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("microphone denied");
  });

  it("returns to idle when stop fails after the native session has ended", async () => {
    vi.mocked(stopRecording).mockRejectedValueOnce(new Error("Local ASR is not configured"));
    window.history.pushState({}, "", "/?surface=recorder");
    render(<App />);

    await emitRecordShortcut("Pressed");
    expect(await screen.findByText("Recording")).toBeInTheDocument();

    await emitRecordShortcut("Released");

    expect(await screen.findByText("Ready")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Local ASR is not configured");
  });

  it("explains copied-only insertion as an auto-paste fallback", async () => {
    vi.mocked(stopRecording).mockResolvedValueOnce({
      result: { kind: "insert_text", text: "hello from voice", source: "local", confidence: null },
      insertion: "copied_only",
    });
    window.history.pushState({}, "", "/?surface=recorder");
    render(<App />);

    await emitRecordShortcut("Pressed");
    await emitRecordShortcut("Released");

    expect(await screen.findByText(/Copied to clipboard; auto-paste failed/)).toHaveTextContent(
      "Copied to clipboard; auto-paste failed. Check Accessibility permission: hello from voice",
    );
  });

  it("explains when no editable text field is focused", async () => {
    vi.mocked(stopRecording).mockResolvedValueOnce({
      result: { kind: "insert_text", text: "hello from voice", source: "local", confidence: null },
      insertion: "no_editable_target",
    });
    window.history.pushState({}, "", "/?surface=recorder");
    render(<App />);

    await emitRecordShortcut("Pressed");
    await emitRecordShortcut("Released");

    expect(await screen.findByText(/No editable text field detected/)).toHaveTextContent(
      "No editable text field detected; copied to clipboard: hello from voice",
    );
  });

  it("explains when Accessibility is unavailable during insertion", async () => {
    vi.mocked(stopRecording).mockResolvedValueOnce({
      result: { kind: "insert_text", text: "hello from voice", source: "local", confidence: null },
      insertion: "accessibility_denied",
    });
    window.history.pushState({}, "", "/?surface=recorder");
    render(<App />);

    await emitRecordShortcut("Pressed");
    await emitRecordShortcut("Released");

    expect(await screen.findByText(/Accessibility permission unavailable/)).toHaveTextContent(
      "Accessibility permission unavailable; copied to clipboard: hello from voice",
    );
  });

  it("explains secure text fields without copying sensitive dictation", async () => {
    vi.mocked(stopRecording).mockResolvedValueOnce({
      result: { kind: "insert_text", text: "secret", source: "local", confidence: null },
      insertion: "secure_field",
    });
    window.history.pushState({}, "", "/?surface=recorder");
    render(<App />);

    await emitRecordShortcut("Pressed");
    await emitRecordShortcut("Released");

    expect(await screen.findByText(/Secure text field detected/)).toHaveTextContent(
      "Secure text field detected; not inserted or copied.",
    );
  });

  it("renders only the recorder on the floating recorder surface", () => {
    window.history.pushState({}, "", "/?surface=recorder");

    render(<App />);

    expect(screen.getByRole("region", { name: "Recorder" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Settings" })).not.toBeInTheDocument();
  });

  it("does not query settings-only permissions from the recorder surface", async () => {
    window.history.pushState({}, "", "/?surface=recorder");

    render(<App />);
    await act(async () => {
      await Promise.resolve();
    });

    expect(listMicrophones).not.toHaveBeenCalled();
    expect(microphoneStatus).not.toHaveBeenCalled();
    expect(accessibilityStatus).not.toHaveBeenCalled();
    expect(localModelSettings).not.toHaveBeenCalled();
    expect(selectedMicrophoneId).not.toHaveBeenCalled();
  });

  it("renders only language controls on the language surface", async () => {
    window.history.pushState({}, "", "/?surface=language");

    render(<App />);

    expect(await screen.findByRole("button", { name: "Recognition language: Auto" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Recorder" })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Settings" })).not.toBeInTheDocument();
  });

  it("cycles recognition language from the language surface", async () => {
    const user = userEvent.setup();
    window.history.pushState({}, "", "/?surface=language");

    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Recognition language: Auto" }));

    expect(setRecognitionLanguage).toHaveBeenCalledWith("en");
  });

  it("opens the language menu from the chevron", async () => {
    const user = userEvent.setup();
    window.history.pushState({}, "", "/?surface=language");

    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Choose recognition language" }));

    expect(setLanguageMenuOpen).toHaveBeenCalledWith(true);
    expect(await screen.findByRole("menuitemradio", { name: "Auto" })).toBeInTheDocument();
  });

  it("does not query settings-only data from the language surface", async () => {
    window.history.pushState({}, "", "/?surface=language");

    render(<App />);
    await screen.findByRole("button", { name: "Recognition language: Auto" });

    expect(listMicrophones).not.toHaveBeenCalled();
    expect(microphoneStatus).not.toHaveBeenCalled();
    expect(accessibilityStatus).not.toHaveBeenCalled();
    expect(localModelSettings).not.toHaveBeenCalled();
    expect(selectedMicrophoneId).not.toHaveBeenCalled();
  });

  it("reveals the language chevron from native hover while the app is inactive", async () => {
    window.history.pushState({}, "", "/?surface=language");

    render(<App />);
    const button = await screen.findByRole("button", { name: "Recognition language: Auto" });
    const toggle = button.closest(".language-toggle");
    expect(toggle).not.toHaveClass("is-native-hovered");

    await emitLanguageHover(true);
    expect(toggle).toHaveClass("is-native-hovered");

    await emitLanguageHover(false);
    expect(toggle).not.toHaveClass("is-native-hovered");
  });

  it("refreshes microphone devices while settings are open", async () => {
    vi.useFakeTimers();
    vi.mocked(listMicrophones)
      .mockResolvedValueOnce([{ id: "default", name: "System Default", isDefault: true }])
      .mockResolvedValueOnce([
        { id: "default", name: "System Default", isDefault: true },
        { id: "usb", name: "USB Mic", isDefault: false },
      ]);

    render(<App />);
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.getByDisplayValue("System Default")).toBeInTheDocument();

    await act(async () => {
      vi.advanceTimersByTime(2000);
      await Promise.resolve();
    });

    expect(screen.getByText("USB Mic")).toBeInTheDocument();
  });
});

async function emitRecordShortcut(payload: string) {
  await waitFor(() => {
    expect(eventListeners.has("wispergo://record-shortcut")).toBe(true);
  });
  await act(async () => {
    eventListeners.get("wispergo://record-shortcut")?.({ payload });
  });
}

async function emitLanguageHover(payload: boolean) {
  await waitFor(() => {
    expect(eventListeners.has("wispergo://language-hover-changed")).toBe(true);
  });
  await act(async () => {
    eventListeners.get("wispergo://language-hover-changed")?.({ payload });
  });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });

  return { promise, resolve, reject };
}
