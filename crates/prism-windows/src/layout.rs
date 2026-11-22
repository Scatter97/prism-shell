//! DPI-aware physical-pixel geometry for the Windows shell surfaces.
//!
//! The taskbar surface is drawn inside a full-work-area-width, 100 logical-pixel
//! window. Keeping the window taller than the dock gives the shadow room; the
//! host makes the transparent margins click-through. All functions use
//! physical pixels so mixed-DPI layout math can be tested without Win32.

pub const TASKBAR_WINDOW_HEIGHT: f32 = 100.0;
pub const TASKBAR_TOP_INSET: f32 = 24.0;
pub const TASKBAR_HEIGHT: f32 = 62.0;
pub const TASKBAR_BOTTOM_GAP: f32 = 14.0;
pub const SURFACE_MARGIN: f32 = 24.0;
pub const PANEL_GAP: f32 = 10.0;
pub const PANEL_SCREEN_MARGIN: f32 = 8.0;
pub const LAUNCHER_GAP: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl PhysicalRect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn right(self) -> i32 {
        self.x + self.width
    }

    pub const fn bottom(self) -> i32 {
        self.y + self.height
    }

    pub const fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

pub fn scaled(value: f32, scale: f32) -> i32 {
    (value * scale).round() as i32
}

/// Full-width taskbar window at the bottom of the monitor work area.
pub fn taskbar_window(work: PhysicalRect, scale: f32) -> PhysicalRect {
    let height = scaled(TASKBAR_WINDOW_HEIGHT, scale);
    PhysicalRect::new(work.x, work.y + work.height - height, work.width, height)
}

/// The visible dock surface inside the taskbar window.
pub fn dock_rect(work: PhysicalRect, scale: f32, dock_width_logical: f32) -> PhysicalRect {
    let window = taskbar_window(work, scale);
    let width = scaled(dock_width_logical, scale).min(window.width);
    PhysicalRect::new(
        window.x + (window.width - width) / 2,
        window.y + scaled(TASKBAR_TOP_INSET, scale),
        width,
        scaled(TASKBAR_HEIGHT, scale),
    )
}

/// Width formula shared by the Slint taskbar surface and the Windows host.
/// The `stage_width` is logical.
pub fn dock_width(stage_width_logical: f32, app_count: usize, overflow: bool) -> f32 {
    let content = 314.0 + app_count as f32 * 50.0 + if overflow { 50.0 } else { 0.0 };
    content.min((stage_width_logical - 48.0).max(320.0))
}

/// Panel rectangle for the Control Center, anchored above the right side of
/// the dock. `panel_*` values are logical. The window adds transparent shadow
/// margins via [`inflate`].
pub fn control_center_panel(
    work: PhysicalRect,
    dock: PhysicalRect,
    panel_width_logical: f32,
    panel_height_logical: f32,
    scale: f32,
) -> PhysicalRect {
    let width = scaled(panel_width_logical, scale);
    let height = scaled(panel_height_logical, scale);
    let x = dock.right() - width + scaled(2.0, scale);
    let y = dock.y - scaled(PANEL_GAP, scale) - height;
    let margin = scaled(PANEL_SCREEN_MARGIN, scale);
    let rect = PhysicalRect::new(x, y, width, height);
    clamp_to_work(rect, work, margin)
}

/// Panel rectangle for the launcher, anchored above the launcher button.
pub fn launcher_panel(
    work: PhysicalRect,
    dock: PhysicalRect,
    panel_width_logical: f32,
    panel_height_logical: f32,
    scale: f32,
) -> PhysicalRect {
    let width = scaled(panel_width_logical, scale);
    let height = scaled(panel_height_logical, scale);
    let x = dock.x + scaled(2.0, scale);
    let y = dock.y - scaled(LAUNCHER_GAP, scale) - height;
    let margin = scaled(PANEL_SCREEN_MARGIN, scale);
    clamp_to_work(PhysicalRect::new(x, y, width, height), work, margin)
}

/// Expand a panel rect by the transparent shadow margin used by the native
/// window host.
pub fn inflate(rect: PhysicalRect, scale: f32) -> PhysicalRect {
    let margin = scaled(SURFACE_MARGIN, scale);
    PhysicalRect::new(
        rect.x - margin,
        rect.y - margin,
        rect.width + margin * 2,
        rect.height + margin * 2,
    )
}

/// Keep a panel fully inside the work area with a small edge margin.
pub fn clamp_to_work(rect: PhysicalRect, work: PhysicalRect, margin: i32) -> PhysicalRect {
    let max_width = (work.width - margin * 2).max(1);
    let max_height = (work.height - margin * 2).max(1);
    let width = rect.width.min(max_width);
    let height = rect.height.min(max_height);
    let mut x = rect.x;
    let mut y = rect.y;
    if x + width > work.right() - margin {
        x = work.right() - margin - width;
    }
    if x < work.x + margin {
        x = work.x + margin;
    }
    if y + height > work.bottom() - margin {
        y = work.bottom() - margin - height;
    }
    if y < work.y + margin {
        y = work.y + margin;
    }
    PhysicalRect::new(x, y, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work() -> PhysicalRect {
        PhysicalRect::new(0, 0, 1920, 1032)
    }

    #[test]
    fn taskbar_sits_above_the_windows_taskbar() {
        let window = taskbar_window(work(), 1.0);
        assert_eq!(window, PhysicalRect::new(0, 932, 1920, 100));
        let dock = dock_rect(work(), 1.0, 614.0);
        assert_eq!(dock.y, 956);
        assert_eq!(dock.height, 62);
        assert_eq!(dock.bottom(), 1018);
        assert_eq!(work().bottom() - dock.bottom(), 14);
        assert_eq!(dock.x, (1920 - 614) / 2);
    }

    #[test]
    fn control_center_is_right_aligned_and_above_the_dock() {
        let dock = dock_rect(work(), 1.0, 614.0);
        let panel = control_center_panel(work(), dock, 384.0, 900.0, 1.0);
        assert_eq!(panel.width, 384);
        assert_eq!(panel.x + panel.width, dock.right() + 2);
        assert_eq!(panel.bottom(), dock.y - 10);
    }

    #[test]
    fn launcher_is_anchored_above_the_dock_left_edge() {
        let dock = dock_rect(work(), 1.0, 614.0);
        let panel = launcher_panel(work(), dock, 360.0, 452.0, 1.0);
        assert_eq!(panel.x, dock.x + 2);
        assert_eq!(panel.bottom(), dock.y - 12);
        assert!(panel.y >= 8);
    }

    #[test]
    fn scale_changes_are_physical_and_round_trip() {
        let scaled_work = PhysicalRect::new(0, 0, 2560, 1440);
        let dock = dock_rect(scaled_work, 1.5, 614.0);
        assert_eq!(dock.width, scaled(614.0, 1.5));
        let window = taskbar_window(scaled_work, 1.5);
        assert_eq!(window.height, 150);
        assert_eq!(dock.y, window.y + 36);
        assert_eq!(dock.height, 93);
    }

    #[test]
    fn negative_origin_monitors_produce_negative_geometry() {
        let left_work = PhysicalRect::new(-1920, 0, 1920, 1080);
        let dock = dock_rect(left_work, 1.0, 614.0);
        assert!(dock.x < 0);
        assert_eq!(dock.right() - left_work.x, 1267);
        let panel = launcher_panel(left_work, dock, 360.0, 452.0, 1.0);
        assert!(panel.x >= left_work.x + 8);
    }

    #[test]
    fn clamping_keeps_panels_inside_small_work_areas() {
        let small = PhysicalRect::new(0, 0, 800, 600);
        let dock = dock_rect(small, 1.0, 614.0);
        let panel = control_center_panel(small, dock, 384.0, 900.0, 1.0);
        assert!(panel.y >= 8);
        assert!(panel.bottom() <= 592);
        assert!(panel.x >= 8);
        assert!(panel.right() <= 792);
    }

    #[test]
    fn inflated_window_contains_the_panel_and_margins() {
        let panel = PhysicalRect::new(100, 200, 384, 500);
        let window = inflate(panel, 1.25);
        assert_eq!(window.x, 70);
        assert_eq!(window.y, 170);
        assert_eq!(window.width, 384 + 60);
        assert_eq!(window.height, 500 + 60);
    }

    #[test]
    fn dock_width_matches_the_slint_formula() {
        assert_eq!(dock_width(1920.0, 6, false), 614.0);
        assert_eq!(dock_width(1280.0, 20, true), 1232.0);
        assert_eq!(dock_width(800.0, 30, true), 752.0);
    }
}
