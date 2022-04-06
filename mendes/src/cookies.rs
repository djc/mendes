use std::convert::TryFrom;
use std::fmt::Write;
use std::str;
use std::time::{Duration, SystemTime};

use data_encoding::BASE64URL_NOPAD;
use http::header::InvalidHeaderValue;
use http::{HeaderMap, HeaderValue};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

use crate::key::{NONCE_LEN, TAG_LEN};

pub use crate::key::Key;
pub use bincode;
pub use mendes_macros::cookie;

#[cfg(feature = "application")]
#[cfg_attr(docsrs, doc(cfg(feature = "application")))]
pub use application::{AppWithAeadKey, AppWithCookies};

#[cfg(feature = "application")]
#[cfg_attr(docsrs, doc(cfg(feature = "application")))]
mod application {
    use super::*;
    use http::header::SET_COOKIE;

    pub use crate::key::AppWithAeadKey;

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

    fn decode(value: &str, key: &Key) -> Option<Self> {
        let mut bytes = BASE64URL_NOPAD.decode(value.as_bytes()).ok()?;
        let plain = key.decrypt(Self::NAME.as_bytes(), &mut bytes).ok()?;

        let cookie = bincode::deserialize::<Cookie<Self>>(plain).ok()?;
        match SystemTime::now() < cookie.expires {
            true => Some(cookie.data),
            false => None,
        }
    }
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

fn extract<T: CookieData>(key: &Key, headers: &HeaderMap) -> Option<T> {
    let cookies = headers.get("cookie")?;
    let cookies = str::from_utf8(cookies.as_ref()).ok()?;
    let name = T::NAME;
    for cookie in cookies.split(';') {
        let cookie = cookie.trim_start();
        if cookie.len() < (name.len() + 1 + NONCE_LEN + TAG_LEN)
            || !cookie.starts_with(name)
            || cookie.as_bytes()[name.len()] != b'='
        {
            continue;
        }

        let encoded = &cookie[name.len() + 1..];
        match T::decode(encoded, key) {
            Some(data) => return Some(data),
            None => continue,
        }
    }

    None
}

fn store<T: CookieData>(key: &Key, data: T) -> Result<HeaderValue, Error> {
    let expiration = T::expires().unwrap_or_else(|| Duration::new(NO_EXPIRY, 0));
    let expires = SystemTime::now()
        .checked_add(expiration)
        .ok_or(Error::ExpiryWindowTooLong)?;
    let cookie = Cookie { expires, data };

    let mut bytes = bincode::serialize(&cookie)?;
    key.encrypt(T::NAME.as_bytes(), &mut bytes)?;

    let mut s = format!("{}={}; Path=/", T::NAME, BASE64URL_NOPAD.encode(&bytes));
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
    #[error("non-ASCII cookie name")]
    InvalidCookieName(#[from] InvalidHeaderValue),
    #[error("key error: {0}")]
    Key(#[from] crate::key::Error),
}

const NO_EXPIRY: u64 = 60 * 60 * 24 * 4000;
