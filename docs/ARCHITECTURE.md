# Prism Shell — Architecture

Status: Phase 1 foundation, Phase 2 visual prototype, and Phase 3A native Windows host are
**Implemented**. Phase 1/2 are **Tested on Linux**; the Windows host is **successfully
cross-compiled** for `x86_64-pc-windows-msvc` but has not executed on Windows. All Windows
runtime behavior remains **Unverified**.

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
│  prism-windows: HWND host, DWM, EnumWindows/WinEventHook, shell icons,    │
│  activation, battery/clock. Future: audio/radio/media, recovery tool.     │
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
unavailable rather than simulating them. It has not run on Windows yet.

### `prism-ui`

- Slint components: taskbar, Control Center, launcher, window previews, toasts, power
  confirmation, developer HUD, static desktop context.
- Central design tokens (`tokens.slint`): colors, typography, radii, spacing, motion.
- Central asset registry (`icons.slint`): embedded SVG icons and app icons.
- `state.slint`: the `UiState` global (models, view values, semantic callbacks). Each top-level
  production surface owns its own instance; the native controller writes the taskbar, Control
  Center, and launcher instances separately.
- `shell-taskbar.slint`, `shell-control-center.slint`, `shell-launcher.slint`: production
  top-level windows. They reuse the same taskbar/CC/launcher components and design tokens as the
  preview stage but have transparent backgrounds and no simulated desktop.
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
- `platform`: real launch/activate/minimize, Settings URIs, workstation lock, power status. No mock
  fallback.
- `controller`: native event loop, three Slint surfaces, `SetWinEventHook` invalidation, bounded
  5 s reconciliation, 300 ms timer, layout updates.

Not yet implemented: AUMID extraction for packaged apps, worker-thread device APIs, Core Audio,
radios, brightness, GSMTC, tray/notifications, thumbnails, and an independent recovery executable.

Cross-compilation proves the API surface links. It does not prove HWND style changes, DWM
composition, foreground activation, or enumeration behavior on Windows; see
`docs/WINDOWS_VALIDATION.md`.

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
- The preview is development-only. Every overlay is hidden from hit-testing when closed and
  waits for its close animation before becoming invisible.

### Threading (current)

- Slint UI and the native controller run on the UI thread. Win32 enumeration, icon extraction, and
  power-status reads are synchronous but bounded; they are not yet moved to a worker.
- `SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_OBJECT_CREATE/NAMECHANGE,
  EVENT_SYSTEM_MINIMIZESTART/END)` only sets an atomic dirty flag. A 300 ms Slint timer refreshes
  the model when dirty; a full reconciliation also runs every 5 s. The clock updates only when the
  minute changes. No continuous render loop is used.
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
3. **Launcher** — anchored above the launcher button.
4. **Window previews / tooltips** — short-lived, always-on-top, owned or no-activate surfaces.
5. **Power confirmation** — deliberate destructive-action gate.

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
  `SWP_SHOWWINDOW` and an explicit `SetForegroundWindow` because they need focus.
- The host polls the primary work area and window DPI on each 300 ms tick and repositions after a
  scale/work-area change. `WM_DPICHANGED` is not handled explicitly.

Win32 style changes after winit creates the window, DWM alpha with the FemtoVG/OpenGL renderer, and
foreground permission from a NOACTIVATE taskbar are **unverified** until run on Windows 11.

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

V1 preview and native shell introduce **no** shell modifications:

- No Explorer replacement or termination.
- No system-shell registration.
- No registry changes.
- No taskbar hiding.
- No auto-start.
- No process injection, DLL injection, or Explorer patching.
- No system binary modification.

`prism-shell` on Windows now runs a real overlay shell but still makes **no** shell
modifications. Deeper integration remains future and requires:

1. An independent recovery executable with no dependency on the running shell.
2. A watchdog/restore marker that can restore the taskbar after a crash.
3. Validated hide/show on primary and secondary taskbars, Explorer restart, reboot, and crash.
4. Opt-in only; never automatic; never the default V1 behavior.
5. VM testing before physical hardware.

## 7. Configuration, diagnostics, privacy

- Config path: `$XDG_CONFIG_HOME/prism-shell/config.json` (Linux) or
  `%APPDATA%\Prism\config.json` (Windows), overridable with `PRISM_CONFIG`/`--config`.
- Missing config uses defaults. Corrupt/invalid config falls back safely, logs a warning, and
  shows a non-blocking error toast in the preview. The file is never overwritten.
- Logs go to stderr and `$XDG_STATE_HOME/prism-shell/logs/prism-preview.log` (Linux) or
  `%LOCALAPPDATA%\Prism\logs\prism-shell.log` (Windows native shell). Do not log window titles,
  SSIDs, media metadata, or secrets. The native controller logs window counts and ids, not titles.
- The product works offline. There is no telemetry or network dependency.
- Mock data is deterministic and fictional. The preview visibly labels itself development-only.

## 8. Performance

- Slint's dependency-driven property system avoids idle repaints. The preview has one 1 s timer
  for the clock/media and single-shot timers for interaction states. The native shell has a 300 ms
  timer that only refreshes when an event flag is set or the 5 s reconciliation is due.
- Icons/fonts are embedded; no runtime asset I/O.
- No Windows performance baseline exists. Measure cold start, idle CPU/GPU, RSS, first frame,
  Control Center open latency, and behavior with many windows before making performance claims.
  Window enumeration/icon extraction currently run synchronously on the UI thread.
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
| Taskbar hiding is unsafe/undocumented | Critical | Not implemented; independent recovery + explicit approval required |
| Tray/notification interception expectations | High | Defer/block; own icons only; state limitations in UI/docs |
| DDC/CI brightness can misbehave | High | No startup probe; opt-in; hide/disable; settings fallback |
| Mixed-DPI geometry is wrong | Medium | PerMonitorV2, one surface per monitor, physical math, matrix test |
| Slint license/attribution unresolved | Medium | Recorded in DECISIONS.md; resolve before distribution |
