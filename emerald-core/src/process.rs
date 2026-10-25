//! Runs of this same executable: child modes for flows that need their own
//! native event loop (the login and captcha webviews), and restarts.

use std::io::Write;
use std::process::{Command, Stdio};

/// Runs this binary with `flag` (child mode), feeding `input` on stdin, and
/// returns the trimmed stdout of a successful run.
pub fn run_child(flag: &str, input: &[u8]) -> Option<String> {
    let mut child = Command::new(std::env::current_exe().ok()?)
        .arg(flag)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(input).ok()?;
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    let result = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!result.is_empty()).then_some(result)
}

/// Starts this binary afresh and ends the current process. On Unix `exec`
/// replaces the process; on Windows the new one is spawned and this one
/// exits. Either way gpui, the network thread and the gateway session all
/// go away with it. Returns only on failure.
pub fn restart() -> std::io::Error {
    match std::env::current_exe() {
        Ok(exe) => replace_with(&mut Command::new(exe)),
        Err(e) => e,
    }
}

/// Replaces this process with `command`; returns only on failure.
#[cfg(unix)]
fn replace_with(command: &mut Command) -> std::io::Error {
    use std::os::unix::process::CommandExt;
    command.exec()
}

#[cfg(windows)]
fn replace_with(command: &mut Command) -> std::io::Error {
    match command.spawn() {
        Ok(_) => std::process::exit(0),
        Err(e) => e,
    }
}
