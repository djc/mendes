use std::borrow::Cow;
#[cfg(feature = "with-http-body")]
use std::error::Error as StdError;
use std::future::Future;
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
use percent_encoding::percent_decode_str;
use thiserror::Error;

pub use mendes_macros::{handler, route, scope};

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
pub trait Application: Send + Sized {
    type RequestBody: Send;
    type ResponseBody: Send;
    type Error: IntoResponse<Self> + WithStatus + From<Error> + Send;

    async fn handle(cx: Context<Self>) -> Response<Self::ResponseBody>;

    fn from_query<'a, T: serde::Deserialize<'a>>(req: &'a Parts) -> Result<T, Self::Error> {
        let query = req.uri.query().ok_or(Error::QueryMissing)?;
        let data =
            serde_urlencoded::from_bytes::<T>(query.as_bytes()).map_err(Error::QueryDecode)?;
        Ok(data)
    }

    fn from_body_bytes<'de, T: serde::de::Deserialize<'de>>(
        req: &Parts,
        bytes: &'de [u8],
    ) -> Result<T, Error> {
        from_bytes::<T>(req, bytes)
    }

    #[cfg(feature = "with-http-body")]
    #[cfg_attr(docsrs, doc(cfg(feature = "with-http-body")))]
    async fn from_body<T: serde::de::DeserializeOwned>(
        req: &Parts,
        body: Self::RequestBody,
        max_len: usize,
    ) -> Result<T, Error>
    where
        Self::RequestBody: HttpBody + Send,
        <Self::RequestBody as HttpBody>::Data: Send,
        <Self::RequestBody as HttpBody>::Error: Into<Box<dyn StdError + Sync + Send>>,
    {
        // Check if the Content-Length header suggests the body is larger than our max len
        // to avoid allocation if we drop the request in any case.
        let expected_len = match body.size_hint().upper() {
            Some(length) => length,
            None => body.size_hint().lower(),
        };
        if expected_len > max_len as u64 {
            return Err(Error::BodyTooLarge);
        }

        from_body::<Self::RequestBody, T>(req, body, max_len).await
    }

    #[cfg(feature = "with-http-body")]
    #[cfg_attr(docsrs, doc(cfg(feature = "with-http-body")))]
    async fn body_bytes<B>(body: B, max_len: usize) -> Result<Bytes, Error>
    where
        B: HttpBody + Send,
        <B as HttpBody>::Data: Send,
        B::Error: Into<Box<dyn StdError + Sync + Send + 'static>>,
    {
        // Check if the Content-Length header suggests the body is larger than our max len
        // to avoid allocation if we drop the request in any case.
        let expected_len = match body.size_hint().upper() {
            Some(length) => length,
            None => body.size_hint().lower(),
        };
        if expected_len > max_len as u64 {
            return Err(Error::BodyTooLarge);
        }

        Ok(to_bytes(body, max_len).await?)
    }

    fn redirect(status: StatusCode, path: impl AsRef<str>) -> Response<Self::ResponseBody>
    where
        Self::ResponseBody: Default,
    {
        http::Response::builder()
            .status(status)
            .header(LOCATION, path.as_ref())
            .body(Self::ResponseBody::default())
            .unwrap()
    }
}

pub trait WithStatus {}

impl<T> WithStatus for T where StatusCode: for<'a> From<&'a T> {}

pub trait IntoResponse<A: Application> {
    fn into_response(self, app: &A, req: &Parts) -> Response<A::ResponseBody>;
}

impl<A: Application> IntoResponse<A> for Response<A::ResponseBody> {
    fn into_response(self, _: &A, _: &Parts) -> Response<A::ResponseBody> {
        self
    }
}

impl<A: Application, T> IntoResponse<A> for Result<T, A::Error>
where
    T: IntoResponse<A>,
{
    fn into_response(self, app: &A, req: &Parts) -> Response<A::ResponseBody> {
        match self {
            Ok(rsp) => rsp.into_response(app, req),
            Err(e) => e.into_response(app, req),
        }
    }
}

impl<A: Application> IntoResponse<A> for Error {
    fn into_response(self, app: &A, req: &Parts) -> Response<A::ResponseBody> {
        A::Error::from(self).into_response(app, req)
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
    pub app: Arc<A>,
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
    pub fn path(&mut self) -> Option<Cow<'_, str>> {
        path_str(&self.req, &mut self.path).ok().flatten()
    }

    // This should only be used by procedural routing macros.
    #[doc(hidden)]
    pub fn rewind(&mut self) {
        self.path.rewind();
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
}

impl<A: Application> AsMut<Context<A>> for Context<A> {
    fn as_mut(&mut self) -> &mut Context<A> {
        self
    }
}

pub trait FromContext<'a, A>: Sized
where
    A: Application,
{
    fn from_context(
        app: &'a Arc<A>,
        req: &'a Parts,
        state: &mut PathState,
        body: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error>;
}

macro_rules! from_context_from_str {
    ($self:ty) => {
        impl<'a, A: Application> FromContext<'a, A> for $self {
            fn from_context(
                _: &'a Arc<A>,
                req: &'a Parts,
                state: &mut PathState,
                _: &mut Option<A::RequestBody>,
            ) -> Result<Self, A::Error> {
                let s = state
                    .next(req.uri.path())
                    .ok_or(Error::PathComponentMissing.into())?;
                <$self>::from_str(s).map_err(|_| Error::PathParse.into())
            }
        }

        impl<'a, A: Application> FromContext<'a, A> for Option<$self> {
            fn from_context(
                _: &'a Arc<A>,
                req: &'a Parts,
                state: &mut PathState,
                _: &mut Option<A::RequestBody>,
            ) -> Result<Self, A::Error> {
                match state.next(req.uri.path()) {
                    Some(s) => match <$self>::from_str(s) {
                        Ok(v) => Ok(Some(v)),
                        Err(_) => Err(Error::PathParse.into()),
                    },
                    None => Ok(None),
                }
            }
        }
    };
}

impl<'a, A: Application> FromContext<'a, A> for &'a A {
    fn from_context(
        app: &'a Arc<A>,
        _: &'a Parts,
        _: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        Ok(app)
    }
}

impl<'a, A: Application> FromContext<'a, A> for &'a Arc<A> {
    fn from_context(
        app: &'a Arc<A>,
        _: &'a Parts,
        _: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        Ok(app)
    }
}

impl<'a, A: Application> FromContext<'a, A> for &'a http::request::Parts {
    fn from_context(
        _: &'a Arc<A>,
        req: &'a Parts,
        _: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        Ok(req)
    }
}

impl<'a, A: Application> FromContext<'a, A> for Option<&'a [u8]> {
    fn from_context(
        _: &'a Arc<A>,
        req: &'a Parts,
        state: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        Ok(state.next(req.uri.path()).map(|s| s.as_bytes()))
    }
}

impl<'a, A: Application> FromContext<'a, A> for &'a [u8] {
    fn from_context(
        _: &'a Arc<A>,
        req: &'a Parts,
        state: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        state
            .next(req.uri.path())
            .ok_or_else(|| Error::PathComponentMissing.into())
            .map(|s| s.as_bytes())
    }
}

impl<'a, A: Application> FromContext<'a, A> for Option<Cow<'a, str>> {
    fn from_context(
        _: &'a Arc<A>,
        req: &'a Parts,
        state: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        Ok(path_str(req, state)?)
    }
}

impl<'a, A: Application> FromContext<'a, A> for Cow<'a, str> {
    fn from_context(
        _: &'a Arc<A>,
        req: &'a Parts,
        state: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        match path_str(req, state)? {
            Some(s) => Ok(s),
            None => Err(Error::PathComponentMissing.into()),
        }
    }
}

impl<'a, A: Application> FromContext<'a, A> for Option<String> {
    fn from_context(
        _: &'a Arc<A>,
        req: &'a Parts,
        state: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        Ok(path_str(req, state)?.map(|s| s.into_owned()))
    }
}

impl<'a, A: Application> FromContext<'a, A> for String {
    fn from_context(
        _: &'a Arc<A>,
        req: &'a Parts,
        state: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        match path_str(req, state)? {
            Some(s) => Ok(s.into_owned()),
            None => Err(Error::PathComponentMissing.into()),
        }
    }
}

fn path_str<'a>(req: &'a Parts, state: &mut PathState) -> Result<Option<Cow<'a, str>>, Error> {
    let s = match state.next(req.uri.path()) {
        Some(s) => s,
        None => return Ok(None),
    };

    percent_decode_str(s)
        .decode_utf8()
        .map(Some)
        .map_err(|_| Error::PathDecode)
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
    ($req:ident, $bytes:ident) => {{
        let content_type = $req.headers.get("content-type").ok_or(Error::BodyNoType)?;
        let ct_str = content_type.to_str().map_err(|_| {
            Error::BodyUnknownType(String::from_utf8_lossy(content_type.as_bytes()).into_owned())
        })?;

        let mut parts = ct_str.splitn(2, ';');
        match parts.next().map(|s| s.trim()) {
            Some("application/x-www-form-urlencoded") => {
                serde_urlencoded::from_bytes::<T>(&$bytes).map_err(Error::BodyDecodeForm)
            }
            #[cfg(feature = "serde_json")]
            Some("application/json") => {
                serde_json::from_slice::<T>(&$bytes).map_err(Error::BodyDecodeJson)
            }
            #[cfg(feature = "uploads")]
            Some("multipart/form-data") => {
                crate::forms::from_form_data::<T>(&$req.headers, &$bytes)
                    .map_err(Error::BodyDecodeMultipart)
            }
            Some(_) | None => Err(Error::BodyUnknownType(ct_str.to_owned())),
        }
    }};
}

#[doc(hidden)]
pub struct Rest<T>(pub T);

impl<'a, A: Application> FromContext<'a, A> for Rest<&'a [u8]> {
    fn from_context(
        _: &'a Arc<A>,
        req: &'a Parts,
        state: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        Ok(Rest(state.rest(req.uri.path()).as_bytes()))
    }
}

impl<'a, A: Application> FromContext<'a, A> for Rest<Cow<'a, str>> {
    fn from_context(
        _: &'a Arc<A>,
        req: &'a Parts,
        state: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        Ok(Rest(
            percent_decode_str(state.rest(req.uri.path()))
                .decode_utf8()
                .map_err(|_| Error::PathDecode)?,
        ))
    }
}

#[doc(hidden)]
pub struct Query<T>(pub T);

impl<'de, 'a: 'de, A: Application, T> FromContext<'a, A> for Query<T>
where
    T: serde::Deserialize<'de>,
{
    fn from_context(
        _: &'a Arc<A>,
        req: &'a Parts,
        _: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        A::from_query(req).map(Query)
    }
}

#[cfg(feature = "with-http-body")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-http-body")))]
async fn from_body<B, T: serde::de::DeserializeOwned>(
    req: &Parts,
    body: B,
    max_len: usize,
) -> Result<T, Error>
where
    B: HttpBody,
    B::Error: Into<Box<dyn StdError + Send + Sync + 'static>>,
{
    let bytes = to_bytes(body, max_len).await?;
    deserialize_body!(req, bytes)
}

fn from_bytes<'de, T: serde::de::Deserialize<'de>>(
    req: &Parts,
    bytes: &'de [u8],
) -> Result<T, Error> {
    deserialize_body!(req, bytes)
}

#[cfg(feature = "with-http-body")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-http-body")))]
async fn to_bytes<B>(body: B, max_len: usize) -> Result<Bytes, Error>
where
    B: HttpBody,
    B::Error: Into<Box<dyn StdError + Send + Sync + 'static>>,
{
    pin_utils::pin_mut!(body);

    // If there's only 1 chunk, we can just return Buf::to_bytes()
    let mut first = if let Some(buf) = body.data().await {
        buf.map_err(|err| Error::BodyReceive(err.into()))?
    } else {
        return Ok(Bytes::new());
    };

    let mut received = first.remaining();
    if received > max_len {
        return Err(Error::BodyTooLarge);
    }

    let second = if let Some(buf) = body.data().await {
        buf.map_err(|err| Error::BodyReceive(err.into()))?
    } else {
        return Ok(first.copy_to_bytes(first.remaining()));
    };

    received += second.remaining();
    if received > max_len {
        return Err(Error::BodyTooLarge);
    }

    // With more than 1 buf, we gotta flatten into a Vec first.
    let cap = first.remaining() + second.remaining() + body.size_hint().lower() as usize;
    let mut vec = Vec::with_capacity(cap);
    vec.put(first);
    vec.put(second);

    while let Some(buf) = body.data().await {
        let buf = buf.map_err(|err| Error::BodyReceive(err.into()))?;
        received += buf.remaining();
        if received > max_len {
            return Err(Error::BodyTooLarge);
        }

        vec.put(buf);
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

#[derive(Debug, Error)]
pub enum Error {
    #[error("method not allowed")]
    MethodNotAllowed,
    #[error("no matching routes")]
    PathNotFound,
    #[error("missing path component")]
    PathComponentMissing,
    #[error("unable to parse path component")]
    PathParse,
    #[error("unable to decode UTF-8 from path component")]
    PathDecode,
    #[error("no query in request URL")]
    QueryMissing,
    #[error("unable to decode request URI query: {0}")]
    QueryDecode(serde_urlencoded::de::Error),
    #[cfg(feature = "with-http-body")]
    #[error("unable to receive request body: {0}")]
    BodyReceive(Box<dyn StdError + Send + Sync + 'static>),
    #[cfg(feature = "with-http-body")]
    #[error("request body too large")]
    BodyTooLarge,
    #[cfg(feature = "json")]
    #[error("unable to decode body as JSON: {0}")]
    BodyDecodeJson(#[from] serde_json::Error),
    #[error("unable to decode body as form data: {0}")]
    BodyDecodeForm(serde_urlencoded::de::Error),
    #[cfg(feature = "uploads")]
    #[error("unable to decode body as multipart form data: {0}")]
    BodyDecodeMultipart(#[from] crate::multipart::Error),
    #[error("content type on request body unknown: {0}")]
    BodyUnknownType(String),
    #[error("no content type on request body")]
    BodyNoType,
    #[cfg(feature = "static")]
    #[error("file not found")]
    FileNotFound,
}

impl From<&Error> for StatusCode {
    fn from(e: &Error) -> StatusCode {
        use Error::*;
        match e {
            MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            QueryMissing | QueryDecode(_) | BodyNoType => StatusCode::BAD_REQUEST,
            BodyUnknownType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            PathNotFound | PathComponentMissing | PathParse | PathDecode => StatusCode::NOT_FOUND,
            #[cfg(feature = "with-http-body")]
            BodyReceive(_) => StatusCode::INTERNAL_SERVER_ERROR,
            #[cfg(feature = "with-http-body")]
            BodyTooLarge => StatusCode::BAD_REQUEST,
            BodyDecodeForm(_) => StatusCode::UNPROCESSABLE_ENTITY,
            #[cfg(feature = "json")]
            BodyDecodeJson(_) => StatusCode::UNPROCESSABLE_ENTITY,
            #[cfg(feature = "uploads")]
            BodyDecodeMultipart(_) => StatusCode::UNPROCESSABLE_ENTITY,
            #[cfg(feature = "static")]
            FileNotFound => StatusCode::NOT_FOUND,
        }
    }
}

/// Extension trait for serving an `Application` on the given `SocketAddr`
#[async_trait]
pub trait Server: Application {
    type ServerError;

    async fn serve(self, addr: &SocketAddr) -> Result<(), Self::ServerError>;
    async fn serve_with_graceful_shutdown(
        self,
        addr: &SocketAddr,
        signal: impl Future<Output = ()> + Send,
    ) -> Result<(), Self::ServerError>;
}
