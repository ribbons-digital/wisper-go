import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { FloatingRecorder } from "./FloatingRecorder";

describe("FloatingRecorder", () => {
  it("renders a keyboard-only shortcut prompt while idle", () => {
    render(<FloatingRecorder status="idle" />);

    expect(screen.getByRole("region", { name: "Recorder" })).toHaveTextContent("Ready");
    expect(screen.getByText("hold Command + Shift + Space")).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("renders a concise recording prompt without controls", () => {
    render(<FloatingRecorder status="recording" />);

    expect(screen.getByRole("region", { name: "Recorder" })).toHaveTextContent("Recording");
    expect(screen.getByText("release to insert")).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("renders processing without exposing controls", () => {
    render(<FloatingRecorder status="idle" busy />);

    expect(screen.getByRole("region", { name: "Recorder" })).toHaveTextContent("Processing");
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
