# Current Status — Prism Shell

Last updated: 2026-11-21 (late evening). Target: Windows 11 x64. Development host: Linux.

## Status summary

| Milestone | Status | Evidence |
|---|---|---|
| Phase 0 — research/feasibility | **Approved direction; research complete** | `docs/research/windows-api-notes.md`, `docs/WINDOWS_API_RESEARCH.md`, `docs/UI_FRAMEWORK_EVALUATION.md`, `docs/DECISIONS.md` |
| Phase 1 — native foundation | **Implemented; Tested on Linux** | workspace builds, 53 tests, config/logging, platform traits, preview, fail-safe Windows bootstrap |
| Phase 2 — taskbar + Control Center prototype | **Implemented; Tested on Linux** | interactive Slint taskbar, Control Center, launcher, previews, toasts, power confirmation, responsive/DPI screenshots |
| Phase 3 — native Windows window integration | **Not started / Unverified** | no Win32 enumeration, activation, HWND styling, or real window previews yet |
| Windows compile path | **Implemented; cross-build succeeded** | `prism-shell.exe` + `prism-preview.exe` PE32+ x86-64 via cargo-xwin MSVC |
| Windows runtime validation | **Not tested** | must be run on real Windows 11 |
| Taskbar coexistence/hiding | **Deferred/Blocked** | no hiding or Explorer changes; independent recovery required first |
| Phase 4+ real system integrations | **Deferred** | audio/Wi-Fi/BT/brightness/media/tray/notifications are mock-only in this milestone |

The application is **not** a functional Windows shell yet. It is an evidence-based foundation plus
a real, interactive Linux preview of the intended V1 interface.

## What is implemented

### Foundation

- Cargo workspace with clean crate boundaries:
  - `prism-core` — dock model, system capability/state model, layout/overflow, clock, config.
  - `prism-platform` — `SystemServices`/`WindowServices` traits and an explicit mock backend.
  - `prism-ui` — Slint UI, design tokens, embedded assets, view mapping.
  - `prism-app` — preview/production binaries, CLI, config, logging, preview controller.
- Deterministic mock scenarios: Laptop, Desktop, Minimal, Faults, Many apps (29 apps).
- Error handling:
  - Capability vs runtime phase is explicit; unsupported/permission/unavailable are distinct.
  - Pending operations reconcile; failures do not apply optimistic state.
  - Corrupt configuration falls back safely and never rewrites the file.
- Logging: stderr + best-effort file log with non-blocking guard; no secrets/titles/SSIDs.
- Fail-safe Windows bootstrap: native warning message box; no shell changes; no mock data passed
  off as real system state.
- Linux preview includes a visible “DEVELOPMENT PREVIEW · MOCK DATA” identity and preview-only
  messaging on settings/power/audio actions.

### Taskbar/dock

- Bottom-centered floating surface with rounded corners, glass-like alpha surface, controlled
  border, and elevation shadow.
- Launcher button with original Prism mark.
- Application model: pinned, running, active, multiple windows, running/active indicators.
- Click intents: launch, activate, minimize active, restore minimized, MRU selection.
- Hover feedback, scale-on-hover icon, tooltip for pinned non-running apps, aligned window
  previews for running apps.
- Window preview cards with mock thumbnails, titles, close action, minimized state.
- Overflow chip with deterministic capacity calculation (verified at 1280 with 29 apps).
- Status cluster: volume/mute, Wi-Fi, battery (only when present), real local clock/date,
  Control Center trigger.
- Keyboard Escape closes transient surfaces; search field focuses when the launcher opens.

### Control Center

- Anchored above the right side of the dock, matching material/radius/border/shadow.
- Two-column quick tiles for Wi-Fi, Bluetooth, Focus, Night light with capability, pending, and
  failed states; unsupported/permission states are visually muted and explain the fallback.
- Volume slider + mute + expandable mock output list; external volume changes are modeled by the
  snapshot contract.
- Brightness slider only when supported; honest “Unsupported” + Display settings fallback.
- Connectivity card with Wi-Fi/Bluetooth/Ethernet status and Settings entry points.
- Battery card hidden on systems without a battery.
- Now-playing card with generated cover, title/artist/album, progress/times, previous/play/next;
  empty and error states.
- Pinned utility bar: Settings, Lock, Sleep, Power. Power opens a deliberate confirmation sheet
  with Restart/Shut down; no action is taken in the preview.
- Constrained-height scrolling with a visible scrollbar; utility bar stays reachable.
- Footer: “Preview backend · no system changes.”

### Preview/testing workflow

- Rootless Xvfb workflow with `scripts/dev/virtual-display.sh`.
- ffmpeg full-display screenshots + Slint `take_snapshot` UI-only PNGs.
- `--scale` forces `SLINT_SCALE_FACTOR` before backend init for reproducible DPI captures.
- Real interaction validation with portable xdotool on Xvfb.

## Verification executed

- `cargo fmt --all -- --check`: passed.
- `cargo test --workspace`: **53 passed, 0 failed**.
  - core 27, platform 9, app 13, UI 4.
- `cargo clippy --workspace --all-targets`: no warnings/errors after cleanup.
- `cargo xwin build --target x86_64-pc-windows-msvc -p prism-app --bins`: **succeeded**.
  - `prism-shell.exe`: PE32+ x86-64, debug.
  - `prism-preview.exe`: PE32+ x86-64, debug.
- Linux visual captures: 1920×1080, 1280×800, 2560×1440 at 125%/150% scale, 1920×1080 at 200%
  stress scale; screenshots under `artifacts/screenshots/`.
- `scripts/test/ui-snapshots.sh`: 14 automated UI-only captures at 100%, 125%, 150%, 200% and
  1280×800; PNG dimensions verified (1920×1080 physical for scale checks, 1280×800 for the small
  canvas).
- Interactions verified: CC open/close (Escape/outside click), Wi-Fi pending→off,
  volume drag, audio picker, launcher filter + app launch, tooltip, hover preview, window
  activate/close, minimize without preview re-open, power confirmation, Faults Bluetooth error.
- Full interaction details and artifact names are in `docs/TESTING.md`.

## Windows build details

- Rust target: `x86_64-pc-windows-msvc`.
- Tool: `cargo-xwin 0.23.1`.
- The build downloaded the MSVC CRT/Windows SDK and compiled all pure-Rust dependencies,
  including Slint/FemtoVG/winit/AccessKit.
- **Not run:** the `.exe` files have not been executed on Windows, not even under Wine. No claim
  about DWM transparency, taskbar behavior, focus, audio, radios, or media is established by the
  cross-build.

## Known limitations

- Production Windows integration is absent. No HWND host, window tracking, app icons, launching,
  activation, or real previews.
- The preview stage is one window with a static mock desktop. Production needs separate
  non-activating taskbar/CC/launcher/preview surfaces; that architecture is planned but not
  validated.
- DWM backdrop/blur is not implemented; preview translucency is alpha compositing only.
- No real audio, Wi-Fi/Bluetooth, brightness, media, battery/network, settings, lock, sleep,
  restart, or shutdown operations.
- No tray replication, notification interception, Start menu replacement, or custom app icons
  from real processes.
- No taskbar hiding and no AppBar work-area reservation.
- No auto-start, installer, update mechanism, recovery executable, or code signing.
- No real multi-monitor surface placement; only logical-size/DPI layout tests.
- Keyboard navigation is partial: Escape and launcher search work; full Tab/focus traversal,
  Narrator verification, and high-contrast mode are not complete.
- No performance measurements on Windows. The Linux preview was rendered with software llvmpipe
  under Xvfb; that is not a performance benchmark.
- Slint license choice (GPLv3 vs royalty-free attribution) requires an owner decision before
  distribution.

## Risks requiring Windows validation

1. **Transparent/frameless Slint window with DWM** — alpha, shadow, corners, renderer/driver
   behavior, especially NVIDIA OpenGL.
2. **Non-activating taskbar + app activation** — `WS_EX_NOACTIVATE`/`WM_MOUSEACTIVATE` and
   foreground activation restrictions; UIPI/elevated targets.
3. **Window enumeration/identity/UWP grouping** — AUMIDs, frame/child PIDs, cloaked windows,
   owned/tool windows, icon extraction.
4. **Taskbar coexistence** — maximized/fullscreen overlap, multi-monitor, Explorer restart.
5. **Core Audio/GSMTC/Radios unpackaged capability** — consent/policy and default-device limits.
6. **Brightness DDC/CI safety** — monitor support and kernel-risk mitigation.
7. **Mixed-DPI/multi-monitor geometry** — physical/logical conversion, WM_DPICHANGED, hotplug.
8. **Recovery** — only relevant if taskbar hiding is ever attempted; currently blocked.

## Recommended next development phase

**Phase 3A — Windows transparent host spike** (before full integration):

1. Build a tiny `prism-windows` host that creates one frameless, transparent, always-on-top Slint
   window via raw HWND; validate DWM corners/backdrop, alpha, z-order, non-activation, and focus
   on real Windows 11 at 100/125/150/200%.
2. Test FemtoVG first; have a Skia/wgpu/software fallback plan.
3. Only after the window host is proven, implement enumeration → identity → activation with a
   bounded event queue and reconciliation.
4. Add a native Windows CI job for build/test/fmt/clippy; keep runtime validation on a dedicated
   VM/physical machine.

Do not start taskbar hiding or AppBar reservation until the overlay host, activation, and an
independent recovery tool are proven.
