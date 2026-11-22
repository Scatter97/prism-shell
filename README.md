# Prism Shell

A native Windows 11 floating taskbar/dock and Control Center, written in Rust.

> **Status: Phase 3A native Windows shell implemented, successfully cross-compiled, not yet run on
> Windows.** The Linux preview is still the only runtime-tested UI. The Windows executable adds a
> real overlay taskbar/Control Center/launcher, real window tracking, icons, and activation, but
> every Windows runtime behavior is **unverified** until the checklist in
> `docs/WINDOWS_VALIDATION.md` is executed. It does **not** replace Explorer, hide the Windows
> taskbar, modify the registry, or present mock data as real Windows system state.

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

The Windows binary is a separate, real backend:

```bash
cargo xwin build --release --target x86_64-pc-windows-msvc -p prism-app --bin prism-shell
```

It creates a floating taskbar, Control Center, and launcher as native windows alongside Explorer,
with real top-level window enumeration, executable-based grouping, shell icons, launch/activation,
clock, battery, native Settings shortcuts, and workstation lock. Audio, Wi-Fi/Bluetooth toggles,
brightness, and media are shown as unavailable rather than faked. See
`docs/WINDOWS_VALIDATION.md` before running it; it has not been validated on Windows 11 yet.

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

`prism-shell.exe` is the native Windows shell. It runs alongside Explorer and makes no shell
modification. `prism-preview.exe` is the explicit development preview and still uses mock data.
Cross-compilation is not runtime validation; see `docs/CURRENT_STATUS.md` for the exact evidence
and remaining unknowns.

To build and run it on Windows, follow [`docs/INSTALL_WINDOWS.md`](docs/INSTALL_WINDOWS.md).
There is no signed installer or prebuilt release yet.

## Safety

V1 runs alongside Explorer. It does not:

- replace or terminate Explorer;
- modify system-shell registration or system binaries;
- hide or disable the original taskbar;
- modify the registry;
- auto-start;
- inject into processes or use undocumented Explorer hooks;
- require administrator privileges.

Taskbar hiding, AppBar work-area reservation, multi-monitor surfaces, tray replication,
notification interception, and audio/radio/brightness/media controls are **deferred** until the
current overlay is validated on Windows 11. See `docs/WINDOWS_API_RESEARCH.md` and
`docs/DECISIONS.md`.

## Documentation

- [Project specification](docs/PROJECT_SPEC.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Windows API research](docs/WINDOWS_API_RESEARCH.md) and [detailed notes](docs/research/windows-api-notes.md)
- [UI framework evaluation](docs/UI_FRAMEWORK_EVALUATION.md)
- [Development guide](docs/DEVELOPMENT.md)
- [Install on Windows 11](docs/INSTALL_WINDOWS.md)
- [Testing](docs/TESTING.md)
- [Windows 11 validation checklist](docs/WINDOWS_VALIDATION.md)
- [Current status](docs/CURRENT_STATUS.md)
- [Decisions](docs/DECISIONS.md)

## Licensing

The repository does not yet declare a product license. Slint offers GPLv3 or a royalty-free
license with attribution; that choice must be made before distribution. Inter is bundled under
the SIL Open Font License; Lucide icons are ISC. See `docs/DECISIONS.md` D-019.
