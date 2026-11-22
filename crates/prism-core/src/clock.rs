use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockText {
    pub time: String,
    /// Short date, without a leading zero on the day where the platform allows
    /// it (`Sat, Nov 21`).
    pub date: String,
    pub hour: u32,
    pub minute: u32,
}

/// Format the clock deterministically from a local naive date-time.
///
/// Windows locale preferences will choose `use_24_hour` later. Keeping the
/// formatting function pure makes date rollover and midnight behavior testable.
pub fn format_clock(now: NaiveDateTime, use_24_hour: bool) -> ClockText {
    let time = if use_24_hour {
        format!("{:02}:{:02}", now.hour(), now.minute())
    } else {
        let (hour, suffix) = match now.hour() {
            0 => (12, "AM"),
            1..=11 => (now.hour(), "AM"),
            12 => (12, "PM"),
            _ => (now.hour() - 12, "PM"),
        };
        format!("{hour}:{:02} {suffix}", now.minute())
    };

    ClockText {
        time,
        date: format_short_date(now.date()),
        hour: now.hour(),
        minute: now.minute(),
    }
}

fn format_short_date(date: NaiveDate) -> String {
    let weekday = match date.weekday().num_days_from_monday() {
        0 => "Mon",
        1 => "Tue",
        2 => "Wed",
        3 => "Thu",
        4 => "Fri",
        5 => "Sat",
        _ => "Sun",
    };
    let month = match date.month() {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        _ => "Dec",
    };
    format!("{weekday}, {month} {}", date.day())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 0)
            .unwrap()
    }

    #[test]
    fn formats_24_hour_time() {
        let clock = format_clock(dt(2026, 11, 21, 21, 7), true);
        assert_eq!(clock.time, "21:07");
        assert_eq!(clock.date, "Sat, Nov 21");
    }

    #[test]
    fn formats_midnight_in_both_modes() {
        assert_eq!(format_clock(dt(2026, 11, 21, 0, 0), true).time, "00:00");
        assert_eq!(format_clock(dt(2026, 11, 21, 0, 0), false).time, "12:00 AM");
    }

    #[test]
    fn formats_noon_and_evening_in_12_hour_mode() {
        assert_eq!(
            format_clock(dt(2026, 11, 21, 12, 0), false).time,
            "12:00 PM"
        );
        assert_eq!(
            format_clock(dt(2026, 11, 21, 23, 59), false).time,
            "11:59 PM"
        );
    }

    #[test]
    fn date_rollover_uses_new_day() {
        let before = format_clock(dt(2026, 12, 31, 23, 59), true);
        let after = format_clock(dt(2027, 1, 1, 0, 0), true);
        assert_eq!(before.date, "Thu, Dec 31");
        assert_eq!(after.date, "Fri, Jan 1");
    }
}
