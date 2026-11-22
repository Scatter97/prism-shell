# Installing Prism Shell on Windows 11

Status: **source build only. No signed binary release or installer yet.** Basic taskbar rendering,
native icons, click activation, and coexistence with the original taskbar are owner-confirmed on
Windows 11. Live previews and the experimental taskbar/Windows-key integrations are not yet
runtime-validated. See [`WINDOWS_VALIDATION.md`](WINDOWS_VALIDATION.md) before running it.

By default Prism Shell is a normal desktop application. It runs alongside Explorer, does not
replace the Windows shell, does not hide the taskbar, and does not add itself to startup. Explorer
remains available at all times. Taskbar replacement is an explicit, disabled-by-default experiment
with a separate recovery executable.

## 1. Requirements

- Windows 11 x64.
- [Git for Windows](https://git-scm.com/download/win).
- Rust stable with the MSVC toolchain.
- Visual Studio 2022 or 2026 Build Tools with the **Desktop development with C++** workload and a
  Windows 11 SDK. Visual Studio 2026 works with current stable Rust; the full IDE is also fine if
  the C++ workload is installed.

Administrator rights are not needed to run Prism. Installing the build tools may prompt for
elevation.

## 2. Install the build tools

Using `winget` in PowerShell:

```powershell
winget install --id Git.Git -e --source winget
winget install --id Rustlang.Rustup -e --source winget
winget install --id Microsoft.VisualStudio.BuildTools -e --source winget
```

The last command installs the current Build Tools (Visual Studio 2026). You can substitute
`Microsoft.VisualStudio.2022.BuildTools` if you prefer the 2022 toolset.

Then open **Visual Studio Installer**, choose **Modify** for Build Tools, and enable **Desktop
development with C++** (including the MSVC compiler and Windows 11 SDK). The Windows SDK component
is required even if you are not targeting Windows 11-specific APIs; without it, Rust may report
`link.exe` not found even when the C++ tools are present.

If you use Visual Studio 2026 and hit linker detection problems, update rustup and Rust first:

```powershell
rustup self update
rustup update stable
```

Close and reopen PowerShell so `cargo`, `rustup`, and `git` are on `PATH`.

## 3. Clone and build

```powershell
git clone https://github.com/Scatter97/prism-shell.git
cd prism-shell
rustup default stable-x86_64-pc-windows-msvc
cargo build --release -p prism-app --bins
```

The executables are:

```text
target\release\prism-shell.exe
target\release\prism-recovery.exe
target\release\prism-preview.exe   (mock development preview, not the shell)
```

The build may take several minutes the first time. If `link.exe` is not found, the C++ workload
from step 2 is missing.

## 4. Run it

```powershell
.\target\release\prism-shell.exe
```

Expected behavior:

- A floating Prism dock appears above the normal Windows taskbar.
- The Windows taskbar stays visible and usable.
- Hover a running app for about 300 ms to see DWM live window previews.
- Click the Control Center trigger on the dock's right side, then use **Exit** to close Prism.
- Logs are written to `%LOCALAPPDATA%\Prism\logs\prism-shell.log`.

Do **not** start with `--experimental-taskbar-replacement` on a machine you need. The recovery
mechanism has not been validated on Windows 11 yet. Default mode changes no shell state.

There is no console window in the release build. If Prism exits immediately, check the log and run
the debug build for more output:

```powershell
cargo run -p prism-app --bin prism-shell
```

## 5. Experimental recovery and taskbar replacement

Only test this on a disposable Windows 11 VM or spare machine. Copy `prism-recovery.exe` next to
`prism-shell.exe`, then verify the independent restore path first:

```powershell
.\prism-recovery.exe --status
.\prism-recovery.exe --restore
```

Start replacement mode explicitly:

```powershell
.\prism-shell.exe --experimental-taskbar-replacement
```

Use the Control Center **Restore bar** action to exit replacement mode. Forced-kill and
heartbeat-loss recovery must pass the Phase B gate in
[`WINDOWS_VALIDATION.md`](WINDOWS_VALIDATION.md) before this is used on a daily-driver machine.

If anything goes wrong and Prism is unresponsive:

```powershell
.\prism-recovery.exe --restore
```

This tool does not need Prism to be running. It never terminates Explorer.

Standalone Windows-key mode is also off by default:

```powershell
.\prism-shell.exe --experimental-windows-key --experimental-windows-key-fallback-hotkey
```

The fallback is `Ctrl+Alt+Space` unless configured. Test every native `Win+…` shortcut before
relying on it.

## 6. Optional shortcut

Right-click `target\release\prism-shell.exe`, choose **Show more options → Send to → Desktop
(create shortcut)**. Do not add it to the Startup folder or Task Scheduler; automatic startup is
not supported in V1.

## 7. First-run checklist

At minimum, confirm:

- Explorer and the Windows taskbar are still visible.
- The dock appears above the taskbar and its transparent sides do not block desktop clicks.
- Clicking a pinned/running app launches or activates it.
- Hovering a running app shows a live preview after ~300 ms; clicking a card activates that window.
- Control Center opens above the dock and closes on outside click or Escape.
- Exit removes Prism without affecting Explorer.

The full checklist, including DPI, focus, grouping, and recovery tests, is in
[`WINDOWS_VALIDATION.md`](WINDOWS_VALIDATION.md).

## 8. Uninstall

1. Exit Prism from the Control Center.
2. Delete the cloned `prism-shell` folder and the shortcut, if any.
3. Optionally delete `%LOCALAPPDATA%\Prism` (logs) and `%APPDATA%\Prism` (config).

Prism does not modify the registry, shell registration, startup entries, or the Windows taskbar.

## 9. Why there is no downloadable binary yet

Two project decisions block publishing a prebuilt release:

- The Slint license choice (GPLv3 or the royalty-free option with attribution) is unresolved.
  See `DECISIONS.md` D-019.
- Basic taskbar/icon/activation/coexistence is owner-confirmed, but DWM live previews, the
  independent recovery watchdog, and the Windows-key hook are not runtime-validated. Taskbar
  replacement must stay disabled until its recovery gate passes. See `CURRENT_STATUS.md`.

Once those are resolved, a tagged GitHub Release can provide an unsigned test build. Until then,
building from source is the supported path.

## 10. Troubleshooting

| Symptom | Action |
|---|---|
| `cargo` or `rustup` not found | Reopen PowerShell; rerun the rustup installer if needed. |
| `link.exe` not found / `msvc` target missing | Add the **Desktop development with C++** workload and the Windows 11 SDK component; update rustup/Rust; reopen PowerShell. |
| Prism exits immediately | Read `%LOCALAPPDATA%\Prism\logs\prism-shell.log`; run the debug build. |
| `window handle unavailable: the underlying handle cannot be represented...` | Your build predates `3936efd`. Run `git pull`, rebuild, and retry. |
| Dock is invisible or has a black box | Record GPU/driver and scale; try `SLINT_BACKEND=software`; report the log. |
| Clicking apps does not activate them | Record whether the target is elevated; this is a known unverified foreground path. |
| Dock covers a maximized window | Expected: no AppBar work-area reservation in this build. |
| Windows taskbar disappeared | Only possible if you explicitly enabled replacement mode. Run `prism-recovery.exe --restore`, then stop Prism and verify Explorer is running. |
| Live previews are black/empty | Record GPU/driver and renderer; check `prism-shell.log` for DWM thumbnail errors. Card chrome remaining visible with no live content is the intended fallback. |
| A `Win+…` shortcut stopped working | Exit Prism or remove `--experimental-windows-key`; use only the fallback hotkey while reporting the shortcut. |
