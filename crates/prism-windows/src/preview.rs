//! DWM live window thumbnails for the production preview surface.
//!
//! The Slint preview window is the DWM destination window. Thumbnails are
//! registered against its HWND and positioned over the card thumbnail slots;
//! Slint continues to draw the surrounding chrome, titles, and buttons. DWM
//! draws each thumbnail above the destination window's content, so every
//! interactive control is deliberately outside the thumbnail rectangles.
//!
//! Every thumbnail is unregistered when the preview closes, when its source
//! window disappears, or when the shell shuts down. Minimized windows are not
//! registered: the card shows its "Minimized" state and clicking it restores
//! the window.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{
    DWM_THUMBNAIL_PROPERTIES, DWM_TNP_OPACITY, DWM_TNP_RECTDESTINATION, DWM_TNP_RECTSOURCE,
    DWM_TNP_VISIBLE, DwmQueryThumbnailSourceSize, DwmRegisterThumbnail, DwmUnregisterThumbnail,
    DwmUpdateThumbnailProperties,
};
use windows::Win32::UI::WindowsAndMessaging::IsWindow;
use windows::core::BOOL;

use crate::layout::{self, PhysicalRect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThumbnailTarget {
    pub hwnd: u64,
    pub minimized: bool,
}

struct LiveThumbnail {
    source: u64,
    handle: isize,
    destination: PhysicalRect,
}

#[derive(Default)]
pub struct PreviewThumbnails {
    thumbnails: Vec<LiveThumbnail>,
    destination: Option<HWND>,
}

impl PreviewThumbnails {
    pub fn new() -> Self {
        Self::default()
    }

    /// Synchronize the registered DWM thumbnails with the current preview.
    ///
    /// `preview_window` is in screen coordinates; DWM destination rectangles
    /// are converted to the preview window's client coordinates here.
    pub fn sync(
        &mut self,
        destination: HWND,
        preview_window: PhysicalRect,
        scale: f32,
        targets: &[ThumbnailTarget],
    ) {
        if self.destination != Some(destination) {
            self.clear();
            self.destination = Some(destination);
        }

        let wanted: Vec<(usize, ThumbnailTarget)> = targets
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, target)| {
                !target.minimized
                    && unsafe { IsWindow(Some(HWND(target.hwnd as usize as *mut _))).as_bool() }
            })
            .collect();

        self.thumbnails.retain(|thumbnail| {
            let keep = wanted
                .iter()
                .any(|(_, target)| target.hwnd == thumbnail.source);
            if !keep {
                unsafe {
                    let _ = DwmUnregisterThumbnail(thumbnail.handle);
                }
            }
            keep
        });

        for (index, target) in wanted {
            let destination_rect =
                layout::preview_thumbnail_destination(preview_window, index, scale);
            let handle = if let Some(existing) = self
                .thumbnails
                .iter_mut()
                .find(|thumbnail| thumbnail.source == target.hwnd)
            {
                if existing.destination == destination_rect {
                    continue;
                }
                existing.destination = destination_rect;
                existing.handle
            } else {
                let source = HWND(target.hwnd as usize as *mut _);
                match unsafe { DwmRegisterThumbnail(destination, source) } {
                    Ok(handle) => {
                        self.thumbnails.push(LiveThumbnail {
                            source: target.hwnd,
                            handle,
                            destination: destination_rect,
                        });
                        handle
                    }
                    Err(error) => {
                        tracing::debug!(
                            hwnd = target.hwnd,
                            %error,
                            "DWM thumbnail registration failed"
                        );
                        continue;
                    }
                }
            };

            let crop = unsafe { DwmQueryThumbnailSourceSize(handle) }
                .ok()
                .and_then(|source_size| {
                    layout::thumbnail_source_rect(
                        source_size.cx,
                        source_size.cy,
                        destination_rect.width,
                        destination_rect.height,
                    )
                });
            let rc_source = crop.map_or_else(RECT::default, |crop| RECT {
                left: crop.x,
                top: crop.y,
                right: crop.right(),
                bottom: crop.bottom(),
            });
            let properties = DWM_THUMBNAIL_PROPERTIES {
                dwFlags: DWM_TNP_RECTDESTINATION
                    | DWM_TNP_VISIBLE
                    | DWM_TNP_OPACITY
                    | if crop.is_some() {
                        DWM_TNP_RECTSOURCE
                    } else {
                        0
                    },
                rcDestination: RECT {
                    left: destination_rect.x - preview_window.x,
                    top: destination_rect.y - preview_window.y,
                    right: destination_rect.right() - preview_window.x,
                    bottom: destination_rect.bottom() - preview_window.y,
                },
                rcSource: rc_source,
                opacity: 255,
                fVisible: BOOL(1),
                fSourceClientAreaOnly: BOOL(0),
            };

            if let Err(error) = unsafe { DwmUpdateThumbnailProperties(handle, &properties) } {
                tracing::debug!(
                    hwnd = target.hwnd,
                    %error,
                    "DWM thumbnail update failed; dropping the thumbnail"
                );
                unsafe {
                    let _ = DwmUnregisterThumbnail(handle);
                }
                self.thumbnails
                    .retain(|thumbnail| thumbnail.handle != handle);
            }
        }
    }

    /// Unregister every thumbnail. Safe to call repeatedly.
    pub fn clear(&mut self) {
        for thumbnail in self.thumbnails.drain(..) {
            unsafe {
                let _ = DwmUnregisterThumbnail(thumbnail.handle);
            }
        }
        self.destination = None;
    }
}

impl Drop for PreviewThumbnails {
    fn drop(&mut self) {
        self.clear();
    }
}
