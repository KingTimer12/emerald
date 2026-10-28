//! Release signatures: ed25519 over the SHA-512 of the whole file
//! (Ed25519ph), so a download is hashed as it arrives instead of being
//! held in memory to be checked.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use sha2::Sha512;

/// Binds signatures to this use, so no other ed25519 signature made with
/// the same key passes for a release.
const CONTEXT: &[u8] = b"emerald-desktop release";

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
    use sha2::Digest;

    use super::*;

    fn digest(data: &[u8]) -> Sha512 {
        let mut hasher = Sha512::new();
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
}
