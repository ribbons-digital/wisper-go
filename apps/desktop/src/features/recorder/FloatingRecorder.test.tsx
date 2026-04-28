import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { FloatingRecorder } from "./FloatingRecorder";

describe("FloatingRecorder", () => {
  it("starts and stops recording in toggle mode", async () => {
    const user = userEvent.setup();
    const onStart = vi.fn();
    const onStop = vi.fn();

    const { rerender } = render(
      <FloatingRecorder
        status="idle"
        mode="toggle"
        onStart={onStart}
        onStop={onStop}
        onCancel={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Start recording" }));
    expect(onStart).toHaveBeenCalledWith("toggle");

    rerender(
      <FloatingRecorder
        status="recording"
        mode="toggle"
        onStart={onStart}
        onStop={onStop}
        onCancel={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Stop recording" }));
    expect(onStop).toHaveBeenCalledWith("floating_button");
  });
});
