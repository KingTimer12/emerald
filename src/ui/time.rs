//! Message timestamps in the user's timezone, worded like the pt-BR client.

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
    // SAFETY: localtime_r only writes into the zeroed `tm` we own.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&time, &mut tm) }.is_null() {
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

/// "Hoje às 14:05", "Ontem às 14:05" or "02/01/2025 14:05".
pub fn header(secs: i64) -> String {
    let Some(at) = local(secs) else { return String::new() };
    let same_day = |other: Option<Local>| other.is_some_and(|o| (o.year, o.yday) == (at.year, at.yday));
    let now = now_secs();
    if same_day(local(now)) {
        format!("Hoje às {:02}:{:02}", at.hour, at.minute)
    } else if same_day(local(now - 86_400)) {
        format!("Ontem às {:02}:{:02}", at.hour, at.minute)
    } else {
        format!(
            "{:02}/{:02}/{} {:02}:{:02}",
            at.day, at.month, at.year, at.hour, at.minute
        )
    }
}

/// "14:05", for compact mode.
pub fn short(secs: i64) -> String {
    local(secs).map_or_else(String::new, |at| format!("{:02}:{:02}", at.hour, at.minute))
}
