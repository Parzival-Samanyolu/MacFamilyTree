//! Password-based encryption for exported project files: Argon2id key derivation + ChaCha20-Poly1305.
//!
//! Format: `KTENC1` | salt (16) | nonce (12) | ciphertext+tag. The header is authenticated as associated data.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::ChaCha20Poly1305;

const MAGIC: &[u8; 6] = b"KTENC1";
const SALT: usize = 16;
const NONCE: usize = 12;

#[derive(Debug, PartialEq, Eq)]
pub enum CryptoError {
    NotEncrypted,
    WrongPasswordOrCorrupt,
    EmptyPassword,
    Random,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CryptoError::NotEncrypted => "this file is not a KinTree encrypted backup",
            CryptoError::WrongPasswordOrCorrupt => "wrong password, or the file is damaged",
            CryptoError::EmptyPassword => "a password is required",
            CryptoError::Random => "no secure random source available",
        })
    }
}
impl std::error::Error for CryptoError {}

fn derive(password: &str, salt: &[u8], fast: bool) -> [u8; 32] {
    // 19 MiB / 2 passes is the OWASP minimum for Argon2id; tests use a cheap setting.
    let params = if fast {
        Params::new(8, 1, 1, Some(32))
    } else {
        Params::new(19 * 1024, 2, 1, Some(32))
    }
    .expect("static argon2 parameters");
    let mut out = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .expect("argon2 with valid parameters");
    out
}

fn encrypt_with(data: &[u8], password: &str, fast: bool) -> Result<Vec<u8>, CryptoError> {
    if password.is_empty() {
        return Err(CryptoError::EmptyPassword);
    }
    let mut salt = [0u8; SALT];
    let mut nonce = [0u8; NONCE];
    getrandom::fill(&mut salt).map_err(|_| CryptoError::Random)?;
    getrandom::fill(&mut nonce).map_err(|_| CryptoError::Random)?;
    let mut head = Vec::with_capacity(MAGIC.len() + SALT + NONCE + 1);
    head.extend_from_slice(MAGIC);
    head.push(u8::from(fast));
    head.extend_from_slice(&salt);
    head.extend_from_slice(&nonce);
    let key = derive(password, &salt, fast);
    let cipher =
        ChaCha20Poly1305::new_from_slice(&key).map_err(|_| CryptoError::WrongPasswordOrCorrupt)?;
    let ct = cipher
        .encrypt(
            nonce
                .as_slice()
                .try_into()
                .map_err(|_| CryptoError::Random)?,
            Payload {
                msg: data,
                aad: &head,
            },
        )
        .map_err(|_| CryptoError::WrongPasswordOrCorrupt)?;
    head.extend_from_slice(&ct);
    Ok(head)
}

pub fn encrypt(data: &[u8], password: &str) -> Result<Vec<u8>, CryptoError> {
    encrypt_with(data, password, false)
}

pub fn is_encrypted(data: &[u8]) -> bool {
    data.starts_with(MAGIC)
}

pub fn decrypt(blob: &[u8], password: &str) -> Result<Vec<u8>, CryptoError> {
    let hlen = MAGIC.len() + 1 + SALT + NONCE;
    if !is_encrypted(blob) || blob.len() < hlen + 16 {
        return Err(CryptoError::NotEncrypted);
    }
    if password.is_empty() {
        return Err(CryptoError::EmptyPassword);
    }
    let fast = blob[MAGIC.len()] == 1;
    let salt = &blob[MAGIC.len() + 1..MAGIC.len() + 1 + SALT];
    let nonce = &blob[MAGIC.len() + 1 + SALT..hlen];
    let key = derive(password, salt, fast);
    let cipher =
        ChaCha20Poly1305::new_from_slice(&key).map_err(|_| CryptoError::WrongPasswordOrCorrupt)?;
    cipher
        .decrypt(
            nonce
                .try_into()
                .map_err(|_| CryptoError::WrongPasswordOrCorrupt)?,
            Payload {
                msg: &blob[hlen..],
                aad: &blob[..hlen],
            },
        )
        .map_err(|_| CryptoError::WrongPasswordOrCorrupt)
}

#[doc(hidden)]
pub fn encrypt_fast_for_tests(data: &[u8], password: &str) -> Result<Vec<u8>, CryptoError> {
    encrypt_with(data, password, true)
}
