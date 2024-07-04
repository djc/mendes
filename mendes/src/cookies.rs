#[cfg(feature = "application")]
use std::convert::TryFrom;
#[cfg(feature = "application")]
use std::fmt::Write;
use std::str;
#[cfg(feature = "application")]
use std::time::Duration;
use std::time::SystemTime;

use data_encoding::BASE64URL_NOPAD;
use http::header::InvalidHeaderValue;
#[cfg(feature = "application")]
use http::header::COOKIE;
#[cfg(feature = "application")]
use http::{HeaderMap, HeaderValue};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "application")]
use crate::key::{NONCE_LEN, TAG_LEN};

pub use crate::key::Key;
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
        fn cookie<T: CookieData + DeserializeOwned>(&self, headers: &HeaderMap) -> Option<T> {
            extract(self.key(), headers)
        }

        /// Set cookie value by appending a `Set-Cookie` to the given `HeaderMap`
        ///
        /// If `data` is `Some`, a new value will be set. If the value is `None`, an
        /// empty value is set with an expiry time in the past, causing the cookie
        /// to be deleted in compliant clients.
        fn set_cookie<T: CookieData + Serialize>(
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
        fn set_cookie_header<T: CookieData + Serialize>(
            &self,
            data: Option<T>,
        ) -> Result<HeaderValue, Error> {
            self.set_cookie_from_parts(T::NAME, data, &T::meta())
        }

        /// Assemble a `Set-Cookie` `HeaderValue` from parts
        fn set_cookie_from_parts(
            &self,
            name: &str,
            value: Option<impl Serialize>,
            meta: &CookieMeta<'_>,
        ) -> Result<HeaderValue, Error> {
            let value = value
                .map(|data| Cookie::encode(name, data, meta, self.key()))
                .transpose()?;
            cookie(name, value.as_deref(), meta)
        }
    }

    impl<A: AppWithAeadKey> AppWithCookies for A {}
}

/// Data to be stored in a cookie
///
/// This is usually derived through the `cookie` procedural attribute macro.
pub trait CookieData {
    fn decode(value: &str, key: &Key) -> Option<Self>
    where
        Self: DeserializeOwned,
    {
        let mut bytes = BASE64URL_NOPAD.decode(value.as_bytes()).ok()?;
        let plain = key.decrypt(Self::NAME.as_bytes(), &mut bytes).ok()?;

        let cookie = postcard::from_bytes::<Cookie<Self>>(plain).ok()?;
        match SystemTime::now() < cookie.expires {
            true => Some(cookie.data),
            false => None,
        }
    }

    fn meta() -> CookieMeta<'static> {
        CookieMeta::default()
    }

    /// The name to use for the cookie
    const NAME: &'static str;
}

pub struct CookieMeta<'a> {
    /// Defines the host to which the cookie will be sent
    pub domain: Option<&'a str>,
    /// Forbid JavaScript access to the cookie
    ///
    /// Defaults to `false`.
    pub http_only: bool,
    /// The maximum age for the cookie in seconds
    ///
    /// Defaults to 6 hours.
    pub max_age: u32,
    /// Set a path prefix to constrain use of the cookie
    ///
    /// The browser default here is to use the current directory (removing the last path
    /// segment from the current URL), which seems pretty useless. Instead, we default to `/` here.
    pub path: &'a str,
    /// Controls whether the cookie is sent with cross-origin requests
    ///
    /// Defaults to `Some(SameSite::None)`.
    pub same_site: Option<SameSite>,
    /// Restrict the cookie to being sent only over secure connections
    ///
    /// Defaults to `true`.
    pub secure: bool,
}

impl Default for CookieMeta<'static> {
    fn default() -> Self {
        Self {
            domain: None,
            http_only: false,
            max_age: 6 * 60 * 60,
            path: "/",
            same_site: Some(SameSite::None),
            secure: true,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
struct Cookie<T> {
    expires: SystemTime,
    data: T,
}

#[cfg(feature = "application")]
impl<T: Serialize> Cookie<T> {
    fn encode(name: &str, data: T, meta: &CookieMeta<'_>, key: &Key) -> Result<String, Error> {
        let expires = SystemTime::now()
            .checked_add(Duration::new(meta.max_age as u64, 0))
            .ok_or(Error::ExpiryWindowTooLong)?;

        let mut bytes = postcard::to_stdvec(&Cookie { expires, data })?;
        key.encrypt(name.as_bytes(), &mut bytes)?;
        Ok(BASE64URL_NOPAD.encode(&bytes))
    }
}

#[cfg(feature = "application")]
fn extract<T: CookieData + DeserializeOwned>(key: &Key, headers: &HeaderMap) -> Option<T> {
    let name = T::NAME;
    // HTTP/2 allows for multiple cookie headers.
    // https://datatracker.ietf.org/doc/html/rfc9113#name-compressing-the-cookie-head
    for value in headers.get_all(COOKIE) {
        let value = match str::from_utf8(value.as_ref()) {
            Ok(value) => value,
            Err(_) => continue,
        };
        // A single cookie header can contain multiple cookies (delimited by ;)
        // even if there are multiple cookie headers.
        for cookie in value.split(';') {
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
    }
    None
}

#[cfg(feature = "application")]
fn cookie(name: &str, value: Option<&str>, meta: &CookieMeta<'_>) -> Result<HeaderValue, Error> {
    let mut s = match value {
        Some(value) => format!(
            "{}={}; Max-Age={}; Path={}",
            name, value, meta.max_age, meta.path,
        ),
        None => format!(
            "{}=None; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path={}",
            name, meta.path,
        ),
    };

    if let Some(domain) = meta.domain {
        write!(s, "; Domain={domain}").unwrap();
    }

    if meta.http_only {
        write!(s, "; HttpOnly").unwrap();
    }

    if let Some(same_site) = meta.same_site {
        write!(s, "; SameSite={same_site:?}").unwrap();
    }

    if meta.secure {
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
    DataSerializationFailed(#[from] postcard::Error),
    #[error("expiry window too long")]
    ExpiryWindowTooLong,
    #[error("non-ASCII cookie name")]
    InvalidCookieName(#[from] InvalidHeaderValue),
    #[error("key error: {0}")]
    Key(#[from] crate::key::Error),
}

#[cfg(test)]
mod test {
    use http::{header, HeaderMap};
    use serde::{Deserialize, Serialize};

    use super::*;

    /// This test checks that we can extract a cookie from a request that uses multiple cookies in a single header
    #[test]
    fn test_multiple_cookies_in_single_header() {
        let key = crate::key::Key::from_hex_lower(
            b"db9881d396644d64818c0bc192d161addb9881d396644d64818c0bc192d161ad",
        )
        .unwrap();
        let session = Session { id: 2 };

        let mut headers = HeaderMap::new();
        let meta = Session::meta();
        let cookie_value = Cookie::encode(Session::NAME, session, &meta, &key).unwrap();
        let header_value = format!("_internal_s=logs=1&id=toast;Session={cookie_value};RefreshToken=tWEnTuXNfmCV_ZNYZQXvMeZ8AN5KUqas7vsqY1wwcWa6TfxYEqekcBVIpagFXn06XsHSN8GZQqGi2w1jd2Atj-aEwNq2wknQjpmxFKIMAnOYFd6gcCoG6Q").parse().unwrap();
        headers.insert(header::COOKIE, header_value);

        assert_eq!(super::extract::<Session>(&key, &headers).unwrap().id, 2);
    }

    /// This test checks that we can extract a cookie from a request that uses separate headers for each cookie
    #[test]
    fn test_separate_cookie_headers() {
        let key = crate::key::Key::from_hex_lower(
            b"db9881d396644d64818c0bc192d161addb9881d396644d64818c0bc192d161ad",
        )
        .unwrap();
        let session = Session { id: 2 };

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "_internal_s=logs=1&id=toast;".parse().unwrap(),
        );

        let meta = Session::meta();
        let cookie_value = Cookie::encode(Session::NAME, session, &meta, &key).unwrap();
        headers.append(
            header::COOKIE,
            format!("Session={cookie_value}").parse().unwrap(),
        );
        headers.append(header::COOKIE, "RefreshToken=tWEnTuXNfmCV_ZNYZQXvMeZ8AN5KUqas7vsqY1wwcWa6TfxYEqekcBVIpagFXn06XsHSN8GZQqGi2w1jd2Atj-aEwNq2wknQjpmxFKIMAnOYFd6gcCoG6Q".parse().unwrap());

        assert_eq!(super::extract::<Session>(&key, &headers).unwrap().id, 2);
    }

    #[derive(Clone, Copy, Debug, Deserialize, Serialize)]
    pub struct Session {
        id: i64,
    }

    impl super::CookieData for Session {
        const NAME: &'static str = "Session";
    }
}
