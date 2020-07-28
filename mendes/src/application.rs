use std::error::Error as StdError;
use std::fmt;
use std::net::SocketAddr;
use std::str;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
#[cfg(feature = "with-http-body")]
use bytes::{Buf, BufMut, Bytes};
use http::header::LOCATION;
use http::request::Parts;
use http::Request;
use http::{Response, StatusCode};
#[cfg(feature = "with-http-body")]
use http_body::Body as HttpBody;

pub use mendes_macros::{dispatch, get, handler, post};

/// Main interface for an application or service
///
/// The `Application` holds state and routes request to the proper handlers. A handler gets
/// an immutable reference to the `Application` to access application state. Common usage
/// for this would be to hold a persistent storage connection pool.
///
/// The `Application` also helps process handler reponses. It can handle errors and turn them
/// into HTTP responses, using the `error()` method to transform the `Error` associated type
/// into a `Response`.
#[async_trait]
pub trait Application: Sized {
    type RequestBody;
    type ResponseBody;
    type Error: From<ClientError> + Responder<Self>;

    async fn handle(cx: Context<Self>) -> Response<Self::ResponseBody>;

    fn from_body_bytes<'de, T: serde::de::Deserialize<'de>>(
        req: &Parts,
        bytes: &'de [u8],
    ) -> Result<T, FromBodyError> {
        from_bytes::<T>(req, bytes)
    }

    #[cfg(feature = "with-http-body")]
    async fn from_body<T: serde::de::DeserializeOwned>(
        req: &Parts,
        body: Self::RequestBody,
    ) -> Result<T, FromBodyError>
    where
        Self::RequestBody: HttpBody + Send,
        <Self::RequestBody as HttpBody>::Data: Send,
        <Self::RequestBody as HttpBody>::Error: Into<Box<dyn StdError + Sync + Send>>,
    {
        from_body::<Self::RequestBody, T>(req, body).await
    }

    #[cfg(feature = "with-http-body")]
    async fn body_bytes<B>(body: B) -> Result<Bytes, B::Error>
    where
        B: HttpBody + Send,
        <B as HttpBody>::Data: Send,
    {
        Ok(to_bytes(body).await?)
    }

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

impl<A: Application> Responder<A> for Response<A::ResponseBody> {
    fn into_response(self, _: &A) -> Response<A::ResponseBody> {
        self
    }
}

impl<A: Application> Responder<A> for Result<Response<A::ResponseBody>, A::Error> {
    fn into_response(self, app: &A) -> Response<A::ResponseBody> {
        match self {
            Ok(rsp) => rsp,
            Err(e) => e.into_response(app),
        }
    }
}

impl<A: Application> Responder<A> for ClientError {
    fn into_response(self, app: &A) -> Response<A::ResponseBody> {
        A::Error::from(self).into_response(app)
    }
}

/// Maintains state during the routing of requests to the selected handler
///
/// The `Context` is created by the `Server` (or similar code) from a `Request` and
/// reference-counted `Application` instance. It is used to yield parts of the request
/// to a handler or routing context through implementations of the `FromContext` trait.
/// To this end, it immediately decouples the request's headers from its body, because
/// the former are kept alive throughout the request while the body may be ignored
/// for HEAD/GET requests or will be asynchronously consumed by the handler if necessary.
///
/// Once the request reaches a destination handler, it will typically be destructed into
/// its (remaining) constituent parts for further use by the handler's code. (This is usually
/// taken care of by one of the handler family of procedural macros, like `get`.)
pub struct Context<A>
where
    A: Application,
{
    #[doc(hidden)]
    pub app: Arc<A>,
    #[doc(hidden)]
    pub req: http::request::Parts,
    #[doc(hidden)]
    pub body: Option<A::RequestBody>,
    #[doc(hidden)]
    pub path: PathState,
}

impl<A> Context<A>
where
    A: Application,
{
    // This should only be used by procedural routing macros.
    #[doc(hidden)]
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

    // This should only be used by procedural routing macros.
    #[doc(hidden)]
    pub fn next_path(&mut self) -> Option<&str> {
        self.path.next(&self.req.uri.path())
    }

    // This should only be used by procedural routing macros.
    #[doc(hidden)]
    pub fn rest(&mut self) -> &str {
        self.path.rest(&self.req.uri.path())
    }

    // This should only be used by procedural routing macros.
    #[doc(hidden)]
    pub fn rewind(mut self) -> Self {
        self.path.rewind();
        self
    }

    // This should only be used by procedural routing macros.
    #[doc(hidden)]
    pub fn take_body(&mut self) -> Option<A::RequestBody> {
        self.body.take()
    }

    // This should only be used by procedural routing macros.
    #[doc(hidden)]
    pub fn app(&self) -> &Arc<A> {
        &self.app
    }

    // This should only be used by procedural routing macros.
    #[doc(hidden)]
    pub fn method(&self) -> &http::Method {
        &self.req.method
    }

    // This should only be used by procedural routing macros.
    #[doc(hidden)]
    pub fn uri(&self) -> &http::uri::Uri {
        &self.req.uri
    }

    // This should only be used by procedural routing macros.
    #[doc(hidden)]
    pub fn headers(&self) -> &http::HeaderMap {
        &self.req.headers
    }

    // This should only be used by procedural routing macros.
    #[doc(hidden)]
    pub fn query<'de, T: serde::de::Deserialize<'de>>(&'de self) -> Result<T, ClientError> {
        let query = self.req.uri.query().ok_or(ClientError::BadRequest)?;
        serde_urlencoded::from_bytes::<T>(query.as_bytes()).map_err(|_| ClientError::BadRequest)
    }

    #[doc(hidden)]
    pub fn error(&self, e: ClientError) -> Response<A::ResponseBody> {
        e.into_response(&*self.app)
    }
}

pub trait FromContext<'a, A>: Sized
where
    A: Application,
{
    fn from_context(
        req: &'a Parts,
        state: &mut PathState,
        body: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error>;
}

macro_rules! from_context_from_str {
    ($self:ty) => {
        impl<'a, A: Application> FromContext<'a, A> for $self {
            fn from_context(
                req: &'a Parts,
                state: &mut PathState,
                _: &mut Option<A::RequestBody>,
            ) -> Result<Self, A::Error> {
                let s = state.next(&req.uri.path()).ok_or(ClientError::NotFound)?;
                <$self>::from_str(s).map_err(|_| ClientError::NotFound.into())
            }
        }

        impl<'a, A: Application> FromContext<'a, A> for Option<$self> {
            fn from_context(
                req: &'a Parts,
                state: &mut PathState,
                _: &mut Option<A::RequestBody>,
            ) -> Result<Self, A::Error> {
                match state.next(&req.uri.path()) {
                    Some(s) => match <$self>::from_str(s) {
                        Ok(v) => Ok(Some(v)),
                        Err(_) => Err(ClientError::NotFound.into()),
                    },
                    None => Ok(None),
                }
            }
        }
    };
}

impl<'a, A: Application> FromContext<'a, A> for &'a http::request::Parts {
    fn from_context(
        req: &'a Parts,
        _: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        Ok(req)
    }
}

impl<'a, A: Application> FromContext<'a, A> for Option<&'a str> {
    fn from_context(
        req: &'a Parts,
        state: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        Ok(state.next(&req.uri.path()))
    }
}

impl<'a, A: Application> FromContext<'a, A> for &'a str {
    fn from_context(
        req: &'a Parts,
        state: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        state
            .next(&req.uri.path())
            .ok_or_else(|| ClientError::NotFound.into())
    }
}

from_context_from_str!(bool);
from_context_from_str!(char);
from_context_from_str!(f32);
from_context_from_str!(f64);
from_context_from_str!(i8);
from_context_from_str!(i16);
from_context_from_str!(i32);
from_context_from_str!(i64);
from_context_from_str!(i128);
from_context_from_str!(isize);
from_context_from_str!(u8);
from_context_from_str!(u16);
from_context_from_str!(u32);
from_context_from_str!(u64);
from_context_from_str!(u128);
from_context_from_str!(usize);

macro_rules! deserialize_body {
    ($req:ident, $bytes:ident) => {
        match $req.headers.get("content-type") {
            Some(t) if t == "application/x-www-form-urlencoded" => {
                serde_urlencoded::from_bytes::<T>(&$bytes)
                    .map_err(|e| FromBodyError::Deserialize(e.into()))
            }
            #[cfg(feature = "serde_json")]
            Some(t) if t == "application/json" => serde_json::from_slice::<T>(&$bytes)
                .map_err(|e| FromBodyError::Deserialize(e.into())),
            #[cfg(feature = "uploads")]
            Some(t) if t.as_bytes().starts_with(b"multipart/form-data") => {
                crate::forms::from_form_data::<T>(&$req.headers, &$bytes)
                    .map_err(|e| FromBodyError::Deserialize(e.into()))
            }
            None => Err(FromBodyError::NoType),
            _ => Err(FromBodyError::UnknownType),
        }
    };
}

#[cfg(feature = "with-http-body")]
async fn from_body<B, T: serde::de::DeserializeOwned>(
    req: &Parts,
    body: B,
) -> Result<T, FromBodyError>
where
    B: HttpBody,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    let bytes = to_bytes(body)
        .await
        .map_err(|e| FromBodyError::Receive(e.into()))?;
    deserialize_body!(req, bytes)
}

fn from_bytes<'de, T: serde::de::Deserialize<'de>>(
    req: &Parts,
    bytes: &'de [u8],
) -> Result<T, FromBodyError> {
    deserialize_body!(req, bytes)
}

pub enum FromBodyError {
    Receive(Box<dyn StdError + Send + Sync>),
    Deserialize(Box<dyn StdError + Send + Sync>),
    NoType,
    UnknownType,
}

impl fmt::Display for FromBodyError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        use FromBodyError::*;
        match self {
            Receive(e) => write!(fmt, "{}", e),
            Deserialize(e) => write!(fmt, "{}", e),
            NoType => write!(fmt, "no Content-Type found in request"),
            UnknownType => write!(fmt, "unsupported Content-Type in request"),
        }
    }
}

impl From<FromBodyError> for ClientError {
    fn from(e: FromBodyError) -> ClientError {
        use FromBodyError::*;
        match e {
            Receive(_) => ClientError::BadRequest,
            Deserialize(_) => ClientError::UnprocessableEntity,
            NoType => ClientError::BadRequest,
            UnknownType => ClientError::UnsupportedMediaType,
        }
    }
}

#[cfg(feature = "with-http-body")]
async fn to_bytes<T>(body: T) -> Result<Bytes, T::Error>
where
    T: HttpBody,
{
    futures_util::pin_mut!(body);

    // If there's only 1 chunk, we can just return Buf::to_bytes()
    let mut first = if let Some(buf) = body.data().await {
        buf?
    } else {
        return Ok(Bytes::new());
    };

    let second = if let Some(buf) = body.data().await {
        buf?
    } else {
        return Ok(first.to_bytes());
    };

    // With more than 1 buf, we gotta flatten into a Vec first.
    let cap = first.remaining() + second.remaining() + body.size_hint().lower() as usize;
    let mut vec = Vec::with_capacity(cap);
    vec.put(first);
    vec.put(second);

    while let Some(buf) = body.data().await {
        vec.put(buf?);
    }

    Ok(vec.into())
}

// This should only be used by procedural routing macros.
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

    // This should only be used by procedural routing macros.
    #[doc(hidden)]
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

    // This should only be used by procedural routing macros.
    #[doc(hidden)]
    pub fn rest<'r>(&mut self, path: &'r str) -> &'r str {
        let start = match self.next.take() {
            Some(v) => v,
            None => return "",
        };

        self.prev = Some(start);
        &path[start..]
    }

    // This should only be used by procedural routing macros.
    #[doc(hidden)]
    pub fn rewind(&mut self) {
        self.next = self.prev.take();
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

/// Extension trait for serving an `Application` on the given `SocketAddr`
#[async_trait]
pub trait Server: Application {
    type ServerError;

    async fn serve(self, addr: &SocketAddr) -> Result<(), Self::ServerError>;
}
