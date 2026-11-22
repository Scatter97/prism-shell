
# Prism Shell — V1 Project Specification

Version: 0.1
Status: Initial design baseline
Target OS: Windows 11 x64
Development OS: Linux
Primary implementation: Rust
Product category: Native desktop shell enhancement

---

# 1. Executive Summary

Prism Shell is a modern, lightweight alternative desktop interface for Windows 11.

The initial release focuses on two integrated components:

1. A floating taskbar and application dock.
2. A cohesive Control Center for system interactions.

Prism Shell should feel like an original desktop environment while retaining compatibility with Windows applications and services.

The default experience should work without extensive setup or configuration.

V1 is not a full theme editor or customization platform.

The architecture must preserve the possibility of extensive user customization in later versions.

---

# 2. Product Goals

## 2.1 Primary goals

- Deliver one polished desktop design.
- Offer a practical replacement experience for the visible Windows taskbar.
- Make pinned and running applications easy to access.
- Provide window switching and previews.
- Centralize frequently used system controls.
- Provide clear feedback for all supported system actions.
- Support multi-monitor systems.
- Remain performant and stable.
- Preserve access to the original Windows desktop.
- Support development on Linux.

## 2.2 Product principles

1. Reliability before novelty.
2. Real functionality before visual imitation.
3. Simplicity before unnecessary configurability.
4. Consistent interaction design.
5. Minimal background resource usage.
6. Graceful handling of unsupported Windows APIs.
7. Safe restoration of original Windows behavior.
8. Architecture that can evolve without a rewrite.

## 2.3 Non-goals for V1

- Full user-facing theme editor.
- Drag-and-drop customization studio.
- Community theme marketplace.
- Complete macOS or Linux desktop emulation.
- Plugin marketplace.
- Full desktop widget ecosystem.
- Complete Explorer replacement.
- Kernel or driver development.
- Modification of Windows system files.
- Unsupported process injection.
- Cross-device synchronization.
- Full notification interception.
- Guaranteed replication of every third-party tray icon.

These remain possible future research or development areas.

---

# 3. V1 Scope and Release Strategy

There are three implementation levels.

## 3.1 Required foundation

The application must:

- Open as a native Windows desktop application.
- Display a functional floating taskbar.
- Track eligible application windows.
- Launch applications.
- Activate and minimize windows.
- Display application icons.
- Display running and active states.
- Show basic window previews.
- Open and close the Control Center.
- Display actual time and system status.
- Provide supported volume control.
- Show connectivity information.
- Provide native Windows Settings fallbacks.
- Recover safely from failures.

## 3.2 Target enhancements

Target enhancements include:

- Application pinning.
- Multi-window preview management.
- Audio output device selection.
- Brightness control where supported.
- Bluetooth status.
- Battery status.
- Media session controls.
- More complete multi-monitor taskbar behavior.
- Consistent keyboard navigation.
- Additional power and utility actions.

Individual enhancements may be deferred if blocked by platform limitations.

## 3.3 Experimental and research features

- Native taskbar hiding strategies.
- Advanced Explorer coexistence.
- Third-party system tray replication.
- Native notification interception.
- Deep system control integrations.

These require evidence and explicit approval before use in production.

---

# 4. Visual Design Language

## 4.1 Overall appearance

The initial theme should be:

- Dark by default.
- Modern.
- Minimal.
- Frosted/translucent where supported.
- Rounded without excessive softness.
- Compact but comfortable.
- Visually polished.
- Distinct from standard Windows 11.

The interface should not resemble a bulky tablet interface.

## 4.2 Preliminary design tokens

These values are starting targets and must be visually tested.

Colors:

- Main taskbar surface: dark neutral with controlled transparency.
- Control Center surface: slightly more opaque than taskbar.
- Primary text: near white.
- Secondary text: muted cool gray.
- Accent: restrained blue or blue-violet.
- Hover surface: subtle light overlay.
- Active surface: stronger accent-tinted overlay.
- Borders: low-opacity neutral highlights.

Example starting palette:

- Background reference: #151924
- Elevated surface: #222838
- Primary text: #F5F7FA
- Secondary text: #A6AFBF
- Accent: #89A8FF
- Critical action: #F17C82

Do not hardcode these values throughout the application.

Centralize them as internal design tokens.

## 4.3 Typography

Preferred initial font:
Segoe UI Variable on supported Windows installations.

Fallback:
A suitable system sans-serif.

Design priorities:
- Clear hierarchy.
- Legible small labels.
- Consistent font weights.
- Proper text truncation.
- Correct DPI scaling.

## 4.4 Corners

Preliminary targets:

- Main taskbar radius: 18 px.
- Control Center radius: 20 px.
- Quick-control tile radius: 14 px.
- Preview card radius: 14 px.
- Hover background radius: 10 px.

## 4.5 Materials

Use:
- Translucent panels where supported.
- Subtle blur.
- Faint outlines.
- Gentle elevation shadows.
- Controlled accent highlights.

Avoid:
- Extremely transparent unreadable surfaces.
- Excessive glow.
- Overly saturated gradients.
- Unnecessary glass layers.
- Unbounded shader effects.

Implement solid fallback surfaces where material effects are unavailable.

---

# 5. Taskbar and Dock

## 5.1 Core design

The default taskbar is a single floating, rounded surface.

It is positioned near the bottom of the primary monitor.

Three functional zones:

LEFT:
- Launcher button.

CENTER:
- Pinned applications.
- Running applications.
- Active-window indicators.

RIGHT:
- System status.
- Audio.
- Connectivity.
- Battery when present.
- Clock/date.
- Control Center entry point.

The layout must remain visually balanced as applications open or close.

## 5.2 Preliminary measurements

At 100% DPI scaling:

- Target bar height: 60 px.
- Bottom screen gap: 14 px.
- Default application icon size: 32 px.
- Icon interaction target: approximately 44 px.
- Horizontal icon gap: 4–6 px.
- Internal horizontal padding: 10–14 px.
- Section divider thickness: 1 px.
- Running indicator height: 3 px.
- Hover scale: approximately 1.06–1.10.

These are logical layout targets, not raw physical-pixel assumptions.

Final values must be tested at:
- 100% scaling.
- 125% scaling.
- 150% scaling.
- 200% scaling.

## 5.3 Dock width

The bar may resize according to content.

It must not exceed safe available screen width.

When content becomes excessive:
- Preserve critical system controls.
- Preserve launcher access.
- Avoid overlapping sections.
- Offer a deterministic overflow strategy.
- Keep running applications accessible.

The center region must not push the right status region offscreen.

## 5.4 Launcher button

Use an original Prism Shell icon.

Do not use the Windows logo as the application identity.

Initial behavior:
- Click opens the launcher entry surface.
- Clicking again closes it.
- Escape closes the open launcher.

The full Start-menu replacement is outside the immediate taskbar and Control Center milestone.

For the first working vertical slice:
- The button may open a small functional application launcher.
- It must not be an inert placeholder.
- Opening the native Windows Start menu is an acceptable temporary fallback if documented.

Do not advertise full Start-menu replacement until implemented.

## 5.5 Application icons

Show actual application icons when available.

Use consistent visual framing and layout.

Do not arbitrarily replace app icons with generic symbols when reliable icons can be obtained.

Represent pinned and running applications consistently.

The app model should distinguish:
- Application identity.
- Launch target.
- Running process information.
- Associated windows.
- Pinned status.
- Current active window.

Handle missing icons gracefully.

## 5.6 Running indicators

States:

NOT RUNNING:
- No running indicator.

RUNNING:
- Small subdued indicator below the app icon.

ACTIVE:
- More prominent accent indicator.

MULTIPLE WINDOWS:
- Distinguishable multi-window state where practical.

Indicators must remain readable over different wallpapers.

Do not rely exclusively on color.

## 5.7 App interaction rules

Single click on a non-running pinned app:
- Launch the application.

Single click on a running inactive app:
- Activate the most relevant application window.

Single click on an active app:
- Apply the documented minimize behavior where possible.

For an app with multiple windows:
- Provide access to all eligible windows.
- Avoid unexpectedly hiding unrelated windows.

Right-click:
- Show a context menu with supported actions.

Initial context actions:
- Open.
- Show windows.
- Pin or unpin, once pinning exists.
- Close window where appropriate.

Dangerous or destructive actions must not be triggered accidentally.

## 5.8 Hover states

On hover:
- Show a subtle background tile.
- Apply restrained icon scaling.
- Display an application tooltip when useful.

If the application is running:
- Show a window preview after a short delay.

Avoid repeated preview creation as the pointer moves within the same icon.

## 5.9 Window previews

Previews open above the dock.

Each preview may include:
- Application icon.
- Window title.
- Window thumbnail.
- Close action.

For multiple windows:
- Show all eligible windows within a constrained preview surface.
- Allow selecting a specific window.
- Provide overflow or scrolling when necessary.

Initial motion target:
- Short fade.
- Slight vertical translation.
- No exaggerated scaling.

Thumbnails must degrade gracefully if unavailable.

Privacy-sensitive content must not be stored unnecessarily.

## 5.10 Auto-hide

V1 default:
Always visible.

Do not implement a complex auto-hide engine before core taskbar correctness is established.

The architecture should not assume permanent visibility.

Future modes may include:
- Auto-hide.
- Smart auto-hide.
- Edge reveal.
- Maximized-window adaptive appearance.

## 5.11 Maximized windows

Floating taskbars can overlap usable desktop content.

Research and validate:
- AppBar reservation where suitable.
- Work-area behavior.
- Floating dock positioning.
- Full-screen application handling.
- Games and exclusive fullscreen modes.
- Remote desktop sessions.

Do not assume that a visually floating bar automatically reserves desktop space.

The chosen V1 strategy must be tested on Windows.

---

# 6. Multi-Monitor Behavior

V1 must not assume a single display.

Required foundations:
- Enumerate connected monitors.
- Identify primary monitor.
- Track physical and logical bounds.
- Handle scale differences.
- Reposition UI on display changes.
- Recover when a display disconnects.
- Prevent panels from opening offscreen.

Initial default:
- Main taskbar on the primary display.
- Additional-display dock behavior to be implemented after primary-display stability.
- Control Center opens on the display associated with the originating taskbar.

Long-term design:
Independent dock and Control Center surfaces on each monitor.

No user-facing per-monitor customization is required in V1.

---

# 7. Control Center

## 7.1 Purpose

Provide fast access to common Windows system controls without requiring users to navigate multiple settings windows.

The Control Center must work as part of Prism Shell rather than as a separate-looking utility.

## 7.2 Position and geometry

Default:
- Anchored above the right-hand status region.
- On the same monitor as its trigger.
- Opens upward.
- Dismisses on Escape or outside click.

Preliminary geometry:
- Target width: 380 px.
- Preferred content-driven height.
- Maximum height based on monitor work area.
- Gap above taskbar: 10 px.
- Panel padding: 18 px.
- Section spacing: 14 px.

When space is constrained:
- Keep the panel onscreen.
- Scroll its content if needed.
- Preserve access to essential controls.

## 7.3 Control Center layout

Sections, in order:

1. Quick controls.
2. Volume and brightness.
3. Connectivity and device information.
4. Media.
5. Utilities and power.

Sections may collapse or hide if the associated hardware or services are unavailable.

## 7.4 Quick controls

Initial candidates:
- Wi-Fi.
- Bluetooth.
- Focus / Do Not Disturb.
- Night light.
- Airplane mode where available.
- Dark mode where safely supported.

Visual design:
- Two-column tile grid.
- Rounded rectangular tiles.
- Icon and concise label.
- Clear active/inactive state.
- Accent fill for active controls.
- Accessible focus and disabled states.

Every tile must have a validated capability contract.

Possible results:
- Direct action performed.
- Native Windows panel opened.
- Read-only status.
- Unsupported on this device.

A read-only item must not look like an enabled toggle.

Do not claim that all these toggles can be changed through public Windows APIs.

## 7.5 Volume slider

Requirements:
- Read actual Windows volume.
- Update the displayed level.
- Allow dragging.
- Reflect external volume changes.
- Handle mute state.
- Handle unavailable audio endpoints.

Preferred integration:
Supported Windows Core Audio APIs.

Do not block the UI thread.

## 7.6 Audio device selection

Display available supported audio output endpoints.

Allow choosing an output where a validated and appropriately supported implementation exists.

Research default-device switching carefully.

If direct switching requires an unsuitable or undocumented interface, use the Windows output selector or Settings as fallback.

Do not silently depend on an undocumented API.

## 7.7 Brightness control

Brightness support is hardware-dependent.

Research:
- Built-in laptop displays.
- External monitors.
- Supported display control APIs.
- DDC/CI limitations.
- Permission and driver requirements.

When brightness control is unsupported:
- Hide or disable the slider.
- Explain the limitation if relevant.
- Offer a native Windows settings shortcut where useful.

Never display a working-looking brightness slider that has no effect.

## 7.8 Connectivity

Display:
- Current connection status.
- Connection type where available.
- Wi-Fi status and network name where accessible.
- Bluetooth state where accessible.

Do not expose sensitive connectivity information in logs.

If toggling an adapter is unsupported:
- Show status.
- Provide a native Windows action.

## 7.9 Battery and power

If a battery exists:
- Show charge percentage.
- Show charging state.
- Show power connection state where available.

On a desktop without a battery:
- Do not display an empty or fake battery indicator.

Power mode information may be included where supported.

## 7.10 Media controls

Show a media card when a compatible active media session is available.

Card contents:
- Album art where accessible.
- Track title.
- Artist.
- Play/pause.
- Previous.
- Next.

When no media session exists:
- Hide the card, or show a compact empty state if justified.

Research Windows Global System Media Transport Controls session integration.

Treat media applications with incomplete metadata gracefully.

## 7.11 Utility actions

Candidates:
- Windows Settings.
- Lock workstation.
- Sleep.
- Restart.
- Shut down.

Initial safety rules:
- Lock may execute directly through supported OS functionality.
- Sleep must handle permission and hardware support.
- Restart and shutdown must require deliberate confirmation.
- Never trigger destructive power actions from a hover.
- Preserve the user's ability to cancel.

## 7.12 Panel interaction

Opening:
- Highlight the originating taskbar status control.
- Show panel with smooth fade and vertical movement.
- Set focus appropriately.

Closing:
- Outside click.
- Escape.
- Repeated trigger click.
- Appropriate system transitions.

Prevent accidental reopening from focus events.

Handle rapid repeated clicks without animation corruption.

---

# 8. System Tray and Clock

## 8.1 System tray strategy

V1 must provide reliable core system indicators.

Do not assume complete third-party tray integration is immediately available.

The architecture should permit a future tray backend.

For the initial release:
- Implement Prism-owned status icons.
- Integrate actual Windows system information.
- Preserve access to original Windows notification-area functionality.
- Keep third-party tray replication behind a feasibility gate.

## 8.2 Clock and date

Display actual system-local time.

Suggested initial layout:
- Time on the primary line.
- Short date on a secondary line where space permits.

Handle:
- Time changes.
- Date rollover.
- Time zone changes.
- 12/24-hour formatting from system preferences where appropriate.

Avoid unnecessarily high-frequency clock updates.

## 8.3 Notification indicator

A notification entry point may be included in the taskbar.

Until a reliable notification integration exists:
- Open the native Windows notification surface where possible.
- Do not fabricate notification history or counts.
- Do not claim to capture notifications from arbitrary applications.

---

# 9. Visual Motion Specification

Motion should communicate state without becoming distracting.

Preliminary targets:

Dock icon hover:
- 100–150 ms.

Control Center open:
- 180–240 ms.

Control Center close:
- 140–200 ms.

Window preview appearance:
- 150–220 ms.

Control toggle feedback:
- 100–180 ms.

Guidelines:
- Prefer opacity and transform animations.
- Avoid expensive layout recalculation.
- Interrupt animations safely.
- Avoid visible lag between input and state changes.
- Respect reduced-motion preferences.

When reduced motion is enabled:
- Remove large transforms.
- Minimize nonessential animation.
- Preserve clear state transitions.

---

# 10. Interaction and State Model

Every interactive element needs explicit states.

Common states:
- Idle.
- Hovered.
- Pressed.
- Focused.
- Active.
- Disabled.
- Loading.
- Error.

System operations must have state reconciliation.

Example:
1. User activates Wi-Fi control.
2. Application checks capability.
3. Supported operation is requested.
4. UI displays pending state if necessary.
5. Windows reports actual resulting state.
6. UI updates accordingly.
7. Failures are communicated nonintrusively.

Do not treat a click as proof of successful state change.

---

# 11. Proposed System Architecture

Separate the application into modules with clear responsibilities.

## 11.1 Core

Responsibilities:
- Application lifecycle.
- State coordination.
- Event handling.
- Diagnostics.
- Configuration loading.
- Shutdown and recovery.

## 11.2 Window tracking

Responsibilities:
- Eligible window discovery.
- Foreground window tracking.
- Application grouping.
- Window lifecycle.
- Activation and minimization.
- Window metadata.

## 11.3 Taskbar model

Responsibilities:
- Pinned application entries.
- Running application entries.
- Active-state calculation.
- Window ordering.
- Interaction commands.

## 11.4 Control Center model

Responsibilities:
- Device status.
- Capability reporting.
- Control commands.
- Loading/error state.
- Media session state.

## 11.5 Native UI

Responsibilities:
- Dock rendering.
- Control Center rendering.
- Previews.
- Animations.
- Accessibility.
- Input handling.
- DPI-aware layout.

The native UI must not directly own complex Windows integration logic.

## 11.6 Platform adapter

Responsibilities:
- Windows-specific APIs.
- Linux preview mocks.
- OS capability detection.
- Platform error mapping.

Use explicit interfaces rather than conditional checks throughout presentation code.

## 11.7 Recovery

Responsibilities:
- Safe startup.
- Crash detection.
- Restore Windows UI.
- Configuration fallback.
- Failure diagnostics.

Recovery must remain isolated from complicated visual logic.

---

# 12. Preliminary Repository Layout

prism-shell/
  AGENTS.md
  Cargo.toml
  Cargo.lock
  README.md

  docs/
    PROJECT_SPEC.md
    ARCHITECTURE.md
    WINDOWS_API_RESEARCH.md
    DEVELOPMENT.md
    TESTING.md
    CURRENT_STATUS.md
    DECISIONS.md

  crates/
    prism-core/
    prism-platform/
    prism-windows/
    prism-ui/
    prism-app/

  assets/
    icons/
    fonts-reference/
    images/

  tests/
    integration/

  scripts/
    dev/
    test/
    package/

This is an initial architecture proposal.

The agent may recommend a simpler structure if it better suits the actual codebase.

Do not bundle proprietary font files without appropriate rights.

---

# 13. Linux Development Preview

A Linux preview is required for rapid visual iteration.

The preview should render:
- The dock.
- Taskbar sections.
- App icons.
- Hover states.
- Running indicators.
- Window previews.
- Control Center.
- System control states.
- Media cards.
- Error and unsupported states.

Mock data should include:
- Several pinned applications.
- Multiple running applications.
- A foreground application.
- An application with multiple windows.
- Wi-Fi connected/disconnected states.
- Bluetooth on/off states.
- A laptop with battery.
- A desktop without battery.
- Active media playback.
- Unsupported brightness.
- No audio endpoint.
- A disconnected monitor event.

Use deterministic fixtures.

The preview should support screenshots.

The preview must be visibly identified as development-only.

Linux preview success does not imply Windows shell integration success.

---

# 14. Windows Integration Research

Before major native implementation, document:

1. Window enumeration and filtering.
2. Foreground application tracking.
3. Application identity and grouping.
4. Window activation restrictions.
5. Taskbar coexistence.
6. Desktop work-area reservation.
7. Shell hook availability.
8. Tray icon limitations.
9. Notification access limitations.
10. Volume control.
11. Audio endpoint selection.
12. Brightness.
13. Connectivity state.
14. Bluetooth.
15. Media sessions.
16. Display and DPI events.
17. Recovery on application crash.

For each integration, record:
- Proposed API.
- Official documentation.
- Supported Windows versions.
- Permissions.
- Reliability.
- Known limitations.
- Fallback.
- Testing requirements.
- Feasibility status.

Distinguish:
- Documented and supported.
- Documented but limited.
- Undocumented.
- Unproven.
- Infeasible under current constraints.

---

# 15. Testing Plan

## 15.1 Core tests

- Application identity resolution.
- Window grouping.
- State transitions.
- Window removal.
- Configuration validation.
- Error mapping.

## 15.2 UI tests

- Dock layout.
- Icon spacing.
- Hover behavior.
- Window previews.
- Panel open/close.
- Keyboard navigation.
- Control feedback.
- Dynamic content changes.

## 15.3 Display tests

- 1920x1080 at 100%.
- 2560x1440 at 125%.
- 3840x2160 at 150% or 200%.
- Mixed-DPI monitors.
- Monitor disconnect/reconnect.
- Primary display changes.
- Portrait display.
- Fullscreen applications.

These are target test configurations rather than an exhaustive supported-display list.

## 15.4 Windows integration tests

- Native application launching.
- Window activation.
- Multiple-window grouping.
- Minimized application restoration.
- Elevated application behavior.
- Explorer restart.
- Sleep/resume.
- Display changes.
- Audio device changes.
- Network changes.
- Windows session lock/unlock.

## 15.5 Recovery tests

- Kill Prism Shell unexpectedly.
- Corrupt its configuration.
- Terminate during Control Center interaction.
- Disconnect a monitor while a panel is open.
- Restart Explorer.
- Launch while Explorer is unavailable.
- Simulate startup failure.
- Attempt recovery after a failed update.

The original Windows environment must remain recoverable.

---

# 16. Performance Objectives

The project should prioritize low background usage.

Provisional goals:
- Responsive input.
- Smooth animations on ordinary Windows 11 hardware.
- Minimal idle CPU usage.
- No unnecessary render loop while idle.
- Bounded background polling.
- No persistent memory growth during ordinary usage.
- Prompt window tracking.
- Efficient icon and thumbnail caching.

Numerical budgets must be set after obtaining actual measurements.

Measure:
- Cold startup.
- Warm startup.
- Idle CPU.
- Idle GPU.
- Resident memory.
- Animation frame timing.
- Response to application launches.
- Control Center open latency.
- Resource behavior with many windows.
- Multi-monitor resource consumption.

Do not claim performance targets have been achieved until measured.

---

# 17. Security and Recovery

Prism Shell must not compromise Windows stability.

Constraints:
- Normal user privileges by default.
- No Windows system binary replacement.
- No mandatory Explorer replacement.
- No unapproved injection.
- No mandatory network connection.
- No silent system-wide registry modification.
- No automatic destructive system operations.

Startup recovery:
- Detect prior failed startup where feasible.
- Avoid restart loops.
- Fall back to a safe configuration.
- Provide a reliable exit path.

If shell integration hides Windows UI:
- An independent restore mechanism must exist.
- Restoration must be tested on actual Windows.
- The feature must not activate before validation.

---

# 18. Future Customization Compatibility

Do not build a full theme system in V1.

However, organize the application so future customization can be added.

Future needs:
- Design tokens.
- Replaceable UI components.
- Layout definitions.
- Alternative taskbars.
- Alternative Control Center layouts.
- Theme packages.
- Profile storage.
- Desktop widgets.
- Extension permissions.

For V1, use one built-in design.

Avoid coupling core Windows integrations to a specific taskbar geometry.

Do not introduce unnecessary plugin abstractions before a real need exists.

---

# 19. Development Milestones

## Phase 0 — Research and feasibility

Deliver:
- Windows API research.
- UI framework evaluation.
- Cross-compilation experiment.
- Architectural decision records.
- Risk register.
- Updated development plan.

Exit criteria:
- Plausible technical stack established.
- Critical limitations documented.
- Safe integration strategy selected.
- Unknowns clearly identified.

## Phase 1 — Native foundation

Deliver:
- Rust workspace.
- Core application lifecycle.
- Platform abstraction.
- Logging.
- Basic configuration.
- Linux development preview.
- Windows application bootstrap.

Exit criteria:
- Linux build works.
- Windows build works.
- Basic UI starts in supported environments.
- Core unit tests pass.

## Phase 2 — Taskbar prototype

Deliver:
- Dock layout.
- Launcher surface.
- App icon rendering.
- Mock application data.
- Running indicators.
- Hover effects.
- Preview design.
- Animation states.

Exit criteria:
- Visually coherent Linux preview.
- Keyboard and mouse interactions work.
- UI tests pass.

## Phase 3 — Native window integration

Deliver:
- Real Windows window enumeration.
- Application grouping.
- Foreground tracking.
- Launching and activation.
- Window lifecycle handling.
- Basic real window previews.

Exit criteria:
- Tested on real Windows 11.
- Common native apps work.
- Windows can always be restored.
- Unsupported window types handled gracefully.

## Phase 4 — Control Center UI

Deliver:
- Complete panel layout.
- Quick-control tiles.
- Sliders.
- Device status.
- Media card.
- Utility actions.
- Loading and error states.
- Linux preview fixtures.

Exit criteria:
- Visual system is consistent with the dock.
- Component tests pass.
- No inert controls misrepresented as functional.

## Phase 5 — Windows system integration

Deliver:
- Real volume integration.
- Device status.
- Connectivity.
- Battery information.
- Media sessions.
- Brightness where supported.
- Native fallbacks for unsupported controls.

Exit criteria:
- Actual hardware state reflected accurately.
- Tested on Windows 11.
- Failures handled safely.

## Phase 6 — Shell coexistence and recovery

Deliver:
- Validated taskbar coexistence strategy.
- Fullscreen handling.
- Work-area behavior.
- Recovery controls.
- Crash and startup safeguards.
- Explorer interaction tests.

Exit criteria:
- Proven recovery.
- No unrecoverable shell state.
- No unauthorized invasive modifications.

## Phase 7 — Multi-monitor and polish

Deliver:
- Multi-monitor behavior.
- DPI correctness.
- Responsive geometry.
- Accessibility improvements.
- Final animations.
- Performance optimization.

Exit criteria:
- Required display scenarios tested.
- No major UI overflow.
- Resource performance measured.
- Production limitations documented.

## Phase 8 — Release candidate

Deliver:
- Windows build artifact.
- Installer or portable packaging strategy.
- Release documentation.
- Known limitations.
- Rollback instructions.
- Reproducible build process.

Exit criteria:
- Full Windows test matrix completed.
- Recovery validated.
- No unresolved critical failures.
- Explicit release approval obtained.

---

# 20. Completion Definition

Prism Shell V1 can be considered ready for release when:

- It runs on supported Windows 11 systems.
- It provides a functional dock/taskbar experience.
- Core app interactions work.
- Control Center features operate correctly or use honest fallbacks.
- The visual design is consistent.
- Normal desktop usage remains practical.
- Multi-monitor support meets its documented scope.
- The application behaves correctly under common failures.
- Users can reliably restore normal Windows operation.
- Performance has been measured and accepted.
- Documentation accurately reflects implemented functionality.

A successful Linux preview is not sufficient.

A successful Windows build is not sufficient.

A functioning prototype is not automatically a release candidate.

---

# 21. Current Project Status

Approved:
- Overall V1 direction.
- Floating taskbar/dock baseline.
- Three-zone taskbar structure.
- Integrated Control Center concept.
- Safety-first Windows integration.
- Future customization compatibility.

Proposed, requiring implementation validation:
- Exact dimensions and colors.
- Window-preview live-thumbnail behavior on the transparent Slint destination.
- Experimental taskbar replacement/recovery on real Windows 11.
- Standalone Windows-key shortcut preservation.
- Audio device switching strategy.
- Tray and notification behavior.

Implemented but still requiring the Windows 11 runtime gate:
- Native Windows taskbar, launcher, Control Center, and DWM live previews.
- Independent `prism-recovery.exe` and opt-in taskbar replacement.
- Opt-in standalone Windows-key launcher with fallback hotkey.

Not yet implemented:
- Audio/radio/brightness/media controls.
- AppBar work-area reservation and multi-monitor dock instances.
- Tray/notification integration.
- Installer, code signing, auto-start, and release tooling.

Next step:
Execute `docs/WINDOWS_VALIDATION.md`, starting with Phase A previews and the Phase B recovery gate.
