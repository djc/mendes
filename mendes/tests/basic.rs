#![cfg(feature = "application")]

use std::sync::Arc;

use async_trait::async_trait;
use mendes::application::Responder;
use mendes::http::{Method, Request, Response, StatusCode};
use mendes::{handler, route, scope, Application, Context};

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
async fn test_numbered_invalid() {
    let rsp = handle(path_request("/numbered/Foo")).await;
    assert_eq!(rsp.status(), StatusCode::NOT_FOUND);
    assert_eq!(rsp.into_body(), "unable to parse path component");
}

#[tokio::test]
async fn test_numbered() {
    let rsp = handle(path_request("/numbered/2016")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(rsp.into_body(), "ID = 2016");
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
async fn basic() {
    let rsp = handle(path_request("/hello")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
}

fn path_request(path: &str) -> Request<()> {
    Request::builder()
        .uri(format!("https://example.com{}", path))
        .body(())
        .unwrap()
}

async fn handle(req: Request<()>) -> Response<String> {
    Arc::new(App {}).handle(req).await
}

struct App {}

#[async_trait]
impl Application for App {
    type RequestBody = ();
    type ResponseBody = String;
    type Error = Error;

    #[route]
    async fn handle(
        self: Arc<App>,
        req: Request<Self::RequestBody>,
    ) -> Response<Self::ResponseBody> {
        path! {
            Some("hello") => hello,
            Some("named") => named,
            Some("numbered") => numbered,
            Some("nested") => path! {
                Some("right") => nested_right,
                _ => nested_rest,
            },
            Some("scoped") => scoped,
            Some("method") => method! {
                GET => hello,
                POST => named,
            }
        }
    }
}

#[scope]
async fn scoped(cx: &mut Context<App>) -> Response<String> {
    path! {
        Some("right") => nested_right,
        _ => nested_rest,
    }
}

#[handler(GET)]
async fn nested_rest(_: &App, #[rest] path: &str) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("nested rest {}", path))
        .unwrap())
}

#[handler(GET)]
async fn nested_right(_: &App, num: usize) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("nested right {}", num))
        .unwrap())
}

#[handler(GET)]
async fn numbered(_: &App, num: usize) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("ID = {}", num))
        .unwrap())
}

#[handler(get, post)]
async fn named(_: &App, name: String) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("Hello, {}", name))
        .unwrap())
}

#[handler(GET)]
async fn hello(_: &App) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body("Hello, world".into())
        .unwrap())
}

#[derive(Debug)]
enum Error {
    Mendes(mendes::Error),
}

impl From<mendes::Error> for Error {
    fn from(e: mendes::Error) -> Self {
        Error::Mendes(e)
    }
}

impl From<&Error> for StatusCode {
    fn from(e: &Error) -> StatusCode {
        let Error::Mendes(e) = e;
        StatusCode::from(e)
    }
}

impl Responder<App> for Error {
    fn into_response(self, _: &App) -> Response<String> {
        let Error::Mendes(err) = self;
        Response::builder()
            .status(StatusCode::from(&err))
            .body(err.to_string())
            .unwrap()
    }
}
