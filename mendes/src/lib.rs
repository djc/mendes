#[cfg(feature = "http")]
pub use http;

#[cfg(feature = "application")]
pub mod application;
#[cfg(feature = "application")]
pub use application::{dispatch, handler, Application, ClientError, Context, FromContext};

#[cfg(feature = "cookies")]
pub mod cookies;

#[cfg(feature = "forms")]
pub mod forms;

#[cfg(feature = "hyper")]
pub mod hyper;

#[cfg(feature = "models")]
pub mod models;

#[cfg(feature = "uploads")]
mod multipart;

pub mod types {
    pub const HTML: &str = "text/html";
    pub const JSON: &str = "application/json";
}
