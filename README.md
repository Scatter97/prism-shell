# Prism Shell

A native Windows 11 floating taskbar/dock and Control Center, written in Rust.

> **Status: development preview.** The Windows integration is not implemented yet. This milestone
> delivers the architecture, a Linux-previewable interactive taskbar and Control Center, a
> platform abstraction, and a successful Windows MSVC cross-build. It does **not** replace
> Explorer, hide the Windows taskbar, modify the registry, or claim that mocks are real Windows
> system state.

## What you can run today

The Linux preview renders a real Slint UI with explicit mock services:

```bash
cargo run -p prism-app --bin prism-preview -- --scenario laptop --open-cc
```

It includes:

- a floating, rounded taskbar with launcher, app icons, running/active indicators, clock, battery,
  Wi-Fi/volume status, and Control Center trigger;
- a Control Center with quick tiles, volume/brightness sliders, audio output selection,
  connectivity, battery, now-playing media, and pinned power/settings actions;
- a functional launcher with search;
- application hover tooltips and multi-window previews;
- pending, failed, unavailable, and unsupported control states;
- deterministic fixtures: Laptop, Desktop, Minimal, Faults, and Many apps.

The preview is visibly labeled **DEVELOPMENT PREVIEW · MOCK DATA**. Simulated settings/power/media
actions say "preview only" and never touch the host system.

## Screenshots

The repository does not commit generated screenshots. Create deterministic UI captures with:

```bash
scripts/test/ui-snapshots.sh
```

Artifacts land in `artifacts/screenshots/` (gitignored). See `docs/TESTING.md` for the list of
states captured and the virtual-display workflow.

## Build and test

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets
cargo build -p prism-app --bin prism-preview
```

Windows MSVC cross-build from Linux:

```bash
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin --locked
cargo xwin build --target x86_64-pc-windows-msvc -p prism-app --bins
```

`prism-shell.exe` is currently a fail-safe bootstrap: it warns that Windows integration is not
available and exits, without changing the shell. `prism-preview.exe` is the explicit development
preview and still uses mock data.

## Safety

V1 runs alongside Explorer. It does not:

- replace or terminate Explorer;
- modify system-shell registration or system binaries;
- hide or disable the original taskbar;
- modify the registry;
- auto-start;
- inject into processes or use undocumented Explorer hooks;
- require administrator privileges.

Taskbar hiding, AppBar work-area reservation, tray replication, notification interception, and
real system controls are **deferred** until validated on Windows 11. See
`docs/WINDOWS_API_RESEARCH.md` and `docs/DECISIONS.md`.

## Documentation

- [Project specification](docs/PROJECT_SPEC.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Windows API research](docs/WINDOWS_API_RESEARCH.md) and [detailed notes](docs/research/windows-api-notes.md)
- [UI framework evaluation](docs/UI_FRAMEWORK_EVALUATION.md)
- [Development guide](docs/DEVELOPMENT.md)
- [Testing](docs/TESTING.md)
- [Current status](docs/CURRENT_STATUS.md)
- [Decisions](docs/DECISIONS.md)

## Licensing

The repository does not yet declare a product license. Slint offers GPLv3 or a royalty-free
license with attribution; that choice must be made before distribution. Inter is bundled under
the SIL Open Font License; Lucide icons are ISC. See `docs/DECISIONS.md` D-019.
