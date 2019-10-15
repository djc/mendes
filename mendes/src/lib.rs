use http::Response;
pub use mendes_derive::dispatch;

pub trait Application {
    type ResponseBody;
    type Error;

    fn error(&self, error: Self::Error) -> Response<Self::ResponseBody>;
}
