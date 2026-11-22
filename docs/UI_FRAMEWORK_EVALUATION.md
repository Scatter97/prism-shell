# UI Framework Evaluation — Prism Shell V1

Status: Evaluation complete for the V1 prototype. Recommendation: **Slint 1.19** for the first
implementation, with the presentation layer isolated so the window host and renderer can be
replaced if Windows validation disproves a critical assumption.

Date: 2026-11-21. Target: Windows 11 x64. Development host: Linux (KDE Wayland + rootless Xvfb).

This is an engineering evaluation, not a claim that every candidate was built into a working
taskbar. The prototype in this repository is the evidence for the Slint column. Claims about the
alternatives are based on their documented architecture and known constraints and are labelled
accordingly.

## 1. Requirements that actually decide the choice

A desktop shell is not a normal application window. The framework must support, or leave a
documented escape hatch for:

1. Frameless, transparent, always-on-top surfaces.
2. Non-activating input (`WS_EX_NOACTIVATE` / `WM_MOUSEACTIVATE`) without fighting the toolkit.
3. Multiple independent top-level surfaces: taskbar, Control Center, launcher, previews.
4. Per-monitor DPI and physical screen coordinates.
5. GPU rendering with low idle cost and no continuous repaint loop.
6. Native Win32 window handles for DWM backdrop, thumbnails, and window styles.
7. AccessKit/Windows accessibility.
8. A Linux preview that uses the same UI code and supports deterministic screenshots.
9. A small, auditable dependency footprint (no bundled browser engine).
10. A credible answer to licensing and long-term maintenance.

Most failures happen at the boundary between the toolkit and Win32, not in the drawing API.
That is why the evaluation weights window ownership and escape hatches as heavily as visual
quality.

## 2. Options considered

| Option | Language / rendering | Linux preview | Windows native surface | Multi-window / transparency | Main strength | Main risk |
|---|---|---|---|---|---|---|
| **Slint 1.19** | Rust DSL + Rust; FemtoVG/OpenGL, Skia or software | Yes, winit backend | Yes, winit + Win32 HWND access | Multi-window since 1.7; `no-frame`, `background: transparent`, `always-on-top`; raw-window-handle/winit access | Declarative design system, GPU, AccessKit, tiny runtime, compile-time assets | Transparency path on Windows depends on renderer/driver; low-level Win32 styles require host code |
| **egui / eframe** | Rust immediate mode; wgpu/glow | Yes | Yes, winit viewports | Deferred viewports; transparency works but historically had Windows child-viewport bugs | Excellent GPU integration, fast iteration, strong screenshot/test tooling (`egui_kittest`) | Immediate mode repaints; premium custom widgets require a large bespoke paint layer; accessibility less complete |
| **iced** | Rust; wgpu | Yes | Yes | Multi-window support exists but is less mature; transparency/effects are limited | Elm-style state, pure Rust, good docs | More work for a dock-style layered surface; fewer escape hatches |
| **Tauri 2 + WebView2** | Rust host + HTML/CSS/JS UI | Yes (WebKitGTK) | Yes | Transparent/decorless/always-on-top/skip-taskbar windows are configurable | Best visual iteration and CSS effects; web ecosystem | WebView2 runtime, per-window web process, memory and startup cost; UI is not Rust-native; policy and audit surface is larger |
| **Qt 6 / cxx-qt** | C++ Qt + Rust bridge | Yes | Yes | Mature window flags, transparency, DWM integration | Mature widgets, accessibility, multi-monitor | C++ toolchain, deployment, licensing, Rust/C++ boundary complexity; cross-compiling from Linux is painful |
| **Avalonia / WinUI 3** | .NET / C# (WinUI also C++) | Partial / no | Yes | Mature on Windows | Fast Windows UI development | Abandons Rust as the primary language; packaging/runtime constraints |
| **Custom Win32 + Direct2D/D3D11/Skia** | Rust + Win32 | Separate Linux renderer needed | Maximum control | Full control | No toolkit mismatch; smallest possible shell surface | Highest engineering cost and risk; every state, animation, accessibility, text and input primitive must be built |

Rejected early: Flutter (not Rust, heavyweight engine), Electron (not native, heavy), a pure
Win32-only UI (no credible Linux preview and much higher cost), and a web UI inside a native
host (Tauri) because V1 prioritises a native, low-footprint shell and Rust ownership.

## 3. Detailed comparison

### 3.1 Slint

**Advantages**

- The UI is declarative and compiled to Rust, so design tokens, layout, states, and transitions
  live in one maintainable system. The prototype's entire visual language is in `tokens.slint`.
- GPU-accelerated FemtoVG by default, with Skia and software renderer paths. A software renderer
  makes deterministic screenshots and CI possible.
- AccessKit accessibility is a default feature (Slint 1.19).
- `Window` exposes `no-frame`, `always-on-top`, and `background`; `Window::window_handle()` and
  the winit accessor provide the HWND needed for documented Win32/DWM configuration.
- Multiple top-level components are supported (since 1.7). The preview currently uses one stage,
  but the plan is separate taskbar/Control Center surfaces.
- `Window::take_snapshot()` returns a pixel buffer. We verified a 1920×1080 PNG capture and a
  1600×1000 capture at an explicit scale factor.
- Images and fonts are embedded at compile time (`@image-url`, font imports), so there are no
  runtime asset paths to break in a packaged Windows build.
- The dependency graph is small compared with a browser engine.

**Disadvantages / uncertainties**

- Transparent windows on Windows combine Slint's renderer with DWM composition and the graphics
  driver. The repository's reference pattern (Slint + Mica template) documents an NVIDIA OpenGL
  "GDI compatibility" issue where transparent regions can render black; a wgpu/Skia or software
  fallback may be needed. **Not validated on Windows yet.**
- Slint does not expose every Win32 window style as a first-class property. The production host
  must apply `WS_EX_NOACTIVATE`, `WS_EX_TOOLWINDOW`, `WM_MOUSEACTIVATE`, and DWM attributes to
  the HWND after creation. This is a bounded amount of documented Win32 code, not a blocker, but
  it must be tested per renderer.
- Effects are 2D and intentionally limited. Backdrop blur must come from DWM (Mica/Acrylic) or a
  composition layer, not from arbitrary in-app shaders. The preview therefore uses alpha
  compositing and a "visually equivalent" translucent surface, not true blur.
- The royalty-free license requires attribution; GPLv3 is the other open-source option. This is a
  product/legal decision that must be made before distribution (see DECISIONS.md).
- A toolkit swap would require rewriting the `.slint` presentation layer, but the core,
  platform traits, state model, and Windows host would survive. The risk is contained.

**Windows compatibility:** compile target is `x86_64-pc-windows-msvc`; winit is the windowing
backend. Win32/DWM escape hatches are available through the raw HWND. Runtime transparency,
non-activation, multi-window z-order, and DWM backdrop remain **Unverified**.

**Linux compatibility:** verified with the winit/X11 backend under Xvfb and on KDE Wayland for
interactive checks. `SLINT_SCALE_FACTOR` is supported by the winit adapter; we use it for the DPI
matrix and expose `--scale`.

**Performance:** Slint is dependency-driven; a property that does not change does not force a
repaint. The preview has a 1 s clock/media timer (clock writes only when the minute changes) and
single-shot timers for hover/pending/toast. No benchmark on Windows has been run. FemtoVG uses
OpenGL; Skia/wgpu alternatives need measurement.

**Testing:** `take_snapshot` is a real offscreen capture path; the preview's `--screenshot` mode
was executed. Interaction testing was performed with `xdotool` against the Xvfb window.

### 3.2 egui / eframe

**Advantages:** easiest custom painting, excellent wgpu backend, mature multi-viewport API,
`egui_kittest` snapshot testing, no DSL. A shell could be built quickly.

**Disadvantages:** immediate-mode means the UI normally repaints continuously unless carefully
gated; a premium design system (typography, layered surfaces, transitions, focus rings) is a large
bespoke painting layer; text shaping and accessibility are improving but less complete than
AccessKit-in-Slint for this use case. Multi-viewport transparency on Windows has a history of
bugs. A serious candidate, but a worse fit for a design-token-driven product UI.

**Recommendation:** keep as the fallback if Slint's Windows transparency or multi-window host
fails validation. The core/platform boundaries would still apply.

### 3.3 iced

**Advantages:** pure Rust, wgpu, clean architecture, good documentation. Good for conventional
application windows.

**Disadvantages:** transparency, layered surfaces, non-activating windows, and multi-window
orchestration are less mature. The taskbar/Control Center interaction model would require more
custom infrastructure than Slint or egui. Not selected.

### 3.4 Tauri 2 + WebView2

**Advantages:** CSS makes the visual layer fast to iterate; transparent, frameless, always-on-top,
skip-taskbar windows are supported by the runtime; a web UI is easy to test with Playwright.

**Disadvantages:** a WebView2 runtime dependency and per-window web processes conflict with the
low-memory, low-idle, native-shell goal. Rust ownership of the UI is weakened. Network-capable
web content and a larger dependency surface increase the security review. Not selected for V1;
could be revisited for settings/theme editor surfaces later.

### 3.5 Qt / Avalonia / WinUI

Mature, but they either abandon Rust as the primary language or add a C++/.NET toolchain and
deployment layer that is disproportionate for a single polished shell. Qt's licensing and
cross-compilation burden are also significant. Not selected.

### 3.6 Custom Win32 renderer

Maximum control and no toolkit mismatch, but the cost is a full text, layout, animation,
accessibility, and input stack. It also makes the Linux preview a second implementation. Not
selected for V1; the platform abstraction leaves room for a custom host if needed.

## 4. Decision

Use **Slint 1.19** for V1, with these conditions:

1. Keep all Win32 calls in a `prism-windows` host layer. The UI and core crates must not depend on
   `windows` or HWND types.
2. Validate transparent, frameless, always-on-top, non-activating windows on real Windows 11
   before declaring the shell surface production-ready. Test FemtoVG first; if transparency is
   unreliable, evaluate Slint's Skia/wgpu renderer or a software fallback.
3. Keep the Control Center/taskbar as separate top-level surfaces in production, even though the
   Linux preview uses one stage. The preview stage is a development harness, not the production
   window model.
4. Do not use DWM blur as a correctness requirement. The UI must look correct with an opaque or
   alpha-composited fallback.
5. Resolve the Slint license/attribution decision before distributing binaries.

## 5. Remaining evaluation gaps

- Native Windows transparency/focus/z-order with the Slint renderer.
- DWM Mica/Acrylic as a backdrop for a frameless, transparent Slint window.
- Multi-window input/focus behavior for non-activating surfaces.
- GPU/CPU idle measurements on real Windows hardware.
- Accessibility tree verification with Narrator.
- Packaging and code signing.
