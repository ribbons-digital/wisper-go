# R1 First-Run Setup and Model Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a fresh end-user install guide users through permissions and default model downloads, and prevent unclear dictation failures before the app is ready.

**Architecture:** Add a small setup-readiness layer that combines permission and Asset readiness, show the settings/setup window automatically when setup is incomplete, harden `start_recording` with a clear not-ready error for missing microphone or required model assets, and reshape the settings surface into a setup checklist. Keep the existing Asset downloader and model activation behavior; do not change ASR/cleanup/insertion semantics.

**Tech Stack:** Tauri v2, Rust desktop commands, React 18, Vitest/Testing Library, existing `AssetDownloadStatus`, macOS permission helpers.

---

## Scope

Implement R1 from `docs/superpowers/specs/2026-06-20-release-readiness-and-ui-polish-design.md`.

In scope:

- Auto-show the settings/setup window when required setup is incomplete.
- Present a setup checklist for microphone permission, Accessibility permission, and required default model Assets.
- Keep the existing automatic default Asset download and retry flow.
- Return a clear error if the shortcut starts dictation before required setup can support recording/transcription.
- Update README, roadmap, and HANDOFF.

Out of scope:

- Byte-level download progress.
- Icon changes.
- Recording waveform UI.
- GitHub Actions release pipeline.
- Cleanup model picker.
- Streaming partial transcripts.

## Files

- Modify: `apps/desktop/src-tauri/src/lib.rs`
  - Add setup auto-show helper and tests.
  - Expose `show_settings` internally so recording/startup can reuse it if needed.
- Modify: `apps/desktop/src-tauri/src/commands/recording.rs`
  - Add readiness guard before live recording starts.
  - Keep test helper paths working.
- Modify: `apps/desktop/src-tauri/src/commands/assets.rs`
  - Reuse existing default Asset readiness helpers if needed.
  - Prefer adding small helper functions rather than duplicating manifest/storage logic.
- Modify: `apps/desktop/src/features/settings/SettingsPanel.tsx`
  - Add setup status/checklist UI.
  - Preserve model settings and permission actions.
- Modify: `apps/desktop/src/features/settings/SettingsPanel.test.tsx`
  - Add RED tests for setup checklist states.
- Modify: `apps/desktop/src/app/App.test.tsx`
  - Add RED test for recorder not-ready error path.
- Modify: `apps/desktop/src/styles.css`
  - Add restrained product UI styling for setup checklist.
- Modify: `README.md`
  - Document first-run setup behavior.
- Modify: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
  - Mark R1 in progress/done as appropriate.
- Modify: `HANDOFF.md`
  - Sync current slice and next step.

## Task 1: Settings setup checklist UI

**Files:**

- Modify: `apps/desktop/src/features/settings/SettingsPanel.test.tsx`
- Modify: `apps/desktop/src/features/settings/SettingsPanel.tsx`
- Modify: `apps/desktop/src/styles.css`

- [ ] **Step 1: Write failing checklist tests**

Add tests near existing SettingsPanel permission/model tests:

```tsx
it("shows setup needed when microphone, accessibility, or models are missing", async () => {
  const { assetReadiness } = await import("../../lib/tauriApi");
  vi.mocked(assetReadiness).mockResolvedValueOnce({
    state: "missing",
    assetId: "medium",
    displayName: "Whisper medium",
  });

  renderSettingsPanel({
    microphone: { granted: false, canPrompt: true },
    accessibility: { granted: false, canPrompt: true },
  });

  expect(await screen.findByText("Setup needed")).toBeInTheDocument();
  expect(screen.getByText("Microphone permission")).toBeInTheDocument();
  expect(screen.getByText("Accessibility permission")).toBeInTheDocument();
  expect(screen.getByText("Required local models")).toBeInTheDocument();
});

it("shows ready when permissions and required models are ready", async () => {
  const { assetReadiness } = await import("../../lib/tauriApi");
  vi.mocked(assetReadiness).mockResolvedValueOnce({ state: "ready" });

  renderSettingsPanel();

  expect(await screen.findByText("Ready for dictation")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
pnpm --dir apps/desktop test src/features/settings/SettingsPanel.test.tsx
```

Expected: FAIL because `Setup needed`, `Ready for dictation`, and checklist labels do not exist.

- [ ] **Step 3: Implement minimal checklist UI**

In `SettingsPanel.tsx`:

- Add a helper such as `setupState(microphone, accessibility, assetStatus)`.
- Render a top section with:
  - `Ready for dictation` when microphone/accessibility are granted and assets are ready.
  - `Setup needed` otherwise.
- Render checklist rows:
  - Microphone permission: Ready / Needs permission.
  - Accessibility permission: Ready / Needs permission.
  - Required local models: Ready / Downloading / Needs download / Failed.
- Keep existing buttons for permission grant, refresh, and retry download.

- [ ] **Step 4: Add restrained styles**

In `styles.css`, add classes such as:

```css
.setup-summary {
  display: grid;
  gap: 10px;
  padding: 14px;
  border: 1px solid #d6dde5;
  border-radius: 10px;
  background: #f7f9fb;
}

.setup-summary h2 {
  margin: 0;
  color: #18202a;
  font-size: 1rem;
}

.setup-checklist {
  display: grid;
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.setup-checklist li {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  color: #53657a;
  font-size: 0.9rem;
}
```

- [ ] **Step 5: Verify GREEN**

Run:

```bash
pnpm --dir apps/desktop test src/features/settings/SettingsPanel.test.tsx
```

Expected: PASS.

## Task 2: Backend launch readiness and auto-show setup window

**Files:**

- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/commands/assets.rs`

- [ ] **Step 1: Write failing pure Rust tests**

In `apps/desktop/src-tauri/src/lib.rs` tests, add pure tests for auto-show decision:

```rust
#[test]
fn setup_window_opens_when_required_setup_is_incomplete() {
    assert!(setup_window_should_open(false, true, true));
    assert!(setup_window_should_open(true, false, true));
    assert!(setup_window_should_open(true, true, false));
}

#[test]
fn setup_window_stays_hidden_when_required_setup_is_complete() {
    assert!(!setup_window_should_open(true, true, true));
}
```

If adding an asset helper in `commands/assets.rs`, add a pure unit test there for converting `AssetDownloadStatus` to a boolean readiness value.

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test -p wispergo-desktop setup_window_opens_when_required_setup_is_incomplete setup_window_stays_hidden_when_required_setup_is_complete
```

Expected: FAIL because helper does not exist.

- [ ] **Step 3: Implement setup-window decision helper**

Add to `lib.rs` near `show_settings`:

```rust
fn setup_window_should_open(
    microphone_granted: bool,
    accessibility_granted: bool,
    required_assets_ready: bool,
) -> bool {
    !microphone_granted || !accessibility_granted || !required_assets_ready
}
```

Add an internal app helper:

```rust
fn show_settings_if_setup_required(app: &tauri::AppHandle) {
    let microphone_ready = microphone_status().granted;
    let accessibility_ready = accessibility_status().granted;
    let assets_ready = matches!(asset_readiness(app.clone()), Ok(AssetDownloadStatus::Ready));

    if setup_window_should_open(microphone_ready, accessibility_ready, assets_ready) {
        let _ = show_settings(app);
    }
}
```

Call it during `setup` after menu/window setup so the `main` window can be shown.

- [ ] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p wispergo-desktop setup_window_opens_when_required_setup_is_incomplete setup_window_stays_hidden_when_required_setup_is_complete
```

Expected: PASS.

## Task 3: Dictation not-ready guard

**Files:**

- Modify: `apps/desktop/src-tauri/src/commands/recording.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` if `show_settings` is reused.
- Modify: `apps/desktop/src/app/App.test.tsx`

- [ ] **Step 1: Write failing frontend test for not-ready message**

In `App.test.tsx`, add a recorder surface test:

```tsx
it("shows a setup message when recording cannot start before setup is ready", async () => {
  window.history.pushState({}, "", "/?surface=recorder");
  vi.mocked(startRecording).mockRejectedValueOnce(
    new Error("Finish Wispergo setup before dictating: download required models."),
  );

  render(<App />);
  await emitFloatingChromeExpanded(true);
  await emitRecordShortcut("Pressed");

  expect(await screen.findByText(/Finish Wispergo setup before dictating/)).toBeInTheDocument();
  expect(stopRecording).not.toHaveBeenCalled();
});
```

- [ ] **Step 2: Run test and verify RED if UI does not show recorder errors**

Run:

```bash
pnpm --dir apps/desktop test src/app/App.test.tsx -- --runInBand
```

Expected: FAIL if recorder-surface errors are not rendered. If it already passes because existing error rendering covers it, keep the test as regression coverage and move to backend tests.

- [ ] **Step 3: Write failing backend readiness tests**

Add pure helper tests for dictation readiness in `recording.rs` or a shared helper:

```rust
#[test]
fn dictation_readiness_requires_microphone_and_assets() {
    assert!(dictation_not_ready_message(false, true).contains("microphone"));
    assert!(dictation_not_ready_message(true, false).contains("models"));
    assert!(dictation_not_ready_message(true, true).is_empty());
}
```

- [ ] **Step 4: Implement backend guard**

Change command signature to include `AppHandle`:

```rust
pub fn start_recording(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: String,
) -> Result<(), String> {
    ensure_ready_to_start_dictation(&app)?;
    state.start_recording(&mode)
}
```

Implement `ensure_ready_to_start_dictation` to require:

- Microphone permission granted.
- Required default Assets ready.

Accessibility remains part of setup checklist but does not block recording, because the app can still copy/fallback when insertion is unavailable.

Return clear errors:

- `Finish Wispergo setup before dictating: grant microphone permission.`
- `Finish Wispergo setup before dictating: download required models.`

If feasible, call `crate::show_settings(&app)` before returning the error so the user sees setup.

- [ ] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p wispergo-desktop dictation_readiness_requires_microphone_and_assets
pnpm --dir apps/desktop test src/app/App.test.tsx
```

Expected: PASS.

## Task 4: Documentation and roadmap status

**Files:**

- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- Modify: `HANDOFF.md`

- [ ] **Step 1: Update README**

Document:

- First launch opens setup if permissions or required models are missing.
- Required default models download automatically from settings/setup.
- Dictation requires microphone permission and required models.
- Accessibility is needed for insertion; without it, insertion may fall back or fail.

- [ ] **Step 2: Update roadmap**

Mark R1 as done when implementation and verification pass:

```markdown
- **R1 First-run setup and model readiness UX** ✅
```

- [ ] **Step 3: Update HANDOFF**

Sync with roadmap and current branch/PR status.

## Task 5: Full verification and PR

- [ ] **Step 1: Run targeted checks**

```bash
pnpm --dir apps/desktop test src/features/settings/SettingsPanel.test.tsx
pnpm --dir apps/desktop test src/app/App.test.tsx
cargo test -p wispergo-desktop setup_window_opens_when_required_setup_is_incomplete setup_window_stays_hidden_when_required_setup_is_complete
cargo test -p wispergo-desktop dictation_readiness_requires_microphone_and_assets
```

- [ ] **Step 2: Run standard PR gates**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy -p wispergo-core --all-targets -- -D warnings
cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings
cargo clippy -p wispergo-desktop --all-targets -- -D warnings
pnpm test:ts
pnpm desktop:build
```

After any `pnpm` command, remove Corepack's out-of-scope `packageManager` field if it appears in root `package.json`.

- [ ] **Step 3: Manual smoke**

Use a temporary clean app-support directory if possible, or temporarily move the existing app-support folder aside:

```bash
mv "$HOME/Library/Application Support/com.ribbonsdigital.wispergo" \
  "$HOME/Library/Application Support/com.ribbonsdigital.wispergo.backup-r1"
open target/release/bundle/macos/Wispergo.app
```

Expected:

- Setup/settings window appears if required setup is incomplete.
- Required model download begins or is clearly prompted.
- Retry path is visible on failure.
- Shortcut before setup shows a clear setup-needed error.

Restore existing app support after smoke.

- [ ] **Step 4: Commit and PR**

Commit message:

```bash
git commit -m "feat(desktop): guide first-run setup readiness"
```

PR title:

```text
feat(desktop): guide first-run setup readiness
```

PR body includes changed files, verification commands, and manual smoke result.

## Self-review checklist

- R1 auto-show setup: Task 2.
- R1 setup checklist: Task 1.
- R1 not-ready shortcut behavior: Task 3.
- R1 docs/roadmap/handoff: Task 4.
- Verification and manual clean-app-support smoke: Task 5.
- No icon, waveform, CI, or streaming work included.
