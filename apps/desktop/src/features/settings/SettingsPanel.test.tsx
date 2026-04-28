import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";

describe("SettingsPanel", () => {
  it("changes recording mode", async () => {
    const user = userEvent.setup();
    const onModeChange = vi.fn();

    render(
      <SettingsPanel
        mode="toggle"
        fallbackPolicy="prefer_local_ask_before_cloud"
        onModeChange={onModeChange}
      />,
    );

    await user.selectOptions(screen.getByLabelText("Recording mode"), "press_and_hold");
    expect(onModeChange).toHaveBeenCalledWith("press_and_hold");
  });
});
