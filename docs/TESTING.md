# Testing — Prism Shell

Status: portable unit tests, Linux visual tests, virtual-display interaction tests, and a Windows
MSVC cross-build were executed. Native Windows runtime integration was **not** executed and must
not be reported as passing.

Last update: 2026-11-21.

## 1. Test layers

| Layer | What it protects | Where |
|---|---|---|
| Unit/domain | Grouping, click intents, window lifecycle, overflow math, config validation, clock rollover, capability/phase states | `prism-core` tests |
| Mock platform | Deterministic snapshots, pending/error reconciliation, media transitions, scenario integrity | `prism-platform` tests |
| View mapping | Icon/copy mapping, launcher filtering, durations, audio default | `prism-ui/src/model.rs` tests |
| App support | CLI parsing, config fallback, screenshot path validation, toggle-id parsing | `prism-app` tests |
| Visual | Layout, typography, spacing, overflow, DPI behavior, state presentation | Xvfb screenshots + manual review |
| Interaction | Real pointer/keyboard state transitions | `xdotool` against Xvfb |
| Windows compile | MSVC target, link, PE output | `cargo xwin` |
| Windows runtime | Explorer/DWM/taskbar/Core Audio/radios/media/recovery | **Not run** |

## 2. Commands and results

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets
cargo xwin build --target x86_64-pc-windows-msvc -p prism-app --bins
```

Executed results:

- **Formatting:** `cargo fmt --all -- --check` passed.
- **Tests:** 53 passed, 0 failed, 0 ignored:
  - `prism-core`: 27
  - `prism-platform`: 9
  - `prism-app`: 13
  - `prism-ui`: 4
  - binaries/doc tests: 0
- **Clippy:** `cargo clippy --workspace --all-targets` completed with no warnings or errors after
  cleanup.
- **Windows cross-build:** `cargo xwin build --target x86_64-pc-windows-msvc -p prism-app --bins`
  produced `prism-shell.exe` and `prism-preview.exe` (PE32+ x86-64). Static CRT link succeeded; the
  import table has no VC runtime DLL. Cross-compilation proves the toolchain/link path only.
- **Windows runtime:** not executed. No `Tested on Windows` label applies to any feature.
- **Automated UI snapshot matrix:** `scripts/test/ui-snapshots.sh` generated 14 UI-only PNGs at
  1920×1080, 1280×800, and 125%/150%/200% scale. All files were non-empty; dimensions were checked
  with `file` (1920×1080 for scale checks, 1280×800 for the small canvas).

The Windows binaries are debug builds. A release cross-build and native `windows-latest` CI job
remain future work.

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
- No content is positioned from a single hardcoded resolution; the taskbar centers from stage
  width, and the CC anchors to the taskbar's right edge with clamping.

## 5. What was not tested

Do not claim these as complete:

- Real Windows 11 runtime: transparent/frameless window, DWM backdrop, focus/z-order,
  non-activating taskbar.
- Win32 window enumeration, AUMID/UWP grouping, activation/minimization, elevated/UIPI targets.
- Explorer/taskbar coexistence, AppBar reservation, fullscreen/games, Explorer restart.
- Taskbar hiding/restoration (deliberately not implemented).
- Core Audio, default-device changes, radio/Wi-Fi/BT toggles, location consent.
- Brightness WMI/DDC-CI and monitor safety.
- GSMTC media sessions in an unpackaged binary.
- Tray replication and notification interception (deferred/blocked).
- Mixed-DPI multi-monitor, RDP, portrait, hotplug, primary-display changes.
- Narrator/AccessKit verification; full keyboard tab navigation; high-contrast mode.
- Performance baselines, memory/GPU measurements, 8-hour soak, 100+ windows.
- Installer, auto-start, update/recovery executable, code signing.
- Windows release build.

## 6. Windows 11 validation plan (Phase 3+)

Run on a disposable Windows 11 VM first, normal user, Explorer available:

1. Launch the fail-safe `prism-shell.exe`; confirm the warning appears and no shell state changes.
2. Build/run the preview explicitly; verify DPI and rendering at 100/125/150/200%.
3. Implement a minimal taskbar/CC host with documented HWND/DWM styles; verify transparency,
   non-activating input, z-order, and focus with a second monitor and mixed DPI.
4. Enumerate windows and compare grouping with Task View/Alt+Tab for Win32, UWP, packaged,
   multi-window, minimized, owned, tool, elevated, cloaked/virtual desktop.
5. Test click/activation/minimize/chooser and `FlashWindowEx` failure fallback.
6. Test Core Audio read/write/notifications, no-endpoint and exclusive-mode cases; no
   `IPolicyConfig`.
7. Test radio state and toggles in the unpackaged binary; record consent/policy outcomes.
8. Test GSMTC in the unpackaged binary; hide/disable unsupported controls.
9. Test brightness only on a spare monitor/laptop with opt-in; no startup DDC/CI probe.
10. Test DWM previews for minimized/UWP/protected/occluded windows; no capture persistence.
11. Test recovery only after an independent recovery tool exists; never auto-start/hide by
    default.
12. Measure startup, idle CPU/GPU, RSS, open latency, and 100+ window behavior.

## 7. Reproducibility

- Rust version is pinned by `rust-toolchain.toml`; `Cargo.lock` pins dependency versions.
- Mock fixtures use deterministic ids/order; window ids are generated deterministically.
- The virtual display is disposable and rootless.
- Screenshots use an explicit scale for logical-pixel review.
- No test weakens a production guard to pass. Mock failures are deliberate and do not mutate
  authoritative state.
