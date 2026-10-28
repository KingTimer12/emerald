//! Release signatures: ed25519 over a SHA-512 (Ed25519ph), so a download
//! is hashed as it arrives instead of being held in memory to be checked.
//! The hash starts with the release's version and platform, then the
//! file: a signature only vouches for that file as that version for that
//! platform, so an older build cannot be offered again as a newer one.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use sha2::{Digest, Sha512};

/// Binds signatures to this use, so no other ed25519 signature made with
/// the same key passes for a release.
const CONTEXT: &[u8] = b"emerald-desktop release";

/// The hash a release's file is fed into, already bound to `version`
/// (a leading `v` ignored) and `platform`.
pub fn begin(version: &str, platform: &str) -> Sha512 {
    let mut digest = Sha512::new();
    for part in [b"emerald release".as_slice(), version.trim().trim_start_matches('v').as_bytes(), platform.as_bytes()] {
        digest.update((part.len() as u64).to_le_bytes());
        digest.update(part);
    }
    digest
}

/// A release key's public half, from its base64 form.
pub fn public_key(base64: &str) -> Option<VerifyingKey> {
    let bytes: [u8; 32] = STANDARD.decode(base64.trim()).ok()?.try_into().ok()?;
    VerifyingKey::from_bytes(&bytes).ok()
}

/// Whether `signature` (base64) signs the file hashed into `digest`.
pub fn verify(key: &VerifyingKey, digest: Sha512, signature: &str) -> bool {
    let Ok(bytes) = STANDARD.decode(signature.trim()) else { return false };
    let Ok(signature) = Signature::from_slice(&bytes) else { return false };
    key.verify_prehashed_strict(digest, Some(CONTEXT), &signature).is_ok()
}

/// Signs the file hashed into `digest`, as the release does; base64.
pub fn sign(key: &SigningKey, digest: Sha512) -> String {
    let signature = key.sign_prehashed(digest, Some(CONTEXT)).expect("the context is short enough");
    STANDARD.encode(signature.to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(data: &[u8]) -> Sha512 {
        let mut hasher = begin("1.0.0", "macos-aarch64");
        hasher.update(data);
        hasher
    }

    #[test]
    fn only_the_signed_file_passes() {
        let secret = SigningKey::from_bytes(&[7; 32]);
        let public = public_key(&STANDARD.encode(secret.verifying_key().to_bytes())).unwrap();
        let signature = sign(&secret, digest(b"release"));
        assert!(verify(&public, digest(b"release"), &signature));
        assert!(!verify(&public, digest(b"tampered"), &signature));
        assert!(!verify(&public, digest(b"release"), "not base64!"));
        let other = SigningKey::from_bytes(&[8; 32]).verifying_key();
        assert!(!verify(&other, digest(b"release"), &signature));
    }

    #[test]
    fn a_signature_holds_for_one_version_and_platform() {
        let secret = SigningKey::from_bytes(&[7; 32]);
        let signature = sign(&secret, digest(b"release"));
        let mut newer = begin("2.0.0", "macos-aarch64");
        newer.update(b"release");
        assert!(!verify(&secret.verifying_key(), newer, &signature));
        let mut elsewhere = begin("1.0.0", "windows-x86_64");
        elsewhere.update(b"release");
        assert!(!verify(&secret.verifying_key(), elsewhere, &signature));
    }
}
