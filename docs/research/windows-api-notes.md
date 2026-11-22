# Windows API Research Notes — Prism Shell

Status: Phase 0 research note. Target: Windows 11 x64, normal user, no process injection and no Explorer patching. Research date: 2026-11-21.

No Windows 11 runtime test was executed for this note. Everything Windows-specific is **Proposed** until Phase 3 actually runs on Windows 11. Linux preview success is not evidence of Windows shell integration.

Evidence labels: **Documented** = Microsoft API reference/guide; **Documented-limited** = documented but materially constrained; **Observed/undocumented** = no Microsoft contract, never a V1 foundation; **Unverified** = needs a real Windows 11 test.

## V1 decision summary

| Area | V1 recommendation | Status |
|---|---|---|
| Enumeration | `EnumWindows` snapshot; filter invisible, cloaked, tool, owned, shell, duplicate UWP | Proposed |
| Events | `SetWinEventHook` out-of-context + coalescing queue + reconciliation | Proposed |
| Identity | AUMID first, exe path fallback, host-process heuristic last | UWP heuristics unverified |
| Activation | Click-context `SetForegroundWindow`; `SW_RESTORE`/`SW_MINIMIZE`; flash on denial | Proposed |
| Taskbar | Floating overlay only; no AppBar reservation, no hiding | AppBar deferred; hiding deferred/blocked |
| DPI | PerMonitorV2, one surface per monitor, physical-pixel geometry | Proposed |
| Audio | Volume/mute via `IAudioEndpointVolume`; no default switch | Device switch deferred |
| Radios/Wi-Fi/BT | Status; radio toggles only after validation; connect via Settings | Toggles unverified |
| Brightness | WMI internal, DDC/CI opt-in external, else hide/disable | Deferred/opt-in |
| Media | GSMTC card; only supported controls enabled | Unpackaged capability unverified |
| Tray/notifications | Prism-owned icons only; no third-party capture | Deferred/blocked |
| Previews | DWM thumbnail primary; icon+title fallback | Proposed |
| Build/CI | cargo-xwin + native `windows-latest`; Wine only for pure logic | Proposed |

Windows 11 already exceeds every minimum OS version below; they are recorded for portability, not because V1 must support older Windows.

## 1. Top-level window enumeration and filtering

**APIs:** `EnumWindows`, `IsWindowVisible`, `GetWindowLongPtrW(GWL_EXSTYLE)`, `GetAncestor`, `GetWindow(GW_OWNER)`, `GetWindowThreadProcessId`, `GetClassNameW`, `GetWindowTextW`, `IsIconic`, `IsWindow`, `DwmGetWindowAttribute`. [R1][R2][R3]

`EnumWindows` enumerates top-level windows and, since Windows 8, is documented to enumerate only top-level windows of desktop apps; it does not enumerate child windows. Treat every result as a snapshot: an `HWND` can be destroyed or recycled between calls, and `IsWindow` is explicitly unreliable for windows the caller did not create. [R1][R3]

**Minimum OS:** `EnumWindows`/`IsWindowVisible` are Windows 2000+; DWM checks are Vista+; all available on Windows 11. **Fallback:** if identity or bounds are unavailable, show icon + title rather than dropping the app.

| Filter | Why |
|---|---|
| Invalid/own process | Prism's dock, panels, previews, message windows must never appear |
| `!IsWindowVisible` | `WS_VISIBLE` style test; necessary but not proof of being unobscured [R2] |
| `DWMWA_CLOAKED != 0` | Cloaked windows are composed but invisible; catches suspended UWP and other virtual desktops [R2] |
| `WS_EX_TOOLWINDOW` without `WS_EX_APPWINDOW` | Tool windows are excluded from taskbar/Alt+Tab by design [R1] |
| Owned window without `WS_EX_APPWINDOW` | Owned dialogs belong to their owner; the Shell taskbar buttons unowned windows [R1][R4] |
| Known shell/desktop classes | `Progman`, `WorkerW`, `Shell_TrayWnd`, `Shell_SecondaryTrayWnd`, tray hosts; class names are **observed/undocumented** [R12] |
| Duplicate UWP surface | Hosted frame + child `CoreWindow` can both be visible; count once (observed heuristic) [R28] |

Keep minimized windows (`IsIconic`) and elevated windows. Do not silently drop an elevated app because activation may fail; show it with a limited-capability state. Do not reject solely because `GetWindowTextW` is empty: it returns the caption without hanging on an unresponsive process, but returns empty when there is no caption, and some apps are briefly titleless during startup. [R3][R9] Do not reject all `explorer.exe` windows; File Explorer windows are legitimate apps.

**UWP note.** Microsoft documents `CoreWindow` as the UWP window abstraction but does not document `ApplicationFrameWindow`, `ApplicationFrameHost.exe`, or the frame/child relationship as a shell contract. In practice, UWP frames appear as desktop top-level windows hosted by `ApplicationFrameHost.exe`, with app content in a child `CoreWindow`. This is **Observed/undocumented**; test on each Windows 11 build. [R1][R28]

**Bounds.** `GetWindowRect` is DPI-virtualized and may include invisible resize borders; `DWMWA_EXTENDED_FRAME_BOUNDS` is the visible frame in screen space but is not DPI-adjusted. Use DWM bounds for previews/hit-testing, fallback to `GetWindowRect` only on failure. [R2][R14]

**Reliability:** high for plain Win32 snapshots; medium around UWP hosting, elevated/protected windows, and HWND reuse. Map every failure to “skip this entry,” bound enumeration time, and never call blocking cross-process APIs from the UI thread.

## 2. Event-driven tracking

Use `SetWinEventHook` with `WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS`. Out-of-context callbacks are not mapped into the target process; events are queued, asynchronous, and guaranteed sequential. The calling thread must have a message loop, and events are delivered on that thread. Minimum: Windows 2000+. [R5]

Do **not** use `WINEVENT_INCONTEXT`: it requires a DLL mapped into the generating process, which conflicts with the no-injection policy; Microsoft also notes it does not load into Store app processes unless installed by UIAccess tools. [R5]

Hooks: `EVENT_OBJECT_CREATE/DESTROY/SHOW/HIDE/NAMECHANGE`, `EVENT_OBJECT_CLOAKED/UNCLOAKED`, `EVENT_SYSTEM_FOREGROUND`, `EVENT_SYSTEM_MINIMIZESTART/MINIMIZEEND`, `EVENT_OBJECT_LOCATIONCHANGE`, `EVENT_SYSTEM_DESKTOPSWITCH`. [R5] For object events accept only `idObject == OBJID_WINDOW` and `idChild == CHILDID_SELF`; `CREATE` also fires for child controls, and `LOCATIONCHANGE` is not sent for every child when a parent moves. [R5]

**Pipeline:** callback timestamps and enqueues `(event, hwnd)` into a bounded MPSC queue; a worker coalesces per `HWND` with a 50–150 ms debounce, re-queries, and updates the model. Never call `SetWinEventHook`/`UnhookWinEvent` in a callback; `UnhookWinEvent` must run on the installing thread; reentrancy can reorder events. [R5]

**Reconciliation is mandatory.** Re-enumerate on startup, `WM_DISPLAYCHANGE`, `WM_SETTINGCHANGE` (`SPI_SETWORKAREA`), `TaskbarCreated`, session unlock, and a bounded 2–5 s timer while visible. A zero hook handle must fall back to polling, not leave an empty model. `EVENT_SYSTEM_FOREGROUND` gives the foreground HWND; map via `GetAncestor(GA_ROOT)` before matching, and preserve the last non-Prism active app when Prism itself is foreground. `GetForegroundWindow` can return `NULL` during transitions. [R3][R5][R11][R14]

**Reliability:** high event coverage, not guaranteed delivery. UIPI can block inspection of higher-integrity windows; test an elevated console/app and degrade honestly. [R9]

## 3. App identity and grouping

`QueryFullProcessImageNameW` returns the full image path from a process handle opened with `PROCESS_QUERY_LIMITED_INFORMATION` and is the reliable fallback for classic Win32 apps. [R6]

**Minimum OS:** `SHGetPropertyStoreForWindow` is Windows 7+; `GetApplicationUserModelId` is Windows 8+. Both are available on Windows 11. **Fallback:** normalized exe path, then class/host heuristic, never a fabricated AUMID.

`GetApplicationUserModelId` returns the explicit process AUMID and returns `APPMODEL_ERROR_NO_APPLICATION` when there is no application identity. Microsoft states that applications cannot retrieve a system-assigned AppUserModelID, so AUMID is authoritative when present but a fallback is always required. [R6]

Window-level AUMID is `System.AppUserModel.ID` (`PKEY_AppUserModel_ID`), readable through the property store from `SHGetPropertyStoreForWindow`. A window-level AUMID overrides a process-level AUMID. The API is documented for setting; cross-process reading is normal `IPropertyStore::GetValue`, but behavior on hosted/UWP frames is not contractually documented. Treat as **Documented-limited/Observed**. [R7]

Grouping priority: (1) window AUMID; (2) process AUMID; (3) hosted UWP frame AUMID → child `CoreWindow` process AUMID → packaged-app fallback; (4) normalized full image path; (5) host-process special case only when no AUMID. Never group solely by PID for UWP: the frame belongs to `ApplicationFrameHost.exe`, so unrelated UWP apps share a PID. Never group solely by exe path for multi-AUMID apps (browsers, PWAs, suites, hosts). [R6][R28]

**Launching:** `IApplicationActivationManager::ActivateApplication` activates a UWP app by AUMID and returns the process ID. [R8] For classic desktop apps use `ShellExecuteExW` (or resolve a `.lnk` via `IShellLinkW` then execute); `ShellExecuteEx` may delegate to shell extensions, wants COM initialized, and must not run under loader lock. [R8]

**Elevated/protected limits:** `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)`, AUMID/path queries, and cross-integrity messages may fail; UIPI blocks hooks/messages into higher-integrity processes. Do not use `PROCESS_VM_READ`, injection, or elevation tricks; show a limited state and test activation separately. [R6][R9]

**Reliability:** high for AUMID-bearing apps and classic exes; medium for hosted UWP, host processes, and elevated/protected apps. Record actual AUMID/frame/child-PID behavior per Windows build.

## 4. Activation and minimization

**Minimum OS:** the activation/minimization APIs are Windows 2000+; behavior is still constrained on Windows 11. `SetForegroundWindow` is heavily restricted. The caller must be a desktop app, the foreground process must not have called `LockSetForegroundWindow`, no menu may be active, and at least one of: foreground lock timeout expired, caller is foreground, caller was started by foreground, no foreground exists, caller received the last input event, or either process is being debugged. Microsoft adds that a process can still be denied even when it meets these conditions. [R9]

The practical Prism path is a **user click**: the click gives Prism the last input event, so `SetForegroundWindow` is normally permitted. Check the return value; on failure do not loop or force focus. Call `FlashWindowEx(FLASHW_TRAY | FLASHW_TIMERNOFG)` and present a non-destructive “couldn't switch” state. [R9]

`AllowSetForegroundWindow` only lets a process that already has foreground privilege pass it to another process; it cannot grant Prism privilege it lacks. Use it only for a helper that must activate something, never as a self-grant. [R9]

Restore/minimize: `ShowWindow(SW_RESTORE)` then `SetForegroundWindow` for a minimized target; `ShowWindow(SW_MINIMIZE)` for the active single-window app (this activates the next top-level window); `SW_SHOWMINNOACTIVE` when activation is undesirable; for multi-window apps show the chooser instead of minimizing all. `IsIconic` reports minimized state. `ShowWindow` is not a focus-stealing bypass. [R9]

`AttachThreadInput` is **not** the primary path: it fails if either thread has no message queue, resets keyboard state, cannot attach across desktops, and UIPI restricts attaching to a higher-integrity input queue. `SetWindowPos(HWND_TOPMOST)` also does not bypass foreground permission. [R9]

**Reliability:** high for same-integrity, user-initiated clicks; medium for UWP/minimized; low for elevated targets where UIPI applies. Always have `FlashWindowEx` as the honest fallback.

## 5. Taskbar coexistence, AppBar, fullscreen, and hiding

**Minimum OS:** AppBar APIs are XP+; `SHQueryUserNotificationState` is Vista+; the taskbar-hiding class names have no documented minimum or contract. **Fallback:** floating overlay with no work-area reservation.

**AppBar is not a taskbar replacement.** `SHAppBarMessage` supports `ABM_NEW`, `ABM_QUERYPOS`, `ABM_SETPOS`, `ABM_REMOVE`, `ABM_GETTASKBARPOS`, and `ABM_GETSTATE`. Registration requires a top-level window and an application-defined callback message; unregister with `ABM_REMOVE` before destroying the window. AppBars are edge-anchored; the system adjusts the proposed rectangle so it does not overlap the taskbar or another appbar; if on the same edge as the taskbar, the system keeps the taskbar outermost. Any number of appbars can exist. [R10] Microsoft's taskbar documentation still describes Shell-managed taskbar buttons and the notification area; the AppBar API provides no way to host taskbar buttons, own the notification area, or replace the taskbar. [R4][R10]

A floating dock with a gap and rounded corners cannot reserve exactly its visual rectangle: `ABM_SETPOS` reserves an edge-anchored rectangle and may adjust it. **V1 recommendation:** overlay the dock without reserving work area; if “reserve space” is later requested, implement it as a separate appbar-style strip behind a flag, not as a property of the floating visual. [R10]

**Fullscreen.** If Prism registers an appbar, `ABN_FULLSCREENAPP` reports full-screen start/close and the appbar should drop to the bottom of the z-order. `SHQueryUserNotificationState` exposes `QUNS_BUSY` (full-screen or presentation), `QUNS_RUNNING_D3D_FULL_SCREEN` (exclusive D3D), and `QUNS_PRESENTATION_MODE`. Microsoft explicitly notes that no notification is sent when a full-screen app starts or stops, so a timer/foreground-event recheck is required. Borderless fullscreen is not guaranteed to set either value; add a foreground-bounds heuristic and test games/video/RDP. [R10][R11]

**Taskbar state.** `ABM_GETTASKBARPOS` returns the system taskbar rectangle; it applies only to the system taskbar, not third-party appbars, and Microsoft directs you to `GetMonitorInfo` for the real work area. `ABM_GETSTATE` returns autohide/always-on-top; as of Windows 7 `ABS_ALWAYSONTOP` is no longer returned because the taskbar is always on top. `GetMonitorInfo(...).rcWork` is the reliable work area. [R10][R14]

**Hiding the taskbar (deferred; unsafe).** The common technique is `FindWindowW(L"Shell_TrayWnd", NULL)` + `ShowWindow(hwnd, SW_HIDE)`. `FindWindowW`/`ShowWindow` are documented, but `Shell_TrayWnd` and `Shell_SecondaryTrayWnd` are **not documented in the taskbar API reference**; Microsoft-hosted Q&A answers use them, which is not a contract. Windows 11's taskbar is Explorer-internal (Q&A shows XAML content), so its hierarchy can change between builds. [R4][R12]

Consequences: Explorer can recreate/reset the taskbar; the secondary taskbar must be handled separately; a crash while hidden can leave the user without a taskbar; there is no supported state/notification contract. **Do not use registry `StuckRects3` or similar Explorer state edits**; they are undocumented, persistent, and can make recovery harder.

Current implementation: **default mode does not hide the taskbar**. The opt-in experiment now
includes a standalone `prism-recovery.exe` (runnable from `Win+R`/Task Manager or manually), a
heartbeat/watchdog that restores after a crash or freeze, a marker recording original visibility,
startup stale-marker cleanup, and `ShowWindow(SW_SHOW)` restoration for both taskbar classes.
Explorer is never restarted or terminated. The feature remains disabled by default and blocked for
enablement until tested on a disposable Windows 11 VM; real Explorer restart, secondary-taskbar,
and reboot behavior are still unverified. [R12]

**Reliability:** high for a plain overlay; medium for AppBar reservation across DPI/multi-monitor/fullscreen; low/unsafe for taskbar hiding.

## 6. DPI and multi-monitor correctness

**Minimum OS:** `GetDpiForWindow` is Windows 10 1607+; PerMonitorV2 is Windows 10 1703+. Both are available on Windows 11. **Fallback:** clamp to the nearest monitor work area and use the primary monitor if a surface cannot be placed.

Use **PerMonitorV2** awareness. Set it in the application manifest (`PerMonitorV2`) or call `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` before any HWND exists. Microsoft recommends the manifest; once awareness is set, later calls fail with `ERROR_ACCESS_DENIED`. [R13]

**winit/Slint pitfall:** winit's Windows backend already calls `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)` during initialization (falling back to PMv1); Slint's default winit backend therefore makes the process DPI-aware before your code creates a window. Do not call `SetProcessDpiAwarenessContext` after Slint initialization and expect success; use a manifest for a guarantee and treat winit's call as a no-op/fallback. [R15]

- `GetDpiForWindow(hwnd)` returns the effective DPI for a window; use it for the dock surface. `GetDpiForMonitor` is explicitly **not DPI-aware** and should not be used from a per-monitor-aware thread. [R13]
- `WM_DPICHANGED` supplies a suggested rectangle; honor it to avoid recursive DPI-change cycles. winit handles it; Slint dispatches `ScaleFactorChanged`. Do not override the backend with hardcoded sizes. [R13][R15]
- `Slint::Window::set_position` and `set_size` use **physical screen pixels** (position includes the frame); Slint UI coordinates are logical. Convert with `Window::scale_factor()`. [R15]
- winit `outer_position` is physical and may be negative; monitor coordinates are a virtual-screen space with an arbitrary origin, not `(0,0)`. [R15]
- `GetMonitorInfo` `rcMonitor`/`rcWork` are physical virtual-screen rectangles in a PMv2 process; derive logical sizes per monitor, not from a global 96-DPI assumption. [R13][R14]
- `EnumDisplayMonitors` includes pseudo-monitors associated with mirroring drivers; filter/handle them, and use `MONITORINFOF_PRIMARY` plus the device name for identity. `MonitorFromWindow` uses the largest intersection and, for minimized windows, the pre-minimize rectangle; use `MONITOR_DEFAULTTONEAREST` for deterministic fallback. [R14]
- `WM_DISPLAYCHANGE` reports resolution changes only; work-area changes arrive via `WM_SETTINGCHANGE` (`SPI_SETWORKAREA`). Re-enumerate on both and keep a bounded reconciliation timer. [R14]
- `GetWindowRect` is DPI-virtualized and may include invisible borders; DWM extended frame bounds are not DPI-adjusted. Use DWM bounds for previews and convert explicitly. [R14]
- Do not span one window across monitors with different scale factors; use one surface per monitor. Otherwise one monitor's DPI is applied to the whole surface and content on the other monitor has the wrong physical size. [R13]

Test: 100/125/150/200%, mixed-DPI pair, primary on left/right/top, portrait, negative coordinates, disconnect/reconnect, resolution change while open, RDP, primary change across sign-out/sign-in.

**Reliability:** high with PMv2 and one surface per monitor; medium on mixed-DPI hotplug, monitor removal, and RDP; geometry bugs are user-visible immediately.

## 7. Core Audio: volume, mute, meters, and device changes

`IMMDeviceEnumerator` enumerates endpoint devices; `EnumAudioEndpoints` returns an `IMMDeviceCollection` of `IMMDevice` objects, and `GetDefaultAudioEndpoint(eRender, role, ...)` returns the default render endpoint for `eConsole`, `eMultimedia`, or `eCommunications`. Activate `IAudioEndpointVolume`/`IAudioMeterInformation` on the chosen `IMMDevice`. Roles are independent and may all point at one device; query the role that matches the UI and do not assume they differ. Minimum: Core Audio is Vista+; no special permission is required for normal endpoint volume/mute. [R16]

- `IAudioEndpointVolume::GetMasterVolumeLevelScalar`/`SetMasterVolumeLevelScalar` use normalized 0.0–1.0 values. Use scalar values for the slider, not dB; the dB range is driver-dependent. [R16]
- `GetMute`/`SetMute` control endpoint mute; `VolumeStepUp`/`VolumeStepDown` respect the user's configured step and are better for wheel/keyboard increments.
- Hardware volume/mute is used when available; otherwise the audio engine implements it in software. In exclusive mode, software volume may not affect the stream, but notifications still occur. [R16]
- `IAudioEndpointVolumeCallback::OnNotify` reports external changes; keep it nonblocking and never unregister/release the final reference inside it. [R16]
- `IAudioMeterInformation` is optional; a software meter reports 0.0 in exclusive mode. Poll only while the Control Center is visible. [R16]
- `IMMNotificationClient::OnDefaultDeviceChanged`/`OnDeviceRemoved`/`OnDeviceAdded`/`OnDeviceStateChanged` report endpoint changes; callbacks must be nonblocking and must not register/unregister or release the final MMDevice object inside a callback. [R16]
- `GetDefaultAudioEndpoint` can return `E_NOTFOUND`; hide/disable the slider and show an honest “no audio output” state. [R16]

**Default-device switching is not supported by the documented Core Audio API.** Microsoft publishes no default-endpoint setter in the Core Audio reference. Microsoft Q&A answers point to `IPolicyConfig`, an interface with no published header/documentation in the Core Audio API set; it has changed across releases and must not be a product dependency. [R17] Fallback: show the device list read-only and open `ms-settings:sound` or `ms-settings:sound-devices`. [R17]

**Reliability:** high for volume/mute; medium for endpoint changes; unsupported for default switching. Initialize COM on a dedicated worker, marshal to the UI, and never block the UI on enumeration or volume writes. [R16]

## 8. Radios, Wi-Fi, and Bluetooth

**Radios.** `Radio.GetRadiosAsync()` enumerates radios; enumeration/state reading are documented as always allowed. Minimum: `Windows.Devices.Radios` is Windows 10 10240+. `RequestAccessAsync()` requests permission to **change** radio state and may prompt; `SetStateAsync` returns `RadioAccessStatus` and is subject to permission, hardware switches, and policy. `RadioState.Disabled` is reserved for the OS and cannot be set. Observe `StateChanged` and re-read `State`; a hardware kill switch can override an “Allowed” request. [R18]

The API metadata lists `radios`/`cellularDeviceControl` capabilities for `SetStateAsync`; capability declarations are a packaged-app manifest concept. How they apply to an **unpackaged** Prism build is not documented: test `GetRadiosAsync`/`RequestAccessAsync`/`SetStateAsync` from the actual unsigned/unpackaged binary before claiming a toggle. [R18]

**Wi-Fi.** `WlanOpenHandle` → `WlanEnumInterfaces` → `WlanQueryInterface(current_connection)` gives interface/SSID/signal/connection attributes; `WlanGetAvailableNetworkList` lists visible networks. Minimum: WLAN API is Vista+. `WlanConnect` requires a profile (or discovery parameters); `WlanSetProfile` requires profile XML and permissions; `WlanDisconnect` requires execute access on an all-user profile; `WlanRegisterNotification` delivers notifications (do not register/close inside the callback); free memory with `WlanFreeMemory` and close with `WlanCloseHandle`. [R19]

**Location consent is the constraint.** Since the fall 2024 behavior change, `WlanGetAvailableNetworkList`, `WlanGetNetworkBssList`, `WlanQueryInterface(current_connection)`, and `WlanScan` return `ERROR_ACCESS_DENIED` without precise-location consent. The system may show a one-time per-app prompt in the user's context and outside `System32`. `WlanRegisterNotification` with `WLAN_NOTIFICATION_SOURCE_MSM` also requires `wiFiControl` + location consent. [R20]

Recommendation: read connectivity without SSID via `NetworkInformation.GetInternetConnectionProfile()`/`GetNetworkConnectivityLevel()` and `NetworkStatusChanged`; if SSID is needed, prefer `WlanConnectionProfileDetails.GetConnectedSsid()` and handle denial with `ms-settings:privacy-location`; never log SSIDs; do not build a Wi-Fi picker in V1—open `ms-settings:network-wifi`; do not scan on a timer. [R17][R20]

**Bluetooth.** Status is `Radio.Kind == Bluetooth` + `State`; toggling uses the same permission/policy path. A paired-device inventory can use `DeviceInformation`/`DeviceWatcher`; pairing, removal, and their consent prompts should use native Settings/Quick Settings in V1, not a custom pairing stack. [R18]

**Reliability:** Wi-Fi/Bluetooth status medium (policy/hardware/location); radio toggles unverified unpackaged; network connect deferred.

## 9. Brightness

Two mechanisms; do not present them as one universal API.

**Internal/laptop panels (WMI).** `WmiMonitorBrightness` (`root\wmi`) exposes `CurrentBrightness`, `Levels`, `Level[]`; `WmiMonitorBrightnessMethods.WmiSetBrightness(Timeout, Brightness)` sets percentage. Vista+; support depends on panel/ACPI/driver; many desktops and some laptops do not expose them. [R21]

**External monitors (DDC/CI via Dxva2).** `GetPhysicalMonitorsFromHMONITOR` → `GetMonitorCapabilities` → `GetMonitorBrightness`/`SetMonitorBrightness` (or low-level VCP calls) use VESA MCCS over DDC/CI. Vista+; calls take about 40–50 ms; `MC_CAPS_BRIGHTNESS` indicates support. [R22]

Microsoft warns that many monitors do not fully implement MCCS, so these calls “might result in undefined monitor behavior” and should not be used on arbitrary monitors without physical validation. PowerToys documents that on a small number of monitors, reading a malformed DDC/CI capabilities string can trigger a Windows kernel bug and BSOD. [R22]

Therefore: no DDC/CI probe at startup; explicit opt-in; monitor exclusion list; cache capabilities; debounce slider writes 100–200 ms; WMI for internal panels; DDC/CI only for external monitors that pass capability checks; if neither works, hide/disable the slider and offer `ms-settings:display`. Never show a working-looking slider with no effect. HDR/SDR content brightness is a separate compositor concept; DDC/CI changes panel backlight, not necessarily Windows SDR-content brightness. This is **Unverified**; test HDR on/off and do not claim equivalence. [R17][R22]

**Reliability:** low/medium; hardware-dependent; the probe itself is a risk.

## 10. Media sessions (GSMTC)

`GlobalSystemMediaTransportControlsSessionManager.RequestAsync()` returns a manager; `GetSessions()` lists sessions and `GetCurrentSession()` returns the system's best guess. Sessions exist only for apps that integrated `SystemMediaTransportControls`; absence is normal. Minimum: Windows 10 1809 (build 17763). [R23]

- `SourceAppUserModelId` identifies the source app.
- `TryGetMediaPropertiesAsync()` returns title, artist, album title/artist, track number, genres, subtitle, playback type, thumbnail stream. [R23]
- `GetTimelineProperties()` returns position, start/end, min/max seek, `LastUpdatedTime`; interpolate position only while playing. [R23]
- `GetPlaybackInfo().Controls` exposes `IsPlayEnabled`, `IsPauseEnabled`, `IsPlayPauseToggleEnabled`, `IsNextEnabled`, `IsPreviousEnabled`, `IsStopEnabled`, etc.; disable/hide unsupported buttons. [R23]
- `TryPlayAsync`/`TryPauseAsync`/`TryTogglePlayPauseAsync`/`TrySkipNextAsync`/`TrySkipPreviousAsync`/`TryStopAsync`/`TryChangePlaybackPositionAsync` return success/failure; failure must reconcile UI. [R23]
- Events: manager `SessionsChanged`/`CurrentSessionChanged`; session `MediaPropertiesChanged`/`PlaybackInfoChanged`/`TimelinePropertiesChanged`. [R23]

**Capability caveat.** Microsoft lists `globalMediaControl`; capability declarations are a packaged-app manifest feature. Whether an unpackaged Prism build receives access is **not documented**. Test `RequestAsync` from the real unpackaged binary on Windows 11 24H2/25H2; if denied, package a helper with identity/capability or defer the card. Do not claim media support from a UWP sample. [R23]

UI: hide the card when there is no session; handle null thumbnail/missing artist/album; do not fabricate a track or progress; decode art off the UI thread; never persist metadata.

**Reliability:** medium; depends on app SMTC integration and capability.

## 11. Tray and notification limitations

`Shell_NotifyIcon` adds/modifies/deletes an icon in the taskbar status area **for the calling app**, identified by `hWnd` + `uID`/`GUID`. It is not an enumeration or ownership API. Microsoft documents no supported API to enumerate all third-party tray icons, read their menus, or take ownership. Minimum: `Shell_NotifyIcon` is XP+; `UserNotificationListener` is Windows 10 1607+. Windows 11's tray/overflow is Explorer-internal and its hierarchy is not a documented contract. [R24][R12]

`TaskbarCreated` is a registered message broadcast when Explorer creates/recreates the taskbar; apps re-add their own icons. This is the supported pattern for Prism-owned icons, not third-party icons. [R24]

Notification interception:

- `UserNotificationListener` can read other apps' toast notifications, but requires the `userNotificationListener` capability (packaged app), explicit user consent via `RequestAccessAsync` on a UI thread, and can be revoked; after revocation APIs silently return empty. Only toast notifications are supported. [R25]
- `ToastNotificationManager.History.GetHistory()` returns history **for the calling app only**. [R25]
- There is no documented URI to open the Windows 11 Notification Center. In the official URI lists we checked, only `ms-settings:` and app-specific schemes are documented; `ms-actioncenter:` is not. Do not ship an unverified `ms-actioncenter:` guess as the only entry point. [R17][R25]
- V1 fallback: Prism-owned status icons plus `ms-settings:notifications` for configuration. Do not fabricate a count or history. [R17]

**V1 decision:** third-party tray replication and arbitrary notification interception are **Deferred/Blocked**. Revisit only with a packaged helper, consent UX, privacy review, and evidence that the capability is acceptable.

**Reliability:** high for Prism-owned icons; low/unsupported for third-party tray enumeration and arbitrary notification capture.

## 12. Cross-compilation, CI, and window previews

- `cargo-xwin` cross-compiles to `x86_64-pc-windows-msvc` using the Microsoft CRT/Windows SDK and clang/lld. `rustup target add x86_64-pc-windows-msvc`, then `cargo xwin build --target x86_64-pc-windows-msvc`. It downloads Microsoft SDK/CRT bits under Microsoft's license; review that license for distribution. [R27]
- Native CI: `windows-latest` GitHub-hosted runner for MSVC build, unit tests, fmt, clippy. Runners are fresh VMs suitable for deterministic build/test, not interactive Explorer/DWM/taskbar validation. [R27]
- Cross-compilation proves compile/link, not runtime behavior. [R27]

**Minimum OS for previews:** DWM thumbnails are Vista+; `Windows.Graphics.Capture` is Windows 10 1803+; `PrintWindow` is XP+. **Fallback:** icon + title if thumbnail registration/capture fails.

**Why Wine cannot validate shell integration.** Wine is a compatibility layer that implements Windows API calls using Unix/X11/macOS equivalents; it is not Windows Explorer, DWM, the Windows 11 taskbar, the audio engine, or the radio/notification stack. `cargo xwin test` under Wine is useful for pure logic/unit tests, but says nothing about AppBar behavior, taskbar hiding recovery, DWM thumbnails, UWP hosting, Core Audio endpoints, radios, or notification consent. [R27]

| Preview option | What it gives | Limits | Use |
|---|---|---|---|
| `DwmRegisterThumbnail` | Live compositor thumbnail of a top-level source into a top-level destination owned by the caller | No pixels; destination must be ours; source top-level; minimized/cloaked/protected/UWP behavior not fully documented; no capture consent | **Primary hover preview** |
| `Windows.Graphics.Capture` | D3D11 frames with system picker consent and yellow border | User must pick item; consent/border; heavier; not per-hover | Explicit screenshot |
| `PrintWindow` | Synchronous copy into a DC; target processes `WM_PRINT` | Can block; may fail/black for hardware/UWP; privacy-sensitive | Last-resort static fallback, off by default |

DWM thumbnails are the best fit: the destination must be the desktop or owned by the calling process, preventing one app from drawing another app's content into a third window. `DwmUpdateThumbnailProperties` controls destination/visibility/opacity/source-client-area; `DwmQueryThumbnailSourceSize` gives aspect; unregister with `DwmUnregisterThumbnail` in the registering process. [R26] For Slint, obtain the destination HWND via `Window::window_handle()` (the `raw-window-handle-06` feature); it may only become available after the event loop has created the window. [R15]

No capture-consent UI is part of the DWM thumbnail path; content is composed only into Prism's preview surface. Treat that as a privacy responsibility: no persistence, no logging, hide on lock, no hidden/background capture. Test protected/DRM windows; black frames are acceptable degradation.

`Windows.Graphics.Capture` deliberately invokes secure UI for the user to pick the item and draws a yellow border; borderless requires `graphicsCaptureWithoutBorder` + explicit `GraphicsCaptureAccess.RequestAccessAsync` consent. It is the wrong tool for automatic hover previews. `PrintWindow` is synchronous, depends on the target processing `WM_PRINT`, and can make the UI unresponsive; never call it on the UI thread or per hover. [R26]

**Reliability:** high for DWM thumbnails on normally composed windows; medium/unknown for minimized, cloaked, UWP, and protected content; low for `PrintWindow` on hardware-accelerated/UWP targets.

## Risk register

| Priority | Risk | Evidence | Mitigation |
|---|---|---|---|
| **High** | Taskbar hiding strands user or breaks after Explorer update | Undocumented classes; Explorer-internal taskbar [R12] | No V1 hiding; independent recovery + watchdog + VM test |
| **High** | UWP identity/grouping/activation wrong (shared frame PID) | AUMID rules [R6]; frame observation [R28] | AUMID-first, child PID heuristic, never PID-only; UWP matrix |
| **High** | Foreground activation fails for elevated/UIPI targets | Foreground restrictions/UIPI [R9] | Click context; check return; `FlashWindowEx`; honest failure |
| **High** | Unpackaged WinRT capability breaks media/radio | `globalMediaControl` [R23], `radios` [R18] | Test real binary; package helper or defer |
| **High** | DDC/CI probe misbehaves/crashes monitor/system | MCCS warning + PowerToys BSOD [R22] | Opt-in; no startup probe; exclusion list; debounce |
| **High** | Wi-Fi SSID/scan fails or prompts unexpectedly | Location change [R20] | Status without SSID; consent UX; Settings fallback; no SSID logs |
| **High** | Users expect third-party tray/notification replication | Per-app `Shell_NotifyIcon` [R24]; packaged listener [R25] | Defer/block in V1; own icons only; communicate |
| **Medium** | Event storms/reentrancy corrupt state | Hook reentrancy/message loop [R5] | Queue + coalesce; no blocking; reconciliation |
| **Medium** | Mixed-DPI/multi-monitor offscreen/blurry | PMv2/physical coords [R13][R14]; Slint units [R15] | Per-monitor surfaces; physical math; scale events; matrix |
| **Medium** | AppBar reservation breaks floating design/maximized windows | Edge anchoring/outermost taskbar [R10] | No reservation in V1; opt-in strip later |
| **Medium** | Audio device switching unsupported / fake picker | No documented setter [R17] | Read-only list + `ms-settings:sound`; no `IPolicyConfig` |
| **Medium** | Brightness presented as universal | WMI vs DDC/CI split [R21][R22] | Per-monitor capability states; hide/disable; Settings |
| **Medium** | Explorer restart loses icons/state | `TaskbarCreated` [R24] | Re-add own icons; re-enumerate; re-register appbar |
| **Medium** | Preview capture blocks or leaks sensitive content | `PrintWindow` blocking [R26]; DWM/capture [R26] | DWM primary; no persistence; no UI-thread capture |
| **Low** | Empty titles/odd icons | `GetWindowTextW` [R3] | AUMID/exe fallback; keep icon; no crash |
| **Low** | Clock/time-zone/date rollover | Win32 time APIs | Low-frequency system-time refresh; DST tests |
| **Low** | Toolchain drift | cargo-xwin/MSVC/SDK [R27] | Pin versions; build Linux + native CI; record |

## Phase 3 validation checklist

Run on Windows 11 x64 24H2/25H2+, normal user; use a disposable VM for destructive/taskbar experiments.

- **Enumeration:** compare against Task View/Alt+Tab for Win32, UWP, packaged, multi-window, minimized, owned dialog, tool window, elevated, cloaked/virtual desktop; exclude shell/Prism/duplicate UWP; kill windows mid-enumeration; empty-title fallback.
- **Events:** create/destroy/show/hide/rename/foreground/minimize/restore/location within 150 ms; drag/resize storm no stall/dupes; lock/unlock, virtual desktop, Explorer restart, sleep/resume; hook failure falls back to polling.
- **Identity:** one app groups, two AUMIDs stay separate; UWP Settings/Calculator/Photos; Edge/Chrome/VS Code/Explorer/elevated; `APPMODEL_ERROR_NO_APPLICATION` fallback; record frame/child-PID behavior per build.
- **Activation:** background click activates; active click minimizes/chooser; minimized restore; multi-window choice; elevated failure state; log `SetForegroundWindow` result; `FlashWindowEx`; no hover focus steal; keyboard works.
- **Taskbar:** dock with bottom/top/left/right taskbar, autohide on/off, secondary monitor; maximized and fullscreen exclusive/borderless; optional AppBar reserve; **VM-only** hide/show primary+secondary, Explorer restart, Prism kill, reboot; independent recovery always restores; never auto-start.
- **DPI:** 100/125/150/200%, mixed-DPI, portrait, negative coords, primary change, disconnect/reconnect, RDP; onscreen after `WM_DPICHANGED`; Slint scale matches `GetDpiForWindow`; physical position/size correct.
- **Audio:** volume/mute read/write and external changes; default change/unplug/replug; no endpoint; communications role; exclusive-mode app; callback nonblocking; Settings fallback; no `IPolicyConfig`.
- **Radios/Wi-Fi/BT:** state read; toggle where permitted; hardware/policy denial; location prompt/denial; SSID/scan behavior; no SSID logs; Settings pages; record unpackaged radio capability.
- **Brightness:** internal WMI read/write; external DDC/CI read/write; unsupported monitor hides; HDR on/off; sleep/resume; no startup probe; malformed-capabilities monitor no crash.
- **Media:** session add/remove; play/pause/next/prev; metadata/art/timeline; unsupported controls disabled; null metadata; unpackaged `RequestAsync` result.
- **Tray/notifications:** own icon add/remove; `TaskbarCreated`; no third-party enumeration; packaged listener experiment only; own-app toast history; documented Settings entry point.
- **Previews:** DWM thumbnail Win32/UWP/minimized/occluded; mixed-DPI/multi-monitor; no stale/black frames; cleanup/no leaks; protected content degrades; no persistence/capture without consent.
- **Reliability/perf:** 8-hour soak, 100+ windows, rapid open/close, Explorer restart, Prism crash, config corruption, startup failure/no restart loop; measure idle CPU/GPU, startup, memory, preview cost; recovery tool from `Win+R`/Task Manager.

**Not tested yet:** everything above. Mark **Tested on Windows** only after execution on real hardware/VM.

## References

- [R1] [EnumWindows](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumwindows) · [IsWindowVisible](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-iswindowvisible) · [GetWindowLongPtrW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowlongptrw) · [Extended Window Styles](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles)
- [R2] [DwmGetWindowAttribute](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmgetwindowattribute) · [DWMWINDOWATTRIBUTE](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute)
- [R3] [GetAncestor](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getancestor) · [GetWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindow) · [GetWindowThreadProcessId](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid) · [GetClassNameW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclassnamew) · [GetWindowTextW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowtextw) · [IsIconic](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-isiconic) · [IsWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-iswindow)
- [R4] [The Taskbar](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar)
- [R5] [SetWinEventHook](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwineventhook) · [Event Constants](https://learn.microsoft.com/en-us/windows/win32/winauto/event-constants) · [Object Identifiers](https://learn.microsoft.com/en-us/windows/win32/winauto/object-identifiers) · [UnhookWinEvent](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-unhookwinevent)
- [R6] [QueryFullProcessImageNameW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew) · [GetApplicationUserModelId](https://learn.microsoft.com/en-us/windows/win32/api/appmodel/nf-appmodel-getapplicationusermodelid) · [Application User Model IDs](https://learn.microsoft.com/en-us/windows/win32/shell/appids)
- [R7] [SHGetPropertyStoreForWindow](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetpropertystoreforwindow) · [System.AppUserModel.ID](https://learn.microsoft.com/en-us/windows/win32/properties/props-system-appusermodel-id)
- [R8] [IApplicationActivationManager](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-iapplicationactivationmanager) · [ActivateApplication](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iapplicationactivationmanager-activateapplication) · [ShellExecuteExW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecuteexw) · [IShellLinkW](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ishelllinkw)
- [R9] [SetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow) · [AllowSetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-allowsetforegroundwindow) · [ShowWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow) · [AttachThreadInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-attachthreadinput) · [UIPI/UIAccess policy](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/security-policy-settings/user-account-control-allow-uiaccess-applications-to-prompt-for-elevation-without-using-the-secure-desktop) · [FlashWindowEx](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-flashwindowex) · [SetWindowPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos) · [GetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getforegroundwindow)
- [R10] [SHAppBarMessage](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shappbarmessage) · [Using Application Desktop Toolbars](https://learn.microsoft.com/en-us/windows/win32/shell/application-desktop-toolbars) · [ABM_GETTASKBARPOS](https://learn.microsoft.com/en-us/windows/win32/shell/abm-gettaskbarpos) · [ABM_GETSTATE](https://learn.microsoft.com/en-us/windows/win32/shell/abm-getstate)
- [R11] [SHQueryUserNotificationState](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shqueryusernotificationstate) · [QUERY_USER_NOTIFICATION_STATE](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ne-shellapi-query_user_notification_state)
- [R12] [FindWindowW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-findwindoww) · [Microsoft Q&A: taskbar window hierarchy](https://learn.microsoft.com/en-us/answers/questions/1139557/vb-net-windows-display-text-in-windows-taskbar) — Q&A usage, not documented API
- [R13] [High DPI Desktop Application Development](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows) · [SetProcessDpiAwarenessContext](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setprocessdpiawarenesscontext) · [GetDpiForWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getdpiforwindow) · [WM_DPICHANGED](https://learn.microsoft.com/en-us/windows/win32/hidpi/wm-dpichanged) · [GetDpiForMonitor](https://learn.microsoft.com/en-us/windows/win32/api/shellscalingapi/nf-shellscalingapi-getdpiformonitor) · [DPI_AWARENESS_CONTEXT](https://learn.microsoft.com/en-us/windows/win32/hidpi/dpi-awareness-context) · [Setting default DPI awareness](https://learn.microsoft.com/en-us/windows/win32/hidpi/setting-the-default-dpi-awareness-for-a-process)
- [R14] [EnumDisplayMonitors](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumdisplaymonitors) · [GetMonitorInfoW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmonitorinfow) · [MonitorFromWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-monitorfromwindow) · [WM_DISPLAYCHANGE](https://learn.microsoft.com/en-us/windows/win32/gdi/wm-displaychange) · [WM_SETTINGCHANGE](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-settingchange) · [GetWindowRect](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowrect)
- [R15] [winit Window](https://docs.rs/winit/latest/winit/window/struct.Window.html) · [winit Windows DPI source](https://docs.rs/winit/latest/src/winit/platform_impl/windows/dpi.rs.html) · [Slint Window](https://docs.rs/slint/latest/slint/struct.Window.html) · [Slint WinitWindowAccessor](https://docs.rs/slint/latest/slint/winit_030/trait.WinitWindowAccessor.html)
- [R16] [IMMDeviceEnumerator](https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nn-mmdeviceapi-immdeviceenumerator) · [IMMDeviceCollection](https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nn-mmdeviceapi-immdevicecollection) · [GetDefaultAudioEndpoint](https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nf-mmdeviceapi-immdeviceenumerator-getdefaultaudioendpoint) · [Device Roles](https://learn.microsoft.com/en-us/windows/win32/coreaudio/device-roles) · [IAudioEndpointVolume](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudioendpointvolume) · [IAudioEndpointVolumeCallback](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudioendpointvolumecallback) · [IAudioMeterInformation](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudiometerinformation) · [IMMNotificationClient](https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nn-mmdeviceapi-immnotificationclient)
- [R17] [Microsoft Q&A: default audio endpoint / IPolicyConfig](https://learn.microsoft.com/en-us/answers/questions/1162167/how-to-find-audio-devices-that-are-connected) · [Launch Windows Settings / ms-settings URI](https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings)
- [R18] [Radio](https://learn.microsoft.com/en-us/uwp/api/windows.devices.radios.radio) · [Radio.RequestAccessAsync](https://learn.microsoft.com/en-us/uwp/api/windows.devices.radios.radio.requestaccessasync) · [Radio.SetStateAsync](https://learn.microsoft.com/en-us/uwp/api/windows.devices.radios.radio.setstateasync) · [RadioAccessStatus](https://learn.microsoft.com/en-us/uwp/api/windows.devices.radios.radioaccessstatus) · [DeviceInformation](https://learn.microsoft.com/en-us/uwp/api/windows.devices.enumeration.deviceinformation) · [App capability declarations](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/app-capability-declarations)
- [R19] [WlanOpenHandle](https://learn.microsoft.com/en-us/windows/win32/api/wlanapi/nf-wlanapi-wlanopenhandle) · [WlanEnumInterfaces](https://learn.microsoft.com/en-us/windows/win32/api/wlanapi/nf-wlanapi-wlanenuminterfaces) · [WlanQueryInterface](https://learn.microsoft.com/en-us/windows/win32/api/wlanapi/nf-wlanapi-wlanqueryinterface) · [WlanGetAvailableNetworkList](https://learn.microsoft.com/en-us/windows/win32/api/wlanapi/nf-wlanapi-wlangetavailablenetworklist) · [WlanConnect](https://learn.microsoft.com/en-us/windows/win32/api/wlanapi/nf-wlanapi-wlanconnect) · [WlanDisconnect](https://learn.microsoft.com/en-us/windows/win32/api/wlanapi/nf-wlanapi-wlandisconnect) · [WlanSetProfile](https://learn.microsoft.com/en-us/windows/win32/api/wlanapi/nf-wlanapi-wlansetprofile) · [WlanRegisterNotification](https://learn.microsoft.com/en-us/windows/win32/api/wlanapi/nf-wlanapi-wlanregisternotification) · [WlanCloseHandle](https://learn.microsoft.com/en-us/windows/win32/api/wlanapi/nf-wlanapi-wlanclosehandle) · [WlanFreeMemory](https://learn.microsoft.com/en-us/windows/win32/api/wlanapi/nf-wlanapi-wlanfreememory)
- [R20] [Changes to API behavior for Wi-Fi access and location](https://learn.microsoft.com/en-us/windows/win32/nativewifi/wi-fi-access-location-changes) · [WlanConnectionProfileDetails.GetConnectedSsid](https://learn.microsoft.com/en-us/uwp/api/windows.networking.connectivity.wlanconnectionprofiledetails.getconnectedssid) · [NetworkInformation](https://learn.microsoft.com/en-us/uwp/api/windows.networking.connectivity.networkinformation)
- [R21] [WmiMonitorBrightness](https://learn.microsoft.com/en-us/windows/win32/wmicoreprov/wmimonitorbrightness) · [WmiMonitorBrightnessMethods](https://learn.microsoft.com/en-us/windows/win32/wmicoreprov/wmimonitorbrightnessmethods) · [WmiSetBrightness](https://learn.microsoft.com/en-us/windows/win32/wmicoreprov/wmisetbrightness-method-in-class-wmimonitorbrightnessmethods)
- [R22] [GetPhysicalMonitorsFromHMONITOR](https://learn.microsoft.com/en-us/windows/win32/api/physicalmonitorenumerationapi/nf-physicalmonitorenumerationapi-getphysicalmonitorsfromhmonitor) · [GetMonitorCapabilities](https://learn.microsoft.com/en-us/windows/win32/api/highlevelmonitorconfigurationapi/nf-highlevelmonitorconfigurationapi-getmonitorcapabilities) · [GetMonitorBrightness](https://learn.microsoft.com/en-us/windows/win32/api/highlevelmonitorconfigurationapi/nf-highlevelmonitorconfigurationapi-getmonitorbrightness) · [SetMonitorBrightness](https://learn.microsoft.com/en-us/windows/win32/api/highlevelmonitorconfigurationapi/nf-highlevelmonitorconfigurationapi-setmonitorbrightness) · [GetVCPFeatureAndVCPFeatureReply](https://learn.microsoft.com/en-us/windows/win32/api/lowlevelmonitorconfigurationapi/nf-lowlevelmonitorconfigurationapi-getvcpfeatureandvcpfeaturereply) · [PowerToys Power Display](https://learn.microsoft.com/en-us/windows/powertoys/power-display)
- [R23] [GSMTC Session Manager](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager) · [GSMTC Session](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssession) · [Playback Info](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionplaybackinfo) · [Media Properties](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmediaproperties) · [Timeline Properties](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessiontimelineproperties) · [Playback Controls](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionplaybackcontrols)
- [R24] [Shell_NotifyIconW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyiconw) · [The Taskbar](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar)
- [R25] [Notification listener](https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/notification-listener) · [UserNotificationListener](https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications.management.usernotificationlistener) · [ToastNotificationHistory.GetHistory](https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications.toastnotificationhistory.gethistory) · [Launch default app for a URI](https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-default-app)
- [R26] [DwmRegisterThumbnail](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmregisterthumbnail) · [DwmUpdateThumbnailProperties](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmupdatethumbnailproperties) · [DwmQueryThumbnailSourceSize](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmquerythumbnailsourcesize) · [DwmUnregisterThumbnail](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmunregisterthumbnail) · [PrintWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-printwindow) · [Screen capture](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture) · [GraphicsCaptureSession.IsBorderRequired](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession.isborderrequired)
- [R27] [cargo-xwin](https://github.com/rust-cross/cargo-xwin) · [rustup cross-compilation](https://rust-lang.github.io/rustup/cross-compilation.html) · [GitHub-hosted runners](https://docs.github.com/en/actions/using-github-hosted-runners/about-github-hosted-runners) · [Wine README](https://github.com/wine-mirror/wine)
- [R28] [CoreWindow](https://learn.microsoft.com/en-us/uwp/api/windows.ui.core.corewindow) · [Microsoft Q&A: UWP frame/CoreWindow observation](https://learn.microsoft.com/en-us/answers/questions/219889/bring-uwp-app-window-to-front-and-move-it) — observed behavior, not a documented contract
