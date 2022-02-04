#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "http")]
#[cfg_attr(docsrs, doc(cfg(feature = "http")))]
/// Re-export of the http crate
pub use http;

#[cfg(feature = "application")]
#[cfg_attr(docsrs, doc(cfg(feature = "application")))]
/// Core of the Mendes web application toolkit
pub mod application;
#[cfg(feature = "application")]
pub use application::{handler, route, scope, Application, Context, Error, FromContext};

#[cfg(feature = "cookies")]
#[cfg_attr(docsrs, doc(cfg(feature = "cookies")))]
/// Cookie support
pub mod cookies;

#[cfg(feature = "key")]
#[cfg_attr(docsrs, doc(cfg(feature = "key")))]
/// AEAD encryption/decryption support
pub mod key;

#[cfg(feature = "forms")]
#[cfg_attr(docsrs, doc(cfg(feature = "forms")))]
/// Form generation and data validation
pub mod forms;

/// Some helperrs
pub mod utils;

#[cfg(feature = "hyper")]
#[cfg_attr(docsrs, doc(cfg(feature = "hyper")))]
/// Optional features that require hyper
pub mod hyper;

#[doc(hidden)]
#[cfg(feature = "models")]
#[cfg_attr(docsrs, doc(cfg(feature = "models")))]
/// A nascent attempt at a Rusty ORM
pub mod models;

#[cfg(feature = "uploads")]
mod multipart;

/// Some content type definitions
pub mod types {
    pub const HTML: &str = "text/html";
    pub const JSON: &str = "application/json";
}
