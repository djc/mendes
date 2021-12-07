use std::convert::TryInto;

use data_encoding::HEXLOWER;
use ring::rand::SecureRandom;
use ring::{aead, rand};
use thiserror::Error;

#[cfg(feature = "application")]
use crate::application::Application;

/// Give mendes-based APIs access to an AEAD key for the `Application`
///
/// AEAD (Authenticated Encryption with Associated Data) encrypts data and authenticates
/// it such that other parties cannot read or manipulate the encrypted data. Currently
/// mendes uses this only to encrypt and authenticate cookie data.
#[cfg(feature = "application")]
#[cfg_attr(docsrs, doc(cfg(feature = "application")))]
pub trait AppWithAeadKey: Application {
    fn key(&self) -> &Key;
}

/// An encryption key to authenticate and encrypt/decrypt cookie values
///
/// This currently uses the ChaCha20-Poly1305 algorithm as defined in RFC 7539.
pub struct Key(aead::LessSafeKey);

impl Key {
    /// Create a new `Key` from the given secret key
    pub fn new(secret: &[u8; 32]) -> Self {
        Self(aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::CHACHA20_POLY1305, secret).unwrap(),
        ))
    }

    /// Create key from slice of hexadecimal characters
    ///
    /// This will fail if the length of the slice is not equal to 32.
    #[cfg(feature = "application")]
    pub fn from_hex_lower(s: &[u8]) -> Result<Self, Error> {
        let bytes = HEXLOWER
            .decode(s)
            .map_err(|_| Error::InvalidKeyCharacters)?;
        Ok(Self::new(
            (&*bytes).try_into().map_err(|_| Error::InvalidKeyLength)?,
        ))
    }

    pub fn decrypt<'a>(&self, aad: &[u8], input: &'a mut [u8]) -> Result<&'a [u8], Error> {
        if input.len() <= NONCE_LEN {
            return Err(Error::Decryption);
        }

        let ad = aead::Aad::from(aad);
        let (sealed, nonce) = input.split_at_mut(input.len() - NONCE_LEN);
        aead::Nonce::try_assume_unique_for_key(nonce)
            .and_then(move |nonce| self.0.open_in_place(nonce, ad, sealed))
            .map(|plain| &*plain)
            .map_err(|_| Error::Decryption)
    }

    pub fn encrypt(&self, aad: &[u8], buf: &mut Vec<u8>) -> Result<(), Error> {
        let mut nonce_buf = [0; NONCE_LEN];
        rand::SystemRandom::new()
            .fill(&mut nonce_buf)
            .map_err(|_| Error::GetRandomFailed)?;
        let nonce = aead::Nonce::try_assume_unique_for_key(&nonce_buf).unwrap(); // valid nonce length

        let aad = aead::Aad::from(aad);
        self.0.seal_in_place_append_tag(nonce, aad, buf).unwrap(); // unique nonce
        buf.extend(&nonce_buf);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to decrypt")]
    Decryption,
    #[error("failed to acquire random bytes for nonce")]
    GetRandomFailed,
    #[error("invalid key characters")]
    InvalidKeyCharacters,
    #[error("invalid key length")]
    InvalidKeyLength,
}

pub(crate) const NONCE_LEN: usize = 12;
pub(crate) const TAG_LEN: usize = 16;
