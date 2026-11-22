# Architecture Decision Records — Prism Shell

Status labels: Proposed, Approved, Implemented, Tested on Linux, Tested on Windows, Deferred,
Blocked. Dates are 2026-11-21 unless noted.

## D-001 — Rust stable as the primary language

**Status: Approved; Implemented; Tested on Linux.**

Use Rust stable (1.100.0 in this environment, edition 2024) for the shell host, domain logic,
platform abstractions, and preview. Win32/COM/WinRT interop will use `windows-rs` inside a future
`prism-windows` crate. Keep `unsafe` out of portable crates (`unsafe_code = forbid` in
`prism-core` and `prism-platform`); confine platform unsafe blocks to the Windows host.

**Why:** memory safety, no GC, small binaries, strong workspace boundaries, excellent FFI, one
language for Linux preview and Windows production. **Cost:** Win32/COM lifetimes and async WinRT
interop remain complex; the ecosystem is smaller than C#/C++ for Windows UI.

## D-002 — Slint 1.19 for the V1 UI, with a renderer/host escape hatch

**Status: Approved for prototype; Implemented; Tested on Linux. Windows surface Unverified.**

Choose Slint because it provides a Rust-native declarative UI, GPU rendering, AccessKit,
transparent/frameless/always-on-top window properties, multi-window support, raw HWND access, and
`take_snapshot` for testing. Keep Win32 calls out of the UI. Validate transparent, non-activating
windows on real Windows 11 before committing to production. Keep egui as the fallback toolkit.

**Rejected:** Tauri/WebView2 (memory/runtime/native-Rust concerns), Qt (C++/deployment/licensing),
Avalonia/WinUI (not Rust), custom Win32 renderer (cost/risk), iced (less mature layered surfaces).
Full comparison: `docs/UI_FRAMEWORK_EVALUATION.md`.

**Open risk:** Windows transparency with FemtoVG/OpenGL and DWM; NVIDIA's OpenGL GDI compatibility
can break alpha. Phase 3 must test and may switch to Skia/wgpu or a solid fallback.

## D-003 — Crate boundaries

**Status: Implemented.**

`prism-core` (portable domain, no I/O), `prism-platform` (traits + mock), `prism-ui` (Slint,
tokens, assets, view mapping), `prism-app` (lifecycle, CLI, config, logging, controller), and a
future `prism-windows` (Win32/DWM/WinRT host). Do not scatter `cfg(windows)` or Win32 calls
through UI code.

**Why:** testability, a Linux preview that shares the real UI, and the ability to validate a
Windows host without rewriting presentation.

## D-004 — One preview stage, separate production surfaces

**Status: Implemented for the preview; production model Approved/Proposed.**

The Linux preview composites the taskbar, Control Center, launcher, previews, and a static mock
desktop into one borderless stage. This is a development harness for visual iteration and
deterministic screenshots, clearly labeled. Production will use separate top-level surfaces so the
taskbar/panels can be independently positioned, non-activating, and multi-monitor aware.

**Why not jump straight to separate windows?** The stage gives a single reproducible screenshot
and avoids premature Linux window-manager complexity. **Risk:** production multi-window focus and
z-order still need Windows validation.

## D-005 — Explicit mock backend; fail-safe Windows bootstrap

**Status: Mock backend Implemented/Tested on Linux. The fail-safe bootstrap is superseded by
D-021; the no-silent-mock principle remains.**

`MockPlatform` owns every preview system value and labels simulated actions. The original
`prism-shell.exe` milestone did not run mock data as production: it showed a native "integration
not available" message and exited. Phase 3A replaces that entry point with the real native shell;
`prism-preview` remains explicitly mock-only, and the Windows path never falls back to it.

**Why:** honest behavior, safe default, and a clean seam for the native backend.

## D-006 — No taskbar hiding, no Explorer modification, no auto-start in V1

**Status: Approved; Implemented as a prohibition. Deferred/Blocked for future work.**

Do not call `ShowWindow` on `Shell_TrayWnd`, modify `StuckRects3`, replace Explorer, register a
shell, or add auto-start. The taskbar window classes are undocumented Explorer internals and a
crash while hidden can strand the user. Future hiding requires an independent recovery executable,
watchdog, tested restore, explicit opt-in, and owner approval. This is a safety-critical decision,
not an implementation detail.

## D-007 — Floating overlay without AppBar reservation in V1

**Status: Proposed (implementation deferred).**

`SHAppBarMessage` is not a taskbar replacement, and its edge-anchored rectangle cannot exactly
represent a floating rounded dock. V1 runs as an overlay without reserving work area. Maximized
and fullscreen windows may overlap; Phase 6 will validate a separate opt-in reserved strip if
needed.

## D-008 — No undocumented default-audio-device switching

**Status: Approved. Deferred.**

Documented Core Audio supports volume/mute/notifications, not selecting a default endpoint.
`IPolicyConfig` is undocumented and version-sensitive. V1 shows a read-only device list and opens
the native sound settings/output selector. Do not silently depend on `IPolicyConfig`.

## D-009 — Brightness is capability-gated and never probed blindly

**Status: Proposed; mock states implemented; real integration deferred.**

Internal panels use WMI only where supported; external monitors use DDC/CI only with explicit
opt-in and capability checks. No startup DDC/CI probe (MCCS misbehavior and a documented
kernel-crash risk on malformed capability strings). Hide/disable the slider and offer Display
settings when neither path is validated. Never show a working-looking slider with no effect.

## D-010 — Wi-Fi/Bluetooth status first, native Settings for changes

**Status: Proposed; mock states implemented; real integration deferred.**

Radio/WLAN APIs can read state; changing radios requires consent/policy and unpackaged-app
behavior is unverified. V1 shows status and uses the native Wi-Fi/Bluetooth surfaces for connect,
pair, and toggles that are not proven. Do not log SSIDs. Handle location-consent denial.

## D-011 — Media via GSMTC only after unpackaged capability validation

**Status: Proposed; rich mock card implemented; real integration deferred.**

Use `GlobalSystemMediaTransportControlsSessionManager` only when the real unpackaged binary is
allowed to call it. Respect playback-control capability flags, handle missing metadata/artwork,
and never persist or log media metadata. If access is denied, hide the card or package a
capability-bearing helper after a separate decision.

## D-012 — Tray replication and arbitrary notification capture are out of V1

**Status: Blocked/Deferred.**

`Shell_NotifyIcon` is per-application and cannot enumerate third-party icons. `UserNotificationListener`
requires packaged capability + explicit consent and can be revoked; toast history is per-app.
V1 shows Prism-owned status icons and opens native settings. No fabricated notification counts or
history.

## D-013 — Configuration is versioned and never overwritten

**Status: Implemented; Tested on Linux.**

`PrismConfig` has a version, bounded pins, validation, and safe defaults. Missing config is fine;
corrupt/invalid config is rejected as a whole, logged, and replaced in memory. The file is not
silently rewritten. This prevents a partial upgrade from destroying user intent.

## D-014 — Screenshot strategy: Slint snapshot + virtual display, not a desktop-only check

**Status: Implemented; Tested on Linux.**

Use `Window::take_snapshot` for deterministic UI-only PNGs and rootless Xvfb + ffmpeg for
compositor-level captures. Force `--scale` for reproducible logical-size screenshots because winit
may infer a different scale on a given X server. Do not treat a Linux screenshot as Windows
integration evidence.

## D-015 — DPI/multi-monitor strategy

**Status: Proposed; logical layout tested on Linux.**

Use PerMonitorV2, `GetDpiForWindow`, `WM_DPICHANGED`, `EnumDisplayMonitors`, and work areas in the
Windows host. One surface per monitor; never span mixed-DPI monitors with one window. Slint UI is
logical; the host converts physical screen geometry using the scale factor. Mixed-DPI/RDP/hotplug
remain unverified.

## D-016 — Design-token discipline and no custom theme engine in V1

**Status: Implemented.**

All colors, radii, typography, spacing, and motion timings live in `tokens.slint`. Components do
not invent one-off styling. V1 has one design; no theme editor, plugin marketplace, or scripting
system. The structured token/state boundary is the future extension seam.

## D-017 — Activation plan is honest about foreground restrictions

**Status: Proposed (Windows).**

Use the user-click context and `SetForegroundWindow`, check its return, and use `FlashWindowEx` on
failure. Do not loop, attach thread input as a primary hack, or force topmost as a focus bypass.
Elevated/UIPI targets may be limited; display that honestly. Multi-window apps use the preview
chooser rather than minimizing unrelated windows.

## D-018 — No automatic startup in V1

**Status: Approved. Deferred.**

Do not register Prism as a startup app or system shell. Recovery behavior must be verified first.
The preview is launched manually; the Windows bootstrap is a diagnostic message box. Any future
startup opt-in requires recovery validation and owner approval.

## D-019 — Licensing (unresolved, non-blocking for the prototype)

**Status: Proposed; requires owner decision before distribution.**

Slint is available under GPLv3 or a royalty-free license with attribution requirements. The
repository currently has no chosen product license. Inter is bundled under OFL
(`assets/fonts/OFL.txt`); Lucide icons are ISC (`assets/icons/LICENSE-lucide.txt`); Prism-designed
app icons and mark are original project assets. Resolve the Slint option, attribution placement,
and repository license before shipping binaries.

## D-020 — Build/validation strategy

**Status: Implemented/Tested on Linux.**

Use Cargo workspace + `cargo-xwin` for the MSVC cross-build and native Windows CI later. Treat
cross-compilation as compile/link evidence only. Validate Explorer/DWM/taskbar/Core Audio/radios/
media/recovery on a real Windows 11 VM or physical machine. Wine may supplement pure-logic tests
but cannot validate shell behavior. Do not push or deploy from this environment.

## D-021 — Phase 3A native host: overlay surfaces, no shell takeover

**Status: Implemented; successfully cross-compiled; Windows runtime Unverified.**

Build the first native Windows shell as three ordinary top-level Slint windows hosted by documented
Win32/DWM calls:

- A full-work-area-width, 100-logical-px taskbar window with a centered dock, `WS_EX_TOOLWINDOW |
  WS_EX_TOPMOST | WS_EX_NOACTIVATE`, `MA_NOACTIVATE`, and `HTTRANSPARENT` outside the dock.
- Separate Control Center and launcher tool windows with transparent shadow margins, real focus,
  Escape/focus-loss dismissal, and click-through margins.
- Real `EnumWindows` filtering, per-executable grouping, `SHGetFileInfoW` icons, and
  `ShowWindow`/`SetForegroundWindow` activation with `FlashWindowEx` fallback.
- A deliberately limited Control Center: real clock/battery/Settings/lock/exit; audio, radios,
  brightness, and media are unavailable, not simulated.

Do **not** add AppBar work-area reservation, taskbar hiding, Explorer replacement, registry
changes, auto-start, or an independent recovery executable in this phase. The dock is a primary-
monitor overlay and may cover the bottom of maximized windows. Deeper integration requires the
recovery prerequisites in D-006 and real Windows validation first.

This keeps the shell useful and safe without implying that an overlay is a complete taskbar
replacement. Portable layout/filtering modules are unit-tested on Linux; Win32 behavior is not.

