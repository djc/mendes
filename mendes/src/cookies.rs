use std::convert::{TryFrom, TryInto};
use std::fmt::Write;
use std::str;
use std::time::{Duration, SystemTime};

pub use bincode;
use data_encoding::{BASE64URL_NOPAD, HEXLOWER};
use http::{HeaderMap, HeaderValue};
pub use mendes_macros::cookie;
use ring::rand::SecureRandom;
use ring::{aead, rand};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[cfg(feature = "application")]
pub use application::{AppWithAeadKey, AppWithCookies};

#[cfg(feature = "application")]
mod application {
    use super::*;
    use crate::application::Application;
    use http::header::SET_COOKIE;

    pub trait AppWithCookies: AppWithAeadKey {
        fn cookie<T: CookieData>(&self, headers: &HeaderMap) -> Option<T>;

        fn set_cookie<T: CookieData>(
            &self,
            headers: &mut HeaderMap,
            data: Option<T>,
        ) -> Result<(), ()> {
            headers.append(SET_COOKIE, self.set_cookie_header(data)?);
            Ok(())
        }

        fn set_cookie_header<T: CookieData>(&self, data: Option<T>) -> Result<HeaderValue, ()>;
    }

    impl<A> AppWithCookies for A
    where
        A: AppWithAeadKey,
    {
        fn cookie<T: CookieData>(&self, headers: &HeaderMap) -> Option<T> {
            extract(self.key(), headers)
        }

        fn set_cookie_header<T: CookieData>(&self, data: Option<T>) -> Result<HeaderValue, ()> {
            match data {
                Some(data) => store(self.key(), data),
                None => tombstone(T::NAME),
            }
        }
    }

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

fn store<T: CookieData>(key: &Key, data: T) -> Result<HeaderValue, ()> {
    let expiration = T::expires().unwrap_or_else(|| Duration::new(NO_EXPIRY, 0));
    let expires = SystemTime::now().checked_add(expiration).ok_or(())?;
    let cookie = Cookie { expires, data };
    let bytes = bincode::serialize(&cookie).map_err(|_| ())?;
    let mut data = vec![0; NONCE_LEN + bytes.len() + TAG_LEN];

    let (nonce, in_out) = data.split_at_mut(NONCE_LEN);
    let (plain, tag) = in_out.split_at_mut(bytes.len());
    plain.copy_from_slice(&bytes);

    rand::SystemRandom::new().fill(nonce).map_err(|_| ())?;
    let nonce = aead::Nonce::try_assume_unique_for_key(nonce).map_err(|_| ())?;

    let ad = aead::Aad::from(T::NAME.as_bytes());
    let ad_tag = key
        .0
        .seal_in_place_separate_tag(nonce, ad, plain)
        .map_err(|_| ())?;
    tag.copy_from_slice(ad_tag.as_ref());

    let mut s = format!("{}={}; Path=/", T::NAME, BASE64URL_NOPAD.encode(&data));
    if let Some(duration) = T::expires() {
        let expires = chrono::Utc::now() + chrono::Duration::from_std(duration).map_err(|_| ())?;
        write!(
            s,
            "; Expires={}",
            expires.format("%a, %d %b %Y %H:%M:%S GMT")
        )
        .map_err(|_| ())?;
    }
    HeaderValue::try_from(s).map_err(|_| ())
}

fn tombstone(name: &str) -> Result<HeaderValue, ()> {
    HeaderValue::try_from(format!(
        "{}=None; Path=/; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
        name
    ))
    .map_err(|_| ())
}

const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const NO_EXPIRY: u64 = 60 * 60 * 24 * 4000;
