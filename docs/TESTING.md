# Testing — Prism Shell

Status: portable unit tests, Linux visual tests, virtual-display interaction tests, Windows MSVC
cross-builds, and native `windows-latest` compile/test runs were executed. Basic Windows 11
taskbar/icons/activation/coexistence is owner-confirmed. Live DWM previews, the taskbar recovery
watchdog, taskbar replacement, and the standalone Windows-key hook are not yet runtime-validated.

Last update: 2026-11-22.

## 1. Test layers

| Layer | What it protects | Where |
|---|---|---|
| Unit/domain | Grouping, click intents, window lifecycle, overflow math, config validation, clock rollover, capability/phase states | `prism-core` tests |
| Mock platform | Deterministic snapshots, pending/error reconciliation, media transitions, scenario integrity | `prism-platform` tests |
| View mapping | Icon/copy mapping, launcher filtering, durations, audio default | `prism-ui/src/model.rs` tests |
| App support | CLI parsing, config fallback, screenshot path validation, toggle-id parsing | `prism-app` tests |
| Windows geometry/identity | DPI physical rects, panel/preview clamping, thumbnail cropping, executable normalization, friendly names, window eligibility, grouping | `prism-windows` portable tests |
| Windows keyboard/recovery logic | Standalone Win tap vs combination, auto-repeat/injected input, fallback hotkey parsing, marker round-trip, recovery CLI | `prism-windows` portable tests |
| Visual | Layout, typography, spacing, overflow, DPI behavior, state presentation | Xvfb screenshots + manual review |
| Interaction | Real pointer/keyboard state transitions | `xdotool` against Xvfb |
| Windows compile | MSVC target, link, PE output, static CRT | `cargo xwin` + `windows-latest` CI |
| Windows runtime | HWND styles, DWM alpha/thumbnails, focus/z-order, Explorer coexistence, activation, previews, recovery, Windows key | **Basic taskbar/icons/activation/coexistence owner-confirmed; new phases unverified** |

## 2. Commands and results

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets
cargo xwin build --target x86_64-pc-windows-msvc -p prism-app --bins
cargo xwin build --release --target x86_64-pc-windows-msvc -p prism-app --bins
```

Executed results:

- **Formatting:** `cargo fmt --all -- --check` passed.
- **Tests:** 92 passed, 0 failed, 0 ignored:
  - `prism-core`: 29
  - `prism-platform`: 9
  - `prism-app`: 16
  - `prism-ui`: 4
  - `prism-windows`: 34 portable tests (layout/previews, keyboard state/hotkeys, recovery marker/
    CLI, naming, eligibility, grouping)
  - binaries/doc tests: 0
- **Clippy:** `cargo clippy --workspace --all-targets` completed with no warnings or errors.
- **Windows-target Clippy:** `cargo xwin clippy --target x86_64-pc-windows-msvc --workspace
  --all-targets` completed with no warnings after cleanup.
- **Windows debug cross-build:** `cargo xwin build --target x86_64-pc-windows-msvc -p prism-app
  --bins` produced `prism-shell.exe`, `prism-recovery.exe`, and `prism-preview.exe` (PE32+ x86-64).
  Static CRT link succeeded; the import table has no VC runtime DLL. The only noise is benign
  `LNK4099` warnings from debug CRT PDB references in the xwin SDK.
- **Windows release cross-build:** `cargo xwin build --release --target
  x86_64-pc-windows-msvc -p prism-app --bins` produced a 14.5 MB GUI `prism-shell.exe`, a 325 KB
  console `prism-recovery.exe`, and a 14.8 MB console mock `prism-preview.exe`, with no VC runtime
  import. This proves the toolchain/link path only.
- **Windows runtime:** the owner confirmed basic taskbar rendering, native icons, click activation,
  and coexistence with the original taskbar on Windows 11. Live previews, replacement/recovery, and
  Windows-key interception are **not yet runtime-validated**; use `WINDOWS_VALIDATION.md`.
- **Native Windows CI:** two runs passed on the previous baseline:
  - [44983452807](https://github.com/Scatter97/prism-shell/actions/runs/44983452807) for `25668a9`:
    Linux 9m55s, Windows 14m31s.
  - [44985985957](https://github.com/Scatter97/prism-shell/actions/runs/44985985957) for the HWND
    fix `3936efd`: Linux 15m14s, Windows 19m42s.
  - Both Windows jobs ran format, Clippy, the then-current 72 tests, and debug executables.
  - The runs have one non-blocking annotation: `actions/checkout@v4` is being moved from Node 20 to
    Node 24 by GitHub.
  - These are hosted Windows Server runners, not Windows 11 desktops; they do not validate DWM,
    Explorer, focus, transparency, thumbnails, recovery, or the taskbar overlay.
  - This milestone has not been pushed, so no new CI run is claimed. The local Windows-target
    Clippy/build results above are compile evidence only.
- **CI configuration:** `.github/workflows/ci.yml` now cancels superseded runs on the same ref and
  ignores Markdown-only pushes. The runtime checklist is still the only Windows desktop test.
- **Automated UI snapshot matrix:** `scripts/test/ui-snapshots.sh` generated 14 UI-only PNGs at
  1920×1080, 1280×800, and 125%/150%/200% scale. All files were non-empty; dimensions were checked
  with `file` (1920×1080 for scale checks, 1280×800 for the small canvas). A separate 3840×2160
  capture verified the full UI at 200% on a 4K logical canvas. Re-run after the Phase 3A UI changes
  passed.

## 3. Domain coverage

`prism-core` tests include:

- Pinned-before-running ordering.
- Launch on a stopped pinned app.
- Multiple-window grouping.
- Active-app click → minimize; second click → restore.
- Minimizing one of two windows activates the visible window; all-minimized restores the MRU
  window.
- Closing the last unpinned window removes the entry; closing the last pinned window keeps it.
- Missing-window reconciliation is idempotent.
- Overflow arithmetic at 1920/1280/narrow stages; no offscreen overflow.
- Config round-trip, malformed JSON, unknown version, duplicate pins, missing optional fields.
- 24-hour/12-hour clock, midnight/noon, date rollover.

`prism-platform` tests include:

- Every fixture has a coherent snapshot.
- Wi-Fi pending → reconciled off state.
- Unsupported Night light is distinct from permission-required Bluetooth.
- Fault fixture Bluetooth failure does not leave an optimistic state.
- Volume clamping/mute behavior; no media session error.
- Settings/power actions are explicitly `Simulated`, never real.

`prism-app` tests cover CLI parsing (inline/separate values, ranges, unknown options, scale,
artifact flags), config fallback, screenshot path logic, and toggle ids. `prism-ui` tests cover
copy/mapping/filtering.

`prism-windows` portable tests cover:

- Taskbar window/dock physical geometry at 100% and 150%, including the 14 px bottom gap.
- Control Center right alignment, launcher left alignment, and panel gaps.
- Work-area clamping for small screens and negative-origin monitors.
- Shadow-margin inflation/deflation and the shared Slint dock-width formula.
- Preview panel growth/clamping, app-slot anchoring, thumbnail/card alignment, DPI scaling, and
  center-crop aspect math.
- Executable path normalization, friendly names, and pinned-app matching.
- Window eligibility (tool/owned/cloaked/zero-size/minimized), UWP-host separation, and grouping
  by normalized executable.
- `DockState::record_window_discovered` adding/merging tracked windows.
- Standalone Windows-key tap vs every tested combination, modifier-only combinations, auto-repeat,
  both Windows keys, injected events, orphan key-up, and fallback-hotkey parsing.
- Taskbar recovery marker round-trip, malformed markers, original visibility preservation, and
  recovery CLI validation.

The Win32 modules themselves are only compile-checked. They have no Windows unit tests yet. DWM
thumbnail hosting, the watchdog process, and the low-level hook cannot be tested on Linux.

## 4. Linux visual and interaction tests

### 4.1 Environment

- Rootless Xvfb at 1920×1080, 1280×800, and 2560×1440.
- ffmpeg `x11grab` full-display captures for real compositor output.
- Slint `take_snapshot` for UI-only deterministic captures.
- Real pointer/keyboard events via portable `xdotool`.

### 4.2 Screenshots produced

All paths are under `artifacts/screenshots/` (gitignored, local review artifacts):

| Artifact | What it shows |
|---|---|
| `snapshot-cc-1920x1080.png` | Full Control Center, dock, desktop context, pinned utilities |
| `preview-1920x1080-dock.png` | Taskbar baseline at 1920×1080 |
| `state-launcher.png` | Launcher with pinned/running apps |
| `state-preview.png` | Two-window browser preview |
| `state-confirm.png` | Power confirmation sheet over the CC |
| `state-scenario-desktop.png` | No battery, unsupported brightness, media empty |
| `state-scenario-minimal.png` | No audio, no Wi-Fi adapter, permission-required Bluetooth, no media |
| `state-scenario-faults.png` | Audio endpoint loss, media session error, Bluetooth failure path |
| `preview-1280x800-many-v2.png` | 29-app overflow chip (“+14”) at 1280×800 |
| `preview-1280x800-cc-v3.png` | CC constrained-height scrolling with pinned utilities |
| `dpi-125-cc.png` | 2560×1440 capture with logical 2048×1152 at 125% |
| `dpi-150-cc.png` | 2560×1440 capture with logical 1706×960 at 150% |
| `dpi-200-cc.png` | 1920×1080 capture with logical 960×540 at 200% (stress) |
| `dpi-200-4k.png` | 3840×2160 capture: logical 1920×1080 at 200% (4K native) |
| `interaction2-wifi-pending.png` | Wi-Fi tile pending spinner + “Changing…” |
| `interaction2-power-toast.png` | Confirmed power action, CC closed, top-left toast |
| `interaction3-minimize.png` | Active browser minimized; no hover preview re-opened |
| `interaction4-preview-aligned.png` | Preview anchored above the hovered browser icon |
| `interaction5-faults-error.png` | Bluetooth failure warning, toast, unchanged authoritative state |
| `interaction6-close-window.png` | Window close action + toast |
| `interaction3-tooltip-aligned.png` | Pinned non-running app tooltip above the icon |
| `interaction-search.png` | Launcher filter “ter” → Terminal |
| `interaction-mail-launched.png` | Mail launch via launcher → running/active + success toast |
| `auto-*.png` | 14 automated captures from `scripts/test/ui-snapshots.sh` (dock, CC fixtures, launcher, preview, confirmation, overflow, 125/150/200%) |

The screenshots are **review artifacts**, not pixel-diff baselines. The clock is real local time,
so images vary by capture time. `--scale 1` is required for a 1:1 logical PNG.

### 4.3 Verified interactions

- Control Center opens from the taskbar trigger.
- Escape closes the CC.
- Clicking the wallpaper/desktop mock closes the CC.
- Repeated trigger clicks toggle; closed overlays do not intercept clicks.
- Wi-Fi toggle: pending phase with spinner, then off; taskbar status and connectivity row
  reconcile; toast reports “preview only”.
- Volume slider drag: value and percentage update; mock platform accepts it.
- Audio picker expands, marks the default output, and is scroll-contained on constrained heights.
- Launcher search filters the grid; clicking Mail launches it, closes the launcher, updates the
  active/running state, and shows a success toast.
- Hovering Files (pinned, not running) shows a tooltip above the icon.
- Hovering Browser shows a two-window preview above the icon; clicking a card activates;
  clicking a card close removes the window and shows a toast.
- Clicking the active Browser app minimizes it; a suppressed hover timer prevents the preview
  from immediately reopening.
- Power utility opens a confirmation sheet; Shut down requires a second click; the CC closes and
  the toast states that no action was taken.
- Faults fixture Bluetooth rejection leaves the radio on and displays a warning/error toast.
- Many-app fixture produces an overflow chip at 1280 and 1920.

### 4.4 Responsive/DPI review

- Dock overflow at 1280 with 29 apps: visible icons + “+14” chip; fits inside the stage.
- Control Center at 1280×800 and 150%/200% logical sizes: content scrolls, utility bar stays
  pinned, scrollbar visible.
- At 200% on a 960×540 logical canvas the decorative desktop mock is hidden to avoid a cramped,
  overlapping scene; the shell surfaces remain usable.
- At 200% on a 4K canvas (logical 1920×1080) the full UI renders identically at native 3840×2160
  with no clipping.
- No content is positioned from a single hardcoded resolution; the taskbar centers from stage
  width, and the CC anchors to the taskbar's right edge with clamping.

## 5. What was not tested

Do not claim these as complete:

- Windows 11 desktop runtime for DWM live thumbnails, preview hover handoff, preview activation,
  minimized/destroyed/fullscreen preview behavior, or preview DPI/edge clamping.
- Taskbar replacement hiding/restoration, watchdog timeout after forced kill/suspend, Explorer
  restart, secondary taskbars, or replacement-mode work-area behavior.
- Standalone Windows-key interception and native shortcut preservation with synthetic replay,
  elevated apps, fullscreen apps, or hook cleanup under kill/crash.
- Full Control Center interactions; basic launcher/CC presence is owner-confirmed only.
- Win32 `EnumWindows`/`SetWinEventHook` edge cases across the full app matrix, AUMID/UWP identity,
  icon extraction failures, elevated/UIPI targets.
- Lock/sleep/resume, Explorer restart in default mode, monitor hotplug, RDP.
- Primary-monitor positioning at real 100/125/150/200% DPI, mixed-DPI, multi-monitor.
- Core Audio, default-device changes, radio/Wi-Fi/BT toggles, location consent.
- Brightness WMI/DDC-CI and monitor safety.
- GSMTC media sessions in an unpackaged binary.
- Tray replication and notification interception (deferred/blocked).
- Narrator/AccessKit verification; full keyboard tab navigation; high-contrast mode.
- Performance baselines, memory/GPU measurements, 8-hour soak, 100+ windows.
- Installer, auto-start, code signing, single-instance guard, distribution license.

## 6. Windows 11 validation plan

The executable checklist is `docs/WINDOWS_VALIDATION.md`; `scripts/test/windows-smoke.ps1` is a
non-destructive launch/screenshot/log harness. Neither has run. Start on a disposable Windows 11
x64 VM, normal user, Explorer as the normal shell, single monitor at 100%, then:

1. Confirm `prism-shell.exe` starts and Explorer remains visible/usable.
2. Verify transparency/frameless rendering and the dock work-area position.
3. Verify click-through outside the dock, non-activation, and real app activation/minimize/restore.
4. Compare enumeration/grouping with Task View/Alt+Tab for Win32, UWP, multi-window, minimized,
   owned, tool, elevated, cloaked/virtual-desktop windows.
5. Verify Control Center focus-loss/Escape close, real battery/Settings/lock/exit, and no fake
   unsupported controls.
6. Test 100/125/150/200%, monitor changes, lock/sleep/resume, Explorer restart, and Prism kill.
7. Run Phase A preview checks, then the Phase B recovery gate on a disposable VM, then the Phase C
   Windows-key shortcut matrix.
8. Only after those pass: consider AppBar work-area reservation, multi-monitor surfaces,
   Core Audio, radios/WLAN, brightness, GSMTC, and tray. Each subsystem needs its own API research
   and explicit approval.

## 7. Reproducibility

- Rust version is pinned by `rust-toolchain.toml`; `Cargo.lock` pins dependency versions.
- Mock fixtures use deterministic ids/order; window ids are generated deterministically.
- The virtual display is disposable and rootless.
- Screenshots use an explicit scale for logical-pixel review.
- No test weakens a production guard to pass. Mock failures are deliberate and do not mutate
  authoritative state.
