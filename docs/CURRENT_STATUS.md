# Current Status — Prism Shell

Last updated: 2026-11-21. Target: Windows 11 x64. Development host: Linux.

## Status summary

| Milestone | Status | Evidence |
|---|---|---|
| Phase 0 — research/feasibility | **Research complete** | `docs/research/windows-api-notes.md`, `docs/WINDOWS_API_RESEARCH.md`, `docs/UI_FRAMEWORK_EVALUATION.md` |
| Phase 1 — native foundation | **Implemented; Tested on Linux** | workspace, 72 portable tests, config/logging, platform traits |
| Phase 2 — taskbar + Control Center preview | **Implemented; Tested on Linux** | interactive Slint mock preview, responsive/DPI screenshots |
| Phase 3A — native Windows shell host | **Implemented; successfully cross-compiled; native CI build/test passed** | `prism-windows` crate, Win32/DWM host, taskbar/CC/launcher surfaces |
| Phase 3A — real window tracking | **Implemented; successfully cross-compiled; native CI build/test passed** | `EnumWindows`, icon extraction, foreground events, activation |
| Phase 3A — Windows CI compile/test | **Executed on `windows-latest`** | GitHub Actions run 44983452807: fmt, clippy, 72 tests, debug binaries |
| Phase 3A — Windows runtime validation | **Not executed** | no Windows 11 desktop host/VM available |
| Taskbar hiding / Explorer replacement | **Not implemented; blocked** | deliberate safety boundary |
| Real system controls (audio/radios/brightness/media) | **Not implemented** | shown as unavailable or native Settings fallbacks |

The Linux preview is functional. The Windows executable is a real native-limited shell, but its runtime behavior is **unverified** until the checklist in `docs/WINDOWS_VALIDATION.md` is executed on Windows 11.

## Implemented and tested on Linux

- Portable workspace: `prism-core`, `prism-platform`, `prism-ui`, `prism-app`, `prism-windows` portable modules.
- Dock/window model: grouping, pinned/running order, active state, MRU, launch/activate/minimize/restore/close intents, overflow.
- Control state model: available/unsupported/permission-required/unavailable + idle/pending/failed reconciliation.
- Linux mock preview:
  - Floating taskbar, launcher, Control Center, window previews, tooltips, toasts, power confirmation.
  - Laptop/Desktop/Minimal/Faults/ManyApps fixtures.
  - Real local clock and deterministic screenshots.
- Shared UI changes for the native path:
  - Real image slot + generic fallback for application icons.
  - Separate `ShellTaskbar`, `ShellControlCenter`, `ShellLauncher` top-level components.
  - Production footer/labels and Exit-only utility mode; no fake power toggles.
- 72 unit tests pass:
  - `prism-core`: 28
  - `prism-platform`: 9
  - `prism-app`: 13
  - `prism-ui`: 4
  - `prism-windows`: 18 (portable layout, naming, eligibility, grouping)
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets` is clean (no warnings).
- Linux visual regression: 14 automated snapshots at 1920×1080, 1280×800, 125%/150%/200%, plus 4K 200% earlier; all non-empty and dimension-verified.
- Real pointer/keyboard interaction checks on Xvfb (preview backend only).

## Successfully cross-compiled for Windows

Both Windows binaries build with `cargo xwin` for `x86_64-pc-windows-msvc`:

- `target/x86_64-pc-windows-msvc/release/prism-shell.exe` — 14 MB, Windows GUI subsystem.
- `target/x86_64-pc-windows-msvc/release/prism-preview.exe` — 15 MB, console development preview.
- Static MSVC CRT (`+crt-static`); no `VCRUNTIME140.dll`/MSVCP dependency.
- `prism-shell.exe` imports only system DLLs (kernel32, user32, dwmapi, shell32, comctl32, gdi32, etc.).
- Debug cross-build also succeeds; only benign `LNK4099` debug-CRT PDB warnings.
- GitHub Actions run
  [44983452807](https://github.com/Scatter97/prism-shell/actions/runs/44983452807) passed on
  `windows-latest`: formatting, Clippy, 72 tests, and debug executables. This is a native MSVC
  compile/test on a hosted Windows Server runner, not a Windows 11 desktop session.

### Native Windows path implemented

- `prism-windows` crate, compiled only on Windows except for portable layout/filtering modules.
- Real top-level window enumeration with filtering:
  - Own process, invisible, cloaked, shell/desktop classes, tool windows without `WS_EX_APPWINDOW`, owned windows without `WS_EX_APPWINDOW`, zero-size.
  - Minimized and elevated windows retained; elevated/unknown identities fall back to HWND identity instead of disappearing.
- Application grouping by normalized executable path; `ApplicationFrameHost` UWP frames are kept separate per HWND with title-based names and generic icons (AUMID extraction still future).
- Real foreground tracking with `SetWinEventHook` (`EVENT_SYSTEM_FOREGROUND`, object create/name events, minimize events) plus 5-second reconciliation.
- Real application icons from `SHGetFileInfoW` → RGBA → Slint, cached per app; generic fallback on failure.
- Real activation/minimize/restore:
  - `ShowWindow(SW_RESTORE)`, `SetForegroundWindow`, `FlashWindowEx` on denial, `ShowWindow(SW_MINIMIZE)`.
- Pinned apps: real File Explorer, Command Prompt, Windows PowerShell, and Microsoft Edge when found.
- Real launcher with search, pinned/running apps, real icons, launch/activate.
- Native window hosts:
  - Frameless, transparent Slint surfaces.
  - Taskbar: full work-area-width topmost tool window, `WS_EX_NOACTIVATE`, `WM_MOUSEACTIVATE = MA_NOACTIVATE`, per-pixel click-through outside the visible dock (`WM_NCHITTEST`).
  - Control Center/launcher: topmost tool windows with transparent shadow margins, click-through margins, Escape/focus-loss close, real focus for search.
  - DWM dark mode, no-round native corners, no border; Slint draws the rounded material.
  - Positions above the Windows taskbar using the monitor work area.
- Real Control Center actions:
  - Quick tiles and connectivity links open native Windows Settings pages.
  - Real battery state via `GetSystemPowerStatus`.
  - Real workstation lock via `LockWorkStation`.
  - Exit action closes Prism only.
  - Audio/brightness/media remain explicitly unavailable; no fake sliders or metadata.
- No mock data in the Windows production backend.
- No taskbar hiding, Explorer replacement, registry changes, auto-start, or AppBar reservation.

## Actually tested on Windows 11

**No interactive Windows 11 runtime test has been executed.** The binaries have not been run on a
Windows 11 desktop, not even under Wine.

The native shell code has been compiled and its portable tests run on a hosted `windows-latest`
runner (Windows Server). That proves the MSVC build, link, and portable test behavior; it does
**not** exercise the Slint surfaces, DWM, Explorer, focus rules, or real window tracking.

Therefore the following remain **Unverified**:

- Whether the taskbar/CC/launcher windows open at all and render with correct alpha.
- Frameless/transparent appearance and DWM composition.
- Focus/non-activation behavior and whether `SetForegroundWindow` succeeds from the NOACTIVATE taskbar.
- Z-order, click-through strip, Escape/focus-loss behavior.
- Real window enumeration/grouping, icons, launch, activation, minimize/restore.
- DPI/positioning across 100/125/150/200% and monitor changes.
- Explorer coexistence, process death recovery, lock/sleep/resume.
- Performance, memory, GPU, idle behavior.

## Known limitations / risks for the first Windows session

- The dock is an overlay; it does **not** reserve work area. It can cover the bottom of maximized windows. AppBar reservation is deferred until validated.
- Primary monitor only.
- No taskbar hiding or Explorer replacement.
- No window-preview thumbnails, right-click menu, pinning UI, multi-window chooser, or Start menu.
- The dock model is rebuilt on reconciliation; the active window is preserved, but per-window MRU order is approximate without a chooser.
- UWP grouping is per-window/generic-icon until AUMID extraction is added.
- Elevated/protected apps may deny icon/process inspection or activation; generic/flash fallback.
- Pinned apps are discovered defaults, not yet configurable through the Windows config.
- 32 px shell icons may soften at 150/200% scaling.
- Fullscreen apps/games are not detected; the topmost dock may overlay them.
- No single-instance guard.
- No installer/code signing/auto-start/recovery executable.
- Slint license decision remains open before distribution.

## Recommended next step

**Execute `docs/WINDOWS_VALIDATION.md` on a real Windows 11 x64 VM/physical machine, starting at 100% scaling with a single monitor.** Fix the first blocker found, in this order:

1. Does the taskbar window appear and render alpha correctly?
2. Does clicking a dock app activate/minimize the real target?
3. Does control-center/launcher focus and click-through behave?
4. Do enumeration/icons/grouping make sense for common apps?
5. Then validate DPI, multi-monitor, and work-area reservation.

Do not add taskbar hiding or deeper Explorer integration before the overlay/focus/recovery fundamentals are proven.
