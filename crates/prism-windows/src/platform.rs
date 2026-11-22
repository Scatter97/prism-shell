//! Real Windows actions used by the taskbar and the limited Control Center.
//!
//! System controls that are not implemented yet return `Unsupported`; they are
//! never simulated and never report an optimistic state. Native Settings and
//! workstation lock are real operations.

use prism_core::{
    AudioState, BatteryState, BluetoothState, BrightnessState, Capability, ConnectivityState,
    ControlPhase, MediaState, PowerAction, SystemAction, SystemSnapshot, ToggleId, ToggleState,
    WifiState, WindowId,
};
use prism_platform::{ActionOutcome, LaunchReceipt, PlatformError, SystemServices, WindowServices};
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::Shutdown::LockWorkStation;
use windows::Win32::System::Threading::GetProcessId;
use windows::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FLASHW_TIMERNOFG, FLASHW_TRAY, FLASHWINFO, FlashWindowEx, IsIconic, IsWindow, MB_ICONERROR,
    MB_OK, MB_SETFOREGROUND, MessageBoxW, PostMessageW, SW_MINIMIZE, SW_RESTORE, SW_SHOWNORMAL,
    SetForegroundWindow, ShowWindow, WM_CLOSE,
};
use windows::core::{PCWSTR, w};

use crate::host::to_wide;
use crate::window::hwnd_from_id;

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn open_uri(&self, uri: &str) -> Result<(), PlatformError> {
        let wide = to_wide(uri);
        unsafe {
            let mut info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_FLAG_NO_UI,
                hwnd: HWND::default(),
                lpVerb: w!("open"),
                lpFile: PCWSTR(wide.as_ptr()),
                nShow: SW_SHOWNORMAL.0,
                ..Default::default()
            };
            ShellExecuteExW(&mut info).map_err(|error| {
                PlatformError::failed("ShellExecute", format!("{uri}: {error}"))
            })?;
            if !info.hProcess.0.is_null() {
                let _ = CloseHandle(info.hProcess);
            }
        }
        Ok(())
    }

    fn shell_execute(
        &self,
        target: &str,
        capture_process: bool,
    ) -> Result<Option<u32>, PlatformError> {
        let wide = to_wide(target);
        unsafe {
            let mut info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_FLAG_NO_UI
                    | if capture_process {
                        SEE_MASK_NOCLOSEPROCESS
                    } else {
                        0
                    },
                hwnd: HWND::default(),
                lpVerb: w!("open"),
                lpFile: PCWSTR(wide.as_ptr()),
                nShow: SW_SHOWNORMAL.0,
                ..Default::default()
            };
            ShellExecuteExW(&mut info)
                .map_err(|error| PlatformError::failed("Launch", format!("{target}: {error}")))?;
            if capture_process && !info.hProcess.0.is_null() {
                let pid = GetProcessId(info.hProcess);
                let _ = CloseHandle(info.hProcess);
                return Ok((pid != 0).then_some(pid));
            }
        }
        Ok(None)
    }
}

fn battery_state() -> BatteryState {
    unsafe {
        let mut status = SYSTEM_POWER_STATUS::default();
        if GetSystemPowerStatus(&mut status).is_err() {
            return BatteryState::desktop();
        }
        let present = status.BatteryFlag != 128 && status.BatteryLifePercent != 255;
        if !present {
            return BatteryState::desktop();
        }
        BatteryState {
            present: true,
            percent: status.BatteryLifePercent,
            charging: status.BatteryFlag & 8 != 0,
            capability: Capability::Available,
        }
    }
}

fn unsupported_toggles() -> Vec<ToggleState> {
    ToggleId::ALL
        .into_iter()
        .map(|id| ToggleState {
            id,
            active: false,
            capability: Capability::Unsupported,
            phase: ControlPhase::Idle,
        })
        .collect()
}

impl SystemServices for WindowsPlatform {
    fn snapshot(&self) -> SystemSnapshot {
        SystemSnapshot {
            toggles: unsupported_toggles(),
            audio: AudioState {
                outputs: Vec::new(),
                default_output: None,
                volume: 0,
                muted: true,
                capability: Capability::Unavailable,
                phase: ControlPhase::failed("Audio controls are not implemented in this build."),
            },
            brightness: BrightnessState {
                level: 0,
                capability: Capability::Unsupported,
                phase: ControlPhase::Idle,
                detail: Some("Brightness control is not implemented in this build.".to_owned()),
            },
            connectivity: ConnectivityState {
                wifi: WifiState {
                    enabled: false,
                    connected: false,
                    ssid: None,
                    capability: Capability::Unsupported,
                    phase: ControlPhase::Idle,
                },
                bluetooth: BluetoothState {
                    enabled: false,
                    connected_device: None,
                    capability: Capability::Unsupported,
                    phase: ControlPhase::Idle,
                },
                wired: false,
            },
            battery: battery_state(),
            media: MediaState {
                session: None,
                capability: Capability::Unsupported,
                phase: ControlPhase::Idle,
            },
        }
    }

    fn execute(&mut self, action: SystemAction) -> Result<ActionOutcome, PlatformError> {
        match action {
            SystemAction::OpenSettings(page) => {
                self.open_uri(page.windows_uri())?;
                Ok(ActionOutcome::Applied)
            }
            SystemAction::Power(PowerAction::Lock) => unsafe {
                LockWorkStation()
                    .map(|()| ActionOutcome::Applied)
                    .map_err(|error| PlatformError::failed("Lock workstation", error.to_string()))
            },
            SystemAction::Power(_) => Err(PlatformError::unsupported(
                "Power action",
                "Only workstation lock is exposed in this build.",
            )),
            SystemAction::SetToggle { id, .. } => Err(PlatformError::unsupported(
                format!("{id:?} toggle"),
                "Use the native Windows settings surface.",
            )),
            _ => Err(PlatformError::unsupported(
                "System control",
                "This control is not implemented in this build.",
            )),
        }
    }
}

impl WindowServices for WindowsPlatform {
    fn launch(&mut self, app: &prism_core::AppDefinition) -> Result<LaunchReceipt, PlatformError> {
        if app.launch_target.trim().is_empty() {
            return Err(PlatformError::unsupported(
                "Launch",
                "This application has no resolved launch target.",
            ));
        }
        let pid = self.shell_execute(&app.launch_target, true)?;
        Ok(LaunchReceipt {
            process_id: pid,
            title: app.name.clone(),
        })
    }

    fn activate(&mut self, window: WindowId) -> Result<(), PlatformError> {
        let hwnd = hwnd_from_id(window.get());
        unsafe {
            if !IsWindow(Some(hwnd)).as_bool() {
                return Err(PlatformError::unavailable(
                    "Activate window",
                    "The window no longer exists.",
                ));
            }
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            if !SetForegroundWindow(hwnd).as_bool() {
                let flash = FLASHWINFO {
                    cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                    hwnd,
                    dwFlags: windows::Win32::UI::WindowsAndMessaging::FLASHWINFO_FLAGS(
                        FLASHW_TRAY.0 | FLASHW_TIMERNOFG.0,
                    ),
                    uCount: 3,
                    dwTimeout: 0,
                };
                let _ = FlashWindowEx(&flash);
                tracing::warn!(
                    hwnd = ?hwnd,
                    "SetForegroundWindow was denied; flashed the target window"
                );
            }
        }
        Ok(())
    }

    fn minimize(&mut self, window: WindowId) -> Result<(), PlatformError> {
        let hwnd = hwnd_from_id(window.get());
        unsafe {
            if !IsWindow(Some(hwnd)).as_bool() {
                return Err(PlatformError::unavailable(
                    "Minimize window",
                    "The window no longer exists.",
                ));
            }
            let _ = ShowWindow(hwnd, SW_MINIMIZE);
        }
        Ok(())
    }

    fn close(&mut self, window: WindowId) -> Result<(), PlatformError> {
        let hwnd = hwnd_from_id(window.get());
        unsafe {
            if !IsWindow(Some(hwnd)).as_bool() {
                return Err(PlatformError::unavailable(
                    "Close window",
                    "The window no longer exists.",
                ));
            }
            PostMessageW(
                Some(hwnd),
                WM_CLOSE,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            )
            .map_err(|error| PlatformError::failed("Close window", error.to_string()))?;
        }
        Ok(())
    }
}

pub fn show_error_message(title: &str, message: &str) {
    let title = to_wide(title);
    let message = to_wide(message);
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        );
    }
}
