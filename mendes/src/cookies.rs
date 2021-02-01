use std::convert::{TryFrom, TryInto};
use std::fmt::Write;
use std::str;
use std::time::{Duration, SystemTime};

pub use bincode;
use data_encoding::{BASE64URL_NOPAD, HEXLOWER};
use http::header::InvalidHeaderValue;
use http::{HeaderMap, HeaderValue};
pub use mendes_macros::cookie;
use ring::rand::SecureRandom;
use ring::{aead, rand};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "application")]
#[cfg_attr(docsrs, doc(cfg(feature = "application")))]
pub use application::{AppWithAeadKey, AppWithCookies};

#[cfg(feature = "application")]
#[cfg_attr(docsrs, doc(cfg(feature = "application")))]
mod application {
    use super::*;
    use crate::application::Application;
    use http::header::SET_COOKIE;

    /// Cookie manipulation methods, contingent upon the `Application`'s access to an AEAD `Key`
    pub trait AppWithCookies: AppWithAeadKey {
        /// Extract cookie from the given `HeaderMap` using this `Application`'s `Key`
        ///
        /// Finds the first `Cookie` header whose name matches the given type `T` and
        /// whose value can be successfully decoded, decrypted and has not expired.
        fn cookie<T: CookieData>(&self, headers: &HeaderMap) -> Option<T>;

        /// Set cookie value by appending a `Set-Cookie` to the given `HeaderMap`
        ///
        /// If `data` is `Some`, a new value will be set. If the value is `None`, an
        /// empty value is set with an expiry time in the past, causing the cookie
        /// to be deleted in compliant clients.
        fn set_cookie<T: CookieData>(
            &self,
            headers: &mut HeaderMap,
            data: Option<T>,
        ) -> Result<(), Error> {
            headers.append(SET_COOKIE, self.set_cookie_header(data)?);
            Ok(())
        }

        /// Encode and encrypt the cookie's value into a `Set-Cookie` `HeaderValue`
        ///
        /// If `data` is `Some`, a new value will be set. If the value is `None`, an
        /// empty value is set with an expiry time in the past, causing the cookie
        /// to be deleted in compliant clients.
        fn set_cookie_header<T: CookieData>(&self, data: Option<T>) -> Result<HeaderValue, Error>;
    }

    impl<A> AppWithCookies for A
    where
        A: AppWithAeadKey,
    {
        fn cookie<T: CookieData>(&self, headers: &HeaderMap) -> Option<T> {
            extract(self.key(), headers)
        }

        fn set_cookie_header<T: CookieData>(&self, data: Option<T>) -> Result<HeaderValue, Error> {
            match data {
                Some(data) => store(self.key(), data),
                None => tombstone(T::NAME),
            }
        }
    }

    /// Give mendes-based APIs access to an AEAD key for the `Application`
    ///
    /// AEAD (Authenticated Encryption with Associated Data) encrypts data and authenticates
    /// it such that other parties cannot read or manipulate the encrypted data. Currently
    /// mendes uses this only to encrypt and authenticate cookie data.
    pub trait AppWithAeadKey: Application {
        fn key(&self) -> &Key;
    }
}

/// Data to be stored in a cookie
///
/// This is usually derived through the `cookie` procedural attribute macro.
pub trait CookieData: DeserializeOwned + Serialize {
    /// The name to use for the cookie
    const NAME: &'static str;

    /// The expiry time for the cookie
    ///
    /// The `cookie` macro sets this to 6 hours.
    fn expires() -> Option<Duration>;
}

#[derive(Deserialize, Serialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
struct Cookie<T>
where
    T: CookieData,
{
    expires: SystemTime,
    data: T,
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
    pub fn from_hex_lower(s: &[u8]) -> Result<Self, ()> {
        let bytes = HEXLOWER.decode(s).map_err(|_| ())?;
        Ok(Self::new((&*bytes).try_into().map_err(|_| ())?))
    }
}

fn extract<T: CookieData>(key: &Key, headers: &HeaderMap) -> Option<T> {
    let cookies = headers.get("cookie")?;
    let cookies = str::from_utf8(cookies.as_ref()).ok()?;
    let name = T::NAME;
    for cookie in cookies.split(';') {
        let cookie = cookie.trim_start();
        if cookie.len() < (name.len() + 1 + NONCE_LEN + TAG_LEN) {
            continue;
        }
        if !cookie.starts_with(name) {
            continue;
        }
        if cookie.as_bytes()[name.len()] != b'=' {
            continue;
        }

        let encoded = &cookie[name.len() + 1..];
        let mut bytes = match BASE64URL_NOPAD.decode(encoded.as_bytes()).ok() {
            Some(bytes) => bytes,
            None => continue,
        };

        if bytes.len() <= NONCE_LEN {
            continue;
        }

        let ad = aead::Aad::from(name.as_bytes());
        let (nonce, mut sealed) = bytes.split_at_mut(NONCE_LEN);
        let nonce = match aead::Nonce::try_assume_unique_for_key(nonce).ok() {
            Some(nonce) => nonce,
            None => continue,
        };
        let plain = match key.0.open_in_place(nonce, ad, &mut sealed).ok() {
            Some(plain) => plain,
            None => continue,
        };

        let cookie = match bincode::deserialize::<Cookie<T>>(plain).ok() {
            Some(cookie) => cookie,
            None => continue,
        };

        if cookie.expires < SystemTime::now() {
            continue;
        }

        return Some(cookie.data);
    }

    None
}

fn store<T: CookieData>(key: &Key, data: T) -> Result<HeaderValue, Error> {
    let expiration = T::expires().unwrap_or_else(|| Duration::new(NO_EXPIRY, 0));
    let expires = SystemTime::now()
        .checked_add(expiration)
        .ok_or(Error::ExpiryWindowTooLong)?;
    let cookie = Cookie { expires, data };
    let bytes = bincode::serialize(&cookie)?;
    let mut data = vec![0; NONCE_LEN + bytes.len() + TAG_LEN];

    let (nonce, in_out) = data.split_at_mut(NONCE_LEN);
    let (plain, tag) = in_out.split_at_mut(bytes.len());
    plain.copy_from_slice(&bytes);

    rand::SystemRandom::new()
        .fill(nonce)
        .map_err(|_| Error::GetRandomFailed)?;
    let nonce = aead::Nonce::try_assume_unique_for_key(nonce).unwrap(); // valid nonce length

    let ad = aead::Aad::from(T::NAME.as_bytes());
    let ad_tag = key.0.seal_in_place_separate_tag(nonce, ad, plain).unwrap(); // unique nonce
    tag.copy_from_slice(ad_tag.as_ref());

    let mut s = format!("{}={}; Path=/", T::NAME, BASE64URL_NOPAD.encode(&data));
    if let Some(duration) = T::expires() {
        let expires = chrono::Utc::now()
            + chrono::Duration::from_std(duration).map_err(|_| Error::ExpiryWindowTooLong)?;
        write!(
            s,
            "; Expires={}",
            expires.format("%a, %d %b %Y %H:%M:%S GMT")
        )
        .unwrap(); // writing to a string buffer seems safe enough
    }
    Ok(HeaderValue::try_from(s)?)
}

fn tombstone(name: &str) -> Result<HeaderValue, Error> {
    Ok(HeaderValue::try_from(format!(
        "{}=None; Path=/; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
        name
    ))?)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("unstable to serialize cookie data")]
    DataSerializationFailed(#[from] bincode::Error),
    #[error("expiry window too long")]
    ExpiryWindowTooLong,
    #[error("failed to acquire random bytes for nonce")]
    GetRandomFailed,
    #[error("non-ASCII cookie name")]
    InvalidCookieName(#[from] InvalidHeaderValue),
}

const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const NO_EXPIRY: u64 = 60 * 60 * 24 * 4000;
