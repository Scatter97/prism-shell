//! Win32 window host for the taskbar and its panels.
//!
//! All calls here are documented Win32/DWM APIs. Explorer is not touched: the
//! shell windows are normal tool windows placed inside the monitor work area.

use std::ffi::c_void;
use std::mem::size_of;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, MOD_NOREPEAT, RegisterHotKey,
    SendInput, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, UnregisterHotKey, VIRTUAL_KEY,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GWL_EXSTYLE, GWL_STYLE, GetWindowLongPtrW, HHOOK, HTCLIENT, HTTRANSPARENT,
    HWND_TOPMOST, KBDLLHOOKSTRUCT, LLKHF_INJECTED, LLKHF_LOWER_IL_INJECTED, MA_NOACTIVATE, SW_SHOW,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetForegroundWindow, SetWindowLongPtrW,
    SetWindowPos, SetWindowsHookExW, ShowWindow, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_ACTIVATE,
    WM_HOTKEY, WM_KEYDOWN, WM_KEYUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NCHITTEST, WM_SYSKEYUP,
    WS_CAPTION, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_MAXIMIZEBOX,
    WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
};

use crate::keyboard::{Hotkey, KeyDecision, WinKeyTracker};
use crate::layout::PhysicalRect;

static TASKBAR_DOCK: Mutex<Option<PhysicalRect>> = Mutex::new(None);
static CC_PANEL: Mutex<Option<PhysicalRect>> = Mutex::new(None);
static LAUNCHER_PANEL: Mutex<Option<PhysicalRect>> = Mutex::new(None);
static PREVIEW_PANEL: Mutex<Option<PhysicalRect>> = Mutex::new(None);
static CC_DEACTIVATED: AtomicBool = AtomicBool::new(false);
static LAUNCHER_DEACTIVATED: AtomicBool = AtomicBool::new(false);
static LAUNCHER_SHORTCUT: AtomicBool = AtomicBool::new(false);
static PREVIEW_HOVERED: AtomicBool = AtomicBool::new(false);
static KEY_TRACKER: Mutex<WinKeyTracker> = Mutex::new(WinKeyTracker::new());

const TASKBAR_SUBCLASS_ID: usize = 0x5052_4953;
const CC_SUBCLASS_ID: usize = 0x5052_4943;
const LAUNCHER_SUBCLASS_ID: usize = 0x5052_494C;
const PREVIEW_SUBCLASS_ID: usize = 0x5052_4950;
const FALLBACK_HOTKEY_ID: i32 = 0x5052;
// WM_MOUSELEAVE lives in Win32_UI_Controls in windows-rs; the numeric message
// is stable Win32 ABI and avoids pulling in the whole Controls feature.
const WM_MOUSELEAVE_MESSAGE: u32 = 0x02A3;

pub fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn hwnd_of(window: &slint::Window) -> Result<HWND, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window.window_handle();
    let raw = handle
        .window_handle()
        .map_err(|error| format!("window handle unavailable (window not created yet?): {error}"))?;
    match raw.as_raw() {
        RawWindowHandle::Win32(win32) => Ok(HWND(win32.hwnd.get() as *mut c_void)),
        _ => Err("the Slint window is not backed by a Win32 handle".to_owned()),
    }
}

pub fn system_scale() -> f32 {
    let dpi = unsafe { GetDpiForSystem() };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

pub fn window_scale(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 {
        system_scale()
    } else {
        dpi as f32 / 96.0
    }
}

pub fn primary_work_area() -> PhysicalRect {
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let work = info.rcWork;
            PhysicalRect::new(
                work.left,
                work.top,
                work.right - work.left,
                work.bottom - work.top,
            )
        } else {
            PhysicalRect::new(0, 0, 1920, 1080)
        }
    }
}

/// Full primary-monitor rectangle. Experimental taskbar replacement positions
/// Prism against this (the native taskbar no longer occupies work area).
pub fn primary_monitor_area() -> PhysicalRect {
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let monitor = info.rcMonitor;
            PhysicalRect::new(
                monitor.left,
                monitor.top,
                monitor.right - monitor.left,
                monitor.bottom - monitor.top,
            )
        } else {
            primary_work_area()
        }
    }
}

pub fn set_taskbar_dock_rect(rect: Option<PhysicalRect>) {
    if let Ok(mut dock) = TASKBAR_DOCK.lock() {
        *dock = rect;
    }
}

pub fn set_cc_panel_rect(rect: Option<PhysicalRect>) {
    if let Ok(mut panel) = CC_PANEL.lock() {
        *panel = rect;
    }
}

pub fn set_launcher_panel_rect(rect: Option<PhysicalRect>) {
    if let Ok(mut panel) = LAUNCHER_PANEL.lock() {
        *panel = rect;
    }
}

pub fn set_preview_panel_rect(rect: Option<PhysicalRect>) {
    if let Ok(mut panel) = PREVIEW_PANEL.lock() {
        *panel = rect;
    }
}

/// Native pointer-over-preview state. Slint's root TouchArea can report false
/// while the pointer is over a card (the card's own TouchArea is above it), so
/// the native WM_MOUSEMOVE/WM_MOUSELEAVE tracking is authoritative for keeping
/// the preview open.
pub fn preview_hovered() -> bool {
    PREVIEW_HOVERED.load(Ordering::Acquire)
}

pub fn reset_preview_hover() {
    PREVIEW_HOVERED.store(false, Ordering::Release);
}

fn screen_point(lparam: LPARAM) -> (i32, i32) {
    (
        (lparam.0 & 0xFFFF) as u16 as i16 as i32,
        ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32,
    )
}

pub fn take_cc_deactivation() -> bool {
    CC_DEACTIVATED.swap(false, Ordering::AcqRel)
}

pub fn take_launcher_deactivation() -> bool {
    LAUNCHER_DEACTIVATED.swap(false, Ordering::AcqRel)
}

pub fn clear_deactivation_flags() {
    CC_DEACTIVATED.store(false, Ordering::Release);
    LAUNCHER_DEACTIVATED.store(false, Ordering::Release);
}

unsafe extern "system" fn taskbar_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    match message {
        WM_HOTKEY if wparam.0 == FALLBACK_HOTKEY_ID as usize => {
            LAUNCHER_SHORTCUT.store(true, Ordering::Release);
            LRESULT(0)
        }
        WM_NCHITTEST => {
            let dock = TASKBAR_DOCK.lock().ok().and_then(|dock| *dock);
            if let Some(dock) = dock {
                let (x, y) = screen_point(lparam);
                if dock.contains(x, y) {
                    return LRESULT(HTCLIENT as isize);
                }
                return LRESULT(HTTRANSPARENT as isize);
            }
            LRESULT(HTCLIENT as isize)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        _ => unsafe { DefSubclassProc(hwnd, message, wparam, lparam) },
    }
}

unsafe extern "system" fn cc_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if message == WM_ACTIVATE && (wparam.0 & 0xFFFF) == 0 {
        CC_DEACTIVATED.store(true, Ordering::Release);
    }
    if message == WM_KEYDOWN && wparam.0 == 27 {
        CC_DEACTIVATED.store(true, Ordering::Release);
        return LRESULT(0);
    }
    if message == WM_NCHITTEST {
        let panel = CC_PANEL.lock().ok().and_then(|panel| *panel);
        if let Some(panel) = panel {
            let (x, y) = screen_point(lparam);
            if !panel.contains(x, y) {
                return LRESULT(HTTRANSPARENT as isize);
            }
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn launcher_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if message == WM_ACTIVATE && (wparam.0 & 0xFFFF) == 0 {
        LAUNCHER_DEACTIVATED.store(true, Ordering::Release);
    }
    if message == WM_KEYDOWN && wparam.0 == 27 {
        LAUNCHER_DEACTIVATED.store(true, Ordering::Release);
        return LRESULT(0);
    }
    if message == WM_NCHITTEST {
        let panel = LAUNCHER_PANEL.lock().ok().and_then(|panel| *panel);
        if let Some(panel) = panel {
            let (x, y) = screen_point(lparam);
            if !panel.contains(x, y) {
                return LRESULT(HTTRANSPARENT as isize);
            }
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn preview_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if message == WM_NCHITTEST {
        let panel = PREVIEW_PANEL.lock().ok().and_then(|panel| *panel);
        let Some(panel) = panel else {
            return LRESULT(HTTRANSPARENT as isize);
        };
        let (x, y) = screen_point(lparam);
        if !panel.contains(x, y) {
            return LRESULT(HTTRANSPARENT as isize);
        }
    }
    if message == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    if message == WM_MOUSEMOVE && !PREVIEW_HOVERED.swap(true, Ordering::AcqRel) {
        let mut track = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: 0,
        };
        unsafe {
            let _ = TrackMouseEvent(&mut track);
        }
    }
    if message == WM_MOUSELEAVE_MESSAGE {
        PREVIEW_HOVERED.store(false, Ordering::Release);
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn apply_popup_style(hwnd: HWND, no_activate: bool) {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let style = (style
            & !(WS_CAPTION.0
                | WS_THICKFRAME.0
                | WS_SYSMENU.0
                | WS_MINIMIZEBOX.0
                | WS_MAXIMIZEBOX.0))
            | WS_POPUP.0;
        SetWindowLongPtrW(hwnd, GWL_STYLE, style as isize);

        let mut ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        ex &= !WS_EX_APPWINDOW.0;
        ex |= WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0;
        if no_activate {
            ex |= WS_EX_NOACTIVATE.0;
        } else {
            ex &= !WS_EX_NOACTIVATE.0;
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex as isize);

        let corner = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const c_void,
            size_of::<i32>() as u32,
        );
        let dark: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as *const c_void,
            size_of::<i32>() as u32,
        );
        let no_border: u32 = DWMWA_COLOR_NONE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &no_border as *const _ as *const c_void,
            size_of::<u32>() as u32,
        );
    }
}

pub fn configure_taskbar(hwnd: HWND) {
    apply_popup_style(hwnd, true);
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(taskbar_subclass), TASKBAR_SUBCLASS_ID, 0);
    }
}

pub fn configure_control_center(hwnd: HWND) {
    apply_popup_style(hwnd, false);
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(cc_subclass), CC_SUBCLASS_ID, 0);
    }
}

pub fn configure_launcher(hwnd: HWND) {
    apply_popup_style(hwnd, false);
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(launcher_subclass), LAUNCHER_SUBCLASS_ID, 0);
    }
}

pub fn configure_preview(hwnd: HWND) {
    apply_popup_style(hwnd, true);
    reset_preview_hover();
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(preview_subclass), PREVIEW_SUBCLASS_ID, 0);
    }
}

pub fn position_taskbar(hwnd: HWND, rect: PhysicalRect) -> Result<(), String> {
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
        )
        .map_err(|error| format!("SetWindowPos(taskbar) failed: {error}"))
    }
}

pub fn position_preview(hwnd: HWND, rect: PhysicalRect) -> Result<(), String> {
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
        )
        .map_err(|error| format!("SetWindowPos(preview) failed: {error}"))
    }
}

pub fn position_panel(hwnd: HWND, rect: PhysicalRect, activate: bool) -> Result<(), String> {
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
        )
        .map_err(|error| format!("SetWindowPos(panel) failed: {error}"))?;
        let _ = ShowWindow(hwnd, SW_SHOW);
        if activate {
            let _ = SetForegroundWindow(hwnd);
        }
    }
    Ok(())
}

pub fn hide_window(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, windows::Win32::UI::WindowsAndMessaging::SW_HIDE);
    }
}

unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    let key_up = matches!(wparam.0 as u32, WM_KEYUP | WM_SYSKEYUP);
    let injected =
        info.flags.contains(LLKHF_INJECTED) || info.flags.contains(LLKHF_LOWER_IL_INJECTED);
    let decision = KEY_TRACKER
        .lock()
        .map(|mut tracker| tracker.handle(info.vkCode, key_up, injected))
        .unwrap_or(KeyDecision::PassThrough);

    match decision {
        KeyDecision::PassThrough => {}
        KeyDecision::Suppress => return LRESULT(1),
        KeyDecision::SwallowAndToggleLauncher => {
            LAUNCHER_SHORTCUT.store(true, Ordering::Release);
            return LRESULT(1);
        }
        KeyDecision::ReplayWindowsDown(vk) => {
            // The physical Windows key-down was swallowed. Replay it just
            // before the combination key so the native shortcut still works.
            send_keyboard_input(vk, false);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn send_keyboard_input(vk: u32, key_up: bool) {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk as u16),
                wScan: 0,
                dwFlags: if key_up {
                    KEYBD_EVENT_FLAGS(2)
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
    if sent != 1 {
        tracing::warn!(vk, "synthetic Windows key-down was not accepted");
    }
}

/// Install the global low-level keyboard hook on the calling (UI) thread.
///
/// The hook is removed automatically by Windows when the process exits, and
/// `remove_keyboard_hook` removes it on a clean shutdown.
pub fn install_keyboard_hook() -> Result<HHOOK, String> {
    KEY_TRACKER.lock().map(|mut tracker| tracker.reset()).ok();
    let module = unsafe { GetModuleHandleW(None) }
        .map_err(|error| format!("GetModuleHandleW failed: {error}"))?;
    unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_hook),
            Some(HINSTANCE(module.0)),
            0,
        )
    }
    .map_err(|error| format!("SetWindowsHookExW(WH_KEYBOARD_LL) failed: {error}"))
}

pub fn remove_keyboard_hook(hook: HHOOK) {
    unsafe {
        let _ = UnhookWindowsHookEx(hook);
    }
}

pub fn take_launcher_shortcut() -> bool {
    LAUNCHER_SHORTCUT.swap(false, Ordering::AcqRel)
}

pub fn register_fallback_hotkey(hwnd: HWND, hotkey: Hotkey) -> Result<(), String> {
    unsafe {
        RegisterHotKey(
            Some(hwnd),
            FALLBACK_HOTKEY_ID,
            windows::Win32::UI::Input::KeyboardAndMouse::HOT_KEY_MODIFIERS(
                hotkey.modifiers | MOD_NOREPEAT.0,
            ),
            hotkey.virtual_key,
        )
        .map_err(|error| {
            format!(
                "RegisterHotKey({:#x}, vk {:#x}) failed: {error}",
                hotkey.modifiers, hotkey.virtual_key
            )
        })
    }
}

pub fn unregister_fallback_hotkey(hwnd: HWND) {
    unsafe {
        let _ = UnregisterHotKey(Some(hwnd), FALLBACK_HOTKEY_ID);
    }
}
