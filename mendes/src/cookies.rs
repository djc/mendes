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
                None => cookie::<T>(None),
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

    /// Defines the host to which the cookie will be sent
    fn domain() -> Option<&'static str> {
        None
    }

    /// Forbid JavaScript access to the cookie
    ///
    /// Defaults to `false`.
    fn http_only() -> bool {
        false
    }

    /// The maximum age for the cookie in seconds
    ///
    /// Defaults to 6 hours.
    fn max_age() -> u32 {
        6 * 60 * 60
    }

    /// Set a path prefix to constrain use of the cookie
    ///
    /// The browser default here is to use the current directory (removing the last path
    /// segment from the current URL), which seems pretty useless. Instead, we default to `/` here.
    fn path() -> &'static str {
        "/"
    }

    /// Controls whether the cookie is sent with cross-origin requests
    ///
    /// Defaults to `Some(SameSite::None)`.
    fn same_site() -> Option<SameSite> {
        Some(SameSite::None)
    }

    /// Restrict the cookie to being sent only over secure connections
    ///
    /// Defaults to `true`.
    fn secure() -> bool {
        true
    }

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
    let expires = SystemTime::now()
        .checked_add(Duration::new(T::max_age() as u64, 0))
        .ok_or(Error::ExpiryWindowTooLong)?;

    let mut bytes = bincode::serialize(&Cookie { expires, data })?;
    key.encrypt(T::NAME.as_bytes(), &mut bytes)?;
    let value = BASE64URL_NOPAD.encode(&bytes);
    cookie::<T>(Some(&value))
}

fn cookie<T: CookieData>(value: Option<&str>) -> Result<HeaderValue, Error> {
    let mut s = match value {
        Some(value) => format!(
            "{}={}; Max-Age={}; Path={}",
            T::NAME,
            value,
            T::max_age(),
            T::path(),
        ),
        None => format!(
            "{}=None; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path={}",
            T::NAME,
            T::path(),
        ),
    };

    if let Some(domain) = T::domain() {
        write!(s, "; Domain={}", domain).unwrap();
    }

    if T::http_only() {
        write!(s, "; HttpOnly").unwrap();
    }

    if let Some(same_site) = T::same_site() {
        write!(s, "; SameSite={:?}", same_site).unwrap();
    }

    if T::secure() {
        write!(s, "; Secure").unwrap();
    }

    Ok(HeaderValue::try_from(s)?)
}

#[derive(Debug, Clone, Copy)]
pub enum SameSite {
    Lax,
    None,
    Strict,
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
