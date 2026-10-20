//! The account token: how it is held in memory and where it is stored.
//!
//! Release builds keep it in the OS credential store: the login Keychain on
//! macOS, Credential Manager on Windows (DPAPI, tied to the Windows user).
//! A token left in the old plain file is moved there and the file deleted.
//! Debug builds, and platforms without a store, use the file
//! (`token` under `paths::config_dir`, mode 0600): the Keychain trusts one
//! exact binary, so every rebuild would ask for the password again.

use std::path::PathBuf;

use zeroize::Zeroizing;

/// Service and account names of the credential store item.
const SERVICE: &str = "Emerald";
const ACCOUNT: &str = "discord-token";

/// Wiped from memory when dropped and never printed.
pub struct Token(Zeroizing<String>);

impl Token {
    pub fn new(token: String) -> Option<Self> {
        let token = Zeroizing::new(token);
        let trimmed = token.trim();
        (!trimmed.is_empty()).then(|| Self(Zeroizing::new(trimmed.to_owned())))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(..)")
    }
}

/// `DISCORD_TOKEN` (removed from the environment once read, so child
/// processes do not inherit it), else the stored token.
///
/// Call it before any other thread starts: it edits the environment.
pub fn load() -> Option<Token> {
    if let Some(token) = std::env::var("DISCORD_TOKEN").ok().and_then(Token::new) {
        // SAFETY: main calls this while the process is still single-threaded.
        unsafe { std::env::remove_var("DISCORD_TOKEN") };
        return Some(token);
    }
    Store::default().load()
}

pub fn save(token: &Token) -> std::io::Result<()> {
    Store::default().write(token)
}

/// Forgets the stored token everywhere it might be.
pub fn forget() {
    Store::default().forget();
}

/// Where the token lives on this build and platform.
struct Store {
    /// Credential store service name; `None` keeps the token in `file`.
    secure: Option<&'static str>,
    file: Option<PathBuf>,
}

impl Default for Store {
    fn default() -> Self {
        let secure = !cfg!(debug_assertions) && cfg!(any(target_os = "macos", windows));
        Self {
            secure: secure.then_some(SERVICE),
            file: crate::paths::config_dir().map(|dir| dir.join("token")),
        }
    }
}

impl Store {
    fn name(&self) -> &'static str {
        match self.secure {
            Some(_) if cfg!(windows) => "Credential Manager",
            Some(_) => "Keychain",
            None => "token file",
        }
    }

    fn load(&self) -> Option<Token> {
        let Some(service) = self.secure else { return self.read_file() };
        match os::read(service) {
            Ok(Some(token)) => return Some(token),
            Ok(None) => {}
            Err(e) => log::error!("could not read the token from the {}: {e}", self.name()),
        }
        // A token saved by an older build: move it into the credential store.
        let token = self.read_file()?;
        match os::write(service, &token) {
            Ok(()) => self.remove_file(),
            Err(e) => log::error!("could not move the token to the {}: {e}", self.name()),
        }
        Some(token)
    }

    fn write(&self, token: &Token) -> std::io::Result<()> {
        match self.secure {
            Some(service) => os::write(service, token),
            None => self.write_file(token),
        }
    }

    fn forget(&self) {
        if let Some(service) = self.secure
            && let Err(e) = os::delete(service)
        {
            log::error!("could not delete the token from the {}: {e}", self.name());
        }
        self.remove_file();
    }

    fn read_file(&self) -> Option<Token> {
        Token::new(std::fs::read_to_string(self.file.as_ref()?).ok()?)
    }

    fn remove_file(&self) {
        let Some(path) = &self.file else { return };
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => log::error!("could not delete {}: {e}", path.display()),
        }
    }

    fn write_file(&self, token: &Token) -> std::io::Result<()> {
        use std::io::Write;

        let path = self.file.as_ref().ok_or(std::io::ErrorKind::NotFound)?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        // On Windows the per-user AppData ACL already keeps other users out.
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&tmp)?;
        file.write_all(token.expose().as_bytes())?;
        file.sync_all()?;
        std::fs::rename(tmp, path)
    }
}

#[cfg(target_os = "macos")]
mod os {
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };

    use super::{ACCOUNT, Token};

    /// errSecItemNotFound.
    const NOT_FOUND: i32 = -25300;

    pub fn read(service: &str) -> std::io::Result<Option<Token>> {
        match get_generic_password(service, ACCOUNT) {
            Ok(bytes) => {
                let bytes = zeroize::Zeroizing::new(bytes);
                let text = String::from_utf8(bytes.to_vec()).map_err(std::io::Error::other)?;
                Ok(Token::new(text))
            }
            Err(e) if e.code() == NOT_FOUND => Ok(None),
            Err(e) => Err(std::io::Error::other(e)),
        }
    }

    pub fn write(service: &str, token: &Token) -> std::io::Result<()> {
        set_generic_password(service, ACCOUNT, token.expose().as_bytes()).map_err(std::io::Error::other)
    }

    pub fn delete(service: &str) -> std::io::Result<()> {
        match delete_generic_password(service, ACCOUNT) {
            Err(e) if e.code() != NOT_FOUND => Err(std::io::Error::other(e)),
            _ => Ok(()),
        }
    }
}

#[cfg(windows)]
mod os {
    use windows::Win32::Foundation::ERROR_NOT_FOUND;
    use windows::Win32::Security::Credentials::{
        CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree,
        CredReadW, CredWriteW,
    };
    use windows::core::{HSTRING, PWSTR};

    use super::{ACCOUNT, Token};

    fn target(service: &str) -> HSTRING {
        HSTRING::from(format!("{service}:{ACCOUNT}"))
    }

    fn not_found(e: &windows::core::Error) -> bool {
        e.code() == ERROR_NOT_FOUND.to_hresult()
    }

    pub fn read(service: &str) -> std::io::Result<Option<Token>> {
        let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: CredReadW fills `credential` on success; it is freed below.
        match unsafe { CredReadW(&target(service), CRED_TYPE_GENERIC, None, &mut credential) } {
            Ok(()) => {}
            Err(e) if not_found(&e) => return Ok(None),
            Err(e) => return Err(std::io::Error::other(e)),
        }
        // SAFETY: on success `credential` points at a valid CREDENTIALW whose
        // blob is `CredentialBlobSize` bytes long.
        let bytes = unsafe {
            let c = &*credential;
            let blob = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize);
            let bytes = zeroize::Zeroizing::new(blob.to_vec());
            CredFree(credential.cast());
            bytes
        };
        let text = String::from_utf8(bytes.to_vec()).map_err(std::io::Error::other)?;
        Ok(Token::new(text))
    }

    pub fn write(service: &str, token: &Token) -> std::io::Result<()> {
        let target = target(service);
        let user = HSTRING::from(ACCOUNT);
        let mut blob = token.expose().as_bytes().to_vec();
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_ptr().cast_mut()),
            UserName: PWSTR(user.as_ptr().cast_mut()),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        // SAFETY: every pointer in `credential` outlives the call.
        let result = unsafe { CredWriteW(&credential, 0) };
        zeroize::Zeroize::zeroize(&mut blob);
        result.map_err(std::io::Error::other)
    }

    pub fn delete(service: &str) -> std::io::Result<()> {
        // SAFETY: plain call with an owned, NUL-terminated target name.
        match unsafe { CredDeleteW(&target(service), CRED_TYPE_GENERIC, None) } {
            Err(e) if !not_found(&e) => Err(std::io::Error::other(e)),
            _ => Ok(()),
        }
    }
}

/// No credential store wired up: `Store::default` never picks `Secure`.
#[cfg(not(any(target_os = "macos", windows)))]
mod os {
    use super::Token;

    pub fn read(_: &str) -> std::io::Result<Option<Token>> {
        Ok(None)
    }

    pub fn write(_: &str, _: &Token) -> std::io::Result<()> {
        Err(std::io::ErrorKind::Unsupported.into())
    }

    pub fn delete(_: &str) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_trimmed_and_never_printed() {
        assert!(Token::new("  \n".into()).is_none());
        let token = Token::new(" abc.def \n".into()).unwrap();
        assert_eq!(token.expose(), "abc.def");
        assert_eq!(format!("{token:?}"), "Token(..)");
    }

    /// Under a test-only service name and a temporary file, so the user's
    /// token is never touched.
    #[cfg(any(target_os = "macos", windows))]
    fn test_store(name: &str) -> Store {
        let dir = std::env::temp_dir().join(format!("emerald-token-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Store { secure: Some("Emerald (test)"), file: Some(dir.join("token")) }
    }

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn credential_store_round_trip_and_migration() {
        let store = test_store("store");
        store.forget();
        assert!(store.load().is_none());
        store.write(&Token::new("first".into()).unwrap()).unwrap();
        store.write(&Token::new("second".into()).unwrap()).unwrap();
        assert_eq!(store.load().unwrap().expose(), "second");
        store.forget();
        assert!(store.load().is_none());

        // A plain file from an older build moves into the store and is deleted.
        let file = store.file.clone().unwrap();
        std::fs::write(&file, "from-file\n").unwrap();
        assert_eq!(store.load().unwrap().expose(), "from-file");
        assert!(!file.exists());
        assert_eq!(os::read(store.secure.unwrap()).unwrap().unwrap().expose(), "from-file");
        store.forget();
        std::fs::remove_dir_all(file.parent().unwrap()).ok();
    }
}
