//! Putting a checked download in place. On macOS the archive is unpacked
//! when it arrives and the bundle is swapped on restart; on Windows the
//! installer runs silently on restart and opens the app again.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Packaging;

/// Makes a checked download ready to install: unpacks the macOS bundle
/// next to it. Returns what [`install`] takes.
pub fn stage(download: &Path, packaging: &Packaging) -> io::Result<PathBuf> {
    match packaging {
        Packaging::Bundle(_) => {
            let dir = download.with_extension("staged");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir)?;
            run(Command::new("/usr/bin/tar").arg("-xzf").arg(download).arg("-C").arg(&dir))?;
            std::fs::remove_file(download)?;
            let app = std::fs::read_dir(&dir)?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|path| path.extension().is_some_and(|ext| ext == "app"))
                .ok_or_else(|| io::Error::other("the archive holds no app bundle"))?;
            Ok(app)
        }
        Packaging::Installed(_) => Ok(download.to_owned()),
    }
}

/// Installs `staged` over the running copy and arranges for the new one
/// to open once this process exits. The caller quits right after.
pub fn install(staged: &Path, packaging: &Packaging) -> io::Result<()> {
    match packaging {
        Packaging::Bundle(bundle) => {
            swap_bundle(staged, bundle)?;
            // Opens the new bundle once this process is gone, so the two
            // never run at once.
            Command::new("/bin/sh")
                .arg("-c")
                .arg(r#"while kill -0 "$0" 2>/dev/null; do sleep 0.2; done; exec /usr/bin/open "$1""#)
                .arg(std::process::id().to_string())
                .arg(bundle)
                .spawn()?;
            Ok(())
        }
        Packaging::Installed(_) => {
            // The installer closes what is left of this process and opens
            // the app again when it is done (its `skipifnotsilent` entry).
            Command::new(staged).args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]).spawn()?;
            Ok(())
        }
    }
}

/// Replaces `bundle` with a copy of `staged`, keeping the old one until the
/// new one is in place.
fn swap_bundle(staged: &Path, bundle: &Path) -> io::Result<()> {
    let parent = bundle.parent().ok_or_else(|| io::Error::other("the bundle has no folder"))?;
    let name = bundle.file_name().ok_or_else(|| io::Error::other("the bundle has no name"))?.to_string_lossy();
    let incoming = parent.join(format!(".{name}.new"));
    let outgoing = parent.join(format!(".{name}.old"));
    let _ = std::fs::remove_dir_all(&incoming);
    let _ = std::fs::remove_dir_all(&outgoing);
    // ditto keeps the signature, extended attributes and symlinks.
    run(Command::new("/usr/bin/ditto").arg(staged).arg(&incoming))?;
    std::fs::rename(bundle, &outgoing)?;
    if let Err(e) = std::fs::rename(&incoming, bundle) {
        let _ = std::fs::rename(&outgoing, bundle);
        return Err(e);
    }
    let _ = std::fs::remove_dir_all(&outgoing);
    Ok(())
}

fn run(command: &mut Command) -> io::Result<()> {
    let status = command.status()?;
    if status.success() { Ok(()) } else { Err(io::Error::other(format!("{command:?} failed: {status}"))) }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_bundle_is_swapped_whole() {
        let root = std::env::temp_dir().join(format!("emerald-desktop-swap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let bundle = root.join("Apps/Test.app");
        let staged = root.join("staged/Test.app");
        std::fs::create_dir_all(bundle.join("Contents")).unwrap();
        std::fs::write(bundle.join("Contents/version"), "old").unwrap();
        std::fs::create_dir_all(staged.join("Contents")).unwrap();
        std::fs::write(staged.join("Contents/version"), "new").unwrap();
        if Path::new("/usr/bin/ditto").exists() {
            swap_bundle(&staged, &bundle).unwrap();
            assert_eq!(std::fs::read_to_string(bundle.join("Contents/version")).unwrap(), "new");
            assert!(!root.join("Apps/.Test.app.old").exists());
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}
