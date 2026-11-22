
# AGENTS.md — Prism Shell

## 1. Project Identity

Project name: Prism Shell (working title)
Platform: Windows 11
Development host: Linux
Project type: Native Windows desktop shell and interface enhancement
Primary language: Rust
Status: Initial development
Specification: docs/PROJECT_SPEC.md

Prism Shell is a modern alternative desktop experience for Windows 11.

Its initial release focuses on a polished native taskbar/dock and integrated Control Center. It should feel like a cohesive desktop environment rather than a collection of unrelated customization utilities.

Prism Shell is not a new operating system. Windows 11 remains responsible for applications, processes, security, hardware, and underlying system services.

## 2. Product Direction

The first version must deliver one excellent, opinionated design.

V1 priorities:
1. A functioning floating taskbar/dock.
2. Accurate running-application tracking.
3. Reliable application launching and window activation.
4. A polished Control Center.
5. Working, supported Windows system controls.
6. Smooth animations and responsive interaction.
7. Multi-monitor compatibility.
8. Safe fallback to the original Windows desktop.
9. Low resource consumption.
10. Clear architecture for future expansion.

Do not build a comprehensive customization interface in V1.

Future capabilities must remain architecturally possible:
- User-selectable themes.
- Custom taskbar arrangements.
- Multiple dock styles.
- User-configurable widgets.
- OS-inspired themes.
- Custom Start menus.
- Community theme packs.
- Plugin ecosystem.
- Advanced notification management.
- Workspace profiles.

Do not implement these future features unless explicitly requested.

## 3. Sources of Truth

Read docs/PROJECT_SPEC.md before developing.

Priority order:
1. Explicit current owner instructions.
2. Approved project decisions documented in the repository.
3. docs/PROJECT_SPEC.md.
4. This AGENTS.md.
5. Existing architectural documentation.
6. Established implementation patterns.

If two sources conflict:
- Identify the conflict.
- Do not silently invent a resolution.
- Preserve working behavior and data.
- Present a recommended solution when approval is needed.

Proposals are not automatically approved requirements.

## 4. Agent Responsibilities

Operate as a senior Windows systems engineer, Rust developer, UI engineer, and quality assurance engineer.

Before implementing a major component:
1. Inspect existing code and documentation.
2. Identify relevant constraints.
3. Research the required Windows APIs.
4. Distinguish documented APIs from undocumented behavior.
5. Define the component's responsibilities.
6. Identify failure and recovery scenarios.
7. Establish tests and acceptance criteria.
8. Implement the smallest complete vertical slice.
9. Verify it.
10. Document results.

Do not claim functionality is complete merely because it compiles.

Do not present simulated functionality as real Windows functionality.

## 5. Platform Rules

Production target:
- Windows 11 x64.
- Normal desktop applications under the current user.
- Windows Explorer remains available.
- No administrative privileges required for ordinary usage.

Initial development host:
- Linux.

Use conditional compilation and platform adapters to separate Windows-only behavior from portable code.

Do not scatter Windows-specific API calls throughout the interface code.

Required conceptual separation:
- UI presentation.
- Application state.
- Window and application tracking.
- Platform integration.
- System controls.
- Configuration.
- Recovery.
- Testing and diagnostics.

Use an explicit platform abstraction for features that cannot run on Linux.

A Linux preview backend may use mock data.

A Windows production backend must use actual Windows functionality.

Never silently substitute mock data in production.

## 6. Technology Direction

Preferred starting point:
- Rust stable.
- Slint as an initial candidate for native UI.
- windows-rs for Windows APIs.
- Tokio only where asynchronous operations genuinely require it.
- Serde for structured configuration.
- Tracing for diagnostics.
- Cargo for builds and tests.

These selections are preliminary.

Before committing to the UI framework, verify:
- Linux development compatibility.
- Windows 11 native compilation.
- Transparent and borderless windows.
- Always-on-top and nonactivating window behavior.
- DPI scaling.
- Multi-monitor positioning.
- Input handling.
- Accessibility support.
- GPU rendering performance.
- Screenshot and automated testing feasibility.
- Native window handles and platform interop.

Do not force a framework that fails critical shell requirements.

Prefer simple, maintainable architecture over unnecessary abstractions.

## 7. Windows Integration Safety

Use documented Windows APIs wherever practical.

Relevant integration areas include:
- Win32 windows and messages.
- Window enumeration.
- Window event notifications.
- Process and application identity.
- Desktop work areas.
- Display enumeration.
- DPI awareness.
- Audio endpoint control.
- Media transport sessions.
- Power and battery status.
- Network information.
- System settings launchers.
- Application icons and shortcuts.

Research each API's actual capability before relying on it.

Important:
- Shell_NotifyIcon is not a general-purpose API for enumerating and taking ownership of every third-party notification icon.
- AppBar APIs are not a complete Windows taskbar replacement API.
- Notification interception and native notification-history access may be restricted.
- Some Wi-Fi and Bluetooth operations may require permissions, service support, or native Windows UI.
- Brightness control differs between integrated displays and external monitors.

Do not assume that every desired shell feature has a public API.

Do not use private Explorer structures or undocumented hooks without explicit authorization and a dedicated risk review.

Do not modify Windows system binaries.

Do not replace system DLLs.

Do not disable Windows security mechanisms.

Do not implement process injection or invasive shell patching as a shortcut.

## 8. Critical Recovery Rules

Recovery is a core feature, not an optional enhancement.

The user must be able to return to the normal Windows desktop.

Requirements:
- Never permanently remove Explorer.
- Do not automatically make Prism Shell the system shell in V1.
- Do not modify system-wide shell registration.
- Do not silently disable the original Windows taskbar.
- Do not enable automatic startup before recovery behavior is verified.
- Provide a documented exit mechanism.
- Provide a tested restore-original-desktop action.
- Detect and handle startup failures.
- Prevent restart loops.
- Recover safely from corrupted configuration.
- Make experimental functionality opt-in.

Use a staged shell integration strategy.

During early development, run alongside Explorer with the original taskbar accessible.

Any taskbar-hiding strategy must first be researched and validated on real Windows 11.

When Prism Shell crashes, restoring normal Windows functionality must not depend on Prism Shell remaining alive.

Design and test an independent recovery mechanism if deeper integration is eventually enabled.

Avoid destructive changes to the registry or Explorer configuration.

## 9. V1 UI Design Direction

Visual identity:
- Modern.
- Minimal.
- Premium.
- Clean.
- Responsive.
- Consistent.
- Original rather than a direct clone of another OS.

Taskbar baseline:
- Bottom of the primary display.
- Floating appearance.
- Unified rounded translucent surface.
- Three functional zones:
  1. Launcher.
  2. Applications.
  3. Status and system entry points.

Default behavior:
- Always visible.
- Compact but comfortable dimensions.
- Icon-first application presentation.
- Running and active indicators.
- Hover feedback.
- Window previews.
- Native application icon support.
- No permanent search box.

Control Center baseline:
- Anchored above the right side of the taskbar.
- Matching visual material.
- Quick controls.
- Volume and brightness where available.
- Audio device selection.
- Connectivity information.
- Battery and power information where applicable.
- Now-playing media card.
- Utility and power actions.

The Control Center should look like a natural extension of the taskbar.

Use one consistent visual system for:
- Corner radii.
- Borders.
- Opacity.
- Shadows.
- Icon styling.
- Typography.
- Accent colors.
- Hover states.
- Focus states.
- Motion.

Do not introduce unrelated styling across components.

## 10. UI Design Integrity

Do not substitute an ordinary settings window for the desktop shell.

The actual production application must be capable of presenting desktop-anchored surfaces.

Do not hardcode positioning based on one specific screen resolution.

Support:
- Different resolutions.
- Windows display scaling.
- Multiple displays.
- Changing display configurations.
- Keyboard navigation.
- Accessible focus indication.
- Reduced motion.
- High contrast and readable text.

Avoid decorative animations that interrupt input or delay essential actions.

Prefer composited animations when practical.

Avoid continuous rendering when the interface is idle unless required by an active animation or media element.

## 11. Window Management

Taskbar window tracking must be designed carefully.

Do not assume every top-level HWND represents a taskbar application.

Research:
- Owned windows.
- Tool windows.
- Invisible windows.
- Minimized windows.
- Cloaked windows.
- UWP and packaged applications.
- Multiple windows from one application.
- Elevated applications.
- Virtual desktops.
- Explorer windows.
- Applications that recreate their windows.

Window tracking should be event-driven where reliable.

Use polling only where necessary, with bounded frequency.

Avoid stealing focus.

Handle processes and windows disappearing during user interaction.

Document grouping and identity rules.

## 12. Control Center Integrity

Every interactive control must have a clearly defined state.

Required state types:
- Available.
- Enabled.
- Disabled.
- Loading.
- Unsupported.
- Permission required.
- Failed.

Do not render a control as functional if it does nothing.

Where direct control is unsupported:
- Explain the limitation.
- Open the relevant native Windows settings.
- Keep the interface responsive.

System controls must reflect actual OS state, not merely optimistic local UI state.

If an operation fails, reconcile the displayed state with Windows.

Do not block the UI thread while waiting on device APIs.

Destructive power actions must be deliberate and safe.

## 13. Linux Development Workflow

Linux is the primary coding environment, not the production desktop target.

Maintain a Linux preview application for UI work.

The preview must:
- Reuse as much production UI code as possible.
- Show the real taskbar and Control Center layouts.
- Simulate realistic app and system states.
- Support screenshots.
- Support repeatable interaction tests.
- Be clearly labeled as a development preview.

Mocks belong behind platform interfaces.

Do not make Linux a production support target simply because the preview runs there.

Do not attempt to use Linux-specific APIs as substitutes for production Windows APIs.

## 14. Windows Compilation and Validation

Use a reproducible Windows build workflow.

Evaluate:
- cargo-xwin.
- Windows MSVC Rust targets.
- Windows CI runners.
- Windows 11 VM testing.
- Physical Windows 11 testing.

Pin or document tool versions as appropriate.

Cross-compilation success does not establish runtime correctness.

Wine testing may supplement verification but cannot prove compatibility with Explorer, the notification area, taskbar behavior, or Windows system controls.

Every Windows-specific feature must be tested on actual Windows 11 before being marked complete.

Do not mark integration tests as passed unless they were actually executed.

## 15. Testing Requirements

Maintain appropriate unit, component, integration, and end-to-end tests.

Important test categories:
- Application grouping.
- Window lifecycle events.
- Application launch and activation.
- Dock hover and click states.
- Control Center opening and closing.
- Display scaling.
- Multi-monitor positioning.
- Screen-edge behavior.
- System control state reconciliation.
- Failed API operations.
- Crash recovery.
- Explorer restoration.
- Configuration corruption.
- Startup behavior.
- Keyboard accessibility.
- Memory and resource cleanup.

Use virtual displays or headless UI testing where available.

Do not confuse visual snapshots with actual Windows integration validation.

Maintain deterministic tests where possible.

Do not weaken tests merely to make a failing build pass.

## 16. Performance Requirements

The application should feel faster and lighter than a conventional heavyweight desktop shell.

Priorities:
- Fast startup.
- Low idle CPU consumption.
- Low idle GPU utilization.
- Reasonable memory footprint.
- No unnecessary continuous timers.
- Minimal main-thread blocking.
- Smooth opening and closing animations.
- Reliable response under system load.

Establish performance baselines before optimizing.

Collect measurements on real hardware.

Do not claim specific resource usage numbers without measurements.

Avoid adding heavyweight dependencies without justification.

## 17. Architecture for Future Customization

V1 has one built-in design.

However, do not deeply couple shell integrations to specific visual layouts.

Keep presentation details separate from system integration.

Prefer structured internal component definitions where useful.

The architecture should eventually accommodate:
- Theme tokens.
- Component replacement.
- User profile storage.
- Alternative dock layouts.
- Configurable Control Center modules.
- Multiple theme styles.

Do not prematurely build a marketplace, full theme engine, or scripting system.

Future extensibility must not substantially delay V1.

## 18. Security and Privacy

Use least privilege.

Never require administrator access without documented necessity.

Never execute theme-pack code or external scripts automatically.

Never send personal desktop activity, window titles, network data, or device status to an external service without explicit user authorization.

The initial product should function offline.

Avoid unnecessary telemetry.

Do not log secrets or sensitive window contents.

Treat data from other processes and external packages as untrusted.

Keep dependencies reviewed and maintained.

## 19. Development Process

Use incremental milestones.

Before each milestone:
- State the scope.
- List dependencies.
- Explain technical risks.
- Define acceptance criteria.

During implementation:
- Keep commits focused.
- Preserve buildability.
- Add tests.
- Avoid unrelated refactors.
- Document important architectural choices.
- Report blocked functionality promptly.

After a milestone:
- Run relevant checks.
- Report exactly what passed.
- Report what was not tested.
- Identify remaining limitations.
- Update project status documentation.

Do not silently remove previously working features.

Do not overwrite unrelated user changes.

Do not push, deploy, release, or modify remote repositories unless explicitly requested.

## 20. Documentation

Maintain:
- docs/PROJECT_SPEC.md
- docs/ARCHITECTURE.md
- docs/WINDOWS_API_RESEARCH.md
- docs/DEVELOPMENT.md
- docs/TESTING.md
- docs/CURRENT_STATUS.md
- docs/DECISIONS.md

Create supporting files when they become useful.

Keep documentation synchronized with the implementation.

Use status labels:
- Proposed.
- Approved.
- Implemented.
- Tested on Linux.
- Tested on Windows.
- Blocked.
- Deferred.

Do not label research goals as implemented functionality.

## 21. Communication

Be direct, technical, and transparent.

Explain:
- What changed.
- Why it changed.
- How it was tested.
- What remains uncertain.

When selecting between technologies, compare practical tradeoffs.

Do not claim a feature is impossible without researching alternatives.

Do not claim a feature is feasible without adequate evidence.

Ask for approval before:
- Major architectural deviations.
- Destructive operations.
- Invasive Windows integrations.
- Material changes to approved product behavior.
- Adding significant runtime dependencies.
- Changing the supported platform or V1 scope.

For ordinary implementation details already covered by the specification, proceed without unnecessary questions.

## 22. Initial Mission

Begin with architecture validation and a minimal end-to-end prototype.

Research Windows integration feasibility before implementing invasive functionality.

Build a reusable visual shell foundation.

Deliver a Linux-previewable taskbar and Control Center.

Establish a Windows cross-compilation workflow.

Validate real Windows behavior as soon as a Windows environment is available.

Keep the product focused.

The immediate goal is not a complete desktop customization ecosystem.

The immediate goal is a stable, fast, visually excellent Windows 11 taskbar/dock and Control Center that can evolve into a complete desktop platform.
