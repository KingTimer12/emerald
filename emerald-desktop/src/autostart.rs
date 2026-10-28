//! Opening the app when the user logs in: a LaunchAgent in
//! `~/Library/LaunchAgents` on macOS, a value under the current user's
//! `Run` key on Windows. Both point at the copy running now; turning the
//! option off and on again follows a moved app.

use std::io;

/// Whether the app opens on login. `id` names the entry: a reverse-DNS
/// label on macOS, the value's name on Windows.
pub fn enabled(id: &str) -> bool {
    platform::enabled(id)
}

/// Opens the app on login, or stops doing so.
pub fn set(id: &str, on: bool) -> io::Result<()> {
    platform::set(id, on)
}

#[cfg(target_os = "macos")]
mod platform {
    use std::io;
    use std::path::PathBuf;

    use crate::Packaging;

    fn agent(id: &str) -> io::Result<PathBuf> {
        let home = std::env::var_os("HOME").ok_or_else(|| io::Error::other("no home folder"))?;
        Ok(PathBuf::from(home).join("Library/LaunchAgents").join(format!("{id}.plist")))
    }

    pub fn enabled(id: &str) -> bool {
        agent(id).is_ok_and(|path| path.is_file())
    }

    pub fn set(id: &str, on: bool) -> io::Result<()> {
        let path = agent(id)?;
        if !on {
            return match std::fs::remove_file(&path) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                other => other,
            };
        }
        // A bundle opens through Launch Services, as from the Dock.
        let program: Vec<String> = match Packaging::current() {
            Some(Packaging::Bundle(bundle)) => {
                vec!["/usr/bin/open".into(), "-a".into(), bundle.display().to_string()]
            }
            _ => vec![std::env::current_exe()?.display().to_string()],
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, super::launch_agent(id, &program))
    }
}

#[cfg(windows)]
mod platform {
    use std::io;

    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR};
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
    };
    use windows_core::HSTRING;

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    fn check(error: WIN32_ERROR) -> io::Result<()> {
        error.ok().map_err(|e| io::Error::other(e.message()))
    }

    pub fn enabled(id: &str) -> bool {
        // SAFETY: valid wide strings; only the size is asked for.
        let error = unsafe {
            RegGetValueW(HKEY_CURRENT_USER, &HSTRING::from(RUN), &HSTRING::from(id), RRF_RT_REG_SZ, None, None, None)
        };
        error.is_ok()
    }

    pub fn set(id: &str, on: bool) -> io::Result<()> {
        if !on {
            // SAFETY: valid wide strings.
            let error = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(RUN), &HSTRING::from(id)) };
            return if error == ERROR_FILE_NOT_FOUND { Ok(()) } else { check(error) };
        }
        let command = format!("\"{}\"", std::env::current_exe()?.display());
        let data: Vec<u16> = command.encode_utf16().chain([0]).collect();
        // SAFETY: `data` is a NUL-terminated wide string of the size given.
        let error = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(RUN),
                &HSTRING::from(id),
                REG_SZ.0,
                Some(data.as_ptr().cast()),
                (data.len() * 2) as u32,
            )
        };
        check(error)
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    use std::io;

    pub fn enabled(_: &str) -> bool {
        false
    }

    pub fn set(_: &str, _: bool) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not supported on this platform"))
    }
}

/// A LaunchAgent that runs `program` once at login.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn launch_agent(id: &str, program: &[String]) -> String {
    let arguments: String = program.iter().map(|arg| format!("\t\t<string>{}</string>\n", escape(arg))).collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n\
         \t<key>Label</key>\n\t<string>{}</string>\n\
         \t<key>ProgramArguments</key>\n\t<array>\n{arguments}\t</array>\n\
         \t<key>RunAtLoad</key>\n\t<true/>\n\
         </dict>\n</plist>\n",
        escape(id)
    )
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launch_agent_runs_the_program_at_load() {
        let plist = launch_agent("app.example", &["/usr/bin/open".into(), "-a".into(), "/Apps/A & B.app".into()]);
        assert!(plist.contains("<string>app.example</string>"));
        assert!(plist.contains("<string>/Apps/A &amp; B.app</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>\n\t<true/>"));
    }
}
