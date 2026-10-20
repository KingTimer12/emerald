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

const MONTHS: [&str; 12] = [
    "janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro",
    "outubro", "novembro", "dezembro",
];

/// Identifies the local calendar day, for day separators.
pub fn day_key(secs: i64) -> i32 {
    local(secs).map_or(0, |at| at.year * 1000 + at.yday)
}

/// "19 de outubro de 2026", the day separator label.
pub fn day_label(secs: i64) -> String {
    local(secs).map_or_else(String::new, |at| {
        format!("{} de {} de {}", at.day, MONTHS[(at.month - 1) as usize], at.year)
    })
}

/// "12 de jul. de 2017", as "Membro desde" shows it. May stays whole.
pub fn short_date(secs: i64) -> String {
    local(secs).map_or_else(String::new, |at| {
        let month = MONTHS[(at.month - 1) as usize];
        let month = if month.chars().count() > 4 {
            format!("{}.", month.chars().take(3).collect::<String>())
        } else {
            month.to_owned()
        };
        format!("{} de {month} de {}", at.day, at.year)
    })
}
