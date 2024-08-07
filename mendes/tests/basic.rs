#![cfg(feature = "application")]

use std::borrow::Cow;
use std::sync::Arc;

use async_trait::async_trait;
use mendes::application::{IntoResponse, PathState};
use mendes::http::request::Parts;
use mendes::http::{Method, Request, Response, StatusCode};
use mendes::{handler, route, scope, Application, Context, FromContext};

#[tokio::test]
async fn test_query() {
    let rsp = handle(path_request("/query?foo=3&bar=baz")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(rsp.into_body(), "query: Query { foo: 3, bar: \"baz\" }");
}

#[tokio::test]
async fn test_method_get() {
    let rsp = handle(path_request("/method")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(rsp.into_body(), "Hello, world");
}

#[tokio::test]
async fn test_method_post() {
    let mut req = path_request("/method/post");
    *req.method_mut() = Method::POST;
    let rsp = handle(req).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(rsp.into_body(), "Hello, post");
}

#[tokio::test]
async fn test_magic_405() {
    let mut req = path_request("/method/post");
    *req.method_mut() = Method::PATCH;
    let rsp = handle(req).await;
    assert_eq!(rsp.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_nested_rest() {
    let rsp = handle(path_request("/nested/some/more")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(rsp.into_body(), "nested rest some/more");
}

#[tokio::test]
async fn test_nested_right() {
    let rsp = handle(path_request("/nested/right/2018")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(rsp.into_body(), "nested right 2018");
}

#[tokio::test]
async fn test_inc_invalid() {
    let rsp = handle(path_request("/inc/Foo")).await;
    assert_eq!(rsp.status(), StatusCode::NOT_FOUND);
    assert_eq!(rsp.into_body(), "unable to parse path component");
}

#[tokio::test]
async fn test_inc() {
    let rsp = handle(path_request("/inc/2016")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(rsp.into_body(), "num = 2017");
}

#[tokio::test]
async fn test_named() {
    let rsp = handle(path_request("/named/Foo")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(rsp.into_body(), "Hello, Foo");
}

#[tokio::test]
async fn test_named_no_arg() {
    let rsp = handle(path_request("/named")).await;
    assert_eq!(rsp.status(), StatusCode::NOT_FOUND);
    assert_eq!(rsp.into_body(), "missing path component");
}

#[tokio::test]
async fn test_magic_404() {
    let rsp = handle(path_request("/foo")).await;
    assert_eq!(rsp.status(), StatusCode::NOT_FOUND);
    assert_eq!(rsp.into_body(), "no matching routes");
}

#[tokio::test]
async fn test_custom_error_handler() {
    let rsp = handle(path_request("/custom_hello/true")).await;
    assert_eq!(rsp.status(), StatusCode::IM_USED);

    // This is missing the ContextExtraction path part. In Error the resulting error is serialized as StatusCode::OK.
    // But we want to test the custom error handler which serializes this into StatusCode::IM_A_TEAPOT.
    let rsp = handle(path_request("/custom_hello")).await;
    assert_eq!(rsp.status(), StatusCode::IM_A_TEAPOT);
}

#[tokio::test]
async fn basic() {
    let rsp = handle(path_request("/hello")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
}

fn path_request(path: &str) -> Request<()> {
    Request::builder()
        .uri(format!("https://example.com{path}"))
        .body(())
        .unwrap()
}

async fn handle(req: Request<()>) -> Response<String> {
    App::handle(Context::new(Arc::new(App {}), req)).await
}

struct App {}

#[async_trait]
impl Application for App {
    type RequestBody = ();
    type ResponseBody = String;
    type Error = Error;

    async fn handle(mut cx: Context<Self>) -> Response<Self::ResponseBody> {
        route!(match cx.path() {
            Some("hello") => hello,
            Some("named") => named,
            Some("inc") => inc,
            Some("nested") => match cx.path() {
                Some("right") => nested_right,
                _ => nested_rest,
            },
            Some("scoped") => scoped,
            Some("method") => match cx.method() {
                GET => hello,
                POST => named,
            },
            Some("custom_hello") => custom_error,

            Some("query") => with_query,
        })
    }
}

#[scope]
async fn scoped(cx: &mut Context<App>) -> Response<String> {
    route!(match cx.path() {
        Some("right") => nested_right,
        _ => nested_rest,
    })
}

#[handler(GET)]
async fn with_query(_: &App, #[query] query: Query<'_>) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("query: {query:?}"))
        .unwrap())
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)] // Reflected as part of the `Debug` impl
struct Query<'a> {
    foo: usize,
    bar: Cow<'a, str>,
}

#[handler(GET)]
async fn nested_rest(_: &App, #[rest] path: Cow<'_, str>) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("nested rest {path}"))
        .unwrap())
}

#[handler(GET)]
async fn nested_right(_: &App, num: usize) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("nested right {num}"))
        .unwrap())
}

#[handler(GET)] // use mutable argument to test this case
async fn inc(_: &App, mut num: usize) -> Result<Response<String>, Error> {
    num += 1;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("num = {num}"))
        .unwrap())
}

#[handler(get, post)]
async fn named(_: &App, name: String) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("Hello, {name}"))
        .unwrap())
}

#[handler(GET)]
async fn hello(_: &App) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body("Hello, world".into())
        .unwrap())
}

#[handler(GET)]
async fn custom_error(_: &App, _x: ContextExtraction) -> Result<Response<String>, HandlerError> {
    Err(HandlerError::Test)
}

#[derive(Debug)]
enum Error {
    Mendes(mendes::Error),
    NotTrue,
}

impl From<mendes::Error> for Error {
    fn from(e: mendes::Error) -> Self {
        Error::Mendes(e)
    }
}

impl From<&Error> for StatusCode {
    fn from(e: &Error) -> StatusCode {
        match e {
            Error::Mendes(e) => StatusCode::from(e),
            Error::NotTrue => StatusCode::OK,
        }
    }
}

impl IntoResponse<App> for Error {
    fn into_response(self, _: &App, _: &Parts) -> Response<String> {
        let builder = Response::builder().status(StatusCode::from(&self));
        match self {
            Error::Mendes(err) => builder.body(err.to_string()),
            Error::NotTrue => builder.body("".to_string()),
        }
        .unwrap()
    }
}

enum HandlerError {
    Mendes(mendes::Error),
    NotTrue,
    Test,
}

impl From<mendes::Error> for HandlerError {
    fn from(e: mendes::Error) -> Self {
        Self::Mendes(e)
    }
}

impl From<Error> for HandlerError {
    fn from(e: Error) -> Self {
        match e {
            Error::Mendes(e) => HandlerError::Mendes(e),
            Error::NotTrue => HandlerError::NotTrue,
        }
    }
}

impl IntoResponse<App> for HandlerError {
    fn into_response(self, _: &App, _: &Parts) -> Response<String> {
        let builder = Response::builder();
        match self {
            HandlerError::Mendes(err) => {
                builder.status(StatusCode::from(&err)).body(err.to_string())
            }
            HandlerError::Test => builder.status(StatusCode::IM_USED).body("".to_string()),
            HandlerError::NotTrue => builder.status(StatusCode::IM_A_TEAPOT).body("".to_string()),
        }
        .unwrap()
    }
}

struct ContextExtraction;

impl FromContext<'_, App> for ContextExtraction {
    fn from_context(
        _app: &'_ Arc<App>,
        req: &'_ Parts,
        state: &mut PathState,
        _: &mut Option<<App as Application>::RequestBody>,
    ) -> Result<ContextExtraction, Error> {
        match state.next(req.uri.path()) {
            Some("true") => Ok(ContextExtraction),
            _ => Err(Error::NotTrue),
        }
    }
}
