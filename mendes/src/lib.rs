use std::sync::Arc;

use async_trait::async_trait;
use http::{Request, Response, StatusCode};
use hyper::header::LOCATION;
use hyper::Body;
pub use mendes_derive::{dispatch, handler};

#[async_trait]
pub trait Application: Sized {
    type RequestBody;
    type ResponseBody;
    type Error;

    async fn handle(cx: Context<Self>) -> Response<Self::ResponseBody>;

    fn error(&self, error: Self::Error) -> Response<Self::ResponseBody>;
}

pub struct Context<A>
where
    A: Application,
{
    pub app: Arc<A>,
    pub req: Request<A::RequestBody>,
    next_at: Option<usize>,
}

impl<A> Context<A>
where
    A: Application,
{
    pub fn new(app: Arc<A>, req: Request<A::RequestBody>) -> Context<A> {
        let path = req.uri().path();
        let next_at = if path.is_empty() || path == "/" {
            None
        } else if path.find('/') == Some(0) {
            Some(1)
        } else {
            Some(0)
        };

        Context { app, req, next_at }
    }

    pub fn path(&mut self) -> Option<&str> {
        let start = match self.next_at.as_ref() {
            Some(v) => *v,
            None => return None,
        };

        let path = &self.req.uri().path()[start..];
        if path.is_empty() {
            self.next_at = None;
            return None;
        }

        match path.find('/') {
            Some(end) => {
                self.next_at = Some(start + end + 1);
                Some(&path[..end])
            }
            None => {
                self.next_at = None;
                Some(path)
            }
        }
    }
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
