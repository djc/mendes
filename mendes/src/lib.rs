use std::fmt;

use http::StatusCode;

pub use http;
pub use mendes_macros::{dispatch, handler};

#[cfg(feature = "cookies")]
pub mod cookies;

mod form;
#[cfg(feature = "uploads")]
mod multipart;
mod route;

pub use route::{from_body_bytes, Application, Context, FromContext};

pub mod forms {
    pub use super::form::*;
    #[cfg(feature = "uploads")]
    pub use super::multipart::{from_form_data, File};
}

#[cfg(feature = "hyper")]
pub mod hyper {
    use std::convert::Infallible;
    use std::net::SocketAddr;
    use std::sync::Arc;

    use futures_util::future::FutureExt;
    use http::{header::LOCATION, Response, StatusCode};
    use hyper::service::{make_service_fn, service_fn};
    use hyper::{Body, Server};

    use super::{Application, Context};

    pub async fn run<A>(addr: &SocketAddr, app: A) -> Result<(), hyper::Error>
    where
        A: Application<RequestBody = Body, ResponseBody = Body> + Send + Sync + 'static,
    {
        let app = Arc::new(app);
        Server::bind(addr)
            .serve(make_service_fn(move |_| {
                let app = app.clone();
                async {
                    Ok::<_, Infallible>(service_fn(move |req| {
                        let cx = Context::new(app.clone(), req);
                        A::handle(cx).map(Ok::<_, Infallible>)
                    }))
                }
            }))
            .await
    }

    pub fn redirect(status: StatusCode, path: &str) -> Response<Body> {
        http::Response::builder()
            .status(status)
            .header(LOCATION, path)
            .body(Body::empty())
            .unwrap()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ClientError {
    BadRequest,
    Unauthorized,
    PaymentRequired,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    NotAcceptable,
    ProxyAuthenticationRequired,
    RequestTimeout,
    Conflict,
    Gone,
    LengthRequired,
    PreconditionFailed,
    PayloadTooLarge,
    RequestUriTooLong,
    UnsupportedMediaType,
    RequestedRangeNotSatisfiable,
    ExpectationFailed,
    MisdirectedRequest,
    UnprocessableEntity,
    Locked,
    FailedDependency,
    UpgradeRequired,
    PreconditionRequired,
    TooManyRequests,
    RequestHeaderFieldsTooLarge,
    UnavailableForLegalReasons,
}

impl From<ClientError> for StatusCode {
    fn from(e: ClientError) -> StatusCode {
        use ClientError::*;
        match e {
            BadRequest => StatusCode::BAD_REQUEST,
            Unauthorized => StatusCode::UNAUTHORIZED,
            PaymentRequired => StatusCode::PAYMENT_REQUIRED,
            Forbidden => StatusCode::FORBIDDEN,
            NotFound => StatusCode::NOT_FOUND,
            MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            NotAcceptable => StatusCode::NOT_ACCEPTABLE,
            ProxyAuthenticationRequired => StatusCode::PROXY_AUTHENTICATION_REQUIRED,
            RequestTimeout => StatusCode::REQUEST_TIMEOUT,
            Conflict => StatusCode::CONFLICT,
            Gone => StatusCode::GONE,
            LengthRequired => StatusCode::LENGTH_REQUIRED,
            PreconditionFailed => StatusCode::PRECONDITION_FAILED,
            PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            RequestUriTooLong => StatusCode::URI_TOO_LONG,
            UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            RequestedRangeNotSatisfiable => StatusCode::RANGE_NOT_SATISFIABLE,
            ExpectationFailed => StatusCode::EXPECTATION_FAILED,
            MisdirectedRequest => StatusCode::MISDIRECTED_REQUEST,
            UnprocessableEntity => StatusCode::UNPROCESSABLE_ENTITY,
            Locked => StatusCode::LOCKED,
            FailedDependency => StatusCode::FAILED_DEPENDENCY,
            UpgradeRequired => StatusCode::UPGRADE_REQUIRED,
            PreconditionRequired => StatusCode::PRECONDITION_REQUIRED,
            TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            RequestHeaderFieldsTooLarge => StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            UnavailableForLegalReasons => StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
        }
    }
}

impl std::error::Error for ClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl fmt::Display for ClientError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{}", StatusCode::from(*self))
    }
}

pub mod types {
    pub const HTML: &str = "text/html";
    pub const JSON: &str = "application/json";
}
