//! Portable state for the independent taskbar-recovery mechanism.
//!
//! The marker file is the contract between the running shell, the standalone
//! `prism-recovery.exe` watchdog, and a later manual recovery run. It records
//! which native taskbar windows Prism hid and whether each was visible before.
//! The watchdog never guesses: without an active marker it does not touch
//! Explorer.

use std::path::{Path, PathBuf};

use thiserror::Error;

pub const MARKER_HEADER: &str = "prism-taskbar-replacement-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskbarKind {
    Primary,
    Secondary,
}

impl TaskbarKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "primary" => Some(Self::Primary),
            "secondary" => Some(Self::Secondary),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskbarWindowState {
    pub hwnd: u64,
    pub kind: TaskbarKind,
    pub was_visible: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReplacementMarker {
    pub windows: Vec<TaskbarWindowState>,
}

impl ReplacementMarker {
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    pub fn to_text(&self) -> String {
        let mut text = String::from(MARKER_HEADER);
        text.push('\n');
        for window in &self.windows {
            text.push_str(&format!(
                "{:#x} {} {}\n",
                window.hwnd,
                window.kind.as_str(),
                u8::from(window.was_visible)
            ));
        }
        text
    }

    pub fn parse(text: &str) -> Result<Self, MarkerError> {
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some(MARKER_HEADER) {
            return Err(MarkerError::Header);
        }
        let mut windows = Vec::new();
        for (index, line) in lines.enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut fields = line.split_whitespace();
            let hwnd = fields
                .next()
                .ok_or(MarkerError::Line(index + 2))?
                .trim_start_matches("0x");
            let hwnd = u64::from_str_radix(hwnd, 16).map_err(|_| MarkerError::Line(index + 2))?;
            let kind = fields
                .next()
                .and_then(TaskbarKind::parse)
                .ok_or(MarkerError::Line(index + 2))?;
            let was_visible = match fields.next() {
                Some("1") => true,
                Some("0") => false,
                _ => return Err(MarkerError::Line(index + 2)),
            };
            if fields.next().is_some() {
                return Err(MarkerError::Line(index + 2));
            }
            windows.push(TaskbarWindowState {
                hwnd,
                kind,
                was_visible,
            });
        }
        Ok(Self { windows })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MarkerError {
    #[error("taskbar-replacement marker has an unknown header")]
    Header,
    #[error("taskbar-replacement marker line {0} is malformed")]
    Line(usize),
}

/// Default marker path. The watchdog and shell derive the same path without
/// sharing process state, which is what makes recovery independent.
pub fn marker_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(|base| {
            PathBuf::from(base)
                .join("Prism")
                .join("taskbar-replacement.state")
        })
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub fn read_marker(path: &Path) -> Result<Option<ReplacementMarker>, MarkerError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(MarkerError::Header),
    };
    ReplacementMarker::parse(&text).map(Some)
}

pub fn write_marker(path: &Path, marker: &ReplacementMarker) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, marker.to_text())?;
    // `rename` replaces an existing file on both Windows (MoveFileEx) and Unix.
    std::fs::rename(temporary, path)
}

pub fn remove_marker(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryCommand {
    Restore,
    Status,
    WatchParent(u32),
    Help,
}

pub fn usage() -> &'static str {
    "Prism Shell recovery utility

USAGE:
  prism-recovery --restore
  prism-recovery --status
  prism-recovery --watch-parent <pid>

--restore        Show the native Windows taskbar windows and clear the marker.
--status         Report whether a replacement marker is active.
--watch-parent   Watchdog: restore the native taskbar if the given Prism shell
                 process exits or stops sending heartbeats for several seconds.

This tool does not depend on the Prism shell process remaining responsive. It
never terminates Explorer and never changes registry or startup settings."
}

pub fn parse_args<I>(args: I) -> Result<RecoveryCommand, String>
where
    I: IntoIterator<Item = String>,
{
    let mut iter = args.into_iter();
    let mut command = None;
    while let Some(argument) = iter.next() {
        let (flag, inline) = match argument.split_once('=') {
            Some((flag, value)) => (flag.to_owned(), Some(value.to_owned())),
            None => (argument, None),
        };
        let mut value = || -> Result<String, String> {
            inline
                .clone()
                .or_else(|| iter.next())
                .ok_or_else(|| format!("{flag} requires a value"))
        };
        let parsed = match flag.as_str() {
            "-h" | "--help" => RecoveryCommand::Help,
            "--restore" => RecoveryCommand::Restore,
            "--status" => RecoveryCommand::Status,
            "--watch-parent" => {
                let pid: u32 = value()?
                    .parse()
                    .map_err(|_| "--watch-parent requires a numeric process id".to_owned())?;
                if pid == 0 {
                    return Err("--watch-parent requires a non-zero process id".to_owned());
                }
                RecoveryCommand::WatchParent(pid)
            }
            other => return Err(format!("unknown option '{other}'")),
        };
        if command.replace(parsed).is_some() {
            return Err("only one recovery command may be specified".to_owned());
        }
    }
    command.ok_or_else(|| {
        "no recovery command specified; use --restore, --status, or --watch-parent".to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_path(tag: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "prism-marker-{}-{}-{tag}.state",
            std::process::id(),
            id
        ))
    }

    fn marker() -> ReplacementMarker {
        ReplacementMarker {
            windows: vec![
                TaskbarWindowState {
                    hwnd: 0x1234,
                    kind: TaskbarKind::Primary,
                    was_visible: true,
                },
                TaskbarWindowState {
                    hwnd: 0xABCD,
                    kind: TaskbarKind::Secondary,
                    was_visible: false,
                },
            ],
        }
    }

    #[test]
    fn marker_round_trips_with_original_visibility() {
        let text = marker().to_text();
        assert_eq!(ReplacementMarker::parse(&text).unwrap(), marker());
        let path = temp_path("roundtrip");
        write_marker(&path, &marker()).unwrap();
        assert_eq!(read_marker(&path).unwrap(), Some(marker()));
        remove_marker(&path).unwrap();
        assert_eq!(read_marker(&path).unwrap(), None);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn marker_rejects_unknown_or_malformed_content() {
        assert_eq!(
            ReplacementMarker::parse("something-else\n").unwrap_err(),
            MarkerError::Header
        );
        assert_eq!(
            ReplacementMarker::parse(&format!("{MARKER_HEADER}\nzzz primary 1\n")).unwrap_err(),
            MarkerError::Line(2)
        );
        assert_eq!(
            ReplacementMarker::parse(&format!("{MARKER_HEADER}\n0x1 primary maybe\n")).unwrap_err(),
            MarkerError::Line(2)
        );
    }

    #[test]
    fn recovery_commands_are_explicit_and_validated() {
        assert_eq!(
            parse_args(["--restore".to_owned()]).unwrap(),
            RecoveryCommand::Restore
        );
        assert_eq!(
            parse_args(["--watch-parent".to_owned(), "4242".to_owned()]).unwrap(),
            RecoveryCommand::WatchParent(4242)
        );
        assert!(parse_args(["--watch-parent".to_owned(), "0".to_owned()]).is_err());
        assert!(parse_args(["--unknown".to_owned()]).is_err());
        assert!(parse_args(["--restore".to_owned(), "--status".to_owned()]).is_err());
        assert!(parse_args(Vec::new()).is_err());
    }
}
