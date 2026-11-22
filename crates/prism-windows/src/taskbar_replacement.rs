//! Opt-in native-taskbar replacement with an independent recovery watchdog.
//!
//! This is the only module allowed to hide `Shell_TrayWnd` /
//! `Shell_SecondaryTrayWnd`. It never terminates Explorer, changes registry
//! values, registers Prism as the system shell, or installs persistent hooks.
//! The operation is reversible with `ShowWindow(SW_SHOW)` and is guarded by:
//!
//! 1. A standalone `prism-recovery.exe` watchdog started before hiding.
//! 2. A heartbeat event that the watchdog waits on; process death or a freeze
//!    stops the heartbeat and restores the taskbar after a short timeout.
//! 3. A marker file recording which taskbars Prism hid and whether each was
//!    originally visible, so recovery is idempotent and does not invent state.
//! 4. A startup stale-marker check and a Control Center "Restore bar" action.
//!
//! The class names are observed Explorer internals, not a long-term Microsoft
//! contract. The feature is disabled by default and must be validated on real
//! Windows 11 before use.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HWND, LPARAM, WAIT_EVENT, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::System::Threading::{
    CreateEventW, OpenProcess, PROCESS_SYNCHRONIZE, SetEvent, WaitForMultipleObjects,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, IsWindowVisible, SW_HIDE, SW_SHOW, ShowWindow,
};
use windows::core::{BOOL, PCWSTR};

use crate::recovery::{self, RecoveryCommand, ReplacementMarker, TaskbarKind, TaskbarWindowState};

pub const PRIMARY_TASKBAR_CLASS: &str = "Shell_TrayWnd";
pub const SECONDARY_TASKBAR_CLASS: &str = "Shell_SecondaryTrayWnd";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const HEARTBEAT_TIMEOUT_MS: u32 = 4_000;

#[derive(Clone, Copy, Debug)]
struct NativeTaskbar {
    hwnd: HWND,
    kind: TaskbarKind,
    visible: bool,
}

unsafe extern "system" fn enum_taskbar(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let taskbars = unsafe { &mut *(lparam.0 as *mut Vec<NativeTaskbar>) };
    let mut class = [0u16; 128];
    let length = unsafe { GetClassNameW(hwnd, &mut class) }.max(0) as usize;
    let class = String::from_utf16_lossy(&class[..length.min(class.len())]);
    let kind = if class == PRIMARY_TASKBAR_CLASS {
        TaskbarKind::Primary
    } else if class == SECONDARY_TASKBAR_CLASS {
        TaskbarKind::Secondary
    } else {
        return BOOL(1);
    };
    taskbars.push(NativeTaskbar {
        hwnd,
        kind,
        visible: unsafe { IsWindowVisible(hwnd).as_bool() },
    });
    BOOL(1)
}

fn find_native_taskbars() -> Vec<NativeTaskbar> {
    let mut taskbars: Vec<NativeTaskbar> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(enum_taskbar), LPARAM(&mut taskbars as *mut _ as isize));
    }
    taskbars
}

fn show_taskbar(hwnd: HWND) -> bool {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        IsWindowVisible(hwnd).as_bool()
    }
}

fn hide_taskbar(hwnd: HWND) -> bool {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
        !IsWindowVisible(hwnd).as_bool()
    }
}

fn marker_path() -> Result<PathBuf, String> {
    recovery::marker_path().ok_or_else(|| "LOCALAPPDATA is unavailable".to_owned())
}

fn write_marker(marker: &ReplacementMarker) -> Result<(), String> {
    let path = marker_path()?;
    recovery::write_marker(&path, marker)
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

fn clear_marker() {
    if let Ok(path) = marker_path() {
        let _ = recovery::remove_marker(&path);
    }
}

fn read_marker() -> Result<Option<ReplacementMarker>, String> {
    let path = marker_path()?;
    recovery::read_marker(&path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))
}

/// Hide every currently visible native taskbar and return the recorded
/// pre-hide state. Writes the marker before hiding so a crash at any point is
/// recoverable.
pub fn hide_native_taskbars() -> Result<ReplacementMarker, String> {
    let taskbars = find_native_taskbars();
    let primary = taskbars
        .iter()
        .find(|taskbar| taskbar.kind == TaskbarKind::Primary)
        .ok_or_else(|| "the primary Windows taskbar window was not found".to_owned())?;
    if !primary.visible {
        return Err(
            "the primary Windows taskbar is already hidden; refusing to take ownership".to_owned(),
        );
    }

    let marker = ReplacementMarker {
        windows: taskbars
            .iter()
            .filter(|taskbar| taskbar.visible)
            .map(|taskbar| TaskbarWindowState {
                hwnd: taskbar.hwnd.0 as usize as u64,
                kind: taskbar.kind,
                was_visible: true,
            })
            .collect(),
    };
    write_marker(&marker)?;

    for taskbar in taskbars.iter().filter(|taskbar| taskbar.visible) {
        if !hide_taskbar(taskbar.hwnd) {
            // Never leave a partially hidden desktop. Keep the marker if any
            // restore call fails; the heartbeat is dropped by the caller, so
            // the watchdog retries the recorded state after its timeout.
            let mut restored_all = true;
            for other in taskbars.iter().filter(|other| other.visible) {
                if !show_taskbar(other.hwnd) {
                    restored_all = false;
                }
            }
            if restored_all {
                clear_marker();
            }
            return Err(format!(
                "Windows did not hide a {} taskbar; restore attempted (marker {})",
                taskbar.kind.as_str(),
                if restored_all { "cleared" } else { "retained" }
            ));
        }
    }

    tracing::info!(
        taskbars = marker.windows.len(),
        "native Windows taskbar hidden; recovery marker active"
    );
    Ok(marker)
}

/// Whether Explorer's primary taskbar is currently visible. Used by the shell
/// to stop replacement mode if Explorer restarts or another tool restores the
/// native taskbar; Prism never fights to hide it again automatically.
pub fn native_primary_taskbar_visible() -> bool {
    find_native_taskbars()
        .iter()
        .find(|taskbar| taskbar.kind == TaskbarKind::Primary)
        .is_some_and(|taskbar| taskbar.visible)
}

/// Restore the taskbars recorded by a marker. Called by the watchdog, the
/// shell's clean shutdown, and the Control Center recovery action.
pub fn restore_from_marker(marker: &ReplacementMarker) -> Result<(), String> {
    let taskbars = find_native_taskbars();
    if taskbars.is_empty() {
        return Err(
            "no native Windows taskbar windows were found; keeping the recovery marker".to_owned(),
        );
    }
    let mut failed = Vec::new();

    // The primary taskbar is always shown if it was recorded visible. For
    // secondary taskbars, match the recorded HWND where possible. Explorer can
    // recreate all of them with new HWNDs; in that case show all current
    // secondaries rather than leaving a hidden one behind.
    let recorded_secondaries: Vec<u64> = marker
        .windows
        .iter()
        .filter(|window| window.was_visible && window.kind == TaskbarKind::Secondary)
        .map(|window| window.hwnd)
        .collect();
    let mut unmatched_secondary = false;
    for recorded in &recorded_secondaries {
        let found = taskbars.iter().any(|taskbar| {
            taskbar.kind == TaskbarKind::Secondary && taskbar.hwnd.0 as usize as u64 == *recorded
        });
        if !found {
            unmatched_secondary = true;
        }
    }

    for taskbar in &taskbars {
        let restore = match taskbar.kind {
            TaskbarKind::Primary => marker
                .windows
                .iter()
                .any(|window| window.was_visible && window.kind == TaskbarKind::Primary),
            TaskbarKind::Secondary => {
                let recorded = recorded_secondaries.contains(&(taskbar.hwnd.0 as usize as u64));
                recorded || (unmatched_secondary && !recorded_secondaries.is_empty())
            }
        };
        if restore && !show_taskbar(taskbar.hwnd) {
            failed.push(taskbar.kind.as_str());
        }
    }

    if failed.is_empty() {
        clear_marker();
        tracing::info!(
            taskbars = marker.windows.len(),
            "native Windows taskbar restored"
        );
        Ok(())
    } else {
        Err(format!(
            "Windows did not show {} taskbar window(s); run prism-recovery --restore",
            failed.join(", ")
        ))
    }
}

/// Emergency restore: show every native taskbar currently present, regardless
/// of the marker. Manual recovery must not depend on Prism having run.
pub fn restore_all_native_taskbars() -> Result<usize, String> {
    let taskbars = find_native_taskbars();
    if taskbars.is_empty() {
        return Err(
            "no native Windows taskbar windows were found; is Explorer running?".to_owned(),
        );
    }
    let mut restored = 0;
    for taskbar in &taskbars {
        if show_taskbar(taskbar.hwnd) {
            restored += 1;
        }
    }
    clear_marker();
    if restored == 0 {
        Err("Windows taskbar windows were found but could not be shown".to_owned())
    } else {
        tracing::info!(restored, "native Windows taskbar restore requested");
        Ok(restored)
    }
}

/// If a previous Prism process left a marker behind, restore before the new
/// shell starts. This is a belt-and-braces path if the watchdog was killed too.
pub fn recover_stale_marker() {
    match read_marker() {
        Ok(Some(marker)) if !marker.is_empty() => {
            tracing::warn!(
                taskbars = marker.windows.len(),
                "stale taskbar-replacement marker found; restoring native taskbar"
            );
            if let Err(error) = restore_from_marker(&marker) {
                tracing::error!(%error, "stale taskbar restore failed");
            }
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "could not inspect taskbar-replacement marker"),
    }
}

fn heartbeat_name(parent_pid: u32) -> Vec<u16> {
    format!("Local\\PrismShellHeartbeat-v1-{parent_pid}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

/// A named, auto-reset heartbeat event owned by the shell. The watchdog opens
/// the same name. Process death or suspension stops the pulses.
pub struct Heartbeat {
    handle: HANDLE,
}

impl Heartbeat {
    pub fn create(parent_pid: u32) -> Result<Self, String> {
        let name = heartbeat_name(parent_pid);
        let handle = unsafe { CreateEventW(None, false, false, PCWSTR(name.as_ptr())) }
            .map_err(|error| format!("could not create recovery heartbeat: {error}"))?;
        Ok(Self { handle })
    }

    pub fn pulse(&self) {
        unsafe {
            let _ = SetEvent(self.handle);
        }
    }
}

impl Drop for Heartbeat {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

fn watchdog_event(parent_pid: u32) -> Result<HANDLE, String> {
    let name = heartbeat_name(parent_pid);
    unsafe { CreateEventW(None, false, false, PCWSTR(name.as_ptr())) }
        .map_err(|error| format!("could not open the shell heartbeat: {error}"))
}

fn recovery_executable() -> Result<PathBuf, String> {
    let directory = std::env::current_exe()
        .map_err(|error| format!("could not locate prism-shell.exe: {error}"))?
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "prism-shell.exe has no parent directory".to_owned())?;
    let candidate = directory.join("prism-recovery.exe");
    if candidate.is_file() {
        Ok(candidate)
    } else {
        Err(format!(
            "{} is missing; taskbar replacement is disabled",
            candidate.display()
        ))
    }
}

/// Start the standalone watchdog process. The shell does not keep a handle to
/// it: the heartbeat and marker are the only contracts, so the watchdog keeps
/// working if the shell crashes or hangs.
pub fn spawn_watchdog(parent_pid: u32) -> Result<PathBuf, String> {
    let executable = recovery_executable()?;
    let mut child = Command::new(&executable)
        .arg("--watch-parent")
        .arg(parent_pid.to_string())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|error| format!("could not start {}: {error}", executable.display()))?;

    // Catch an immediately failing binary before the taskbar is touched.
    std::thread::sleep(Duration::from_millis(80));
    match child.try_wait() {
        Ok(Some(status)) => Err(format!(
            "the recovery watchdog exited immediately ({status}); taskbar replacement is disabled"
        )),
        Ok(None) => {
            tracing::info!(watchdog = %executable.display(), "taskbar recovery watchdog armed");
            Ok(executable)
        }
        Err(error) => Err(format!("could not check the recovery watchdog: {error}")),
    }
}

/// Watchdog entry point. It depends only on the heartbeat event, the parent
/// process handle, and the marker file, never on Prism's message loop or main
/// thread.
pub fn run_watchdog(parent_pid: u32) -> Result<(), String> {
    let event = watchdog_event(parent_pid)?;
    let parent = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, parent_pid) }.ok();
    let process_wait_index = parent.map(|_| WAIT_EVENT(WAIT_OBJECT_0.0 + 1));
    let mut idle_timeouts = 0u32;

    let result = loop {
        let handles: Vec<HANDLE> = match parent {
            Some(parent) => vec![event, parent],
            None => vec![event],
        };
        let wait = unsafe { WaitForMultipleObjects(&handles, false, HEARTBEAT_TIMEOUT_MS) };
        if wait == WAIT_OBJECT_0 {
            idle_timeouts = 0;
            continue;
        }

        let parent_exited = process_wait_index == Some(wait);
        if wait != WAIT_TIMEOUT && !parent_exited {
            break Err(format!("heartbeat wait failed (result {})", wait.0));
        }

        match read_marker() {
            Ok(Some(marker)) if !marker.is_empty() => {
                append_log(if parent_exited {
                    "shell exited: restoring the native Windows taskbar"
                } else {
                    "heartbeat stopped: restoring the native Windows taskbar"
                });
                break restore_from_marker(&marker);
            }
            Ok(_) => {
                if parent_exited {
                    append_log("shell exited with no active replacement marker");
                    break Ok(());
                }
                // The shell may still be starting, or paused before hiding.
                // Without a process handle, avoid leaving an orphan watchdog
                // forever; with one, keep waiting for the shell's decision.
                idle_timeouts += 1;
                if parent.is_none() && idle_timeouts >= 15 {
                    append_log("no marker after 60 seconds; watchdog exiting");
                    break Ok(());
                }
            }
            Err(error) => {
                append_log(&format!("watchdog could not read the marker: {error}"));
                // An unreadable marker is exactly when emergency recovery is
                // safest: show the taskbar rather than leave the desktop bare.
                break restore_all_native_taskbars().map(|_| ());
            }
        }
    };

    if let Some(parent) = parent {
        unsafe {
            let _ = CloseHandle(parent);
        }
    }
    result
}

fn append_log(message: &str) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        let path = PathBuf::from(base)
            .join("Prism")
            .join("logs")
            .join("prism-recovery.log");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{timestamp} {message}");
        }
    }
}

/// `prism-recovery.exe` entry point.
pub fn run_recovery_cli() -> i32 {
    let command = match recovery::parse_args(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("{error}\n\n{}", recovery::usage());
            return 2;
        }
    };

    let result = match command {
        RecoveryCommand::Help => {
            println!("{}", recovery::usage());
            return 0;
        }
        RecoveryCommand::Restore => restore_all_native_taskbars().map(|count| {
            println!("Restored {count} native Windows taskbar window(s).");
        }),
        RecoveryCommand::Status => match read_marker() {
            Ok(Some(marker)) if !marker.is_empty() => {
                println!(
                    "Taskbar replacement marker is ACTIVE ({} taskbar window(s)).",
                    marker.windows.len()
                );
                Ok(())
            }
            Ok(_) => {
                println!("No active taskbar replacement marker.");
                Ok(())
            }
            Err(error) => Err(error),
        },
        RecoveryCommand::WatchParent(pid) => run_watchdog(pid),
    };

    match result {
        Ok(()) => 0,
        Err(error) => {
            append_log(&format!("recovery command failed: {error}"));
            eprintln!("{error}");
            1
        }
    }
}
