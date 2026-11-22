# Current Status — Prism Shell

Last updated: 2026-11-22. Target: Windows 11 x64. Development host: Linux.

## Status summary

| Milestone | Status | Evidence |
|---|---|---|
| Phase 0 — research/feasibility | **Research complete** | `docs/research/windows-api-notes.md`, `docs/WINDOWS_API_RESEARCH.md`, `docs/UI_FRAMEWORK_EVALUATION.md` |
| Phase 1 — native foundation | **Implemented; Tested on Linux** | workspace, 92 workspace tests, config/logging, platform traits |
| Phase 2 — taskbar + Control Center preview | **Implemented; Tested on Linux** | interactive Slint mock preview, 14 automated screenshots |
| Phase 3A — native Windows shell host | **Implemented; cross-compiled; owner-confirmed basic Windows 11 runtime** | `prism-windows` crate, Win32/DWM host, taskbar/CC/launcher surfaces |
| Phase 3A — real window tracking | **Implemented; cross-compiled; owner-confirmed basic activation** | `EnumWindows`, icons, foreground events, activation |
| Phase A — live window previews | **Implemented; cross-compiled; unit-tested; Windows runtime Unverified** | `prism-windows/src/preview.rs`, DWM thumbnail layout tests, new preview surface |
| Phase B — independent taskbar recovery | **Implemented; cross-compiled; portable marker tests; Windows recovery Unverified** | `prism-recovery.exe`, watchdog heartbeat, marker file |
| Phase B — native taskbar replacement | **Implemented behind a disabled experimental gate; Blocked for enablement** | `taskbar_replacement.rs`; do not enable until the recovery checklist passes on Windows 11 |
| Phase C — standalone Windows-key launcher | **Implemented behind a disabled experimental gate; Windows runtime Unverified** | portable hook state machine, `WH_KEYBOARD_LL`, Ctrl+Alt+Space fallback |
| Real system controls (audio/radios/brightness/media) | **Not implemented** | shown as unavailable or native Settings fallbacks |

The owner has confirmed on a real Windows 11 desktop that the floating Prism taskbar renders, native
application icons appear, clicking dock icons switches real windows, and the original Windows
taskbar remains visible beneath Prism. The launcher and Control Center are implemented, but not
every interaction has been validated. This supersedes the earlier failed first-start report; the
deferred-HWND fix (`3936efd`) is working in the owner's build.

The new live-preview, taskbar-replacement, and Windows-key paths have **not** been runtime-tested on
Windows 11 in this milestone. Cross-compilation, unit tests, and Linux visual tests are not Windows
runtime evidence.

## This milestone

### Phase A — real window previews

Implemented a fourth production surface, `ShellWindowPreview`, and a DWM thumbnail manager:

- Hovering a running dock icon for 300 ms opens a preview above the dock.
- The preview shows real window content via `DwmRegisterThumbnail` /
  `DwmUpdateThumbnailProperties`, plus the real application icon, window title, application name,
  and a per-window close button.
- Multiple windows are shown as separate cards; clicking a card activates that HWND.
- DWM thumbnails are center-cropped to the card aspect ratio using `DwmQueryThumbnailSourceSize`.
- Minimized windows keep their card and restore action but intentionally do not register a live
  thumbnail; the card shows the existing "Minimized" state.
- Destroyed windows are removed by the existing WinEventHook dirty/reconcile path, and their
  thumbnails are unregistered.
- The preview window is pre-created, frameless, topmost, non-activating, and click-through outside
  its visible panel. Moving from the dock into the preview keeps it open; a native
  `WM_MOUSEMOVE`/`WM_MOUSELEAVE` tracker is authoritative because card TouchAreas sit above the
  Slint root hover area.
- All DWM thumbnails are unregistered on close, app switch, hide, and shutdown. Thumbnails are
  registered only after the 170 ms surface fade so they do not pop through a transparent panel.
- The Linux mock preview still uses its designed thumbnails; the Windows backend sets
  `preview-live-thumbnails` so the mock miniature is not shown behind real thumbnails.

**Windows runtime risk:** DWM thumbnails against a transparent Slint/winit destination HWND are
unverified. If the destination is layered or the renderer prevents DWM composition of thumbnails,
the cards degrade to a neutral placeholder rather than fake content.

### Phase B — safe experimental taskbar replacement

Implemented but **disabled by default**:

- `prism-recovery.exe` is a standalone console binary (325 KB release, only `KERNEL32`, `user32`,
  `oleaut32`, `ntdll`, and a synchronous API-set import).
- Before hiding anything, Prism starts the watchdog and creates a named auto-reset heartbeat.
- `prism-recovery.exe --watch-parent <pid>` restores the taskbar if the heartbeat stops for 4
  seconds (crash, force-termination, freeze) or the parent process exits.
- A marker file at `%LOCALAPPDATA%\Prism\taskbar-replacement.state` records every native taskbar
  HWND and whether it was originally visible. The watchdog does nothing without an active marker.
- Prism also checks for a stale marker at startup and restores before creating any UI.
- Hiding uses `ShowWindow(SW_HIDE)` on `Shell_TrayWnd` / `Shell_SecondaryTrayWnd` only.
  Explorer is never terminated, replaced, injected, or registered as a shell; no registry or
  startup settings are changed.
- Visibility is verified after hiding. A partial failure restores every taskbar before returning.
- In replacement mode the dock is positioned against the full primary monitor rectangle with the
  existing 14 logical-pixel bottom gap, not the still-taskbar-reduced work area.
- The Control Center shows a "Restore bar" action and documents that the recovery executable is the
  independent fallback. Clean exit and "Restore bar" both show the native taskbar again.
- If Explorer restarts or the taskbar becomes visible again, Prism leaves replacement mode rather
  than fighting Explorer.
- Manual recovery works without Prism: `prism-recovery.exe --restore` and `--status`.

**Critical gate:** the independent recovery mechanism has **not** been exercised on a real Windows
11 desktop. Taskbar replacement therefore must stay disabled until the recovery checklist in
`docs/WINDOWS_VALIDATION.md` passes. Do not use `--experimental-taskbar-replacement` on the owner's
machine yet.

### Phase C — standalone Windows key

Implemented but **disabled by default**:

- A low-level keyboard hook (`WH_KEYBOARD_LL`) on the UI thread recognizes a standalone left or
  right Windows key tap. A tap toggles the Prism launcher; a second tap closes it.
- A physical Windows key-down is swallowed. If any other key is pressed while it is held, the hook
  injects a synthetic Windows key-down before passing the combination, preserving `Win+E`,
  `Win+R`, `Win+L`, `Win+D`, `Win+Tab`, `Win+Shift+S`, `Win+I`, `Win+X`, `Win+V`, arrows, and other
  native shortcuts.
- The standalone Windows key-up is swallowed, so no modifier state is left stuck and the native
  Start menu does not receive a complete tap.
- Auto-repeat, both Windows keys, injected input, modifier-only combinations, and orphan key-up
  events have portable unit tests.
- If the hook cannot be installed, Prism automatically registers a fallback hotkey. The default is
  `Ctrl+Alt+Space`; `experimental.windows_key_fallback_shortcut` accepts `Ctrl`, `Alt`, `Shift`, and
  `Win` plus letters, digits, `Space`, `Tab`, F1–F24, arrows, and common navigation keys.
- The hook is removed on clean shutdown. Windows removes it automatically if Prism crashes.
- The integration is not installed unless the config or explicit CLI flag enables it.

**Windows runtime risk:** synthetic replay, elevated foreground apps, exclusive fullscreen,
keyboard-layout edge cases, and Start-menu suppression are unverified. Secure-desktop shortcuts
(such as Ctrl+Alt+Del) never reach the hook.

## Verified on Linux

- `cargo fmt --all -- --check` passed.
- `cargo test --workspace`: **92 passed, 0 failed, 0 ignored**.
  - `prism-core`: 29
  - `prism-platform`: 9
  - `prism-app`: 16
  - `prism-ui`: 4
  - `prism-windows`: 34 portable tests (layout, preview geometry/cropping, keyboard state/hotkey
    parsing, recovery marker/commands, naming, eligibility, grouping)
- `cargo clippy --workspace --all-targets` is clean (no warnings).
- `cargo xwin clippy --target x86_64-pc-windows-msvc --workspace --all-targets` is clean.
- Linux visual regression: `scripts/test/ui-snapshots.sh` regenerated all 14 captures
  (1920×1080, 1280×800, 125%/150%/200%) with the updated preview close-button placement.
- The Linux preview still runs only on mock data and remains visibly labeled as such.

## Cross-compiled for Windows

Release cross-build with `cargo xwin` succeeded:

| Artifact | Size | Subsystem |
|---|---|---|
| `prism-shell.exe` | 14.5 MB | Windows GUI |
| `prism-recovery.exe` | 325 KB | Windows console |
| `prism-preview.exe` | 14.8 MB | Windows console (mock development preview) |

`prism-shell.exe` imports only system DLLs (kernel32, user32, dwmapi, shell32, comctl32, gdi32,
ole32, opengl32, etc.). The static MSVC CRT is used; there is no VCRUNTIME140/MSVCP dependency.
The release link still prints benign `LNK4099` warnings for missing Microsoft CRT PDBs supplied by
cargo-xwin; the binaries are produced successfully.

Native `windows-latest` CI is defined in `.github/workflows/ci.yml`; this milestone has not been
pushed, so no new CI run is claimed. The previous two Windows CI runs passed the older 72-test
baseline:

- [44983452807](https://github.com/Scatter97/prism-shell/actions/runs/44983452807) for `25668a9`:
  Linux 9m55s, Windows 14m31s.
- [44985985957](https://github.com/Scatter97/prism-shell/actions/runs/44985985957) for the HWND fix
  `3936efd`: Linux 15m14s, Windows 19m42s.

Both Windows jobs ran format, Clippy, 72 tests, and debug executables on hosted Windows Server
runners, not Windows 11 desktops. A new CI run should be executed after push.

## Owner-confirmed on Windows 11

- `prism-shell.exe` starts and stays running.
- The floating taskbar renders with native application icons.
- Clicking application icons activates/switches real windows.
- The original Windows taskbar remains visible and usable alongside Prism.
- The launcher and Control Center are implemented; not every interaction is validated.

## Not yet validated on Windows 11

- Live DWM thumbnails, preview positioning, hover handoff, activation from a preview card, close
  buttons, minimized/destroyed/fullscreen windows, and DPI/edge clamping for previews.
- The independent recovery watchdog, taskbar hiding/restoration, Explorer restart behavior,
  multi-monitor secondary taskbars, heartbeat timeout, force-kill recovery, and replacement-mode
  work-area behavior.
- Standalone Windows-key interception, shortcut preservation with the synthetic replay, elevated
  apps, fullscreen apps, and hook cleanup under kill/crash.
- Control Center interactions beyond the owner's basic smoke use; real battery/Settings/lock/exit
  require the existing checklist.

## Known limitations / risks

- DWM thumbnails cannot be rounded by Slint; the card surface is rounded and the thumbnail is
  inset, but the live image itself has square corners.
- Minimized windows show a "Minimized" card without a live thumbnail.
- Previews beyond the monitor width fall back to the existing scrolling card strip; only the
  on-screen card positions map to DWM thumbnails. Typical multi-window apps fit.
- Taskbar replacement cannot reserve work area. Maximized windows still use the Windows work area,
  so the Prism dock overlaps their bottom edge. AppBar reservation remains deferred.
- The notification area and third-party tray icons are not replicated. While the native taskbar is
  hidden, the documented fallback is the Control Center "Restore bar" action or
  `prism-recovery.exe --restore`.
- Replacement mode does not by itself redirect the native Windows key. Without Phase C, pressing Win
  can still open the native Start surface over the hidden taskbar. Enable both experiments only
  after each is validated independently.
- Sleep/resume can make the watchdog treat the paused heartbeat as a failure and safely restore the
  native taskbar, ending replacement mode. This is intentional fail-safe behavior; it must be
  validated and may need a resume-aware heartbeat later.
- The taskbar window class names and the hide/show method are observed Explorer internals, not a
  Microsoft compatibility contract. Future Windows builds may change them.
- The Windows-key hook is a global low-level hook. It is off by default, only runs while Prism is
  alive, and is removed by Windows on process exit, but it still needs real Windows validation.
- Elevated/UIPI targets and exclusive-fullscreen apps may deny activation or overlay behavior.
- UWP grouping still uses per-window frame identities and generic icons until AUMID extraction.
- No installer, code signing, auto-start, single-instance guard, or distribution license decision.
- Audio, radios, brightness, media, tray replication, and notification interception remain
  unavailable or deferred.

## Recommended next step

1. Run the Phase A preview checklist on Windows 11 at 100% and 200% (single monitor first).
2. On a disposable Windows 11 VM or spare machine, run the Phase B recovery checklist **before**
   enabling taskbar replacement on the owner's machine. Capture taskbar visibility after a forced
   Prism kill and a frozen-heartbeat test.
3. Run the Phase C keyboard checklist with every required shortcut, then with an elevated app and a
   fullscreen app. Keep the fallback shortcut configured.
4. Only after 1–3 pass, consider enabling the experimental gates for daily use.
