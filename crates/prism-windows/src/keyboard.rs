//! Portable standalone-Windows-key recognition.
//!
//! The Win32 hook is thin; all state transitions live here so they can be
//! unit-tested without a Windows session.
//!
//! Safety model:
//!
//! - A physical Windows key-down is swallowed, so Windows never enters its
//!   "Windows key held" state for a possible standalone tap.
//! - If another key is pressed while Windows is held, the hook asks the caller
//!   to replay a synthetic Windows key-down before that key reaches the OS.
//!   Win+E, Win+R, Win+L, Win+Shift+S, Win+Tab and every other native shortcut
//!   therefore keep their normal meaning.
//! - A standalone Windows key-up is swallowed and toggles the Prism launcher.
//!   Because the matching key-down was also swallowed, no modifier state is
//!   left stuck and the native Start menu never sees a complete tap.
//! - Injected events are never suppressed and never change the tracker state.
//!
//! If replaying synthetic input fails at runtime, the controller logs it and
//! the configurable Ctrl+Alt+Space fallback remains available.

pub const VK_LWIN_VALUE: u32 = 0x5B;
pub const VK_RWIN_VALUE: u32 = 0x5C;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyDecision {
    /// Deliver the event to Windows unchanged.
    PassThrough,
    /// Swallow the event.
    Suppress,
    /// Swallow the event and toggle the Prism launcher.
    SwallowAndToggleLauncher,
    /// Inject a synthetic key-down for this Windows key, then deliver the
    /// current event unchanged so the native combination works.
    ReplayWindowsDown(u32),
}

#[derive(Clone, Debug, Default)]
pub struct WinKeyTracker {
    win_down: Option<u32>,
    combined: bool,
    replayed: bool,
}

impl WinKeyTracker {
    pub const fn new() -> Self {
        Self {
            win_down: None,
            combined: false,
            replayed: false,
        }
    }

    pub fn is_win_down(&self) -> bool {
        self.win_down.is_some()
    }

    /// Process one low-level keyboard event.
    pub fn handle(&mut self, vk: u32, key_up: bool, injected: bool) -> KeyDecision {
        if injected {
            return KeyDecision::PassThrough;
        }

        let is_win = is_windows_key(vk);
        if !key_up {
            if is_win {
                if let Some(held) = self.win_down {
                    if held == vk {
                        // Keyboard auto-repeat for the same Windows key: keep
                        // swallowing it, but never toggle early or replay it.
                        return KeyDecision::Suppress;
                    }
                    // Both Windows keys held at once: treat it as a
                    // combination and replay the first Windows key-down once.
                    self.combined = true;
                    if self.replayed {
                        return KeyDecision::PassThrough;
                    }
                    self.replayed = true;
                    return KeyDecision::ReplayWindowsDown(held);
                }
                self.win_down = Some(vk);
                self.combined = false;
                self.replayed = false;
                // Never let a standalone tap reach the shell.
                return KeyDecision::Suppress;
            }

            if self.win_down.is_some() {
                self.combined = true;
                if !self.replayed {
                    self.replayed = true;
                    return KeyDecision::ReplayWindowsDown(self.win_down.unwrap_or(vk));
                }
            }
            return KeyDecision::PassThrough;
        }

        if !is_win {
            return KeyDecision::PassThrough;
        }

        if self.win_down.is_none() {
            return KeyDecision::PassThrough;
        }

        let tapped_alone = !self.combined;
        self.win_down = None;
        self.combined = false;
        self.replayed = false;
        if tapped_alone {
            KeyDecision::SwallowAndToggleLauncher
        } else {
            // The OS has seen the synthetic Windows key-down; let it see the
            // matching release so no modifier remains logically held.
            KeyDecision::PassThrough
        }
    }

    /// Release tracking without producing an action, used when the hook is
    /// removed while a Windows key is held.
    pub fn reset(&mut self) {
        self.win_down = None;
        self.combined = false;
        self.replayed = false;
    }
}

pub fn is_windows_key(vk: u32) -> bool {
    vk == VK_LWIN_VALUE || vk == VK_RWIN_VALUE
}

pub const HOTKEY_ALT: u32 = 0x0001;
pub const HOTKEY_CONTROL: u32 = 0x0002;
pub const HOTKEY_SHIFT: u32 = 0x0004;
pub const HOTKEY_WIN: u32 = 0x0008;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    /// Win32 MOD_* bit values (Alt=1, Control=2, Shift=4, Win=8).
    pub modifiers: u32,
    pub virtual_key: u32,
}

/// Parse a small, explicit fallback-hotkey grammar such as `Ctrl+Alt+Space`.
///
/// A modifier is required: a bare letter must never be registered globally.
pub fn parse_hotkey(value: &str) -> Result<Hotkey, String> {
    let mut modifiers = 0u32;
    let mut key: Option<u32> = None;
    for part in value
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let lower = part.to_ascii_lowercase();
        let modifier = match lower.as_str() {
            "alt" => Some(HOTKEY_ALT),
            "ctrl" | "control" => Some(HOTKEY_CONTROL),
            "shift" => Some(HOTKEY_SHIFT),
            "win" | "windows" | "meta" | "super" => Some(HOTKEY_WIN),
            _ => None,
        };
        if let Some(modifier) = modifier {
            modifiers |= modifier;
            continue;
        }
        if key.is_some() {
            return Err(format!("'{value}' contains more than one non-modifier key"));
        }
        key = Some(
            parse_key(part)
                .ok_or_else(|| format!("'{part}' is not a supported fallback-hotkey key"))?,
        );
    }
    let key = key.ok_or_else(|| format!("'{value}' has no key"))?;
    if modifiers == 0 {
        return Err(format!(
            "'{value}' must include at least one modifier (Ctrl, Alt, Shift, or Win)"
        ));
    }
    Ok(Hotkey {
        modifiers,
        virtual_key: key,
    })
}

fn parse_key(value: &str) -> Option<u32> {
    let upper = value.to_ascii_uppercase();
    if upper.len() == 1 {
        let character = upper.as_bytes()[0];
        if character.is_ascii_alphabetic() || character.is_ascii_digit() {
            return Some(u32::from(character));
        }
    }
    Some(match upper.as_str() {
        "SPACE" => 0x20,
        "TAB" => 0x09,
        "ENTER" | "RETURN" => 0x0D,
        "ESC" | "ESCAPE" => 0x1B,
        "LEFT" => 0x25,
        "UP" => 0x26,
        "RIGHT" => 0x27,
        "DOWN" => 0x28,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" | "PGUP" => 0x21,
        "PAGEDOWN" | "PGDN" => 0x22,
        "INSERT" | "INS" => 0x2D,
        "DELETE" | "DEL" => 0x2E,
        _ => {
            if let Some(number) = upper.strip_prefix('F')
                && let Ok(number) = number.parse::<u32>()
                && (1..=24).contains(&number)
            {
                return Some(0x70 + number - 1);
            }
            return None;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_left_and_right_windows_taps_are_swallowed_and_toggle() {
        for vk in [VK_LWIN_VALUE, VK_RWIN_VALUE] {
            let mut tracker = WinKeyTracker::new();
            assert_eq!(tracker.handle(vk, false, false), KeyDecision::Suppress);
            assert_eq!(
                tracker.handle(vk, true, false),
                KeyDecision::SwallowAndToggleLauncher
            );
            assert!(!tracker.is_win_down());
        }
    }

    #[test]
    fn win_combinations_replay_windows_down_and_are_not_swallowed() {
        for other in [0x45, 0x52, 0x4C, 0x44, 0x09, 0x53, 0x49, 0x58, 0x56, 0x25] {
            let mut tracker = WinKeyTracker::new();
            assert_eq!(
                tracker.handle(VK_LWIN_VALUE, false, false),
                KeyDecision::Suppress
            );
            assert_eq!(
                tracker.handle(other, false, false),
                KeyDecision::ReplayWindowsDown(VK_LWIN_VALUE),
                "combination vk {other:#x} must synthesize Windows down"
            );
            assert_eq!(tracker.handle(other, true, false), KeyDecision::PassThrough);
            assert_eq!(
                tracker.handle(VK_LWIN_VALUE, true, false),
                KeyDecision::PassThrough
            );
        }
    }

    #[test]
    fn modifier_only_combination_is_replayed_once() {
        let mut tracker = WinKeyTracker::new();
        tracker.handle(VK_LWIN_VALUE, false, false);
        assert_eq!(
            tracker.handle(0xA0, false, false), // VK_LSHIFT
            KeyDecision::ReplayWindowsDown(VK_LWIN_VALUE)
        );
        assert_eq!(tracker.handle(0x53, false, false), KeyDecision::PassThrough);
        assert_eq!(tracker.handle(0x53, true, false), KeyDecision::PassThrough);
        assert_eq!(tracker.handle(0xA0, true, false), KeyDecision::PassThrough);
        assert_eq!(
            tracker.handle(VK_LWIN_VALUE, true, false),
            KeyDecision::PassThrough
        );
    }

    #[test]
    fn auto_repeat_does_not_toggle_early_or_twice() {
        let mut tracker = WinKeyTracker::new();
        for _ in 0..8 {
            assert_eq!(
                tracker.handle(VK_LWIN_VALUE, false, false),
                KeyDecision::Suppress
            );
        }
        assert_eq!(
            tracker.handle(VK_LWIN_VALUE, true, false),
            KeyDecision::SwallowAndToggleLauncher
        );
        assert_eq!(
            tracker.handle(VK_LWIN_VALUE, true, false),
            KeyDecision::PassThrough
        );
    }

    #[test]
    fn both_windows_keys_are_a_combination_with_one_replay() {
        let mut tracker = WinKeyTracker::new();
        assert_eq!(
            tracker.handle(VK_LWIN_VALUE, false, false),
            KeyDecision::Suppress
        );
        assert_eq!(
            tracker.handle(VK_RWIN_VALUE, false, false),
            KeyDecision::ReplayWindowsDown(VK_LWIN_VALUE)
        );
        assert_eq!(
            tracker.handle(VK_RWIN_VALUE, true, false),
            KeyDecision::PassThrough
        );
        assert_eq!(
            tracker.handle(VK_LWIN_VALUE, true, false),
            KeyDecision::PassThrough
        );
    }

    #[test]
    fn injected_keys_never_change_state() {
        let mut tracker = WinKeyTracker::new();
        assert_eq!(
            tracker.handle(VK_LWIN_VALUE, false, true),
            KeyDecision::PassThrough
        );
        assert_eq!(
            tracker.handle(VK_LWIN_VALUE, true, true),
            KeyDecision::PassThrough
        );
        assert!(!tracker.is_win_down());
    }

    #[test]
    fn orphan_key_up_passes_through() {
        let mut tracker = WinKeyTracker::new();
        assert_eq!(
            tracker.handle(VK_LWIN_VALUE, true, false),
            KeyDecision::PassThrough
        );
    }

    #[test]
    fn fallback_hotkeys_are_parsed_explicitly() {
        assert_eq!(
            parse_hotkey("Ctrl+Alt+Space").unwrap(),
            Hotkey {
                modifiers: HOTKEY_CONTROL | HOTKEY_ALT,
                virtual_key: 0x20
            }
        );
        assert_eq!(
            parse_hotkey("ctrl + shift + f12").unwrap(),
            Hotkey {
                modifiers: HOTKEY_CONTROL | HOTKEY_SHIFT,
                virtual_key: 0x7B
            }
        );
        assert_eq!(
            parse_hotkey("Win+ArrowLeft").unwrap_err(),
            "'ArrowLeft' is not a supported fallback-hotkey key"
        );
        assert_eq!(
            parse_hotkey("Ctrl+Alt").unwrap_err(),
            "'Ctrl+Alt' has no key"
        );
        assert_eq!(
            parse_hotkey("Space").unwrap_err(),
            "'Space' must include at least one modifier (Ctrl, Alt, Shift, or Win)"
        );
        assert!(parse_hotkey("Ctrl+A+B").is_err());
    }
}
