//! When a reminder is due for each of the web client's choices, and the
//! custom date and time typed in "Set Custom Reminder".

use crate::time::{self, DayTime};

const HOUR: i64 = 3600;
const DAY: i64 = 86_400;

/// The "Remind Me" choices, in the web client's order; "Custom..." opens
/// a form instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemindAt {
    In30m,
    In1h,
    In4h,
    /// Today at 9:00 before nine, else tomorrow at 9:00.
    NineAm,
    /// This week's Monday at 9:00 on Sunday or early Monday, else next
    /// week's.
    Monday9,
}

impl RemindAt {
    pub const ALL: [RemindAt; 5] = [Self::In30m, Self::In1h, Self::In4h, Self::NineAm, Self::Monday9];

    /// The label's key; "Today" or "Tomorrow" depends on the hour.
    pub fn label(self, now: DayTime) -> &'static str {
        match self {
            Self::In30m => "saved.in_30m",
            Self::In1h => "saved.in_1h",
            Self::In4h => "saved.in_4h",
            Self::NineAm if now.since_midnight < 9 * HOUR => "saved.today_9",
            Self::NineAm => "saved.tomorrow_9",
            Self::Monday9 => "saved.monday_9",
        }
    }
}

/// When `choice` is due, in Unix seconds, from `now` and where it falls
/// in the local day and week.
pub fn remind_at(choice: RemindAt, now: i64, day: DayTime) -> i64 {
    let midnight = now - day.since_midnight;
    let nine = 9 * HOUR;
    match choice {
        RemindAt::In30m => now + HOUR / 2,
        RemindAt::In1h => now + HOUR,
        RemindAt::In4h => now + 4 * HOUR,
        RemindAt::NineAm if day.since_midnight < nine => midnight + nine,
        RemindAt::NineAm => midnight + DAY + nine,
        RemindAt::Monday9 => {
            // Day of this week (Sunday first) the web client picks: Monday
            // of this week, or of the next.
            let early_monday = day.weekday == 1 && day.since_midnight < nine;
            let target = if day.weekday == 0 || early_monday { 1 } else { 8 };
            midnight + i64::from(target - day.weekday) * DAY + nine
        }
    }
}

/// A custom reminder from the form: `date` as the language writes it
/// (`day_first` for 30/10/2026, else 10/30/2026; 2026-10-30 always works)
/// and `clock` as "15:45" or "3:45 PM". `None` when it does not read as a
/// time, or is not in the future.
pub fn parse_custom(date: &str, clock: &str, day_first: bool, now: i64) -> Option<i64> {
    let (year, month, day) = parse_date(date.trim(), day_first)?;
    let (hour, minute) = parse_clock(clock.trim())?;
    time::local_to_secs(year, month, day, hour, minute).filter(|at| *at > now)
}

fn parse_date(date: &str, day_first: bool) -> Option<(i32, i32, i32)> {
    let parts: Vec<i32> = date.split(['/', '-', '.']).map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    match parts[..] {
        [year, month, day] if year > 31 => Some((year, month, day)),
        [a, b, year] if day_first => Some((year, b, a)),
        [a, b, year] => Some((year, a, b)),
        _ => None,
    }
}

fn parse_clock(clock: &str) -> Option<(i32, i32)> {
    let lower = clock.to_lowercase();
    let (digits, pm) = match lower.strip_suffix("pm").or_else(|| lower.strip_suffix("p.m.")) {
        Some(rest) => (rest.trim().to_owned(), Some(true)),
        None => match lower.strip_suffix("am").or_else(|| lower.strip_suffix("a.m.")) {
            Some(rest) => (rest.trim().to_owned(), Some(false)),
            None => (lower.clone(), None),
        },
    };
    let (hour, minute) = digits.split_once([':', 'h']).unwrap_or((&digits, "0"));
    let hour: i32 = hour.trim().parse().ok()?;
    let minute: i32 = if minute.trim().is_empty() { 0 } else { minute.trim().parse().ok()? };
    let hour = match pm {
        Some(_) if !(1..=12).contains(&hour) => return None,
        Some(true) => hour % 12 + 12,
        Some(false) => hour % 12,
        None => hour,
    };
    Some((hour, minute))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Wednesday at `hour`:`minute`, as seconds since an arbitrary
    /// midnight.
    fn at(weekday: i32, hour: i64, minute: i64) -> (i64, DayTime) {
        let since_midnight = hour * HOUR + minute * 60;
        (10 * DAY + since_midnight, DayTime { since_midnight, weekday })
    }

    #[test]
    fn nine_am_is_today_before_nine_else_tomorrow() {
        let (now, day) = at(3, 8, 59);
        assert_eq!(remind_at(RemindAt::NineAm, now, day), 10 * DAY + 9 * HOUR);
        assert_eq!(RemindAt::NineAm.label(day), "saved.today_9");
        let (now, day) = at(3, 9, 1);
        assert_eq!(remind_at(RemindAt::NineAm, now, day), 11 * DAY + 9 * HOUR);
        assert_eq!(RemindAt::NineAm.label(day), "saved.tomorrow_9");
    }

    #[test]
    fn monday_is_this_weeks_on_sunday_and_early_monday() {
        let (now, day) = at(0, 20, 0);
        assert_eq!(remind_at(RemindAt::Monday9, now, day), 11 * DAY + 9 * HOUR);
        let (now, day) = at(1, 8, 0);
        assert_eq!(remind_at(RemindAt::Monday9, now, day), 10 * DAY + 9 * HOUR);
        let (now, day) = at(1, 10, 0);
        assert_eq!(remind_at(RemindAt::Monday9, now, day), 17 * DAY + 9 * HOUR);
        let (now, day) = at(3, 10, 0);
        assert_eq!(remind_at(RemindAt::Monday9, now, day), 15 * DAY + 9 * HOUR);
    }

    #[test]
    fn in_minutes_and_hours() {
        let (now, day) = at(3, 12, 0);
        assert_eq!(remind_at(RemindAt::In30m, now, day), now + 1800);
        assert_eq!(remind_at(RemindAt::In4h, now, day), now + 4 * HOUR);
    }

    #[test]
    fn custom_times_read_both_orders_and_reject_the_past() {
        let now = time::local_to_secs(2026, 10, 30, 12, 0).unwrap();
        let expected = time::local_to_secs(2026, 11, 2, 15, 45);
        assert_eq!(parse_custom("02/11/2026", "15:45", true, now), expected);
        assert_eq!(parse_custom("11/02/2026", "3:45 PM", false, now), expected);
        assert_eq!(parse_custom("2026-11-02", "15h45", false, now), expected);
        assert_eq!(parse_custom("29/10/2026", "15:45", true, now), None);
        assert_eq!(parse_custom("soon", "15:45", true, now), None);
        assert_eq!(parse_custom("02/11/2026", "25:00", true, now), None);
    }
}
