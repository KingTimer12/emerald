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
    })
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
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
        let month = crate::i18n::text(MONTHS[(at.month - 1) as usize]);
        t!("time.day_label", day = at.day, month = month, year = at.year)
    })
}

/// "12 de jul. de 2017", as "Membro desde" shows it.
pub fn short_date(secs: i64) -> String {
    local(secs).map_or_else(String::new, |at| {
        let month = crate::i18n::text(SHORT_MONTHS[(at.month - 1) as usize]);
        t!("time.short_date", day = at.day, month = month, year = at.year)
    })
}
