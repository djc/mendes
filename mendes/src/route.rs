use std::str;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use http::{Request, Response};
use serde::Deserialize;

pub use http;
pub use mendes_macros::{dispatch, handler};

use super::ClientError;

#[async_trait]
pub trait Application: Sized {
    type RequestBody;
    type ResponseBody;
    type Error: From<ClientError>;

    async fn handle(cx: Context<Self>) -> Response<Self::ResponseBody>;

    fn error(&self, error: Self::Error) -> Response<Self::ResponseBody>;
}

pub struct Context<A>
where
    A: Application,
{
    app: Arc<A>,
    req: http::request::Parts,
    body: Option<A::RequestBody>,
    path: PathState,
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

    fn next(&mut self) -> Option<&str> {
        self.path.next(&self.req.uri.path())
    }

    pub fn rest(&mut self) -> &str {
        self.path.rest(&self.req.uri.path())
    }

    pub fn take_body(&mut self) -> Option<A::RequestBody> {
        self.body.take()
    }

    pub fn rewind(mut self) -> Self {
        self.path.rewind();
        self
    }

    pub fn app(&self) -> &Arc<A> {
        &self.app
    }

    pub fn req(&self) -> &http::request::Parts {
        &self.req
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
    fn from_context<A: Application>(cx: &'a mut Context<A>) -> Result<Self, A::Error>;
}

impl<'a> FromContext<'a> for &'a http::request::Parts {
    fn from_context<A: Application>(cx: &'a mut Context<A>) -> Result<Self, A::Error> {
        Ok(&cx.req)
    }
}

impl<'a> FromContext<'a> for Option<&'a str> {
    fn from_context<A: Application>(cx: &'a mut Context<A>) -> Result<Self, A::Error> {
        Ok(cx.next())
    }
}

impl<'a> FromContext<'a> for &'a str {
    fn from_context<A: Application>(cx: &'a mut Context<A>) -> Result<Self, A::Error> {
        cx.next().ok_or_else(|| ClientError::NotFound.into())
    }
}

impl<'a> FromContext<'a> for usize {
    fn from_context<A: Application>(cx: &'a mut Context<A>) -> Result<Self, A::Error> {
        let s = cx.next().ok_or(ClientError::NotFound)?;
        usize::from_str(s).map_err(|_| ClientError::NotFound.into())
    }
}

impl<'a> FromContext<'a> for i32 {
    fn from_context<A: Application>(cx: &'a mut Context<A>) -> Result<Self, A::Error> {
        let s = cx.next().ok_or(ClientError::NotFound)?;
        i32::from_str(s).map_err(|_| ClientError::NotFound.into())
    }
}

struct PathState {
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

    fn next<'r>(&mut self, path: &'r str) -> Option<&'r str> {
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

    fn rest<'r>(&mut self, path: &'r str) -> &'r str {
        let start = match self.next.take() {
            Some(v) => v,
            None => return "",
        };

        self.prev = Some(start);
        &path[start..]
    }

    fn rewind(&mut self) {
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
