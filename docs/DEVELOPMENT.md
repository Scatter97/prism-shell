# Development Guide — Prism Shell

Status: Linux development workflow and Windows cross-build workflow are implemented. Windows
runtime validation is not yet performed.

## 1. Toolchain

Observed in this environment:

| Tool | Version / notes |
|---|---|
| Rust | rustc 1.100.0 (2026-11-09), cargo 1.100.0 |
| UI toolkit | Slint 1.19.0 (resolved by `Cargo.lock`) |
| Windows cross-build | cargo-xwin 0.23.1, target `x86_64-pc-windows-msvc` |
| Virtual display | Xvfb 21.1.26 extracted rootlessly under `~/.local/opt/prism-xvfb` |
| Screenshot capture | ffmpeg with `x11grab`; Slint `take_snapshot` for UI-only PNGs |
| Interaction driver | portable xdotool 4.20260303.1 (optional; not a runtime dependency) |
| Fonts/icons | Inter (OFL, bundled under `assets/fonts`); Lucide SVGs (ISC, see `assets/icons/LICENSE-lucide.txt`) |

`rust-toolchain.toml` pins stable, rustfmt, clippy, and the Windows MSVC target. This is a
development pin, not a claim that older stable versions are supported.

## 2. Workspace

```
Cargo.toml                 workspace + shared dependencies + release profile
rust-toolchain.toml
crates/
  prism-core/              portable domain/state, no I/O, unsafe forbidden
  prism-platform/          platform traits + explicit mock backend
  prism-ui/                Slint UI, design tokens, assets, view mapping
  prism-app/               preview + production bootstrap, config, logging, controller
assets/
  fonts/                   Inter Regular/Medium/SemiBold + OFL
  icons/status/            Lucide status icons
  icons/apps/              Prism-designed application icons
docs/
scripts/dev/virtual-display.sh
scripts/test/              screenshot/test helpers (see TESTING.md)
```

## 3. Build and test on Linux

```bash
cd /home/joshua/Projects/Prism
. "$HOME/.cargo/env"

cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets
cargo build -p prism-app --bin prism-preview
```

If Clippy reports style-only warnings, do not silence a warning that hides a correctness issue.
The milestone results are in `docs/CURRENT_STATUS.md`.

## 4. Run the Linux preview

```bash
cargo run -p prism-app --bin prism-preview -- \
  --scenario laptop --open-cc
```

Useful options:

```
--scenario laptop|desktop|minimal|faults|many
--width <logical px> --height <logical px>
--scale <0.5..4.0>              force SLINT_SCALE_FACTOR before backend init
--open-cc | --open-launcher
--preview <app-id>              e.g. browser
--confirm                       open the power confirmation sheet
--toast "message"
--reduced-motion
--12h | --no-date
--config <path>
--screenshot <path>             show, render, save PNG, exit
--verbose
```

The preview is explicitly a mock. It does not touch Windows, Explorer, audio, radios, brightness,
media, or the registry. The UI says so in the developer HUD, Control Center footer, toasts, and
the launcher footer.

## 5. Configuration

Resolution order:

1. `--config <path>`.
2. `PRISM_CONFIG` environment variable.
3. Linux: `$XDG_CONFIG_HOME/prism-shell/config.json` or `~/.config/prism-shell/config.json`.
4. Windows: `%APPDATA%\Prism\config.json`.

A missing file is fine. A corrupt/invalid file is rejected as a whole, logged, and replaced in
memory by `PrismConfig::default()`; the file is never overwritten. Run with `--verbose` to see
details.

Minimal example:

```json
{
  "version": 1,
  "pinned_apps": ["files", "browser", "terminal", "mail"],
  "dock": { "show_date": true, "use_24_hour_clock": true },
  "accessibility": { "reduced_motion": false }
}
```

## 6. Virtual display for visual tests

The development host may have a real Wayland session, but visual tests must not depend on it. Use
the rootless Xvfb helper:

```bash
scripts/dev/virtual-display.sh install          # one-time, downloads Arch packages to ~/.local/opt
scripts/dev/virtual-display.sh start 1920x1080
scripts/dev/virtual-display.sh run-app --out artifacts/screenshots/dock.png -- \
  --width 1920 --height 1080
scripts/dev/virtual-display.sh screenshot artifacts/screenshots/manual.png
scripts/dev/virtual-display.sh stop
```

- No sudo. Xvfb/xauth are extracted under `~/.local/opt/prism-xvfb`; state and sockets live under
  `$XDG_RUNTIME_DIR/prism-virtual-display/`.
- The script never touches the live `:0` XWayland display.
- Xvfb has no window manager: windows are undecorated and placed at 0,0, which is useful for shell
  surface screenshots. Do not use it to validate Win32 focus/z-order/AppBar behavior.
- Rendering is software (llvmpipe); colors/layout are representative, GPU performance is not.
- Use `--scale` for DPI tests. For example, to emulate 1920×1080 at 125%:

```bash
SLINT_SCALE_FACTOR=1.25 scripts/dev/virtual-display.sh run-app --out out.png -- \
  --width 1536 --height 864
```

or, without the environment variable:

```bash
prism-preview --width 1536 --height 864 --scale 1.25 --screenshot out.png
```

The explicit `--scale` is the reproducible path; it sets `SLINT_SCALE_FACTOR` before Slint creates
its backend.

For the full automated capture matrix (14 states/resolutions), run:

```bash
scripts/test/ui-snapshots.sh
```

It starts a disposable Xvfb, builds the preview, writes `artifacts/screenshots/auto-*.png`, checks
that each file exists, and stops the display on exit.

## 7. Deterministic UI screenshots

`--screenshot` uses Slint's `Window::take_snapshot`, not a desktop capture. It writes a PNG at the
window's **physical** pixel size. Always pass `--scale 1` when you need a 1:1 logical screenshot:

```bash
DISPLAY=:99 XAUTHORITY="$XDG_RUNTIME_DIR/prism-virtual-display/Xauthority" \
  target/debug/prism-preview \
  --width 1920 --height 1080 --scale 1 \
  --scenario laptop --open-cc \
  --screenshot artifacts/screenshots/snapshot-cc-1920x1080.png
```

Without a forced scale, winit may infer a different scale factor on a given X server, producing a
larger image (for example 3200×2000 for a 1600×1000 logical window at 2×). This is expected; it is
why `--scale` exists.

The clock uses real local time, so screenshots differ by the clock unless you post-process or
freeze the environment. Visual review is manual; state tests are deterministic.

## 8. Windows cross-build

```bash
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin --locked
cargo xwin build --target x86_64-pc-windows-msvc -p prism-app --bins
```

Outputs (debug):

```
target/x86_64-pc-windows-msvc/debug/prism-shell.exe
target/x86_64-pc-windows-msvc/debug/prism-preview.exe
```

The milestone attempt and any errors are recorded in `docs/CURRENT_STATUS.md`. Cross-compiling
proves the Rust/MSVC link path, not Windows runtime behavior.

`prism-shell.exe` in this milestone is a fail-safe bootstrap: it shows a native warning message
box saying integration is unavailable and exits. It does not replace Explorer, hide the taskbar,
modify the registry, auto-start, or run mock data as if it were real system state.

## 9. Windows runtime workflow (next milestone)

1. Use a disposable Windows 11 x64 VM for the first run.
2. Install the Visual Studio Build Tools/Windows SDK if building natively.
3. Run `prism-preview.exe --scenario laptop --open-cc` only as an explicit UI preview.
4. Validate the real shell surface in a staged/test harness before any taskbar coexistence work.
5. Keep `explorer.exe` running and the original taskbar accessible at all times in V1.
6. Use `PrintWindow`/DWM thumbnail tests only after the transparent window host is validated.

A native `windows-latest` CI job may build/test/fmt/clippy, but it cannot validate DWM
transparency, Explorer, the taskbar, Core Audio endpoints, radios, or notification consent.

## 10. Common issues

| Symptom | Cause / action |
|---|---|
| `no Xvfb at ~/.local/opt/prism-xvfb` | Run `scripts/dev/virtual-display.sh install`. |
| Display already in use | `start` picks the next free display; check `scripts/dev/virtual-display.sh status`. |
| Screenshot is 2× expected | Slint inferred a scale factor; pass `--scale 1`. |
| Black gutter around the app | The window is smaller than the Xvfb screen; use matching `--width/--height` or fullscreen. |
| `no method run` in a new binary | Import `slint::ComponentHandle`. |
| Overlays intercept clicks while closed | They use a `present` flag + close timer; keep this pattern when adding panels. |
| Windows build cannot find a linker/C compiler | `cargo xwin` downloads the CRT/SDK; for C dependencies install clang/lld or use a native Windows runner. |
| Taskbar/Explorer changed unexpectedly | Stop; do not add recovery hacks. V1 must never change the shell. |
