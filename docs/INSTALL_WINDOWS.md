# Installing Prism Shell on Windows 11

Status: **source build only. No signed binary release or installer yet.** The Windows runtime is
**unverified**; see [`WINDOWS_VALIDATION.md`](WINDOWS_VALIDATION.md) before running it.

Prism Shell is a normal desktop application. It runs alongside Explorer, does not replace the
Windows shell, does not hide the taskbar, and does not add itself to startup. Explorer remains
available at all times.

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
cargo build --release -p prism-app --bin prism-shell
```

The executable is:

```text
target\release\prism-shell.exe
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
- Click the Control Center trigger on the dock's right side, then use **Exit** to close Prism.
- Logs are written to `%LOCALAPPDATA%\Prism\logs\prism-shell.log`.

There is no console window in the release build. If Prism exits immediately, check the log and run
the debug build for more output:

```powershell
cargo run -p prism-app --bin prism-shell
```

## 5. Optional shortcut

Right-click `target\release\prism-shell.exe`, choose **Show more options → Send to → Desktop
(create shortcut)**. Do not add it to the Startup folder or Task Scheduler; automatic startup is
not supported in V1.

## 6. First-run checklist

At minimum, confirm:

- Explorer and the Windows taskbar are still visible.
- The dock appears above the taskbar and its transparent sides do not block desktop clicks.
- Clicking a pinned/running app launches or activates it.
- Control Center opens above the dock and closes on outside click or Escape.
- Exit removes Prism without affecting Explorer.

The full checklist, including DPI, focus, grouping, and recovery tests, is in
[`WINDOWS_VALIDATION.md`](WINDOWS_VALIDATION.md).

## 7. Uninstall

1. Exit Prism from the Control Center.
2. Delete the cloned `prism-shell` folder and the shortcut, if any.
3. Optionally delete `%LOCALAPPDATA%\Prism` (logs) and `%APPDATA%\Prism` (config).

Prism does not modify the registry, shell registration, startup entries, or the Windows taskbar.

## 8. Why there is no downloadable binary yet

Two project decisions block publishing a prebuilt release:

- The Slint license choice (GPLv3 or the royalty-free option with attribution) is unresolved.
  See `DECISIONS.md` D-019.
- The native Windows host has been cross-compiled but not executed on Windows 11, so its
  transparency, focus, activation, and Explorer coexistence are unverified. See
  `CURRENT_STATUS.md`.

Once those are resolved, a tagged GitHub Release can provide an unsigned test build. Until then,
building from source is the supported path.

## 9. Troubleshooting

| Symptom | Action |
|---|---|
| `cargo` or `rustup` not found | Reopen PowerShell; rerun the rustup installer if needed. |
| `link.exe` not found / `msvc` target missing | Add the **Desktop development with C++** workload and the Windows 11 SDK component; update rustup/Rust; reopen PowerShell. |
| Prism exits immediately | Read `%LOCALAPPDATA%\Prism\logs\prism-shell.log`; run the debug build. |
| Dock is invisible or has a black box | Record GPU/driver and scale; try `SLINT_BACKEND=software`; report the log. |
| Clicking apps does not activate them | Record whether the target is elevated; this is a known unverified foreground path. |
| Dock covers a maximized window | Expected: no AppBar work-area reservation in this build. |
| Windows taskbar disappeared | Prism does not hide it; stop Prism and verify Explorer is running. |
