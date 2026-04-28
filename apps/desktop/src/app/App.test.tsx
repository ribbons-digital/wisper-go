import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { startRecording, stopRecording } from "../lib/tauriApi";

vi.mock("../lib/tauriApi", () => ({
  cancelRecording: vi.fn().mockResolvedValue(undefined),
  fallbackPolicyLabel: vi.fn().mockResolvedValue("prefer_local_ask_before_cloud"),
  recordingStatus: vi.fn().mockResolvedValue("idle"),
  startRecording: vi.fn().mockResolvedValue(undefined),
  stopRecording: vi.fn().mockResolvedValue(undefined),
}));

describe("App", () => {
  beforeEach(() => {
    vi.mocked(startRecording).mockResolvedValue(undefined);
    vi.mocked(stopRecording).mockResolvedValue(undefined);
  });

  it("renders recorder and settings surfaces", async () => {
    render(<App />);

    expect(screen.getByRole("region", { name: "Recorder" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Settings" })).toBeInTheDocument();
  });

  it("starts and stops recording through tauri commands", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Start recording" }));
    expect(startRecording).toHaveBeenCalledWith("toggle");
    expect(await screen.findByText("Recording")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Stop recording" }));
    expect(stopRecording).toHaveBeenCalledWith("floating_button");
    expect(await screen.findByText("Ready")).toBeInTheDocument();
  });

  it("keeps status stable and reports failures when commands reject", async () => {
    const user = userEvent.setup();
    vi.mocked(startRecording).mockRejectedValueOnce(new Error("microphone denied"));
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Start recording" }));

    expect(await screen.findByText("Ready")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("microphone denied");
  });
});
