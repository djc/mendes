use std::fmt;
use std::net::SocketAddr;
use std::str;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use http::header::LOCATION;
use http::request::Parts;
use http::Request;
use http::{Response, StatusCode};
use serde::Deserialize;

pub use mendes_macros::{dispatch, handler};

#[async_trait]
pub trait Application: Sized {
    type RequestBody;
    type ResponseBody;
    type Error: From<ClientError>;

    async fn handle(cx: Context<Self>) -> Response<Self::ResponseBody>;

    fn error(&self, error: Self::Error) -> Response<Self::ResponseBody>;

    fn redirect(status: StatusCode, path: &str) -> Response<Self::ResponseBody>
    where
        Self::ResponseBody: Default,
    {
        http::Response::builder()
            .status(status)
            .header(LOCATION, path)
            .body(Self::ResponseBody::default())
            .unwrap()
    }
}

pub trait Responder<A: Application> {
    fn into_response(self, app: &A) -> Response<A::ResponseBody>;
}

impl<A: Application> Responder<A> for Result<Response<A::ResponseBody>, A::Error> {
    fn into_response(self, app: &A) -> Response<A::ResponseBody> {
        match self {
            Ok(rsp) => rsp,
            Err(e) => app.error(e),
        }
    }
}

impl<A: Application> Responder<A> for Response<A::ResponseBody> {
    fn into_response(self, _: &A) -> Response<A::ResponseBody> {
        self
    }
}

pub struct Context<A>
where
    A: Application,
{
    pub app: Arc<A>,
    pub req: http::request::Parts,
    pub body: Option<A::RequestBody>,
    pub path: PathState,
}

impl<A> Context<A>
where
    A: Application,
{
    pub fn new(app: Arc<A>, req: Request<A::RequestBody>) -> Context<A> {
        let path = PathState::new(req.uri().path());
        let (req, body) = req.into_parts();
        Context {
            app,
            req,
            body: Some(body),
            path,
        }
    }

    #[doc(hidden)]
    pub fn next_path(&mut self) -> Option<&str> {
        self.path.next(&self.req.uri.path())
    }

    #[doc(hidden)]
    pub fn rest(&mut self) -> &str {
        self.path.rest(&self.req.uri.path())
    }

    #[doc(hidden)]
    pub fn rewind(mut self) -> Self {
        self.path.rewind();
        self
    }

    pub fn take_body(&mut self) -> Option<A::RequestBody> {
        self.body.take()
    }

    pub fn app(&self) -> &Arc<A> {
        &self.app
    }

    pub fn method(&self) -> &http::Method {
        &self.req.method
    }

    pub fn uri(&self) -> &http::uri::Uri {
        &self.req.uri
    }

    pub fn headers(&self) -> &http::HeaderMap {
        &self.req.headers
    }
}

pub trait FromContext<'a>: Sized {
    fn from_context<A: Application>(
        req: &'a Parts,
        state: &mut PathState,
    ) -> Result<Self, A::Error>;
}

impl<'a> FromContext<'a> for &'a http::request::Parts {
    fn from_context<A: Application>(req: &'a Parts, _: &mut PathState) -> Result<Self, A::Error> {
        Ok(req)
    }
}

impl<'a> FromContext<'a> for Option<&'a str> {
    fn from_context<A: Application>(
        req: &'a Parts,
        state: &mut PathState,
    ) -> Result<Self, A::Error> {
        Ok(state.next(&req.uri.path()))
    }
}

impl<'a> FromContext<'a> for &'a str {
    fn from_context<A: Application>(
        req: &'a Parts,
        state: &mut PathState,
    ) -> Result<Self, A::Error> {
        state
            .next(&req.uri.path())
            .ok_or_else(|| ClientError::NotFound.into())
    }
}

impl<'a> FromContext<'a> for usize {
    fn from_context<A: Application>(
        req: &'a Parts,
        state: &mut PathState,
    ) -> Result<Self, A::Error> {
        let s = state.next(&req.uri.path()).ok_or(ClientError::NotFound)?;
        usize::from_str(s).map_err(|_| ClientError::NotFound.into())
    }
}

impl<'a> FromContext<'a> for i32 {
    fn from_context<A: Application>(
        req: &'a Parts,
        state: &mut PathState,
    ) -> Result<Self, A::Error> {
        let s = state.next(&req.uri.path()).ok_or(ClientError::NotFound)?;
        i32::from_str(s).map_err(|_| ClientError::NotFound.into())
    }
}

#[doc(hidden)]
#[cfg(feature = "hyper")]
impl<A> Context<A>
where
    A: Application,
    A::RequestBody: hyper::body::HttpBody,
{
    pub async fn retrieve_body(&mut self) -> Result<(), ClientError> {
        let future = self.take_body().ok_or(ClientError::BadRequest)?;
        let bytes = hyper::body::to_bytes(future)
            .await
            .map_err(|_| ClientError::BadRequest)?;
        self.req.extensions.insert(BodyBytes(bytes));
        Ok(())
    }

    pub fn from_body<'a, T>(&'a self) -> Result<T, ClientError>
    where
        T: 'a + Deserialize<'a>,
    {
        let bytes = self.req.extensions.get::<BodyBytes>().unwrap();
        Ok(from_body_bytes(&self.req.headers, &bytes.0)?)
    }
}

#[cfg(feature = "hyper")]
struct BodyBytes(bytes::Bytes);

#[doc(hidden)]
pub struct PathState {
    prev: Option<usize>,
    next: Option<usize>,
}

impl PathState {
    fn new(path: &str) -> Self {
        let next = if path.is_empty() || path == "/" {
            None
        } else if path.find('/') == Some(0) {
            Some(1)
        } else {
            Some(0)
        };
        Self { prev: None, next }
    }

    pub fn next<'r>(&mut self, path: &'r str) -> Option<&'r str> {
        let start = match self.next.as_ref() {
            Some(v) => *v,
            None => return None,
        };

        let path = &path[start..];
        if path.is_empty() {
            self.prev = self.next.take();
            return None;
        }

        match path.find('/') {
            Some(end) => {
                self.prev = self.next.replace(start + end + 1);
                Some(&path[..end])
            }
            None => {
                self.prev = self.next.take();
                Some(path)
            }
        }
    }

    pub fn rest<'r>(&mut self, path: &'r str) -> &'r str {
        let start = match self.next.take() {
            Some(v) => v,
            None => return "",
        };

        self.prev = Some(start);
        &path[start..]
    }

    pub fn rewind(&mut self) {
        self.next = self.prev.take();
    }
}

#[allow(unused_variables, unreachable_code, dead_code)]
pub fn from_body_bytes<'de, T: 'de + Deserialize<'de>>(
    headers: &http::HeaderMap,
    bytes: &'de [u8],
) -> Result<T, ClientError> {
    let inner = match headers.get("content-type") {
        #[cfg(feature = "serde_urlencoded")]
        Some(t) if t == "application/x-www-form-urlencoded" => {
            serde_urlencoded::from_bytes::<T>(bytes)
                .map_err(|_| ClientError::UnprocessableEntity)?
        }
        #[cfg(feature = "serde_json")]
        Some(t) if t == "application/json" => {
            serde_json::from_slice::<T>(bytes).map_err(|_| ClientError::UnprocessableEntity)?
        }
        #[cfg(feature = "uploads")]
        Some(t) if t.as_bytes().starts_with(b"multipart/form-data") => {
            crate::forms::from_form_data::<T>(headers, bytes)
                .map_err(|_| ClientError::UnprocessableEntity)?
        }
        None => return Err(ClientError::BadRequest),
        _ => return Err(ClientError::UnsupportedMediaType),
    };

    Ok(inner)
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

#[async_trait]
pub trait Server: Application {
    type ServerError;

    async fn serve(self, addr: &SocketAddr) -> Result<(), Self::ServerError>;
}
