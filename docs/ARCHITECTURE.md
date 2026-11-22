# Prism Shell — Architecture

Status: Phase 1 foundation and Phase 2 visual prototype are **Implemented** and **Tested on
Linux**. Windows integration is **Proposed/Unverified**; no Windows shell surface has run on
Windows 11 yet.

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
│  │ layout, clock │   │ Windows backend (future, documented APIs)    │    │
│  └───────────────┘   └───────────────────────────────────────────────┘    │
│                                                                            │
│  Future: prism-windows host (HWND styles, DWM, WinEventHook, Core Audio,  │
│  WinRT radios/media, recovery executable)                                 │
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

### `prism-ui`

- Slint components: taskbar, Control Center, launcher, window previews, toasts, power
  confirmation, developer HUD, static desktop context.
- Central design tokens (`tokens.slint`): colors, typography, radii, spacing, motion.
- Central asset registry (`icons.slint`): embedded SVG icons and app icons.
- `state.slint`: one shared `UiState` global (models, view values, semantic callbacks).
- `model.rs`: deterministic domain-to-view mapping (app items, toggle copy, audio outputs,
  window items, icons, time formatting).
- Implements AccessKit roles/labels on the interactive surfaces. Keyboard focus/navigation is
  partial in this milestone; Escape closes transient surfaces.

The UI does not own Windows integration logic. It can be driven by the mock controller or a future
Windows controller through the same `UiState` callbacks.

### `prism-app`

- `prism-preview` binary: Linux development preview, CLI, mock controller, real-time clock,
  hover/pending/toast timers, screenshot mode.
- `prism-shell` binary: production bootstrap. In this milestone it **fails safely**: on Windows it
  shows a native warning message box and exits; on Linux it prints instructions and exits. It
  never substitutes mock data for production.
- `config`: XDG/AppData path resolution, safe fallback, no overwrite.
- `logging`: stderr + best-effort file logging with a non-blocking guard.
- `args`: manual, tested CLI parser; no heavyweight dependency.
- `preview/controller`: UI-callback-to-action state machine; no OS calls.

### Future `prism-windows`

Not created yet. It will own:

- Window enumeration/filtering and event hooks.
- App identity/AUMID/icon extraction.
- Activation/minimization.
- HWND styling, DWM attributes, non-activating input.
- Core Audio, radios/WLAN/Bluetooth, brightness, GSMTC, battery/network.
- A worker-thread bridge to the controller.

Keeping this crate absent today is intentional: the Windows host must be designed against a real
Windows 11 test environment, not guessed from Linux.

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

### Threading (current and planned)

- Slint UI and all preview state live on the UI thread. The mock is synchronous.
- The preview has a 1 s tick; the clock writes only when the minute changes, and media advances
  only while a session exists. There is no continuous render loop.
- The Windows implementation must put Core Audio/WinRT/device enumeration and other potentially
  slow calls on a worker, marshal results back with `slint::invoke_from_event_loop`, and never
  block the UI thread. That is a contract in `SystemServices`; it is not implemented yet.

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

The planned host uses documented Win32/DWM calls on the HWND obtained from Slint:

- `SetWindowLongPtrW` for `WS_EX_TOOLWINDOW`, `WS_EX_NOACTIVATE`, `WS_EX_TOPMOST` as appropriate.
- `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE)` and `SWP_NOACTIVATE` for show/hide.
- `WM_MOUSEACTIVATE` → `MA_NOACTIVATE` for a taskbar that never steals focus on a status click.
- `DwmSetWindowAttribute` for `DWMWA_WINDOW_CORNER_PREFERENCE`,
  `DWMWA_USE_IMMERSIVE_DARK_MODE`, and optionally `DWMWA_SYSTEMBACKDROP_TYPE` (Windows 11
  22621+). These are documented, but DWM behavior with a transparent Slint surface is
  **Unverified**.
- Do not use `WS_EX_LAYERED` if a normal composed window can work; layered windows have extra
  hit-testing and child-window limitations.

The production host must have an explicit fallback surface if DWM backdrop is unavailable: a
solid translucent panel is the visual baseline.

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
- Production should use PerMonitorV2 awareness and one surface per monitor. A window must not span
  monitors with different scale factors.
- Use `GetMonitorInfoW().rcWork` for work areas, `EnumDisplayMonitors` for enumeration,
  `WM_DPICHANGED` for scale changes, `WM_DISPLAYCHANGE`/`WM_SETTINGCHANGE(SPI_SETWORKAREA)` for
  display/work-area changes. Reconcile on primary changes, disconnect/reconnect, RDP, and resume.
- The preview's DPI matrix uses `--scale` (which sets `SLINT_SCALE_FACTOR` before backend init) and
  Xvfb resolutions. This validates layout/DPI independence, not Windows mixed-DPI geometry.

## 6. Recovery and safety

V1 preview and bootstrap introduce **no** shell modifications:

- No Explorer replacement or termination.
- No system-shell registration.
- No registry changes.
- No taskbar hiding.
- No auto-start.
- No process injection, DLL injection, or Explorer patching.
- No system binary modification.

`prism-shell` on Windows is intentionally a fail-safe diagnostics message box in this milestone.
Any future deeper integration requires:

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
  `%LOCALAPPDATA%\Prism\logs\` (Windows). Do not log window titles, SSIDs, media metadata, or
  secrets.
- The product works offline. There is no telemetry or network dependency.
- Mock data is deterministic and fictional. The preview visibly labels itself development-only.

## 8. Performance

- Slint's dependency-driven property system avoids idle repaints. The preview has one 1 s timer
  for the clock/media and single-shot timers for interaction states.
- Icons/fonts are embedded; no runtime asset I/O.
- Future performance work: measure cold start, warm start, idle CPU/GPU, RSS, first frame,
  Control Center open latency, hover preview cost, and behavior with many windows on Windows 11.
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
| Slint transparent window on Windows/DWM renders black or loses alpha | High | Phase 3 hardware test; Skia/wgpu/software fallback; solid-surface fallback |
| Non-activating taskbar cannot reliably activate targets | High | Click-context `SetForegroundWindow`; check return; `FlashWindowEx`; test elevated/UIPI |
| UWP app identity/grouping is wrong | High | AUMID-first, exe fallback, child PID heuristic; UWP matrix |
| Taskbar hiding is unsafe/undocumented | Critical | Not implemented; independent recovery + explicit approval required |
| Tray/notification interception expectations | High | Defer/block; own icons only; state limitations in UI/docs |
| DDC/CI brightness can misbehave | High | No startup probe; opt-in; hide/disable; settings fallback |
| Mixed-DPI geometry is wrong | Medium | PerMonitorV2, one surface per monitor, physical math, matrix test |
| Slint license/attribution unresolved | Medium | Recorded in DECISIONS.md; resolve before distribution |
