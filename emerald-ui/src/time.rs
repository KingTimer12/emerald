//! Message timestamps in the user's timezone, worded like the Discord client
//! in the interface language.

use std::sync::atomic::{AtomicU8, Ordering};

use crate::settings::TimeFormat;

/// The clock Language & Time asks for; `Auto` follows the language.
static TIME_FORMAT: AtomicU8 = AtomicU8::new(0);

pub fn set_time_format(format: TimeFormat) {
    TIME_FORMAT.store(format as u8, Ordering::Relaxed);
}

/// Whether times use a 12-hour clock with AM/PM.
fn twelve_hour() -> bool {
    match TIME_FORMAT.load(Ordering::Relaxed) {
        f if f == TimeFormat::Hours12 as u8 => true,
        f if f == TimeFormat::Hours24 as u8 => false,
        // `time.hour_cycle` is "12" or "24", like the web client's per locale.
        _ => t!("time.hour_cycle") == "12",
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Local {
    year: i32,
    /// Day of the year, 0-based.
    yday: i32,
    month: i32,
    day: i32,
    hour: i32,
    minute: i32,
    second: i32,
    /// Day of the week, 0 for Sunday.
    weekday: i32,
}

fn local(secs: i64) -> Option<Local> {
    let time = secs as libc::time_t;
    // SAFETY: localtime_r / localtime_s only write into the zeroed `tm` we own.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    #[cfg(unix)]
    if unsafe { libc::localtime_r(&time, &mut tm) }.is_null() {
        return None;
    }
    #[cfg(windows)]
    if unsafe { libc::localtime_s(&mut tm, &time) } != 0 {
        return None;
    }
    Some(Local {
        year: tm.tm_year + 1900,
        yday: tm.tm_yday,
        month: tm.tm_mon + 1,
        day: tm.tm_mday,
        hour: tm.tm_hour,
        minute: tm.tm_min,
        second: tm.tm_sec,
        weekday: tm.tm_wday,
    })
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// `secs` as Discord writes timestamps, in UTC: "2025-01-02T14:05:00+00:00".
pub fn iso_utc(secs: i64) -> String {
    // Days to civil date, after Howard Hinnant's algorithm.
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let (hour, minute, second) = (rest / 3600, rest / 60 % 60, rest % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}+00:00")
}

/// "Today at 14:05", "Yesterday at 14:05" or "02/01/2025 14:05", in the
/// interface language.
pub fn header(secs: i64) -> String {
    let Some(at) = local(secs) else { return String::new() };
    let same_day = |other: Option<Local>| other.is_some_and(|o| (o.year, o.yday) == (at.year, at.yday));
    let now = now_secs();
    let time = clock(at);
    if same_day(local(now)) {
        t!("time.today", time = time)
    } else if same_day(local(now - 86_400)) {
        t!("time.yesterday", time = time)
    } else {
        let date = t!(
            "time.numeric_date",
            day = format!("{:02}", at.day),
            month = format!("{:02}", at.month),
            year = at.year
        );
        t!("time.date_time", date = date, time = time)
    }
}

/// "14:05", for compact mode.
pub fn short(secs: i64) -> String {
    local(secs).map_or_else(String::new, clock)
}

/// The time of day as the language writes it: "14:05" or "2:05 PM".
fn clock(at: Local) -> String {
    let minute = format!("{:02}", at.minute);
    if twelve_hour() {
        let hour = match at.hour % 12 {
            0 => 12,
            h => h,
        };
        let ampm = if at.hour < 12 { t!("time.am") } else { t!("time.pm") };
        t!("time.clock_12", hour = hour, minute = minute, ampm = ampm)
    } else {
        t!("time.clock_24", hour = format!("{:02}", at.hour), minute = minute)
    }
}

const MONTHS: [&str; 12] = [
    "time.month.january",
    "time.month.february",
    "time.month.march",
    "time.month.april",
    "time.month.may",
    "time.month.june",
    "time.month.july",
    "time.month.august",
    "time.month.september",
    "time.month.october",
    "time.month.november",
    "time.month.december",
];

const SHORT_MONTHS: [&str; 12] = [
    "time.month_short.january",
    "time.month_short.february",
    "time.month_short.march",
    "time.month_short.april",
    "time.month_short.may",
    "time.month_short.june",
    "time.month_short.july",
    "time.month_short.august",
    "time.month_short.september",
    "time.month_short.october",
    "time.month_short.november",
    "time.month_short.december",
];

/// Identifies the local calendar day, for day separators.
pub fn day_key(secs: i64) -> i32 {
    local(secs).map_or(0, |at| at.year * 1000 + at.yday)
}

/// "19 de outubro de 2026", the day separator label.
pub fn day_label(secs: i64) -> String {
    local(secs).map_or_else(String::new, |at| {
        let month = emerald_core::i18n::text(MONTHS[(at.month - 1) as usize]);
        t!("time.day_label", day = at.day, month = month, year = at.year)
    })
}

const WEEKDAYS: [&str; 7] = [
    "time.weekday.sunday",
    "time.weekday.monday",
    "time.weekday.tuesday",
    "time.weekday.wednesday",
    "time.weekday.thursday",
    "time.weekday.friday",
    "time.weekday.saturday",
];

/// A `<t:secs:style>` timestamp as the reader sees it, in their timezone:
/// `t` "14:05", `T` "14:05:30", `d` "10/24/2026", `D` "October 24, 2026",
/// `f` (the default) with the time, `F` with the weekday too, and `R`
/// relative to now, "3 hours ago".
pub fn stamp(secs: i64, style: char) -> String {
    if style == 'R' {
        return relative(secs - now_secs());
    }
    let Some(at) = local(secs) else { return String::new() };
    let long_date = || {
        let month = emerald_core::i18n::text(MONTHS[(at.month - 1) as usize]);
        t!("time.day_label", day = at.day, month = month, year = at.year)
    };
    match style {
        't' => clock(at),
        'T' => clock_seconds(at),
        'd' => t!(
            "time.numeric_date",
            day = format!("{:02}", at.day),
            month = format!("{:02}", at.month),
            year = at.year
        ),
        'D' => long_date(),
        'F' => {
            let weekday = emerald_core::i18n::text(WEEKDAYS[at.weekday.rem_euclid(7) as usize]);
            let date = t!("time.long_date", weekday = weekday, date = long_date());
            t!("time.date_time", date = date, time = clock(at))
        }
        _ => t!("time.date_time", date = long_date(), time = clock(at)),
    }
}

/// "3 hours ago" or "in 2 days" for a moment `delta` seconds from now, in
/// the largest unit that fits.
fn relative(delta: i64) -> String {
    let amount = duration(delta.unsigned_abs());
    if delta > 0 {
        t!("time.from_now", amount = amount)
    } else {
        t!("time.ago", amount = amount)
    }
}

/// A span of time in the largest unit that fits: "3 hours".
pub fn duration(secs: u64) -> String {
    match secs {
        0..60 => unit(secs, t!("time.unit.second_one"), |count| t!("time.unit.second", count = count)),
        60..3_600 => unit(secs / 60, t!("time.unit.minute_one"), |count| t!("time.unit.minute", count = count)),
        3_600..86_400 => unit(secs / 3_600, t!("time.unit.hour_one"), |count| t!("time.unit.hour", count = count)),
        86_400..2_592_000 => unit(secs / 86_400, t!("time.unit.day_one"), |count| t!("time.unit.day", count = count)),
        2_592_000..31_536_000 => {
            unit(secs / 2_592_000, t!("time.unit.month_one"), |count| t!("time.unit.month", count = count))
        }
        _ => unit(secs / 31_536_000, t!("time.unit.year_one"), |count| t!("time.unit.year", count = count)),
    }
}

/// `one` for a count of one, else `many` of it.
fn unit(count: u64, one: &str, many: impl Fn(u64) -> String) -> String {
    if count == 1 { one.to_owned() } else { many(count) }
}

/// The time of day with seconds: "14:05:30" or "2:05:30 PM".
fn clock_seconds(at: Local) -> String {
    let (minute, second) = (format!("{:02}", at.minute), format!("{:02}", at.second));
    if twelve_hour() {
        let hour = match at.hour % 12 {
            0 => 12,
            h => h,
        };
        let ampm = if at.hour < 12 { t!("time.am") } else { t!("time.pm") };
        t!("time.clock_12_seconds", hour = hour, minute = minute, second = second, ampm = ampm)
    } else {
        t!("time.clock_24_seconds", hour = format!("{:02}", at.hour), minute = minute, second = second)
    }
}

/// "12 de jul. de 2017", as "Membro desde" shows it.
pub fn short_date(secs: i64) -> String {
    local(secs).map_or_else(String::new, |at| {
        let month = emerald_core::i18n::text(SHORT_MONTHS[(at.month - 1) as usize]);
        t!("time.short_date", day = at.day, month = month, year = at.year)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates_match_discord() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00+00:00");
        assert_eq!(iso_utc(1_735_826_700), "2025-01-02T14:05:00+00:00");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00+00:00");
    }

    #[test]
    fn relative_times_pick_the_largest_unit() {
        assert_eq!(relative(-5), "5 seconds ago");
        assert_eq!(relative(-3_600), "1 hour ago");
        assert_eq!(relative(2 * 86_400 + 10), "in 2 days");
        assert_eq!(relative(-400 * 86_400), "1 year ago");
    }
}
