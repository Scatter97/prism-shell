//! Real top-level window enumeration and classification.

use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GWL_EXSTYLE, GetClassNameW, GetForegroundWindow, GetWindow,
    GetWindowLongPtrW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};
use windows::core::BOOL;
use windows::core::PWSTR;

use crate::model::{DiscoveredApp, DiscoveredWindow, WindowFacts, group_window};

pub struct TrackedWindow {
    pub app: DiscoveredApp,
    pub window: DiscoveredWindow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Foreground {
    pub hwnd: u64,
    pub own_process: bool,
}

fn process_image(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut length = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(process);
        result.ok()?;
        let length = length as usize;
        Some(String::from_utf16_lossy(
            &buffer[..length.min(buffer.len())],
        ))
    }
}

unsafe fn window_facts(hwnd: HWND, own_pid: u32) -> Option<WindowFacts> {
    if hwnd.0.is_null() || !unsafe { IsWindowVisible(hwnd).as_bool() } {
        return None;
    }

    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == 0 {
        return None;
    }

    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    let tool_window = style & WS_EX_TOOLWINDOW.0 != 0;
    let app_window = style & WS_EX_APPWINDOW.0 != 0;
    let owner = unsafe { GetWindow(hwnd, GW_OWNER) }.ok();
    let owned = owner.is_some_and(|owner| !owner.0.is_null());

    let mut cloaked = 0u32;
    let cloaked = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut c_void,
            size_of::<u32>() as u32,
        )
        .is_ok()
            && cloaked != 0
    };

    let mut class_buffer = [0u16; 256];
    let class_len = unsafe { GetClassNameW(hwnd, &mut class_buffer) }.max(0) as usize;
    let class_name = String::from_utf16_lossy(&class_buffer[..class_len.min(class_buffer.len())]);

    let mut title_buffer = [0u16; 512];
    let title_len = unsafe { GetWindowTextW(hwnd, &mut title_buffer) }.max(0) as usize;
    let title = String::from_utf16_lossy(&title_buffer[..title_len.min(title_buffer.len())]);

    let mut rect = RECT::default();
    let (width, height) = if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok() {
        (rect.right - rect.left, rect.bottom - rect.top)
    } else {
        (0, 0)
    };

    Some(WindowFacts {
        own_process: pid == own_pid,
        visible: true,
        cloaked,
        tool_window,
        app_window,
        owned,
        width,
        height,
        minimized: unsafe { IsIconic(hwnd).as_bool() },
        class_name,
        process_image: process_image(pid),
        title,
        hwnd: hwnd.0 as usize as u64,
    })
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let own_pid = unsafe { GetCurrentProcessId() };
    let windows = unsafe { &mut *(lparam.0 as *mut Vec<TrackedWindow>) };
    if let Some(facts) = unsafe { window_facts(hwnd, own_pid) }
        && let Some((app, window)) = group_window(&facts)
    {
        windows.push(TrackedWindow { app, window });
    }
    BOOL(1)
}

pub fn enumerate() -> Vec<TrackedWindow> {
    let mut windows: Vec<TrackedWindow> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut windows as *mut _ as isize));
    }
    // EnumWindows order is not a stable user-facing order. Sort by app then
    // HWND so the dock does not reshuffle on every refresh.
    windows.sort_by(|a, b| {
        a.window
            .app_id
            .cmp(&b.window.app_id)
            .then_with(|| a.window.hwnd.cmp(&b.window.hwnd))
    });
    windows
}

pub fn foreground() -> Option<Foreground> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        Some(Foreground {
            hwnd: hwnd.0 as usize as u64,
            own_process: pid == GetCurrentProcessId(),
        })
    }
}

pub fn hwnd_from_id(window_id: u64) -> HWND {
    HWND(window_id as usize as *mut c_void)
}
