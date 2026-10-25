//! Logs through `tracing`: to stderr, and to a daily file kept for a week
//! so a problem can be looked into after the fact. Crates that log through
//! `log` are bridged in.
//!
//! The filter comes from `EMERALD_LOG` (or `RUST_LOG`), in `tracing`'s
//! directive syntax; by default Emerald logs at info and its dependencies
//! only their warnings. Logs never hold the token, and message text only
//! when a filter at trace asks for it.

use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

/// `emerald` matches by prefix, so it covers every `emerald_*` crate.
const DEFAULT_FILTER: &str = "warn,emerald=info";
/// Daily files kept before the oldest goes.
const KEPT_FILES: usize = 7;

/// Keeps the file writer flushing until it drops at exit.
pub struct Logging {
    _file: Option<WorkerGuard>,
    /// Where the log files are, when one could be opened.
    pub dir: Option<PathBuf>,
}

fn filter() -> EnvFilter {
    let directives = std::env::var("EMERALD_LOG")
        .or_else(|_| std::env::var("RUST_LOG"))
        .unwrap_or_else(|_| DEFAULT_FILTER.to_owned());
    EnvFilter::try_new(&directives).unwrap_or_else(|e| {
        eprintln!("ignoring the log filter {directives:?}: {e}");
        EnvFilter::new(DEFAULT_FILTER)
    })
}

/// Logging for the main process: stderr and the log file.
pub fn init() -> Logging {
    let dir = emerald_core::paths::log_dir();
    let file = dir.as_ref().filter(|dir| private_dir(dir)).and_then(|dir| {
        RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("emerald")
            .filename_suffix("log")
            .max_log_files(KEPT_FILES)
            .build(dir)
            .map_err(|e| eprintln!("could not open the log file in {}: {e}", dir.display()))
            .ok()
    });
    let (writer, guard) = match file {
        Some(file) => {
            let (writer, guard) = tracing_appender::non_blocking(file);
            (Some(writer), Some(guard))
        }
        None => (None, None),
    };
    let opened = guard.is_some();
    tracing_subscriber::registry()
        .with(filter())
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(writer.map(|writer| fmt::layer().with_ansi(false).with_writer(writer)))
        .init();
    log_panics();
    Logging {
        _file: guard,
        dir: dir.filter(|_| opened),
    }
}

/// Logs name channels, servers and people by id: only the user reads them.
fn private_dir(dir: &std::path::Path) -> bool {
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("could not create {}: {e}", dir.display());
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)) {
            eprintln!("could not make {} private: {e}", dir.display());
        }
    }
    true
}

/// Logging for the login and captcha child processes: stderr only, so
/// they never write the parent's file at the same time.
pub fn init_child() {
    tracing_subscriber::registry()
        .with(filter())
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();
    log_panics();
}

/// Panics go to the log as well as stderr, with where they happened.
fn log_panics() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        tracing::error!(%location, "panic: {message}");
        default(info);
    }));
}
