use std::convert::TryFrom;
use std::str;
use std::time::{Duration, SystemTime};

pub use bincode;
use data_encoding::BASE64URL_NOPAD;
use http::{HeaderValue, Request};
pub use mendes_macros::cookie;
use ring::rand::SecureRandom;
use ring::{aead, rand};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub fn extract<B, T: CookieData>(name: &str, key: &Key, req: &Request<B>) -> Option<T> {
    let cookies = req.headers().get("cookie")?;
    let cookies = str::from_utf8(cookies.as_ref()).ok()?;

    let mut found = None;
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

        found = Some(&cookie[name.len() + 1..]);
    }

    let encoded = found?;
    let mut bytes = BASE64URL_NOPAD.decode(encoded.as_bytes()).ok()?;
    if bytes.len() <= NONCE_LEN {
        return None;
    }

    let ad = aead::Aad::from(name.as_bytes());
    let (nonce, mut sealed) = bytes.split_at_mut(NONCE_LEN);
    let nonce = aead::Nonce::try_assume_unique_for_key(nonce).ok()?;
    let plain = key.0.open_in_place(nonce, ad, &mut sealed).ok()?;

    let cookie = bincode::deserialize::<Cookie<T>>(plain).ok()?;
    if cookie.expires < SystemTime::now() {
        return None;
    }

    Some(cookie.data)
}

pub fn store<T: CookieData>(name: &str, key: &Key, data: T) -> Result<HeaderValue, ()> {
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

    let ad = aead::Aad::from(name.as_bytes());
    let ad_tag = key
        .0
        .seal_in_place_separate_tag(nonce, ad, plain)
        .map_err(|_| ())?;
    tag.copy_from_slice(ad_tag.as_ref());

    let s = format!("{}={}", name, BASE64URL_NOPAD.encode(&data));
    HeaderValue::try_from(s).map_err(|_| ())
}

pub fn tombstone(name: &str) -> Result<HeaderValue, ()> {
    HeaderValue::try_from(format!("{}=", name)).map_err(|_| ())
}

pub trait CookieData: DeserializeOwned + Serialize {
    fn expires() -> Option<Duration>;
    fn from_header<B>(key: &Key, req: &Request<B>) -> Option<Self>;
    fn to_string(self, key: &Key) -> Result<HeaderValue, ()>;
    fn tombstone() -> Result<http::HeaderValue, ()>;
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

pub struct Key(aead::LessSafeKey);

impl Key {
    pub fn new(secret: &[u8; 32]) -> Result<Self, ()> {
        Ok(Self(aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::CHACHA20_POLY1305, secret).map_err(|_| ())?,
        )))
    }
}

const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const NO_EXPIRY: u64 = 60 * 60 * 24 * 4000;
