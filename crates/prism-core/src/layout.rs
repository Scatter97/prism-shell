//! Pure, DPI-independent layout calculations.
//!
//! The values are logical pixels (`Slint` lengths at scale factor 1.0). The
//! presentation layer converts them to physical pixels through the window's
//! scale factor. Keeping this arithmetic here makes overflow behavior testable
//! without a compositor.

/// Logical width reserved for the launcher, status cluster, internal padding,
/// dividers, and the gap between zones.
pub const DOCK_CHROME_WIDTH: f32 = 392.0;
/// Logical interaction width per application icon.
pub const DOCK_APP_SLOT_WIDTH: f32 = 50.0;
/// Logical width of the overflow chip.
pub const DOCK_OVERFLOW_WIDTH: f32 = 46.0;
/// Minimum outer margin from a screen edge.
pub const DOCK_SCREEN_MARGIN: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DockFit {
    pub visible_apps: usize,
    pub hidden_apps: usize,
}

impl DockFit {
    pub fn has_overflow(self) -> bool {
        self.hidden_apps > 0
    }
}

/// Decide how many dock entries fit at a given logical stage width.
///
/// Critical zones are never sacrificed. At least one application remains
/// visible on absurdly narrow stages, and the result is deterministic.
pub fn fit_dock_apps(total_apps: usize, stage_width: f32) -> DockFit {
    if total_apps == 0 {
        return DockFit {
            visible_apps: 0,
            hidden_apps: 0,
        };
    }

    let usable = (stage_width - (DOCK_SCREEN_MARGIN * 2.0) - DOCK_CHROME_WIDTH).max(0.0);
    let mut capacity = (usable / DOCK_APP_SLOT_WIDTH).floor() as usize;
    capacity = capacity.max(1).min(total_apps);

    // If we hide entries, the overflow chip consumes a slot as well.
    if capacity < total_apps && capacity > 1 {
        let usable_with_chip = (usable - DOCK_OVERFLOW_WIDTH).max(0.0);
        let with_chip = (usable_with_chip / DOCK_APP_SLOT_WIDTH).floor() as usize;
        capacity = with_chip.max(1);
    }

    DockFit {
        visible_apps: capacity,
        hidden_apps: total_apps - capacity,
    }
}

/// Width of the floating dock surface for a given number of visible entries.
/// Capped by the available stage width.
pub fn dock_surface_width(visible_apps: usize, stage_width: f32, overflow: bool) -> f32 {
    let content = DOCK_CHROME_WIDTH
        + (visible_apps as f32 * DOCK_APP_SLOT_WIDTH)
        + if overflow { DOCK_OVERFLOW_WIDTH } else { 0.0 };
    content.min((stage_width - (DOCK_SCREEN_MARGIN * 2.0)).max(320.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_apps_fit_on_a_wide_display() {
        assert_eq!(
            fit_dock_apps(8, 1920.0),
            DockFit {
                visible_apps: 8,
                hidden_apps: 0
            }
        );
    }

    #[test]
    fn overflow_is_deterministic_at_1080p_and_1280() {
        assert_eq!(fit_dock_apps(30, 1920.0).hidden_apps, 2);
        assert_eq!(fit_dock_apps(30, 1280.0).hidden_apps, 15);
    }

    #[test]
    fn at_least_one_app_remains_visible_on_narrow_stages() {
        let fit = fit_dock_apps(12, 480.0);
        assert_eq!(fit.visible_apps, 1);
        assert_eq!(fit.hidden_apps, 11);
    }

    #[test]
    fn zero_apps_never_overflow() {
        assert_eq!(
            fit_dock_apps(0, 1920.0),
            DockFit {
                visible_apps: 0,
                hidden_apps: 0
            }
        );
    }

    #[test]
    fn surface_width_never_exceeds_stage() {
        let width = dock_surface_width(20, 1280.0, true);
        assert!(width <= 1280.0 - (DOCK_SCREEN_MARGIN * 2.0));
        assert!(width >= 320.0);
    }
}
