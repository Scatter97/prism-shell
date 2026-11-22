# Prism Shell

A native Windows 11 floating taskbar/dock and Control Center, written in Rust.

> **Status: native Windows taskbar, icons, click activation, and coexistence with the original
> taskbar are owner-confirmed on Windows 11. Live DWM window previews, opt-in taskbar replacement,
> and standalone Windows-key integration are implemented and cross-compiled but not yet
> runtime-validated.** The Linux preview remains the only fully runtime-tested UI path. Taskbar
> replacement is **off by default** and must not be enabled until its independent recovery
> watchdog passes the Windows 11 gate in `docs/WINDOWS_VALIDATION.md`. Prism never replaces
> Explorer, never changes registry/shell registration, and never presents mock data as real Windows
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

The Windows binary is a separate, real backend:

```bash
cargo xwin build --release --target x86_64-pc-windows-msvc -p prism-app --bins
```

It creates a floating taskbar, Control Center, launcher, and live window-preview surface as native
windows alongside Explorer, with real top-level window enumeration, executable-based grouping,
shell icons, launch/activation, DWM live thumbnails, clock, battery, native Settings shortcuts, and
workstation lock. Audio, Wi-Fi/Bluetooth toggles, brightness, and media are shown as unavailable
rather than faked. Basic taskbar/icons/activation/coexistence are owner-confirmed; previews and the
experimental integrations still need the checklist. See `docs/WINDOWS_VALIDATION.md`.

Build the experimental recovery helper with the same command (it is one of the `prism-app` bins):

```text
target/x86_64-pc-windows-msvc/release/prism-recovery.exe
```

Keep `prism-shell.exe` and `prism-recovery.exe` in the same directory if you intend to test
`--experimental-taskbar-replacement`.

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

## Experimental Windows integrations

These are opt-in, off by default, and currently **not Windows-runtime-validated**:

- **Live previews** are always on in the Windows build; they are separate from the experimental
  gates and need Phase A validation.
- **Taskbar replacement:** `prism-shell.exe --experimental-taskbar-replacement` or
  `"experimental": { "taskbar_replacement": true }` in `%APPDATA%\Prism\config.json`. Requires
  `prism-recovery.exe` beside the shell. **Do not enable until the recovery gate passes.**
- **Standalone Windows key:** `--experimental-windows-key`. A Win tap opens/closes the Prism
  launcher; combinations are preserved by synthetic key-down replay.
- **Fallback shortcut:** `--experimental-windows-key-fallback-hotkey` and the
  `windows_key_fallback_shortcut` config string (default `Ctrl+Alt+Space`).

Manual recovery, independent of Prism:

```powershell
.\prism-recovery.exe --status
.\prism-recovery.exe --restore
```

The watchdog mode is started automatically before hiding:
`prism-recovery.exe --watch-parent <pid>`.

## Safety

By default Prism runs alongside Explorer. It does not:

- replace or terminate Explorer;
- modify system-shell registration or system binaries;
- hide or disable the original taskbar;
- modify the registry;
- auto-start;
- inject into processes or use undocumented Explorer hooks;
- require administrator privileges.

The opt-in taskbar-replacement experiment only calls `ShowWindow` on the observed native taskbar
windows and is backed by a standalone watchdog that restores them after a crash, force-kill, or
4-second heartbeat loss. Explorer stays alive throughout. It remains blocked for enablement until
tested on real Windows 11.

AppBar work-area reservation, multi-monitor dock surfaces, tray replication, notification
interception, and audio/radio/brightness/media controls remain **deferred**. See
`docs/WINDOWS_API_RESEARCH.md` and `docs/DECISIONS.md`.

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
