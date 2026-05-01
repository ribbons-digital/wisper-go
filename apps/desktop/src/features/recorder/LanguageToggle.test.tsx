import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { LanguageToggle } from "./LanguageToggle";

const languages = [
  { value: "auto", label: "Auto" },
  { value: "en", label: "English" },
  { value: "zh", label: "Chinese" },
] as const;

describe("LanguageToggle", () => {
  it("shows a globe for automatic language detection", () => {
    render(
      <LanguageToggle
        language="auto"
        languages={languages}
        menuOpen={false}
        onCycle={vi.fn()}
        onSelect={vi.fn()}
        onMenuOpenChange={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Recognition language: Auto" })).toHaveTextContent("🌐");
  });

  it("shows two-letter language codes for explicit languages", () => {
    render(
      <LanguageToggle
        language="zh"
        languages={languages}
        menuOpen={false}
        onCycle={vi.fn()}
        onSelect={vi.fn()}
        onMenuOpenChange={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Recognition language: Chinese" })).toHaveTextContent("ZH");
  });

  it("cycles when the primary language button is clicked", async () => {
    const user = userEvent.setup();
    const onCycle = vi.fn();
    render(
      <LanguageToggle
        language="auto"
        languages={languages}
        menuOpen={false}
        onCycle={onCycle}
        onSelect={vi.fn()}
        onMenuOpenChange={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Recognition language: Auto" }));

    expect(onCycle).toHaveBeenCalled();
  });

  it("closes the language menu when hovering off the control", async () => {
    const user = userEvent.setup();
    const onMenuOpenChange = vi.fn();
    const { container } = render(
      <LanguageToggle
        language="auto"
        languages={languages}
        menuOpen
        onCycle={vi.fn()}
        onSelect={vi.fn()}
        onMenuOpenChange={onMenuOpenChange}
      />,
    );
    const toggle = container.querySelector(".language-toggle");
    expect(toggle).not.toBeNull();

    await user.hover(screen.getByRole("button", { name: "Recognition language: Auto" }));
    await user.unhover(toggle as Element);

    expect(onMenuOpenChange).toHaveBeenCalledWith(false);
  });

  it("opens menu from chevron and selects a single language", async () => {
    const user = userEvent.setup();
    const onMenuOpenChange = vi.fn();
    const onSelect = vi.fn();
    render(
      <LanguageToggle
        language="en"
        languages={languages}
        menuOpen
        onCycle={vi.fn()}
        onSelect={onSelect}
        onMenuOpenChange={onMenuOpenChange}
      />,
    );

    expect(screen.getByRole("menuitemradio", { name: "English" })).toHaveAttribute("aria-checked", "true");
    await user.click(screen.getByRole("menuitemradio", { name: "Chinese" }));

    expect(onSelect).toHaveBeenCalledWith("zh");
  });
});
