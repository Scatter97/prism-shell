# Windows 11 Validation Checklist

Status: **Attempted once; not passed.** The native shell was launched on Windows 11 and exited
before any surface appeared with `window handle unavailable`. Commit `3936efd` defers HWND
attachment to the first event-loop tick; it has not been re-tested. Do not mark any item below as
passed until it is executed successfully on a real Windows 11 x64 machine or VM.

## Execution log

| Date | Build | Result |
|---|---|---|
| 2026-11-22 | source build before `3936efd` | Failed before UI: `window handle unavailable: the underlying handle cannot be represented using the types in this crate`. HWND requested before winit created it. |
| pending | `3936efd` or newer | Re-run startup and the checklist below. |

## Build artifact

Release build:

```
target/x86_64-pc-windows-msvc/release/prism-shell.exe
```

The executable is unsigned, uses the static MSVC CRT, has no Visual C++
Redistributable dependency, and is a Windows GUI-subsystem binary. It writes logs
to `%LOCALAPPDATA%\Prism\logs\prism-shell.log`.

`prism-preview.exe` is the separate Linux/Windows development preview and still
uses mock data. It is not the Windows shell.

## What this build is supposed to do

- Run alongside Explorer, with the normal Windows taskbar fully visible.
- Show a floating Prism taskbar above the Windows taskbar.
- Show pinned apps that exist on the machine: File Explorer, Command Prompt,
  Windows PowerShell, and Microsoft Edge when found.
- Discover real top-level application windows and group them by executable.
- Show real executable icons (with a generic fallback when extraction fails).
- Launch pinned/non-running apps with `ShellExecuteExW`.
- Activate, restore, and minimize real windows; flash the target if
  `SetForegroundWindow` is denied.
- Open a real launcher with search and real pinned/running apps.
- Open a Control Center that provides real Windows Settings shortcuts, real
  battery status, real workstation lock, and an Exit action.
- Show real time/date and battery state.
- Exit without changing Explorer, the Windows taskbar, the registry, or startup.

## Deliberately not in this build

- No taskbar hiding, Explorer replacement, or shell registration.
- No AppBar/work-area reservation; the floating dock overlays maximized windows.
- No audio controls, Wi-Fi/Bluetooth toggles, brightness, media, tray replication,
  notification interception, or custom power UI. Those controls are shown as
  unavailable or open native Windows Settings.
- No window-preview thumbnails, right-click context menu, Start menu replacement,
  pinning UI, or multi-window chooser.
- No per-monitor dock; the primary display only.
- No auto-start or installer.

## Test environment

Record:

- Windows edition/build (`winver`).
- Physical or VM.
- GPU and driver.
- Display resolution(s) and scale factor(s).
- Number of monitors.
- Whether Explorer is the normal shell.

## Checklist

### 1. Safe startup

- [ ] `explorer.exe` is running before launch.
- [ ] Launch `prism-shell.exe` normally (no admin).
- [ ] Prism process stays alive for 10 seconds.
- [ ] Windows taskbar is still visible and usable.
- [ ] Alt+Tab and Task Manager still work.
- [ ] No new Prism entry appears in the Windows taskbar/Alt+Tab (tool window).
- [ ] No registry, startup, or Explorer changes were made.

### 2. Window surface / transparency

- [ ] The taskbar is frameless with no native title bar.
- [ ] Rounded corners match the Slint surface; no black rectangle around it.
- [ ] The translucent dock surface renders over the desktop (not fully black).
- [ ] Shadow is visible and not clipped in a distracting way.
- [ ] Control Center and launcher also have transparent margins and no black box.
- [ ] Without DWM/composition (RDP edge cases), the fallback is still readable.

**If alpha is black:** record GPU/driver, whether it is NVIDIA/OpenGL, and try
`SLINT_BACKEND=software` before changing code. This is the primary unverified
rendering risk.

### 3. Positioning and click-through

- [ ] The dock is centered at the bottom of the primary work area, above the
  Windows taskbar, with a visible gap.
- [ ] Clicking the transparent strip left/right of the dock reaches the desktop
  or underlying window (no full-width invisible click blocker).
- [ ] Clicking the dock surface works.
- [ ] The Control Center opens above the right side of the dock and stays
  onscreen.
- [ ] The launcher opens above the left side of the dock and stays onscreen.
- [ ] The active display scaling does not clip or off-center any surface.

### 4. Focus and z-order

- [ ] Clicking the taskbar/status area does not steal focus from the active app.
- [ ] Clicking a pinned/running app activates it.
- [ ] Clicking the already-active app minimizes it.
- [ ] Clicking a minimized app restores it.
- [ ] An elevated app (e.g., Task Manager) either activates or visibly flashes;
  the failure is logged and the dock remains responsive.
- [ ] Control Center opens above normal and maximized windows.
- [ ] Clicking outside Control Center closes it.
- [ ] Escape closes Control Center.
- [ ] The taskbar trigger toggles Control Center, including rapid repeated clicks.
- [ ] Launcher receives keyboard focus; typing filters the list; Escape closes.
- [ ] After closing a panel, the previously active app can be used normally.

### 5. Display scaling / DPI

Test at 100%, 125%, 150%, and 200% (as supported by the display):

- [ ] Dock height, icon size, text, and gap scale cleanly.
- [ ] Dock remains centered; no blurry or half-pixel text.
- [ ] Control Center text and utilities fit; content scrolls instead of clipping.
- [ ] Launcher fits onscreen.
- [ ] Change scaling while Prism is running; surfaces reposition within 1 second.
- [ ] Disconnect/reconnect a monitor; the dock returns to the primary work area.
- [ ] Mixed-DPI setup: record what happens. Per-monitor surfaces are not yet
  implemented; the dock is expected on the primary monitor only.

### 6. Applications, grouping, icons

- [ ] Pinned File Explorer, cmd, PowerShell, and Edge (if present) show real icons.
- [ ] Launching a pinned app opens it; it becomes a running entry when its window
  appears.
- [ ] Multiple windows from one executable group under one dock entry.
- [ ] Minimized windows remain in the group and restore correctly.
- [ ] File Explorer windows appear as their own entries/group.
- [ ] Edge/Chrome/VS Code windows group by executable without merging unrelated
  apps.
- [ ] A UWP app (Settings, Calculator, Photos) appears as a separate entry with a
  generic fallback icon, or is explicitly absent; record which.
- [ ] Owned dialogs and tool windows do not create bogus dock entries.
- [ ] Cloaked/virtual-desktop windows do not appear on the wrong desktop.
- [ ] Elevated/protected apps show a generic/limited entry instead of crashing.
- [ ] Closing all windows of an unpinned app removes its entry.
- [ ] Closing a pinned app's last window keeps the pinned entry.

### 7. Launcher

- [ ] Search filters by name/id.
- [ ] Clicking a non-running pinned app launches it.
- [ ] Clicking a running app activates its most recent window.
- [ ] Clicking outside closes the launcher.
- [ ] The Windows Start menu is still available independently.

### 8. Control Center

- [ ] Battery percentage and charging state match Windows.
- [ ] No battery section on a desktop without a battery.
- [ ] Wi-Fi/Bluetooth/Focus/Night-light tiles open the relevant native Windows
  Settings pages; they do not pretend to toggle hardware.
- [ ] Sound/display links open native Settings.
- [ ] Lock actually locks the workstation.
- [ ] Exit closes Prism only; Explorer and the Windows taskbar remain.
- [ ] Unsupported audio/brightness/media sections clearly say they are not
  implemented; no fake sliders or track data.
- [ ] Footer identifies this as a Windows backend, not the mock preview.

### 9. Recovery / coexistence

- [ ] Kill Prism from Task Manager; Explorer and the Windows taskbar are
  unaffected and immediately usable.
- [ ] Restart Explorer while Prism is running; Prism re-enumerates apps and does
  not interfere with the new taskbar.
- [ ] Lock/unlock the session; Prism survives and updates.
- [ ] Sleep/resume; Prism survives and updates.
- [ ] Start Prism twice; record behavior (a single-instance guard is not yet
  implemented).
- [ ] No Prism-created AppBar reservation remains after exit.

### 10. Evidence to attach

- [ ] Screenshot of the dock over the desktop.
- [ ] Screenshot of Control Center.
- [ ] Screenshot of the launcher.
- [ ] Screenshot at each tested scale.
- [ ] `%LOCALAPPDATA%\Prism\logs\prism-shell.log`.
- [ ] `winver`, GPU/driver, display configuration.
- [ ] Notes on every failed item, including whether Explorer remained usable.

## Result template

```
Windows build:
Machine / VM:
GPU / driver:
Displays / scales:
Artifact / commit:
Startup: PASS/FAIL
Transparency: PASS/FAIL
Positioning/click-through: PASS/FAIL
Focus/z-order: PASS/FAIL
DPI: PASS/FAIL
Apps/icons/activation: PASS/FAIL
Launcher: PASS/FAIL
Control Center: PASS/FAIL
Recovery/coexistence: PASS/FAIL
Blocking issues:
```

## Known high-risk items to watch first

1. Black alpha/transparency with FemtoVG/OpenGL or DWM.
2. `SetForegroundWindow` denial when clicking a NOACTIVATE taskbar.
3. Taskbar HWND style changes after winit creation (TOOLWINDOW/NOACTIVATE/TOPMOST).
4. UWP grouping/icon fallback and elevated-process access.
5. Overlay covering the bottom of maximized windows (no AppBar reservation).
6. Primary-monitor-only positioning.

Until this checklist is executed, report Windows integration as **Unverified**.
