import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { LanguageToggle } from "./LanguageToggle";

const languages = [
  { value: "auto", label: "Auto" },
  { value: "en", label: "English" },
  { value: "zh", label: "Chinese / Mixed" },
] as const;

describe("LanguageToggle", () => {
  it("shows the custom globe icon for automatic language detection", () => {
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

    const button = screen.getByRole("button", { name: "Recognition language: Auto" });
    const icon = button.querySelector("img.language-current-icon");

    expect(button).not.toHaveTextContent("🌐");
    expect(icon).not.toBeNull();
    expect(icon).toHaveAttribute("src", expect.stringContaining("globe_icon_white_transparent.svg"));
    expect(icon).toHaveAttribute("aria-hidden", "true");
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

    expect(screen.getByRole("button", { name: "Recognition language: Chinese / Mixed" })).toHaveTextContent("ZH");
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
    await user.click(screen.getByRole("menuitemradio", { name: "Chinese / Mixed" }));

    expect(onSelect).toHaveBeenCalledWith("zh");
  });

  it("marks the control hovered when native inactive-window tracking enters", () => {
    const { container } = render(
      <LanguageToggle
        language="auto"
        languages={languages}
        menuOpen={false}
        nativeHovered
        onCycle={vi.fn()}
        onSelect={vi.fn()}
        onMenuOpenChange={vi.fn()}
      />,
    );

    expect(container.querySelector(".language-toggle")).toHaveClass("is-native-hovered");
  });

  it("clears native inactive-window hover when the active WebView sees mouse leave", async () => {
    const user = userEvent.setup();
    const onNativeHoverEnd = vi.fn();
    const { container } = render(
      <LanguageToggle
        language="auto"
        languages={languages}
        menuOpen={false}
        nativeHovered
        onCycle={vi.fn()}
        onSelect={vi.fn()}
        onMenuOpenChange={vi.fn()}
        onNativeHoverEnd={onNativeHoverEnd}
      />,
    );
    const toggle = container.querySelector(".language-toggle");
    expect(toggle).not.toBeNull();

    await user.hover(toggle as Element);
    await user.unhover(toggle as Element);

    expect(onNativeHoverEnd).toHaveBeenCalled();
  });
});
