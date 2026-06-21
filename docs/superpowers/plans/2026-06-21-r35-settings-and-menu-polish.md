# R3.5 Settings and Menu Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Polish the Settings window into a compact product dashboard and make the menu bar icon open an improved nested native menu on left click.

**Architecture:** Keep settings state and commands unchanged where possible. The frontend reshapes presentation around existing props and callbacks. The Rust tray menu builds native nested submenus and dispatches menu IDs into existing settings/microphone commands.

**Tech Stack:** React, TypeScript, CSS, Vitest, Rust, Tauri v2 native tray/menu APIs.

---

## Files

- Modify: `apps/desktop/src/features/settings/SettingsPanel.tsx`
- Modify: `apps/desktop/src/features/settings/SettingsPanel.test.tsx`
- Modify: `apps/desktop/src/styles.css`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- Modify: `HANDOFF.md`

## Task 1: Settings dashboard UI

- [ ] Add frontend tests proving fallback policy copy is hidden and product copy/actions are visible.
- [ ] Reshape `SettingsPanel.tsx` into a compact dashboard using existing callbacks and settings state.
- [ ] Add CSS for header, cards/rows, compact status chips, and product-styled controls.
- [ ] Run `pnpm --dir apps/desktop test -- SettingsPanel.test.tsx`.

## Task 2: Menu bar nested menu behavior

- [ ] Add Rust source-level tests proving `show_menu_on_left_click(true)` is used and no left-click Settings shortcut remains.
- [ ] Add Rust tests for menu IDs and nested submenu labels/order.
- [ ] Build nested native menu groups for Language, Dictation model, Cleanup, and Microphone above Open Settings.
- [ ] Wire menu events to existing settings mutation paths.
- [ ] Run targeted Rust tests.

## Task 3: Roadmap, handoff, and verification

- [ ] Update roadmap with R3.5 status.
- [ ] Update `HANDOFF.md` with branch/status and next step.
- [ ] Run `pnpm test:ts`.
- [ ] Run `cargo test -p wispergo-desktop`.
- [ ] Run `cargo clippy -p wispergo-desktop --all-targets -- -D warnings`.
- [ ] Run `pnpm desktop:build` and `pnpm check:macos-thin-bundle`.
- [ ] Launch the built app for a smoke check.
