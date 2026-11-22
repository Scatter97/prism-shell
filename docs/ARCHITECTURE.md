# Prism Shell — Architecture

Status: Phase 1 foundation, Phase 2 visual prototype, Phase 3A native Windows host, Phase A live
window previews, Phase B taskbar recovery/replacement, and Phase C Windows-key integration are
**Implemented** and **successfully cross-compiled**. Phase 1/2 and the portable tests are **Tested
on Linux**. The owner has confirmed the basic Windows 11 taskbar, native icons, click activation,
and coexistence with the original taskbar. The new preview, taskbar-replacement, and Windows-key
paths have **not** been runtime-validated on Windows 11; taskbar replacement is disabled by default
and blocked for enablement until its independent recovery mechanism is tested on real Windows.

The design goal is a native, low-footprint Windows 11 shell with one coherent visual system. The
architecture keeps presentation, application state, platform integration, and recovery separate so
the Windows host can be validated without rewriting the UI.

## 1. System context

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Windows 11 (production) or Linux preview                                  │
│                                                                            │
│  ┌──────────────────────────────┐       ┌──────────────────────────────┐   │
│  │ prism-app                    │       │ prism-ui                     │   │
│  │ lifecycle, CLI, config,      │──────▶│ Slint taskbar / Control      │   │
│  │ logging, controller          │◀──────│ Center / launcher / preview  │   │
│  └───────────────┬──────────────┘       └──────────────────────────────┘   │
│                  │                                                         │
│        ┌─────────┴──────────┐                                             │
│        ▼                    ▼                                             │
│  ┌───────────────┐   ┌───────────────────────────────────────────────┐    │
│  │ prism-core    │   │ prism-platform                                │    │
│  │ dock model,   │   │ SystemServices / WindowServices traits        │    │
│  │ system state, │   │ MockPlatform (preview)                       │    │
│  │ layout, clock │   │ WindowsPlatform (prism-windows, real Win32)  │    │
│  └───────────────┘   └───────────────────────────────────────────────┘    │
│                                                                            │
│  prism-windows: HWND host, DWM thumbnails, EnumWindows/WinEventHook,      │
│  icons, activation, battery/clock, keyboard hook, taskbar recovery.       │
└────────────────────────────────────────────────────────────────────────────┘
```

No Slint callback calls an operating-system API directly. The preview controller translates UI
callbacks into semantic actions, executes them through a platform service, and pushes an
authoritative snapshot back into the view.

## 2. Crates

### `prism-core` (portable, no I/O, `unsafe_code = forbid`)

- `ids`: `AppId`, `WindowId`, `IconKey`.
- `dock`: pinned/running application model, window grouping, MRU ordering, click intents,
  activation/minimize/close reconciliation.
- `system`: capability and control-phase model, audio/brightness/connectivity/battery/media state,
  semantic system actions, Settings URIs.
- `layout`: DPI-independent dock capacity/overflow arithmetic.
- `clock`: deterministic clock/date formatting.
- `config`: versioned, validated `PrismConfig` with safe defaults.

The crate deliberately does not know about Slint, Win32, channels, or threads. It is the shared
contract for both the preview and the future Windows backend.

### `prism-platform`

- `SystemServices`: snapshot, execute a semantic action, drain asynchronous events.
- `WindowServices`: launch, activate, minimize, close a window.
- `PlatformError`: Unsupported / PermissionRequired / Unavailable / Failed.
- `ActionOutcome`: Applied / Pending / Simulated.
- `MockPlatform`: explicit Linux development backend with deterministic scenarios
  (Laptop, Desktop, Minimal, Faults, ManyApps), pending-toggle reconciliation, media tick, and
  deliberate failure injection.

`MockPlatform` never pretends to be a real Windows operation. Its UI says "preview only" and
"no system changes".

`WindowsPlatform` lives in `prism-windows` (not `prism-platform`) because it needs the Win32 host:
`ShellExecuteExW` launch, `ShowWindow`/`SetForegroundWindow` activation, `LockWorkStation`, native
Settings URIs, and `GetSystemPowerStatus`. It reports audio, radios, brightness, and media as
unavailable rather than simulating them. The owner has confirmed basic dock launch/activation;
individual Control Center actions still need the validation checklist.

### `prism-ui`

- Slint components: taskbar, Control Center, launcher, window previews, toasts, power
  confirmation, developer HUD, static desktop context.
- Central design tokens (`tokens.slint`): colors, typography, radii, spacing, motion.
- Central asset registry (`icons.slint`): embedded SVG icons and app icons.
- `state.slint`: the `UiState` global (models, view values, semantic callbacks). Each top-level
  production surface owns its own instance; the native controller writes the taskbar, Control
  Center, and launcher instances separately.
- `shell-taskbar.slint`, `shell-control-center.slint`, `shell-launcher.slint`,
  `shell-window-preview.slint`: production top-level windows. They reuse the same taskbar/CC/
  launcher/preview components and design tokens as the preview stage but have transparent
  backgrounds and no simulated desktop.
- Window cards carry a real icon slot and a `preview-live-thumbnails` mode. In production the
  designed miniature is hidden and DWM draws live content over a neutral inset; the mock preview
  keeps the designed miniature.
- `model.rs`: deterministic domain-to-view mapping (app items, toggle copy, audio outputs,
  window items, icons, time formatting). `AppItem.has-icon`/`icon` carry real Windows icons;
  the SVGs are only the fallback.
- Implements AccessKit roles/labels on the interactive surfaces. Keyboard focus/navigation is
  partial in this milestone; Escape closes transient surfaces.

The UI does not own Windows integration logic. It can be driven by the mock controller or a future
Windows controller through the same `UiState` callbacks.

### `prism-app`

- `prism-preview` binary: Linux development preview, CLI, mock controller, real-time clock,
  hover/pending/toast timers, screenshot mode. Still mock-only.
- `prism-shell` binary: Windows production entry point. On Windows it loads config, starts the
  native shell through `prism-windows`, and shows a native error box if startup fails. On Linux it
  prints instructions and exits. It never substitutes mock data for production.
- `config`: XDG/AppData path resolution, safe fallback, no overwrite.
- `logging`: stderr + best-effort file logging with a non-blocking guard.
- `args`: manual, tested CLI parser; no heavyweight dependency.
- `preview/controller`: UI-callback-to-action state machine; no OS calls.

### `prism-windows`

Created in Phase 3A. Portable modules (`layout`, `model`, `naming`) compile and unit-test on Linux;
the Win32 modules are `#[cfg(windows)]`:

- `host`: HWND styles (`WS_EX_TOOLWINDOW`, `WS_EX_TOPMOST`, taskbar `WS_EX_NOACTIVATE`), DWM
  corner/dark/border attributes, `WM_NCHITTEST` click-through, `WM_MOUSEACTIVATE`, work-area
  positioning, panel subclassing. HWND attachment is deferred to the first event-loop tick because
  winit creates the native window asynchronously; startup times out after 5 s with a visible error.
- `window`: `EnumWindows` filtering (own process, invisible, cloaked, shell classes, tool/owned
  windows, zero-size), process path/identity, icon source, minimized/foreground facts.
- `icons`: `SHGetFileInfoW` → `HICON` → RGBA → `slint::Image`, cached per app.
- `preview`: DWM live thumbnails registered against the preview Slint HWND. Source-size query,
  aspect-ratio crop, destination-rect updates, and unregister-on-drop/close. Minimized and invalid
  HWNDs are skipped. Thumbnails are registered after the panel fade.
- `platform`: real launch/activate/minimize, Settings URIs, workstation lock, power status. No mock
  fallback.
- `taskbar_replacement`: opt-in hide/show of `Shell_TrayWnd`/`Shell_SecondaryTrayWnd`, marker file,
  heartbeat, watchdog spawn, manual restore, and `prism-recovery.exe` CLI. Disabled by default.
- `keyboard`: portable standalone-Windows-key state machine plus fallback-hotkey parser. The Win32
  hook injects a synthetic Windows key-down only when a combination is detected; standalone
  key-down/up are swallowed and toggle the launcher.
- `controller`: native event loop, four Slint surfaces, `SetWinEventHook` invalidation, preview
  hover generations and native mouse-leave tracking, bounded 5 s reconciliation, 300 ms timer,
  opt-in 50 ms shortcut timer, layout updates.

Not yet implemented: AUMID extraction for packaged apps, worker-thread device APIs, Core Audio,
radios, brightness, GSMTC, tray/notifications, AppBar work-area reservation, and multi-monitor dock
instances.

Cross-compilation proves the API surface links. It does not prove HWND style changes, DWM
composition, thumbnail hosting, foreground activation, recovery timing, or keyboard-hook behavior
on Windows; see `docs/WINDOWS_VALIDATION.md`.

## 3. Data and control flow

### Preview action flow

```
Slint input
  → UiState callback (semantic id/value)
  → PreviewController
      → MockPlatform (SystemServices/WindowServices)
      → DockState
  → authoritative snapshot + dock model
  → Slint models/properties
  → dependency-driven redraw
```

Key rules:

- A click is not proof of success. Pending operations keep the previous authoritative state and
  display a pending phase. On completion the tile is rebuilt from the returned snapshot.
- On failure, the mock does not apply the optimistic change. Fault fixture Bluetooth remains on
  after a rejected "turn off", and the tile shows a warning plus the error.
- Volume/brightness sliders write the raw value while dragging and reconcile from the platform on
  error. The mock clamps to 0–100.
- Hover previews are opened by a single-shot timer after pointer entry, aligned to the icon's
  known slot. Clicking an app suppresses re-opening for 650 ms so a minimize is not immediately
  undone by the hover timer.
- Every hover/scheduled open carries a generation. Rapid target changes invalidate older timers,
  and close timers are generation-checked. The Linux mock and Windows backend share the same
  semantic callbacks; only the Windows controller registers DWM thumbnails.
- The Linux mock preview's thumbnails are designed placeholders. On Windows, the Slint preview
  surface is the DWM destination: the controller maps each card to a physical destination rect,
  registers one thumbnail per visible non-minimized HWND, and unregisters on close/window loss.
  DWM draws above Slint content, so close buttons sit outside the thumbnail rectangles.
- The preview stays open while the pointer travels from the dock into the panel. On Windows a
  native `WM_MOUSEMOVE`/`WM_MOUSELEAVE` flag is authoritative because card TouchAreas are above
  the root hover area.
- Every overlay is hidden from hit-testing when closed and waits for its close animation before
  becoming invisible.

### Threading (current)

- Slint UI and the native controller run on the UI thread. Win32 enumeration, icon extraction, and
  power-status reads are synchronous but bounded; they are not yet moved to a worker.
- `SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_OBJECT_CREATE/NAMECHANGE,
  EVENT_SYSTEM_MINIMIZESTART/END)` only sets an atomic dirty flag. A 300 ms Slint timer refreshes
  the model when dirty; a full reconciliation also runs every 5 s. The clock updates only when the
  minute changes. No continuous render loop is used.
- The low-level keyboard hook is installed on the UI thread and only performs a tiny state-machine
  update plus, at most, one `SendInput` replay. A 50 ms timer polls the launcher flag only while
  the opt-in shortcut integration is enabled.
- The taskbar watchdog is a separate process. During replacement mode the UI thread pulses a named
  auto-reset event on each 300 ms tick; the watchdog waits on that event and restores after 4 s of
  silence. No recovery path depends on the Prism message loop.
- Future Core Audio/WinRT/device calls must move to a worker and marshal results back with
  `slint::invoke_from_event_loop`. That is a contract in `SystemServices`, not yet implemented for
  those subsystems.

## 4. Window model

### V1 preview

The preview is one borderless stage with a simulated desktop behind the shell surfaces. This is
for visual iteration and deterministic screenshots. It is labeled "DEVELOPMENT PREVIEW · MOCK
DATA". It is **not** the production window model.

### Production target

Separate top-level surfaces:

1. **Taskbar** — always visible, bottom-centered, non-activating, no taskbar entry.
2. **Control Center** — anchored above the right status region, dismisses on outside click/Escape.
3. **Launcher** — anchored above the launcher button, takes focus for search.
4. **Window previews** — separate short-lived topmost non-activating window; the DWM destination
   for live thumbnails. It is pre-created and click-through outside its visible panel.
5. **Power confirmation** — deliberate destructive-action gate (preview/mock path).

The Phase 3A host implements these as documented Win32/DWM calls on the HWND obtained from Slint:

- The taskbar is one full-work-area-width top-level window (100 logical px tall) whose visible dock
  is inset at the bottom center. `WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE` keep it out
  of Alt+Tab and avoid focus theft. `WM_MOUSEACTIVATE` returns `MA_NOACTIVATE`; `WM_NCHITTEST`
  returns `HTTRANSPARENT` outside the dock rectangle so the transparent strip does not block the
  desktop or underlying windows.
- Control Center and launcher are separate topmost tool windows with 24 logical px transparent
  shadow margins. Their `WM_NCHITTEST` is click-through outside the panel rectangle; they take
  real focus for search/keyboard use, close on `WM_ACTIVATE` inactive or Escape, and hide after a
  short Slint close animation.
- `SetWindowLongPtrW` removes native caption/thick-frame/sysmenu styles. `DwmSetWindowAttribute`
  sets `DWMWCP_DONOTROUND`, `DWMWA_USE_IMMERSIVE_DARK_MODE`, and `DWMWA_BORDER_COLOR =
  DWMWA_COLOR_NONE`; Slint draws the rounded material. `WS_EX_LAYERED` is not used.
- `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE|SWP_FRAMECHANGED)` positions the taskbar; panels use
  `SWP_SHOWWINDOW` and an explicit `SetForegroundWindow` because they need focus. The preview uses
  the no-activate path and is pre-created so its first hover does not create a focus-stealing HWND.
- The host polls the primary work area and window DPI on each 300 ms tick and repositions after a
  scale/work-area change. `WM_DPICHANGED` is not handled explicitly.
- Experimental taskbar replacement uses the full primary-monitor rectangle instead of `rcWork`,
  because hiding `Shell_TrayWnd` does not change the Windows work area. The dock keeps a 14 logical
  pixel bottom gap.

The owner has confirmed the basic taskbar rendering, icon extraction, click activation, and
coexistence with the original taskbar. DWM live-thumbnail composition on the transparent preview
destination, preview focus handoff, and the full foreground-permission matrix (elevated/UIPI,
fullscreen) remain **unverified**.

The production host keeps a solid translucent panel as the visual baseline; no DWM backdrop is
required for readability.

### Input/focus

- The taskbar should be non-activating for status/toggle clicks. Selecting an app is a user input
  event, so `SetForegroundWindow` is normally permitted by Windows foreground rules; the call's
  return value must be checked and a `FlashWindowEx` fallback used. Elevated/UIPI targets may
  deny activation; show a limited state, never loop or force focus.
- Multi-window apps: the preview/chooser is the primary selection surface; minimizing all windows
  on an ambiguous click is not acceptable. The current prototype minimizes the active window and
  leaves other windows; this is documented in `DockState::click_intent`.

## 5. DPI and multi-monitor

- Slint layout is logical. The host converts to physical screen coordinates using the window's
  scale factor and `GetDpiForWindow`.
- Phase 3A positions the taskbar and panels on the **primary display only**, using
  `GetMonitorInfoW().rcWork`. It polls DPI/work-area changes on the 300 ms tick. Per-monitor
  surfaces and explicit `WM_DPICHANGED` handling are not implemented.
- A window must not span monitors with different scale factors. The current primary-only design
  avoids that, but mixed-DPI behavior is unverified.
- Future work: `EnumDisplayMonitors`/`WM_DISPLAYCHANGE`/`WM_SETTINGCHANGE(SPI_SETWORKAREA)`,
  per-monitor dock instances, primary-change reconciliation, RDP, hotplug, and resume.
- The preview's DPI matrix uses `--scale` (which sets `SLINT_SCALE_FACTOR` before backend init) and
  Xvfb resolutions. This validates layout/DPI independence, not Windows mixed-DPI geometry.

## 6. Recovery and safety

Default behavior still makes **no** shell modifications:

- No Explorer replacement or termination.
- No system-shell registration.
- No registry changes.
- No taskbar hiding.
- No auto-start.
- No process injection, DLL injection, or Explorer patching.
- No system binary modification.

The new experimental taskbar-replacement path is opt-in and gated by an independent recovery tool:

1. `prism-recovery.exe` is a separate process. It opens the shell's named heartbeat event and is
   not linked to the shell's UI, event loop, or main thread.
2. The shell writes a marker before hiding, recording every native taskbar HWND and its original
   visibility. The watchdog does nothing without this marker.
3. If Prism exits, is force-terminated, or stops pulsing the heartbeat for 4 seconds, the watchdog
   calls `ShowWindow(SW_SHOW)` on the recorded primary/secondary taskbars. If Explorer recreated
   the taskbar with new HWNDs, it shows the current primary and, when needed, all current
   secondaries.
4. A clean exit, the Control Center **Restore bar** action, and a startup stale-marker check all
   restore the taskbar through the same path. `prism-recovery.exe --restore` is an emergency
   manual action that does not require Prism.
5. The shell leaves replacement mode voluntarily if Explorer's taskbar becomes visible again; it
   never re-hides in a loop.
6. The feature is off by default and **must not be enabled** until the recovery gate in
   `docs/WINDOWS_VALIDATION.md` passes on real Windows 11.

Remaining prerequisites before calling replacement production-ready:

- Recovery tested on primary and secondary taskbars, Explorer restart, reboot, forced kill, and a
  frozen heartbeat.
- Work-area/tray/fullscreen behavior documented and accepted.
- A decision on whether to keep the undocumented class-name approach or abandon it if a Windows
  update changes behavior.

## 7. Configuration, diagnostics, privacy

- Config path: `$XDG_CONFIG_HOME/prism-shell/config.json` (Linux) or
  `%APPDATA%\Prism\config.json` (Windows), overridable with `PRISM_CONFIG`/`--config`.
- Missing config uses defaults. Corrupt/invalid config falls back safely, logs a warning, and
  shows a non-blocking error toast in the preview. The file is never overwritten.
- Logs go to stderr and `$XDG_STATE_HOME/prism-shell/logs/prism-preview.log` (Linux) or
  `%LOCALAPPDATA%\Prism\logs\prism-shell.log` (Windows native shell). The recovery watchdog appends
  to `%LOCALAPPDATA%\Prism\logs\prism-recovery.log`. Do not log window titles, SSIDs, media
  metadata, or secrets. The native controller logs window counts and IDs, not titles.
- The experimental taskbar marker lives at
  `%LOCALAPPDATA%\Prism\taskbar-replacement.state`. It contains only taskbar HWNDs and original
  visibility, never window titles or application data.
- The product works offline. There is no telemetry or network dependency.
- Mock data is deterministic and fictional. The preview visibly labels itself development-only.

## 8. Performance

- Slint's dependency-driven property system avoids idle repaints. The preview has one 1 s timer
  for the clock/media and single-shot timers for interaction states. The native shell has a 300 ms
  timer that only refreshes when an event flag is set or the 5 s reconciliation is due.
- The opt-in shortcut timer runs at 50 ms only when Windows-key integration is enabled. The
  taskbar heartbeat is one `SetEvent` per 300 ms tick while replacement mode is active. DWM
  thumbnails are registered once per preview and only updated when geometry changes.
- Icons/fonts are embedded; no runtime asset I/O.
- No Windows performance baseline exists. Measure cold start, idle CPU/GPU, RSS, first frame,
  Control Center open latency, preview hover/open latency, and behavior with many windows before
  making performance claims. Window enumeration/icon extraction currently run synchronously on the
  UI thread.
- The `release` profile uses thin LTO, one codegen unit, symbol stripping, and abort-on-panic.
  These are not performance measurements.

## 9. Extensibility

- Design tokens are centralized; no component invents one-off colors/radii/motion.
- The core/platform boundary permits a Windows backend, a different renderer, or a future theme
  engine without rewriting the domain model.
- V1 intentionally has no theme editor, plugin system, marketplace, or scripting runtime. The
  structured `AppItem`/`ToggleItem`/`UiState` boundary is the extension seam, not a premature
  plugin API.

## 10. Architecture risks

| Risk | Impact | Mitigation |
|---|---|---|
| Slint transparent window on Windows/DWM renders black or loses alpha | High | Phase 3A host implemented; real Windows 11 test required before claiming success; Skia/wgpu/software and solid-surface fallback |
| Non-activating taskbar cannot reliably activate targets | High | Click-context `SetForegroundWindow`; check return; `FlashWindowEx`; test elevated/UIPI |
| Win32 style changes after winit creation are ignored or reset | High | Reapply styles on layout and on panel creation; verify extended styles at runtime |
| Polling misses fast window changes or costs idle CPU | Medium | WinEventHook dirty flag + 5 s reconciliation; measure idle CPU on Windows |
| UWP app identity/grouping is wrong | High | AUMID-first, exe fallback, child PID heuristic; UWP matrix |
| DWM thumbnails fail on the transparent Slint destination | High | Neutral placeholder fallback; test on Windows 11 with the actual renderer |
| Preview hover handoff/activation is unreliable | Medium | Native mouse tracking, generation-guarded timers, click activates the exact HWND |
| Taskbar hiding is unsafe/undocumented | Critical | Opt-in only; independent watchdog + marker; disabled until real Windows recovery passes |
| Watchdog misses a frozen shell | Critical | 4 s heartbeat timeout; forced-kill and suspend tests are mandatory before enablement |
| Windows-key hook breaks a native shortcut | High | Swallow only standalone taps; synthetic replay for combinations; fallback hotkey; full matrix test |
| Tray/notification interception expectations | High | Defer/block; own icons only; state limitations in UI/docs |
| DDC/CI brightness can misbehave | High | No startup probe; opt-in; hide/disable; settings fallback |
| Mixed-DPI geometry is wrong | Medium | PerMonitorV2, one surface per monitor, physical math, matrix test |
| Slint license/attribution unresolved | Medium | Recorded in DECISIONS.md; resolve before distribution |
