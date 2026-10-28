//! One instance at a time. On Windows a mutex in the session's own
//! namespace decides which launch is first; a second launch, as when a link
//! opens the app, hands its arguments to the first through a named pipe
//! (after checking the pipe's server runs in the same session) and exits. macOS needs nothing: Launch Services keeps a
//! bundle to one instance and gives the running one its links.

/// Arguments longer than this are not forwarded.
#[cfg_attr(not(windows), allow(dead_code))]
const MAX_FORWARDED: usize = 4096;

/// The outcome of [`claim`].
pub enum Instance {
    /// This is the only instance. Arguments of later launches arrive here,
    /// one launch per item.
    First(async_channel::Receiver<Vec<String>>),
    /// Another instance was running and has `arguments` now; exit.
    Second,
}

/// Becomes the app's one instance under `id`, or forwards `arguments` to
/// the one already running.
pub fn claim(id: &str, arguments: &[String]) -> Instance {
    platform::claim(id, arguments)
}

/// Launch arguments as forwarded: one per line, so an argument cannot carry
/// a line break.
#[cfg_attr(not(windows), allow(dead_code))]
fn encode(arguments: &[String]) -> Option<String> {
    let text = arguments.iter().filter(|arg| !arg.contains(['\n', '\r'])).cloned().collect::<Vec<_>>().join("\n");
    (text.len() <= MAX_FORWARDED).then_some(text)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn decode(text: &str) -> Vec<String> {
    text.lines().filter(|line| !line.is_empty()).map(str::to_owned).collect()
}

#[cfg(windows)]
mod platform {
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Write};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::time::Duration;

    use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, ERROR_PIPE_CONNECTED, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
    use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND};
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeServerProcessId, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
        PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId};
    use windows_core::HSTRING;

    use super::{Instance, MAX_FORWARDED, decode, encode};

    fn session_of(process: u32) -> Option<u32> {
        let mut session = 0;
        // SAFETY: writes one u32.
        unsafe { ProcessIdToSessionId(process, &mut session) }.ok().map(|()| session)
    }

    /// The session's own pipe: other sessions run their own instance.
    fn pipe_name(id: &str, session: u32) -> String {
        format!(r"\\.\pipe\{id}-{session}")
    }

    /// Takes the session-local mutex that marks the first instance; false
    /// when another instance holds it. Other sessions cannot create objects
    /// in this namespace, so they cannot pose as the first instance.
    fn first(id: &str) -> bool {
        // SAFETY: a valid wide string; the handle is kept open for the life
        // of the process, which is what holds the claim.
        match unsafe { CreateMutexW(None, false, &HSTRING::from(format!(r"Local\{id}"))) } {
            Ok(mutex) => {
                let taken = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
                // SAFETY: owned from here; leaked on purpose while first.
                let mutex = unsafe { OwnedHandle::from_raw_handle(mutex.0) };
                if !taken {
                    std::mem::forget(mutex);
                }
                !taken
            }
            Err(e) => {
                tracing::warn!("instance: {e}");
                true
            }
        }
    }

    fn listen(name: &str, first: bool) -> Option<OwnedHandle> {
        let mut mode = PIPE_ACCESS_INBOUND;
        if first {
            mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }
        // SAFETY: a valid wide string; the default security only lets this
        // user (and administrators) write to the pipe.
        let pipe = unsafe {
            CreateNamedPipeW(
                &HSTRING::from(name),
                mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                0,
                MAX_FORWARDED as u32,
                0,
                None,
            )
        };
        // SAFETY: a fresh handle, owned from here on.
        (pipe != INVALID_HANDLE_VALUE).then(|| unsafe { OwnedHandle::from_raw_handle(pipe.0) })
    }

    pub fn claim(id: &str, arguments: &[String]) -> Instance {
        // SAFETY: no arguments.
        let session = session_of(unsafe { GetCurrentProcessId() }).unwrap_or_default();
        let name = pipe_name(id, session);
        if !first(id) {
            forward(&name, session, arguments);
            return Instance::Second;
        }
        let (sender, launches) = async_channel::bounded(8);
        let Some(mut pipe) = listen(&name, true) else {
            // Someone else holds the name: run, without later launches' links.
            tracing::warn!("instance: the pipe is taken; links from later launches will not arrive");
            return Instance::First(launches);
        };
        let spawned = std::thread::Builder::new().name("instance".into()).spawn(move || {
            loop {
                // SAFETY: `pipe` is an open pipe handle this thread owns.
                let connected = unsafe { ConnectNamedPipe(HANDLE(pipe.as_raw_handle()), None) };
                if connected.is_ok() || connected.as_ref().is_err_and(|e| e.code() == ERROR_PIPE_CONNECTED.to_hresult()) {
                    let file = File::from(pipe);
                    let mut text = String::new();
                    if file.take(MAX_FORWARDED as u64).read_to_string(&mut text).is_ok()
                        && sender.send_blocking(decode(&text)).is_err()
                    {
                        return;
                    }
                }
                match listen(&name, false) {
                    Some(next) => pipe = next,
                    None => return tracing::warn!("instance: the pipe could not be opened again"),
                }
            }
        });
        if let Err(e) = spawned {
            tracing::warn!("instance: {e}");
        }
        Instance::First(launches)
    }

    /// Hands `arguments` to the running instance, waiting a little while
    /// it is between two launches. Nothing is written to a pipe served from
    /// another session.
    fn forward(name: &str, session: u32, arguments: &[String]) {
        let Some(text) = encode(arguments) else { return };
        for _ in 0..10 {
            if let Ok(mut pipe) = OpenOptions::new().write(true).open(name) {
                let mut server = 0;
                // SAFETY: an open pipe handle; writes one u32.
                let served = unsafe { GetNamedPipeServerProcessId(HANDLE(pipe.as_raw_handle()), &mut server) };
                if served.is_err() || session_of(server) != Some(session) {
                    return tracing::warn!("instance: the pipe is not the running instance's");
                }
                if let Err(e) = pipe.write_all(text.as_bytes()) {
                    tracing::warn!("instance: could not forward the launch: {e}");
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        tracing::warn!("instance: the running instance did not answer");
    }
}

#[cfg(not(windows))]
mod platform {
    use super::Instance;

    pub fn claim(_: &str, _: &[String]) -> Instance {
        Instance::First(async_channel::bounded(1).1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_cross_as_lines() {
        let arguments = vec!["discord://-/channels/@me/1".to_owned(), "bad\nline".to_owned()];
        assert_eq!(decode(&encode(&arguments).unwrap()), vec!["discord://-/channels/@me/1"]);
        assert!(encode(&["x".repeat(MAX_FORWARDED + 1)]).is_none());
    }
}
