pub use http;
pub use mendes_macros::{dispatch, handler};

#[cfg(feature = "application")]
pub mod application;
#[cfg(feature = "application")]
pub use application::{from_body_bytes, Application, ClientError, Context, FromContext};

#[cfg(feature = "models")]
pub mod models;

#[cfg(feature = "cookies")]
pub mod cookies;

#[cfg(feature = "forms")]
mod form;

#[cfg(feature = "hyper")]
mod hyper;

#[cfg(feature = "uploads")]
mod multipart;

#[cfg(feature = "forms")]
pub mod forms {
    pub use super::form::*;
    #[cfg(feature = "uploads")]
    pub use super::multipart::{from_form_data, File};
}

pub mod types {
    pub const HTML: &str = "text/html";
    pub const JSON: &str = "application/json";
}
