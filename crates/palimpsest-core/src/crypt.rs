//! Authenticated encryption for the typing diary at rest.
//!
//! The diary is the one file Palimpsest writes that a user may reasonably want
//! protected from anyone else who can read their profile. When a diary password
//! is set the file is sealed here: the password is stretched with Argon2id into a
//! key, and the plaintext is encrypted with XChaCha20-Poly1305 — an AEAD, so a
//! wrong key or a tampered file is *detected*, never silently decrypted into
//! garbage.
//!
//! There is deliberately no key stored anywhere but in the user's head. The
//! password is the only path back to the plaintext, which is why a forgotten one
//! is unrecoverable — the same bargain Punto Switcher's diary makes, made
//! honestly. It also means the engine can append only while the diary is
//! *unlocked* (the derived key held in memory for the session); a locked diary
//! records nothing, because nothing can be encrypted without the key.

use argon2::Argon2;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use getrandom::getrandom;
use serde::{Deserialize, Serialize};

const SALT_LEN: usize = 16;
/// XChaCha20 takes a 24-byte nonce, large enough to pick at random per write
/// without the birthday-bound worry a 12-byte nonce would carry.
const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;

/// A sealed diary: everything needed to decrypt it *given the password*, and
/// nothing that helps without it. This is the whole on-disk file when a password
/// is set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sealed {
    /// Format version, so a future layout can be refused or migrated rather than
    /// misread.
    pub v: u8,
    /// Argon2 salt (base64). Public by design — a salt is not a secret.
    pub salt: String,
    /// XChaCha20 nonce (base64), fresh on every write.
    pub nonce: String,
    /// Ciphertext with its authentication tag (base64).
    pub ct: String,
}

/// Why opening a sealed diary failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenError {
    /// The password did not decrypt it. A wrong password and a tampered file are
    /// indistinguishable here by design — that is what "authenticated" buys.
    WrongPassword,
    /// The envelope itself is malformed (bad base64, unknown version): a damaged
    /// file, not merely a wrong password.
    Corrupt,
}

/// A password stretched into a key, kept with its salt so repeated saves don't
/// pay Argon2 again. Lives in memory only while the diary is unlocked.
#[derive(Clone)]
pub struct Unlocked {
    key: [u8; KEY_LEN],
    salt: [u8; SALT_LEN],
}

fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    // OS randomness. A failure here is unrecoverable and must never be papered
    // over with predictable bytes, so it panics rather than weakening the key.
    getrandom(&mut b).expect("OS RNG");
    b
}

fn derive(password: &str, salt: &[u8; SALT_LEN]) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .expect("argon2 with valid params");
    key
}

impl Unlocked {
    /// Derive the key for a brand-new password, with a fresh random salt. Used
    /// when the user first sets (or changes) the diary password.
    pub fn new(password: &str) -> Self {
        let salt = random::<SALT_LEN>();
        Self { key: derive(password, &salt), salt }
    }

    /// Encrypt `plaintext` under this key, with a fresh nonce each call.
    pub fn seal(&self, plaintext: &[u8]) -> Sealed {
        let nonce = random::<NONCE_LEN>();
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&self.key));
        let ct = cipher
            .encrypt(XNonce::from_slice(&nonce), plaintext)
            .expect("XChaCha20Poly1305 encrypt");
        Sealed {
            v: 1,
            salt: STANDARD.encode(self.salt),
            nonce: STANDARD.encode(nonce),
            ct: STANDARD.encode(ct),
        }
    }
}

/// Try to open `sealed` with `password`. On success returns the plaintext and
/// the [`Unlocked`] key to reuse for subsequent saves — reusing it keeps the same
/// salt, so re-derivation happens once per unlock rather than once per save.
pub fn open(sealed: &Sealed, password: &str) -> Result<(Vec<u8>, Unlocked), OpenError> {
    if sealed.v != 1 {
        return Err(OpenError::Corrupt);
    }
    let salt_vec = STANDARD.decode(&sealed.salt).map_err(|_| OpenError::Corrupt)?;
    let nonce_vec = STANDARD.decode(&sealed.nonce).map_err(|_| OpenError::Corrupt)?;
    let ct = STANDARD.decode(&sealed.ct).map_err(|_| OpenError::Corrupt)?;
    let salt: [u8; SALT_LEN] = salt_vec.try_into().map_err(|_| OpenError::Corrupt)?;
    if nonce_vec.len() != NONCE_LEN {
        return Err(OpenError::Corrupt);
    }
    let key = derive(password, &salt);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let pt = cipher
        .decrypt(XNonce::from_slice(&nonce_vec), ct.as_ref())
        .map_err(|_| OpenError::WrongPassword)?;
    Ok((pt, Unlocked { key, salt }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_with_the_right_password() {
        let u = Unlocked::new("correct horse battery staple");
        let sealed = u.seal(b"hello there this is the diary");
        let (pt, _) = open(&sealed, "correct horse battery staple").unwrap();
        assert_eq!(pt, b"hello there this is the diary");
    }

    #[test]
    fn a_wrong_password_is_rejected_not_garbled() {
        // `.err()` rather than `.unwrap_err()`: the Ok payload holds the key and
        // is deliberately not Debug, so it must not be printed on assert.
        let sealed = Unlocked::new("right").seal(b"secret");
        assert_eq!(open(&sealed, "wrong").err(), Some(OpenError::WrongPassword));
    }

    #[test]
    fn reused_key_keeps_the_salt_so_the_password_still_opens_it() {
        // The save path reuses the Unlocked returned by `open`; a fresh nonce
        // each time, same salt, and the original password must still work.
        let u = Unlocked::new("pw");
        let first = u.seal(b"one");
        let (_, key) = open(&first, "pw").unwrap();
        let second = key.seal(b"two");
        assert_ne!(first.nonce, second.nonce); // fresh nonce per write
        assert_eq!(open(&second, "pw").unwrap().0, b"two");
    }

    #[test]
    fn tampering_is_caught() {
        let mut sealed = Unlocked::new("pw").seal(b"important");
        // Flip a byte of the ciphertext; the auth tag must reject it.
        let mut raw = STANDARD.decode(&sealed.ct).unwrap();
        raw[0] ^= 0xFF;
        sealed.ct = STANDARD.encode(raw);
        assert_eq!(open(&sealed, "pw").err(), Some(OpenError::WrongPassword));
    }

    #[test]
    fn a_corrupt_envelope_is_distinguished_from_a_wrong_password() {
        let sealed = Sealed { v: 1, salt: "!!!".into(), nonce: "!!!".into(), ct: "!!!".into() };
        assert_eq!(open(&sealed, "pw").err(), Some(OpenError::Corrupt));
    }
}
