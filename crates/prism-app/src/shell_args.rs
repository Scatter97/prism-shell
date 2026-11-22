//! Command-line opt-ins for the Windows native shell.
//!
//! These flags are deliberately verbose and are never required for ordinary
//! use. They only turn on gated shell integrations; the default remains a
//! non-invasive overlay alongside Explorer.

use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct ShellArgs {
    pub taskbar_replacement: bool,
    pub windows_key_launcher: bool,
    pub windows_key_fallback_hotkey: bool,
    pub config: Option<PathBuf>,
    pub help: bool,
}

pub fn usage() -> &'static str {
    "Prism Shell — native Windows shell

USAGE:
  prism-shell [OPTIONS]

OPTIONS:
  --config <path>                              load a specific config JSON
  --experimental-taskbar-replacement           opt in to hiding the native taskbar
  --experimental-windows-key                   standalone Win key toggles Prism launcher
  --experimental-windows-key-fallback-hotkey   also register Ctrl+Alt+Space
  -h, --help                                   print this help

SAFETY:
  Taskbar replacement is disabled by default. It also requires
  prism-recovery.exe next to prism-shell.exe. Do not enable it until the
  recovery watchdog has been validated on the target Windows 11 machine.
  The Windows-key hook never suppresses Win+letter/number combinations.

Without these flags Prism runs alongside Explorer and does not hide the native
taskbar or intercept Windows shortcuts."
}

pub fn parse<I>(args: I) -> Result<ShellArgs, String>
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = ShellArgs::default();
    let mut iter = args.into_iter();
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
        match flag.as_str() {
            "-h" | "--help" => parsed.help = true,
            "--config" => parsed.config = Some(PathBuf::from(value()?)),
            "--experimental-taskbar-replacement" => parsed.taskbar_replacement = true,
            "--experimental-windows-key" => parsed.windows_key_launcher = true,
            "--experimental-windows-key-fallback-hotkey" => {
                parsed.windows_key_fallback_hotkey = true;
            }
            other => return Err(format!("unknown option '{other}'")),
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_vec(args: &[&str]) -> Result<ShellArgs, String> {
        parse(args.iter().map(|value| (*value).to_owned()))
    }

    #[test]
    fn default_has_every_integration_disabled() {
        let args = parse_vec(&[]).unwrap();
        assert!(!args.taskbar_replacement);
        assert!(!args.windows_key_launcher);
        assert!(!args.windows_key_fallback_hotkey);
        assert!(args.config.is_none());
    }

    #[test]
    fn opt_in_flags_are_explicit() {
        let args = parse_vec(&[
            "--experimental-taskbar-replacement",
            "--experimental-windows-key",
            "--experimental-windows-key-fallback-hotkey",
        ])
        .unwrap();
        assert!(args.taskbar_replacement);
        assert!(args.windows_key_launcher);
        assert!(args.windows_key_fallback_hotkey);
    }

    #[test]
    fn config_accepts_inline_and_separate_paths() {
        assert_eq!(
            parse_vec(&["--config", "a.json"]).unwrap().config,
            Some(PathBuf::from("a.json"))
        );
        assert_eq!(
            parse_vec(&["--config=b.json"]).unwrap().config,
            Some(PathBuf::from("b.json"))
        );
        assert!(parse_vec(&["--config"]).is_err());
        assert!(parse_vec(&["--unknown"]).is_err());
    }
}
