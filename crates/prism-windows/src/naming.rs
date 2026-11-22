//! Stable application identity and friendlier display names for Windows apps.

/// Normalize a Win32 image path into a stable dock identity.
///
/// Paths are case-insensitive and may arrive with the `\\?\` extended prefix.
/// The full path is retained so two different executables with the same stem
/// do not collide.
pub fn normalized_exe_id(path: &str) -> String {
    let path = path.trim().trim_matches('"');
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    format!("exe:{}", path.replace('/', r"\").to_lowercase())
}

pub fn file_stem(path: &str) -> String {
    let path = path.trim().trim_matches('"');
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    let name = path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .trim_end_matches(".exe");
    name.to_owned()
}

/// Small, deterministic display-name map for the common Windows apps. Unknown
/// processes fall back to their executable stem, not a fabricated product name.
pub fn friendly_app_name(path: &str) -> String {
    let stem = file_stem(path);
    let lower = stem.to_lowercase();
    match lower.as_str() {
        "explorer" => "File Explorer".to_owned(),
        "msedge" => "Microsoft Edge".to_owned(),
        "chrome" => "Google Chrome".to_owned(),
        "firefox" => "Firefox".to_owned(),
        "code" => "Visual Studio Code".to_owned(),
        "notepad" => "Notepad".to_owned(),
        "wt" | "windowsterminal" => "Terminal".to_owned(),
        "cmd" => "Command Prompt".to_owned(),
        "powershell" | "pwsh" => "PowerShell".to_owned(),
        "applicationframehost" => "Windows app".to_owned(),
        "systemsettings" => "Settings".to_owned(),
        "calculatorapp" => "Calculator".to_owned(),
        _ => title_case(&stem),
    }
}

pub fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => {
            let mut out = first.to_uppercase().collect::<String>();
            out.push_str(chars.as_str());
            out
        }
        None => "Application".to_owned(),
    }
}

/// Use the window title where present, but never leave a dock entry blank.
pub fn title_or_name(title: &str, name: &str) -> String {
    let title = title.trim();
    if title.is_empty() {
        name.to_owned()
    } else {
        title.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_ids_are_case_and_prefix_insensitive() {
        assert_eq!(
            normalized_exe_id(r"\\?\C:\Windows\explorer.exe"),
            normalized_exe_id(r"c:\windows\EXPLORER.EXE")
        );
        assert_eq!(
            normalized_exe_id(r"C:\Program Files\App\app.exe"),
            r"exe:c:\program files\app\app.exe"
        );
    }

    #[test]
    fn friendly_names_do_not_leak_mock_identities() {
        assert_eq!(
            friendly_app_name(r"C:\Windows\explorer.exe"),
            "File Explorer"
        );
        assert_eq!(friendly_app_name(r"C:\Apps\MyThing.exe"), "MyThing");
        assert_eq!(file_stem(r"C:\Apps\MyThing.exe"), "MyThing");
    }

    #[test]
    fn blank_titles_fall_back_to_the_app_name() {
        assert_eq!(title_or_name("  ", "Notepad"), "Notepad");
        assert_eq!(
            title_or_name("Untitled - Notepad", "Notepad"),
            "Untitled - Notepad"
        );
    }
}
