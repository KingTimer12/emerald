//! The token in Windows Credential Manager, encrypted with DPAPI for the
//! Windows user.

use windows::Win32::Foundation::ERROR_NOT_FOUND;
use windows::Win32::Security::Credentials::{
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW, CredWriteW,
};
use windows::core::{HSTRING, PWSTR};

use super::{Store, Token};

/// A generic credential named `{service}:{account}`.
pub struct CredentialManager {
    pub service: &'static str,
    pub account: String,
}

impl CredentialManager {
    fn target(&self) -> HSTRING {
        HSTRING::from(format!("{}:{}", self.service, self.account))
    }
}

fn not_found(e: &windows::core::Error) -> bool {
    e.code() == ERROR_NOT_FOUND.to_hresult()
}

impl Store for CredentialManager {
    fn name(&self) -> &'static str {
        "Credential Manager"
    }

    fn read(&self) -> std::io::Result<Option<Token>> {
        let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: CredReadW fills `credential` on success; it is freed below.
        match unsafe { CredReadW(&self.target(), CRED_TYPE_GENERIC, None, &mut credential) } {
            Ok(()) => {}
            Err(e) if not_found(&e) => return Ok(None),
            Err(e) => return Err(std::io::Error::other(e)),
        }
        // SAFETY: on success `credential` points at a valid CREDENTIALW whose
        // blob is `CredentialBlobSize` bytes long.
        let bytes = unsafe {
            let c = &*credential;
            let blob = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize);
            let bytes = blob.to_vec();
            CredFree(credential.cast());
            bytes
        };
        Token::from_bytes(bytes)
    }

    fn write(&self, token: &Token) -> std::io::Result<()> {
        let target = self.target();
        let user = HSTRING::from(self.account.as_str());
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

    fn delete(&self) -> std::io::Result<()> {
        // SAFETY: plain call with an owned, NUL-terminated target name.
        match unsafe { CredDeleteW(&self.target(), CRED_TYPE_GENERIC, None) } {
            Err(e) if !not_found(&e) => Err(std::io::Error::other(e)),
            _ => Ok(()),
        }
    }
}
