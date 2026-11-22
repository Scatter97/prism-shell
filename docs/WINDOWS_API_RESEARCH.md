# Windows API Research — Prism Shell V1

Status: Research complete enough to choose the architecture. Windows runtime integration is
**Proposed/Unverified** until executed on a real Windows 11 machine or VM. Linux preview success
does not validate any of the Windows-specific behavior here.

Detailed citation notes: [`docs/research/windows-api-notes.md`](research/windows-api-notes.md).
Framework evaluation: [`UI_FRAMEWORK_EVALUATION.md`](UI_FRAMEWORK_EVALUATION.md).

Evidence labels used below:

- **Documented** — Microsoft API reference/guide.
- **Documented-limited** — documented but materially constrained.
- **Observed/undocumented** — no Microsoft contract; never a V1 foundation.
- **Unverified** — needs a real Windows 11 test.
- **Deferred** / **Blocked** — not implemented in V1.

## 1. Rust as the implementation language

**Decision: Rust stable 1.100.0 (edition 2024), target `x86_64-pc-windows-msvc`.** Rust gives
memory safety in the UI and state layers, a mature Win32/WinRT binding (`windows-rs`), and a
single language for the shell host, domain logic, and preview. The production binary must not
require the Rust toolchain at runtime.

**Advantages:** strong ownership model for window/focus state; small binaries; no GC; cargo
workspace boundaries; excellent FFI; `cfg(windows)` isolation for platform code.

**Disadvantages/risks:** the Win32/COM/WinRT surface still requires `unsafe` and careful lifetime
management; Windows UI toolkits are less mature than C#/C++ alternatives; async WinRT interop is
verbose. The architecture confines unsafe platform code to a future `prism-windows` crate.

**Status:** selected. Linux build/test verified; Windows build attempted separately (see
DEVELOPMENT.md and CURRENT_STATUS.md).

## 2. UI framework evaluation

**Decision: Slint 1.19 for V1**, with the presentation isolated. Full comparison in
[`UI_FRAMEWORK_EVALUATION.md`](UI_FRAMEWORK_EVALUATION.md).

Options considered: Slint, egui/eframe, iced, Tauri 2 + WebView2, Qt/cxx-qt, Avalonia/WinUI 3, and
a custom Win32 + Direct2D/Skia renderer. Slint won on declarative design-token ownership, native
Rust, GPU rendering, AccessKit, transparent/frameless/always-on-top window properties, raw HWND
access, compile-time assets, and `take_snapshot` testing. The main unresolved risk is transparent
window behavior on Windows/DWM with the chosen renderer; egui is the fallback.

## 3. Native transparent desktop-anchored windows

**APIs:** documented Win32 window styles/attributes plus DWM composition. Slint's `Window`
supports `no-frame`, `always-on-top`, and `background`; the HWND is available through
`raw-window-handle`/winit access. DWM attributes (`DwmSetWindowAttribute`):
`DWMWA_WINDOW_CORNER_PREFERENCE` (Win11 22000+), `DWMWA_USE_IMMERSIVE_DARK_MODE` (Win11 22000+),
`DWMWA_SYSTEMBACKDROP_TYPE` (Win11 22621+), and `DWMWA_BORDER_COLOR`.
[MS Learn: DWMWINDOWATTRIBUTE](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute)
[Slint Window](https://docs.slint.dev/latest/docs/slint/reference/window/window)

**Plan:** a normal composed frameless window with a transparent Slint background; apply DWM
corner/dark-mode/backdrop attributes from the Windows host. Use a solid translucent fallback if
composition/backdrop is unavailable. Do not use `WS_EX_LAYERED` unless testing proves it
necessary; layered windows complicate hit testing and child/popup behavior.

**Status:** Slint window properties and borderless stage verified on Linux. Windows transparency
and DWM integration are **Unverified**. The reference Slint Mica template documents an NVIDIA
OpenGL GDI-compatibility issue where transparent regions can render black; this must be tested
with FemtoVG and a Skia/wgpu/software fallback.

## 4. GPU acceleration and rendering performance

Slint's default desktop renderer is FemtoVG/OpenGL; Skia and software renderers are available.
The prototype keeps idle repaint low through dependency-driven properties and no continuous
animation. The preview's only periodic work is a 1 s clock/media tick; clock writes happen only
when the minute changes.

Windows performance is **Unverified**. Required measurements: cold/warm start, first frame, idle
CPU/GPU, RSS, Control Center open latency, hover preview cost, and behavior with 100+ windows.
Potential issue: OpenGL drivers, remote sessions, and hybrid GPUs. A renderer fallback is part of
the Phase 3 validation plan.

## 5. Borderless popup windows

Frameless windows use `WS_POPUP`-style presentation and Slint `no-frame`. The production host may
need `WM_NCCALCSIZE`/`WM_NCHITTEST` handling only if native resize/snap behavior is wanted; the
taskbar and panels do not need it. The Control Center/launcher/previews are separate top-level
surfaces, not child popups, so they can extend beyond the dock and layer above other windows.

**Status:** borderless single stage verified on Linux; multi-surface production model is planned.
Windows z-order/focus per surface is **Unverified**.

## 6. Input and focus management

Planned documented APIs:

- `WS_EX_NOACTIVATE` + `WM_MOUSEACTIVATE` → `MA_NOACTIVATE` for status clicks that must not take
  focus.
- `SetWindowPos(..., SWP_NOACTIVATE)` for showing surfaces without activation.
- `SetForegroundWindow` for app activation. The call is restricted: the caller must be the
  foreground process, received the last input, was started by the foreground process, etc., and
  Windows may still deny it. Check the result and use `FlashWindowEx` rather than forcing focus.
- `ShowWindow(SW_RESTORE/SW_MINIMIZE)` for target windows. `AllowSetForegroundWindow` only passes
  privilege a process already has; it is not a self-grant.
- Do not make `AttachThreadInput`/`SetWindowPos(HWND_TOPMOST)` the primary activation path.

[MS Learn: SetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow)
[MS Learn: ShowWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow)

**Status:** the preview implements the intended semantic click behaviors (launch, activate,
minimize, restore, multi-window selection) against mocks. Real Win32 activation, UIPI/elevated
targets, and non-activating HWND styles are **Unverified**.

## 7. Window enumeration and application identity

**Enumeration:** `EnumWindows`, `IsWindowVisible`, `GetWindowLongPtrW(GWL_EXSTYLE)`,
`GetWindowThreadProcessId`, `GetWindowTextW`, `IsIconic`, `DwmGetWindowAttribute(DWMWA_CLOAKED)`,
`GetAncestor(GA_ROOTOWNER)`.
[MS Learn: EnumWindows](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumwindows)
[DWMWINDOWATTRIBUTE](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute)

Filter strategy (documented heuristics plus observed shell classes):

- Exclude Prism's own windows, invisible windows, cloaked windows, tool windows without
  `WS_EX_APPWINDOW`, owned windows without `WS_EX_APPWINDOW`, known shell/desktop classes, and
  duplicate UWP host/child surfaces.
- Keep minimized and elevated windows; show a limited state for elevated targets.
- Do not reject solely for an empty title or `explorer.exe` (File Explorer windows are apps).
- `EnumWindows` is a snapshot; HWNDs can disappear/recycle. Reconcile.

**Identity:** window-level AUMID (`PKEY_AppUserModel_ID` via `SHGetPropertyStoreForWindow`), then
process AUMID (`GetApplicationUserModelId`), then normalized full image path
(`QueryFullProcessImageNameW`). Group UWP by the frame/child AUMID carefully; never group solely
by PID (UWP frames share `ApplicationFrameHost.exe`) or solely by exe path (multi-AUMID apps).
[GetApplicationUserModelId](https://learn.microsoft.com/en-us/windows/win32/api/appmodel/nf-appmodel-getapplicationusermodelid)
[QueryFullProcessImageNameW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew)
[SHGetPropertyStoreForWindow](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetpropertystoreforwindow)

**Launching:** `ShellExecuteExW`/`IShellLinkW` for classic apps; `IApplicationActivationManager`
for UWP. Elevated processes may deny inspection/activation under UIPI.

**Status:** portable grouping/identity model and fixtures implemented; Win32 enumeration,
AUMID extraction, icons, and launch/activation are **Deferred to Phase 3**. UWP heuristics are
**Unverified**.

## 8. Windows taskbar coexistence

**AppBar API:** `SHAppBarMessage` (`ABM_NEW`, `ABM_QUERYPOS`, `ABM_SETPOS`, `ABM_REMOVE`,
`ABM_GETTASKBARPOS`, `ABM_GETSTATE`). It reserves an edge-anchored work-area rectangle, not an
arbitrary floating visual, and it is not a taskbar replacement: it cannot host app buttons, own
the notification area, or reproduce Start/Search.
[MS Learn: SHAppBarMessage](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shappbarmessage)
[MS Learn: The Taskbar](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar)

**V1 decision:** run as a floating overlay without reserving work area. Maximized/fullscreen
windows may overlap the dock; full-screen detection and optional reservation are Phase 6/7.
`GetMonitorInfo().rcWork` is the reliable work area. `ABM_GETTASKBARPOS` only describes the system
taskbar.

**Status:** no AppBar registration implemented. Coexistence strategy is **Proposed** and must be
tested on Windows 11 with taskbar on each edge, autohide, maximized, exclusive fullscreen, and
multi-monitor.

## 9. Safe taskbar hiding and restoration

**Not implemented. Deferred and safety-gated.**

The common technique is `FindWindowW(L"Shell_TrayWnd", NULL)` + `ShowWindow(SW_HIDE)`, with
`Shell_SecondaryTrayWnd` for secondary monitors. `FindWindowW`/`ShowWindow` are documented, but
the taskbar window class names and Explorer's internal hierarchy are **Observed/undocumented**;
the Windows 11 taskbar is Explorer/XAML-internal and can change between builds. Registry
`StuckRects3` edits are undocumented and persistent. They are not an acceptable recovery
foundation.

A future opt-in experiment requires:

1. An independent recovery executable that does not link the running shell.
2. A watchdog that restores the taskbar if Prism exits unexpectedly.
3. Tested restore on primary/secondary taskbars, Explorer restart, reboot, and kill -9/crash.
4. No auto-start, no default-on, no system-wide shell registration.
5. A documented rollback and an explicit owner approval.

**Status:** **Deferred/Blocked** for V1. The current bootstrap makes no shell changes.

## 10. Multi-monitor and DPI

Use PerMonitorV2 awareness, set before any HWND (manifest recommended; winit may set it during
backend init). Use `GetDpiForWindow`, `WM_DPICHANGED`, `EnumDisplayMonitors`,
`GetMonitorInfoW`, `MonitorFromWindow(..., MONITOR_DEFAULTTONEAREST)`, `WM_DISPLAYCHANGE`, and
`WM_SETTINGCHANGE(SPI_SETWORKAREA)`. Do not use `GetDpiForMonitor` from a per-monitor-aware
thread. One surface per monitor; do not span mixed-DPI monitors with one window. Convert physical
screen coordinates to/from Slint logical coordinates with the window scale factor.
[High DPI Desktop Application Development](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows)
[WM_DPICHANGED](https://learn.microsoft.com/en-us/windows/win32/hidpi/wm-dpichanged)

**Status:** logical layout and overflow are tested at multiple logical sizes; a Linux DPI matrix
was run with `SLINT_SCALE_FACTOR`. Real Windows mixed-DPI/multi-monitor geometry is **Unverified**.

## 11. Windows Core Audio integration

**Volume/mute:** documented `IMMDeviceEnumerator` + `IAudioEndpointVolume`. Use
`GetMasterVolumeLevelScalar`/`SetMasterVolumeLevelScalar` (0.0–1.0), `GetMute`/`SetMute`,
`VolumeStepUp`/`VolumeStepDown` for step semantics, `RegisterControlChangeNotify` for external
changes, and `IMMNotificationClient` for endpoint changes. No special permission is required for
normal endpoint volume/mute. Handle `E_NOTFOUND`.
[IAudioEndpointVolume](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudioendpointvolume)
[IMMDeviceEnumerator](https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nn-mmdeviceapi-immdeviceenumerator)

**Default-device switching:** no documented Core Audio setter. `IPolicyConfig` is undocumented,
version-sensitive, and must not be a product dependency. V1 fallback: show the device list
read-only and open `ms-settings:sound`/the native output selector. Do not fake a working device
picker.

**Status:** preview volume/mute/device list and reconciliation are implemented against mocks.
Real Core Audio is **Deferred to Phase 5**.

## 12. Wi-Fi and Bluetooth

**Radios:** `Windows.Devices.Radios.Radio.GetRadiosAsync()`,
`RequestAccessAsync()`, `SetStateAsync()`, `StateChanged`. Enumeration/state reads are
documented; changing state requires user consent and may be blocked by policy/hardware. The
`radios`/`cellularDeviceControl` capability declarations are described for packaged apps;
unpackaged desktop behavior is **Unverified**. Never claim a toggle works until the real binary
is tested.
[Radio](https://learn.microsoft.com/en-us/uwp/api/windows.devices.radios.radio)

**Wi-Fi:** WLAN API for status (`WlanOpenHandle`, `WlanEnumInterfaces`, `WlanQueryInterface`,
`WlanRegisterNotification`), with location-consent constraints for SSID/scan on current Windows
11. Prefer `NetworkInformation` for connectivity level and open
`ms-settings:network-wifi` for network selection in V1. Do not log SSIDs. Do not scan on a timer.
[WLAN API](https://learn.microsoft.com/en-us/windows/win32/api/wlanapi/nf-wlanapi-wlanopenhandle)
[Wi-Fi location changes](https://learn.microsoft.com/en-us/windows/win32/nativewifi/wi-fi-access-location-changes)

**Bluetooth:** radio state via `Radio`; paired-device inventory via `DeviceInformation`; pairing
and removal through native Settings/Quick Settings in V1.

**Status:** status/toggle capability states are modeled and demonstrated in mock fixtures.
Real toggles/permissions are **Unverified**; V1 fallback is status + Settings.

## 13. Brightness on laptops and external displays

Two different mechanisms:

- **Internal/laptop panels:** WMI `WmiMonitorBrightness` (read) and
  `WmiMonitorBrightnessMethods.WmiSetBrightness` (write), `root\wmi`. Support is
  panel/ACPI/driver-dependent.
  [WmiMonitorBrightness](https://learn.microsoft.com/en-us/windows/win32/wmicoreprov/wmimonitorbrightness)
- **External monitors:** DDC/CI through `GetPhysicalMonitorsFromHMONITOR` +
  `GetMonitorCapabilities`/`GetMonitorBrightness`/`SetMonitorBrightness` (Dxva2). Calls are slow
  and many monitors implement MCCS incompletely; Microsoft warns of undefined behavior. Some
  monitors have triggered a kernel bug from malformed capability strings. No startup probe; an
  explicit opt-in and monitor exclusion list are required.
  [GetPhysicalMonitorsFromHMONITOR](https://learn.microsoft.com/en-us/windows/win32/api/physicalmonitorenumerationapi/nf-physicalmonitorenumerationapi-getphysicalmonitorsfromhmonitor)

**V1 decision:** show brightness only when a validated supported path exists; otherwise show
"Unsupported" and open `ms-settings:display`. Never render an enabled-looking slider with no
effect. HDR/content brightness is a separate compositor concept; do not claim equivalence.

**Status:** mock supported/unsupported/error states implemented. Real WMI/DDC/CI is **Deferred**.

## 14. Windows media session integration

`Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager.RequestAsync()`, sessions,
media properties, timeline, playback info, and events are documented (Windows 10 1809+). Respect
`Controls.IsPlayEnabled` etc.; hide/disable unsupported transport buttons. Handle missing
artwork/artist/album and session disappearance. Do not persist metadata.
[GSMTC SessionManager](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager)

**Caveat:** `globalMediaControl` is listed as an app capability; packaged capability declarations
do not directly answer whether an unpackaged Prism binary can call the API. Test the real binary on
Windows 11 before claiming support; if denied, package a helper with identity/capability or defer
the card.

**Status:** rich mock media card, play/pause/next/previous, progress, empty/error states
implemented. Real GSMTC is **Deferred to Phase 5**.

## 15. System tray replication limitations

`Shell_NotifyIcon` adds/modifies/deletes **the calling application's own** notification icon. It
does not enumerate or take ownership of third-party tray icons. Windows 11's tray/overflow is
Explorer-internal with no documented enumeration/menu contract. `TaskbarCreated` is the supported
pattern for re-adding Prism-owned icons after an Explorer restart.

**V1 decision:** Prism-owned status icons only; preserve access to the original notification area;
third-party tray replication is **Deferred/Blocked** pending a packaged helper, privacy review, and
evidence of a supported approach.
[Shell_NotifyIcon](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyiconw)

## 16. Notification interception limitations

- `UserNotificationListener` can read notifications, but requires explicit user consent
  (`RequestAccessAsync` on a UI thread), a packaged capability, and can be revoked; only toast
  notifications are covered.
- `ToastNotificationManager.History` only returns the calling app's history.
- There is no verified documented URI for the Windows 11 Notification Center. Do not ship an
  unverified `ms-actioncenter:` guess as the only entry point.
- V1: no fabricated counts/history; open `ms-settings:notifications` for configuration, and do not
  claim arbitrary notification capture.

**Status:** **Deferred/Blocked**.
[UserNotificationListener](https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications.management.usernotificationlistener)

## 17. Windows cross-compilation from Linux

**Tooling:** `rustup target add x86_64-pc-windows-msvc`; `cargo install cargo-xwin --locked`;
`cargo xwin build --target x86_64-pc-windows-msvc -p prism-app --bins`. `cargo-xwin` fetches the
Microsoft CRT/SDK and uses lld-link; pure-Rust dependencies do not need a local C++ toolchain.
Accept the Microsoft SDK/CRT license terms before distributing builds.
[cargo-xwin](https://github.com/rust-cross/cargo-xwin)

**Status:** succeeded in this milestone (debug). `prism-shell.exe` and `prism-preview.exe` are
PE32+ x86-64. Cross-compilation proves compile/link only, not runtime behavior.

## 18. Windows 11 runtime testing and CI

A native `windows-latest` GitHub Actions runner is appropriate for build, unit tests, fmt, and
clippy. It is not a substitute for interactive Explorer/DWM/taskbar/audio/radio validation. Use a
dedicated Windows 11 VM or physical machine for Phase 3 and Phase 6. Wine is useful only for pure
logic tests; it is not Windows Explorer, DWM, Core Audio, radios, or the notification stack.

The project has no remote push in this milestone. A CI workflow can be added when the repository
has a remote and the owner authorizes it.

## 19. Recovery and crash-safe shell behavior

V1 does not hide the taskbar, replace Explorer, register a shell, or auto-start. Therefore a crash
leaves Windows unchanged. The Windows bootstrap in this milestone shows a diagnostic message box
and exits instead of running mock data. Any future hide/deep integration must be independent of
Prism's process and tested from `Win+R`/Task Manager, including Explorer restart and reboot. The
recovery tool must restore both primary and secondary taskbars and must not depend on Prism being
alive. See section 9 and `docs/ARCHITECTURE.md` §6.

## 20. Automated screenshots and interface testing

- Slint `Window::take_snapshot()` is implemented in `prism-preview --screenshot`. It produces
  deterministic PNGs at the window's physical size; `--scale` forces the logical scale factor
  before backend init. Verified at 1600×1000 and 1920×1080.
- Rootless Xvfb + `ffmpeg -f x11grab` captures the full virtual desktop. `scripts/dev/virtual-display.sh`
  provides install/start/stop/screenshot/run-app.
- `xdotool` was used for real pointer/keyboard interaction tests on Xvfb (open/close Control
  Center, toggle Wi-Fi, drag volume, search launcher, hover previews, minimize, close window,
  power confirmation).
- Windows visual validation will need a real session. DWM thumbnails are the primary future
  window-preview mechanism; Windows.Graphics.Capture requires a system picker/consent and is not
  suitable for automatic hover previews. `PrintWindow` is a last-resort fallback and must not run
  on the UI thread.
[DwmRegisterThumbnail](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmregisterthumbnail)
[Windows.Graphics.Capture](https://learn.microsoft.com/en-us/windows/uwp/graphics/capture)

## 21. Feasibility conclusion

| Capability | Feasibility | V1 status |
|---|---|---|
| Floating dock/taskbar UI | High | Implemented in preview |
| App grouping/activation | Medium-high; UIPI/UWP risks | Model implemented; Win32 deferred |
| Taskbar coexistence as overlay | High | Planned; test on Windows |
| AppBar work-area reservation | Medium; not a taskbar replacement | Deferred |
| Hiding the Windows taskbar | Low/safety-critical due to undocumented Explorer internals | **Blocked** without recovery approval |
| Volume/mute | High | Deferred to Phase 5 |
| Default audio switching | Low via documented APIs | Read-only list + Settings fallback |
| Wi-Fi/BT status | Medium (location/policy) | Deferred |
| Wi-Fi/BT toggles | Unverified unpackaged capability | Status + Settings fallback |
| Brightness internal | Medium (hardware-dependent) | Deferred |
| Brightness external DDC/CI | Low-medium; monitor risk | Opt-in only; deferred |
| Media control | Medium (unpackaged capability unverified) | Deferred |
| Third-party tray replication | Low/unsupported | Deferred/blocked |
| Arbitrary notification interception | Low/permissioned | Deferred/blocked |
| Cross-compile from Linux | High for compile/link | Attempted; result in CURRENT_STATUS |
| Windows runtime validation | Mandatory | Not done |
| Recovery from shell changes | High if no changes are made | V1 makes no changes |
