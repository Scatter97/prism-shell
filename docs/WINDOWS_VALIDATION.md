# Windows 11 Validation Checklist

Status: **basic taskbar/icons/activation/coexistence owner-confirmed; new preview, recovery, and
Windows-key paths unverified.** The owner has confirmed on a real Windows 11 desktop that Prism
starts, the floating taskbar renders with native icons, clicking icons switches real windows, and
the original Windows taskbar remains visible. The rest of this checklist, including every item in
Phases A–C below, still requires execution.

## Execution log

| Date | Build | Result |
|---|---|---|
| 2026-11-22 | source build before `3936efd` | Failed before UI: `window handle unavailable: the underlying handle cannot be represented using the types in this crate`. HWND requested before winit created it. |
| 2026-11-22 | owner build after `3936efd` | **Owner-confirmed basic runtime:** taskbar renders, native icons appear, clicking icons switches real windows, original Windows taskbar remains visible. Launcher/Control Center implemented, not fully validated. |
| pending | this milestone (`prism-shell.exe` + `prism-recovery.exe`) | Phase A previews, Phase B recovery/replacement, Phase C Windows key. |

## Build artifacts

Release cross-build:

```
target/x86_64-pc-windows-msvc/release/prism-shell.exe       (GUI, ~14.5 MB)
target/x86_64-pc-windows-msvc/release/prism-recovery.exe    (console, ~325 KB)
target/x86_64-pc-windows-msvc/release/prism-preview.exe     (mock preview, ~14.8 MB)
```

Copy **both** `prism-shell.exe` and `prism-recovery.exe` into the same directory for Phase B.
`prism-shell.exe` is unsigned, uses the static MSVC CRT, and writes logs to
`%LOCALAPPDATA%\Prism\logs\prism-shell.log`. The recovery utility writes its own log to
`%LOCALAPPDATA%\Prism\logs\prism-recovery.log`.

`prism-preview.exe` is the explicit mock development preview; it is not the Windows shell.

## What this build does by default

- Runs alongside Explorer with the normal Windows taskbar visible.
- Shows a floating Prism taskbar above the Windows taskbar.
- Shows pinned apps that exist on the machine: File Explorer, Command Prompt, Windows PowerShell,
  and Microsoft Edge when found.
- Discovers real top-level application windows and groups them by executable.
- Shows real executable icons with a generic fallback.
- Shows real DWM live window previews on hover (new; unverified).
- Launches pinned apps with `ShellExecuteExW`; activates, restores, and minimizes real windows.
- Opens a real launcher with search and real pinned/running apps.
- Opens a Control Center with native Windows Settings shortcuts, battery, lock, and Exit.
- Exits without changing Explorer, the Windows taskbar, the registry, or startup.

Experimental and **off by default**:

- `--experimental-taskbar-replacement` / `experimental.taskbar_replacement`: hide the native
  taskbar and use Prism at the bottom screen edge. Requires `prism-recovery.exe` next to the shell.
- `--experimental-windows-key` / `experimental.windows_key_launcher`: standalone Win key toggles
  the Prism launcher; combinations pass through.
- `--experimental-windows-key-fallback-hotkey` /
  `experimental.windows_key_fallback_hotkey`: also register the fallback shortcut.
- `experimental.windows_key_fallback_shortcut`: default `Ctrl+Alt+Space`.

Do not enable taskbar replacement until the Phase B recovery gate below passes.

## Test environment

Record:

- Windows edition/build (`winver`).
- Physical or VM.
- GPU and driver.
- Display resolution(s) and scale factor(s).
- Number of monitors.
- Whether Explorer is the normal shell.

## Owner-confirmed baseline

These were reported by the owner on a real Windows 11 desktop:

- [x] Prism starts and stays running.
- [x] The floating taskbar renders.
- [x] Native application icons appear.
- [x] Clicking dock icons switches real windows.
- [x] The original Windows taskbar remains visible and usable.
- [ ] Full Control Center interaction matrix (still pending).

## Phase A — live window previews

Run with no experimental flags:

```powershell
.\prism-shell.exe
```

For each item, record the app and result:

- [ ] Hover a running app icon; after ~300 ms a preview appears above the dock.
- [ ] The preview shows actual live window content (not the mock miniature).
- [ ] The application icon and window title are visible.
- [ ] Move the pointer slowly from the dock icon into the preview; it stays open.
- [ ] Leave the combined dock+preview area; the preview closes within about half a second.
- [ ] Click a preview card; that exact window comes to the foreground.
- [ ] Click the per-window close button; only that window closes and the preview updates.
- [ ] A multi-window app shows one card per eligible window.
- [ ] Minimized windows keep a card with a "Minimized" state; clicking restores them.
- [ ] Close a window from another app while its preview is open; its card/thumbnail disappears.
- [ ] Rapidly sweep across several running icons; no stuck preview, flicker loop, or phantom
      thumbnail.
- [ ] Hover an app near the left and right screen edges; the preview stays onscreen.
- [ ] Test at 100%, 125%, 150%, and 200%; thumbnails and cards align at each scale.
- [ ] Test after moving the dock/monitor work area (change taskbar position or resolution).
- [ ] Open a fullscreen app, then hover a dock app; record whether the preview appears and whether
      activation works.
- [ ] Leave Prism idle with no preview open; confirm no repeating DWM work occurs and CPU is low.
- [ ] Exit Prism; confirm no `prism-shell.exe` remains and no DWM thumbnails linger.

If thumbnails are black or missing but the card chrome is correct, record the GPU/driver and
`SLINT_BACKEND` backend, and check the log for `DWM thumbnail registration failed`. This is the
primary unverified Phase A risk.

## Phase B — recovery gate and taskbar replacement

**Stop here unless you are on a disposable Windows 11 VM or a spare machine.** Taskbar replacement
is an experimental, opt-in shell modification. It is blocked for enablement until this entire
section passes. Keep `explorer.exe` running.

### B1. Recovery tool before hiding anything

```powershell
.\prism-recovery.exe --status
```

Expected: `No active taskbar replacement marker.` (`--status` reads no marker and does not touch
Explorer.)

```powershell
.\prism-recovery.exe --restore
```

Expected: it reports restored taskbar windows. On a normal desktop the taskbar is already visible;
this is a safe no-op visibility check.

### B2. Explicit opt-in and restore action

Start with the experimental flag:

```powershell
.\prism-shell.exe --experimental-taskbar-replacement
```

Immediately confirm:

- [ ] The native Windows taskbar disappears.
- [ ] Explorer is still running in Task Manager.
- [ ] Prism remains visible at the bottom edge with a ~14 logical-pixel gap.
- [ ] The dock is horizontally centered and clickable.
- [ ] The Control Center opens and shows **Restore bar**.
- [ ] Click **Restore bar**; the native taskbar returns immediately and Prism moves back above the
      taskbar work area.
- [ ] `prism-recovery.exe --status` reports no active marker after restore.

### B3. Forced-kill recovery (the critical test)

1. Start `prism-shell.exe --experimental-taskbar-replacement` again.
2. Confirm the taskbar is hidden.
3. Open Task Manager and **End task** only `prism-shell.exe`.
4. Wait up to 5 seconds.

Expected and required:

- [ ] The native taskbar returns within ~5 seconds without restarting Explorer.
- [ ] `%LOCALAPPDATA%\Prism\logs\prism-recovery.log` records a watchdog restore.
- [ ] `prism-recovery.exe --status` reports no active marker.
- [ ] The desktop remains usable even if Prism is not restarted.

If this fails, stop immediately. Do not use replacement mode again; run
`prism-recovery.exe --restore`, then report the log. Do not enable the feature on a daily-driver
machine.

### B4. Frozen-heartbeat recovery

Use a VM or a debugger that can suspend `prism-shell.exe` without terminating it.

- [ ] Suspend the Prism process for more than 6 seconds after replacement mode is active.
- [ ] The watchdog restores the taskbar after ~4 seconds of missed heartbeats.
- [ ] Resume/terminate Prism; Windows remains usable.

### B5. Explorer and monitor events

- [ ] While replacement mode is active, restart Explorer from Task Manager. Prism should leave
      replacement mode when the new taskbar appears; it must not hide the new taskbar in a loop.
- [ ] Lock/unlock the session; the taskbar returns after unlock if Explorer recreated it.
- [ ] With multiple monitors, record which secondary taskbars hide and whether
      `prism-recovery.exe --restore` shows all of them.
- [ ] Change display scaling/resolution; Prism repositions at the bottom edge.
- [ ] Do not enable auto-start. There is no startup integration in this build.

### B6. Known replacement-mode limits to verify and record

- Maximized windows still use the Windows work area, so the Prism dock overlaps their bottom edge.
  There is no AppBar reservation.
- The native notification area/tray is not replicated. While hidden, the recovery path is the
  Control Center **Restore bar** action or `prism-recovery.exe --restore`.
- Explorer taskbar class names are undocumented internals and may change in future Windows builds.

## Phase C — standalone Windows key

Start with:

```powershell
.\prism-shell.exe --experimental-windows-key --experimental-windows-key-fallback-hotkey
```

Default fallback: `Ctrl+Alt+Space`. To change it, add to `%APPDATA%\Prism\config.json`:

```json
{
  "version": 1,
  "experimental": {
    "windows_key_launcher": true,
    "windows_key_fallback_hotkey": true,
    "windows_key_fallback_shortcut": "Ctrl+Shift+Space"
  }
}
```

Checklist:

- [ ] Tap and release the left Windows key; Prism launcher opens instead of the Start menu.
- [ ] Tap/release again; Prism launcher closes.
- [ ] Repeat with the right Windows key.
- [ ] Long-press and release: exactly one toggle, no repeated toggles.
- [ ] Repeated quick taps toggle open/closed reliably.
- [ ] `Win+E`, `Win+R`, `Win+L`, `Win+D`, `Win+Tab`, `Win+Shift+S`, `Win+I`, `Win+X`, `Win+V` all
      still work and do not open Prism.
- [ ] `Win+Left/Right/Up/Down` still snap/maximize.
- [ ] `Win+number` still activates taskbar slots where applicable.
- [ ] `Ctrl+Alt+Space` (or the configured fallback) opens/closes Prism independently.
- [ ] Focus an elevated app (Task Manager); press the standalone Win key and the combinations.
      Record any failure/replay behavior.
- [ ] Focus an exclusive-fullscreen app; record whether the launcher appears and whether shortcuts
      still work.
- [ ] Exit Prism normally; the standalone Win key opens the native Start menu again.
- [ ] Kill Prism from Task Manager; the native Win key still opens Start (the hook died with the
      process).
- [ ] `Ctrl+Alt+Del` still opens the secure screen and is never intercepted.

If any native shortcut breaks, disable the experiment immediately and use only the fallback hotkey.
Do not mark Phase C validated without the full shortcut matrix on the target keyboard layout.

## Existing baseline checklist

The following items from the previous milestone still apply and are not superseded by the new
features:

### 1. Safe startup

- [ ] `explorer.exe` is running before launch.
- [ ] Launch normally (no admin).
- [ ] Prism stays alive for 10 seconds.
- [ ] Windows taskbar is visible and usable (default mode).
- [ ] Alt+Tab and Task Manager still work.
- [ ] No Prism entry appears in the Windows taskbar/Alt+Tab.
- [ ] No registry, startup, or Explorer changes were made.

### 2. Surface / transparency

- [ ] Taskbar, Control Center, launcher, and preview are frameless with no title bar.
- [ ] Rounded corners match the Slint surfaces; no black rectangle around them.
- [ ] Translucent surfaces render correctly over the desktop.
- [ ] Shadows are visible and not clipped distractingly.
- [ ] Click-through outside each visible panel works.
- [ ] Without DWM/composition, the fallback is still readable.

### 3. Focus / z-order

- [ ] Clicking the taskbar/status area does not steal focus from the active app.
- [ ] Clicking a pinned/running app activates it.
- [ ] Clicking the already-active app minimizes it.
- [ ] Clicking a minimized app restores it.
- [ ] An elevated app either activates or visibly flashes; Prism stays responsive.
- [ ] Control Center opens above normal and maximized windows.
- [ ] Clicking outside Control Center closes it; Escape closes it.
- [ ] Launcher receives keyboard focus; typing filters; Escape closes.
- [ ] Preview cards activate without leaving Prism focused or stuck on top.

### 4. Applications / grouping / icons

- [ ] Pinned File Explorer, cmd, PowerShell, and Edge (if present) show real icons.
- [ ] Launching a pinned app opens it and adds a running entry.
- [ ] Multiple windows from one executable group under one dock entry.
- [ ] Minimized windows remain in the group and restore correctly.
- [ ] UWP apps record whether they are separate/generic or absent.
- [ ] Owned dialogs/tool windows do not create bogus entries.
- [ ] Elevated/protected apps show a limited entry instead of crashing.
- [ ] Closing all windows removes an unpinned entry; pinned entries stay.

### 5. Control Center

- [ ] Battery percentage/charging match Windows (or no battery section on a desktop).
- [ ] Quick controls open the relevant native Settings pages.
- [ ] Sound/display links open native Settings.
- [ ] Lock actually locks.
- [ ] Exit closes Prism only; Explorer/taskbar remain (or are restored in replacement mode).
- [ ] Unsupported audio/brightness/media sections clearly say so; no fake sliders.

### 6. Recovery / coexistence

- [ ] Kill Prism (default mode); Explorer/taskbar are unaffected.
- [ ] Restart Explorer while Prism is running; Prism re-enumerates and does not interfere.
- [ ] Lock/unlock and sleep/resume; Prism survives.
- [ ] Start Prism twice; record behavior (no single-instance guard yet).
- [ ] No Prism AppBar reservation remains after exit (no AppBar is used).

## Result template

```
Windows build:
Machine / VM:
GPU / driver:
Displays / scales:
Artifact / commit:
Owner-confirmed baseline: PASS
Phase A previews: PASS/FAIL
Phase B recovery tool: PASS/FAIL
Phase B forced-kill restore: PASS/FAIL
Phase B replacement use: PASS/FAIL
Phase C standalone Win key: PASS/FAIL
Phase C shortcut matrix: PASS/FAIL
Default-mode regression: PASS/FAIL
Blocking issues:
Logs/artifacts:
```

## Known high-risk items to watch first

1. DWM thumbnails not appearing against the transparent Slint destination.
2. Preview hover handoff between the dock and the preview window.
3. Watchdog failing to restore after forced termination.
4. Explorer restart creating a new taskbar while Prism remains in replacement mode.
5. Windows-key synthetic replay breaking a native shortcut or leaving modifier state stuck.
6. Maximized-window overlap because replacement mode does not reserve work area.
7. Secondary taskbar restoration on multi-monitor systems.

Until this checklist is executed for the new features, report Phases A–C as **Unverified on
Windows 11**.
