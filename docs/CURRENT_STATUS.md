# Current Status — Prism Shell

Last updated: 2026-11-22. Target: Windows 11 x64. Development host: Linux.

## Status summary

| Milestone | Status | Evidence |
|---|---|---|
| Phase 0 — research/feasibility | **Research complete** | `docs/research/windows-api-notes.md`, `docs/WINDOWS_API_RESEARCH.md`, `docs/UI_FRAMEWORK_EVALUATION.md` |
| Phase 1 — native foundation | **Implemented; Tested on Linux** | workspace, 72 portable tests, config/logging, platform traits |
| Phase 2 — taskbar + Control Center preview | **Implemented; Tested on Linux** | interactive Slint mock preview, responsive/DPI screenshots |
| Phase 3A — native Windows shell host | **Implemented; successfully cross-compiled; native CI build/test passed** | `prism-windows` crate, Win32/DWM host, taskbar/CC/launcher surfaces |
| Phase 3A — real window tracking | **Implemented; successfully cross-compiled; native CI build/test passed** | `EnumWindows`, icon extraction, foreground events, activation |
| Phase 3A — Windows CI compile/test | **Executed on `windows-latest`** | Runs 44983452807 and 44985985957: fmt, clippy, 72 tests, debug binaries |
| Phase 3A — first Windows 11 launch | **Failed at startup; fix pending re-test** | `window handle unavailable` before any surface appeared; fix `3936efd` |
| Phase 3A — Windows runtime validation | **Attempted once; not passed** | startup only; no window, rendering, focus, or activation test completed |
| Taskbar hiding / Explorer replacement | **Not implemented; blocked** | deliberate safety boundary |
| Real system controls (audio/radios/brightness/media) | **Not implemented** | shown as unavailable or native Settings fallbacks |

The Linux preview is functional. The Windows executable is a real native-limited shell. It has
been launched once on Windows 11, where it failed before any surface appeared because the HWND was
requested before winit created it. Commit `3936efd` defers HWND attachment to the first event-loop
tick with a 5-second timeout. That fix is cross-compiled, unit-tested on Linux, and passed native
`windows-latest` CI build/tests, but has **not** been re-tested on Windows; all Windows runtime
behavior remains **Unverified**.

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
  `windows-latest`: formatting, Clippy, 72 tests, and debug executables.
- GitHub Actions run
  [44985985957](https://github.com/Scatter97/prism-shell/actions/runs/44985985957) passed both
  jobs for the HWND fix (`3936efd`): Linux 15m14s, Windows 19m42s, including format, Clippy, 72
  tests, and debug executables.
- These are native MSVC compile/test runs on hosted Windows Server runners, not Windows 11 desktop
  sessions.

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
  - HWND attachment waits for the first event-loop tick (winit creates windows asynchronously)
    and fails visibly after a 5 s startup timeout.
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

**One startup attempt was made on a Windows 11 x64 machine using a source build. It failed before
any Prism surface appeared.**

- Error: `window handle unavailable: the underlying handle cannot be represented using the types
  in this crate`.
- Cause: Slint/winit creates the native Win32 window asynchronously, after the event loop starts.
  The old startup path called `window_handle()` immediately after `show()`, when no HWND existed.
- Fix: commit `3936efd` resolves and configures the taskbar HWND on the first event-loop tick,
  attaches Control Center/launcher HWNDs when each window becomes available, and exits with a
  visible startup error after a 5-second timeout.
- Status: the fix is cross-compiled for `x86_64-pc-windows-msvc`, passed portable tests on Linux,
  and passed native `windows-latest` CI build/tests (run 44985985957). It has **not** been re-run
  on a Windows 11 desktop.

No successful interactive Windows 11 runtime test has been executed, and the binaries have not run
under Wine. The `windows-latest` CI runner (Windows Server) proves the MSVC build, link, and
portable test behavior; it does **not** exercise the Slint surfaces, DWM, Explorer, focus rules, or
real window tracking.

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

**Rebuild from commit `3936efd` or newer, then execute `docs/WINDOWS_VALIDATION.md` on a real
Windows 11 x64 machine, starting at 100% scaling with a single monitor.** The first check is
whether the deferred-HWND startup fix lets the taskbar window appear at all. Then fix the first
blocker found, in this order:

1. Does the taskbar window appear and render alpha correctly?
2. Does clicking a dock app activate/minimize the real target?
3. Does control-center/launcher focus and click-through behave?
4. Do enumeration/icons/grouping make sense for common apps?
5. Then validate DPI, multi-monitor, and work-area reservation.

Do not add taskbar hiding or deeper Explorer integration before the overlay/focus/recovery fundamentals are proven.
