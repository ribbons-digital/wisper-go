import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";

vi.mock("../lib/tauriApi", () => ({
  cancelRecording: vi.fn().mockResolvedValue(undefined),
  fallbackPolicyLabel: vi.fn().mockResolvedValue("prefer_local_ask_before_cloud"),
  recordingStatus: vi.fn().mockResolvedValue("idle"),
  startRecording: vi.fn().mockResolvedValue(undefined),
  stopRecording: vi.fn().mockResolvedValue(undefined),
}));

describe("App", () => {
  it("renders recorder and settings surfaces", async () => {
    render(<App />);

    expect(screen.getByRole("region", { name: "Recorder" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Settings" })).toBeInTheDocument();
  });
});
