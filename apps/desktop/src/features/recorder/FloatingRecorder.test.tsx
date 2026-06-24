import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { FloatingRecorder } from "./FloatingRecorder";

describe("FloatingRecorder", () => {
  it("renders only the minimized handle while collapsed", () => {
    render(<FloatingRecorder status="idle" expanded={false} />);

    expect(screen.getByRole("region", { name: "Recorder" })).toBeInTheDocument();
    expect(screen.getByLabelText("Wispergo idle handle")).toBeInTheDocument();
    expect(screen.queryByText("Ready")).not.toBeInTheDocument();
    expect(screen.queryByText("Hold Command + Shift + Space to dictate")).not.toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("renders a keyboard-only shortcut prompt while expanded and idle", () => {
    render(<FloatingRecorder status="idle" expanded />);

    expect(screen.getByRole("region", { name: "Recorder" })).toHaveTextContent("Ready");
    expect(screen.getByText("Hold Command + Shift + Space to dictate")).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("renders modifier-hold prompt without duplicate hold wording", () => {
    render(<FloatingRecorder status="idle" expanded shortcutLabel="Hold Right ⌘" />);

    expect(screen.getByText("Hold Right ⌘ to dictate")).toBeInTheDocument();
  });

  it("renders a standalone waveform without visible labels while actively recording", () => {
    render(<FloatingRecorder status="recording" expanded />);

    expect(screen.getByRole("region", { name: "Recorder" })).toBeInTheDocument();
    expect(screen.getByLabelText("Recording waveform")).toBeInTheDocument();
    expect(screen.queryByText("Recording")).not.toBeInTheDocument();
    expect(screen.queryByText("release to insert")).not.toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("renders setup-needed guidance without exposing controls while expanded", () => {
    render(<FloatingRecorder status="idle" setupNeeded expanded />);

    expect(screen.getByRole("region", { name: "Recorder" })).toHaveTextContent("Setup needed");
    expect(screen.getByText("open settings to finish")).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("renders processing as the pill instead of the waveform while expanded", () => {
    render(<FloatingRecorder status="recording" busy expanded />);

    expect(screen.getByRole("region", { name: "Recorder" })).toHaveTextContent("Processing");
    expect(screen.queryByLabelText("Recording waveform")).not.toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
