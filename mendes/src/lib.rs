use http::{Response, StatusCode};
use hyper::header::LOCATION;
use hyper::Body;
pub use mendes_derive::dispatch;

pub trait Application {
    type ResponseBody;
    type Error;

    fn error(&self, error: Self::Error) -> Response<Self::ResponseBody>;
}

pub fn redirect(status: StatusCode, path: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(LOCATION, path)
        .body(Body::empty())
        .unwrap()
}

pub mod types {
    pub const HTML: &str = "text/html";
    pub const JSON: &str = "application/json";
}
