//! How the running copy was installed, which decides what can be updated
//! and what starts on login.

use std::path::{Path, PathBuf};

/// Where the running executable came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Packaging {
    /// A macOS app bundle: the `.app` folder.
    Bundle(PathBuf),
    /// Installed by the Windows installer, which leaves its uninstaller
    /// beside the executable: the install folder.
    Installed(PathBuf),
}

impl Packaging {
    /// The running copy's packaging; `None` for a build run from `target/`.
    pub fn current() -> Option<Self> {
        let exe = std::env::current_exe().ok()?;
        Self::of(&exe.canonicalize().unwrap_or(exe))
    }

    fn of(exe: &Path) -> Option<Self> {
        if cfg!(target_os = "macos") {
            // Emerald.app/Contents/MacOS/emerald
            let bundle = exe.parent()?.parent()?.parent()?;
            let is_bundle = exe.parent()?.ends_with("Contents/MacOS")
                && bundle.extension().is_some_and(|ext| ext == "app");
            is_bundle.then(|| Self::Bundle(bundle.to_owned()))
        } else if cfg!(windows) {
            let dir = exe.parent()?;
            dir.join("unins000.exe").is_file().then(|| Self::Installed(dir.to_owned()))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn a_bundle_is_found_from_its_executable() {
        assert_eq!(
            Packaging::of(Path::new("/Applications/Emerald.app/Contents/MacOS/emerald")),
            Some(Packaging::Bundle("/Applications/Emerald.app".into()))
        );
        assert_eq!(Packaging::of(Path::new("/src/emerald/target/release/emerald")), None);
    }
}
