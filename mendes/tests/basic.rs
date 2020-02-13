use std::sync::Arc;

use async_trait::async_trait;
use http::{Request, Response, StatusCode};
use mendes::{dispatch, handler, Application, ClientError, Context};

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
    assert_eq!(rsp.into_body(), "404 Not Found");
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
    assert_eq!(rsp.into_body(), "404 Not Found");
}

#[tokio::test]
async fn test_magic_404() {
    let rsp = handle(path_request("/foo")).await;
    assert_eq!(rsp.status(), StatusCode::NOT_FOUND);
    assert_eq!(rsp.into_body(), "404 Not Found");
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
    let app = Arc::new(App {});
    let cx = Context::new(app, req);
    App::handle(cx).await
}

struct App {}

#[async_trait]
impl Application for App {
    type RequestBody = ();
    type ResponseBody = String;
    type Error = Error;

    #[dispatch]
    async fn handle(mut cx: Context<Self>) -> Response<Self::ResponseBody> {
        path! {
            Some("hello") => hello,
            Some("named") => named,
            Some("numbered") => numbered,
            Some("nested") => path! {
                Some("right") => nested_right,
                _ => nested_rest,
            },
        }
    }

    fn error(&self, err: Error) -> Response<Self::ResponseBody> {
        let Error::Client(err) = err;
        Response::builder()
            .status(StatusCode::from(err))
            .body(err.to_string())
            .unwrap()
    }
}

#[handler(App)]
async fn nested_rest(#[rest] path: &str) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("nested rest {}", path))
        .unwrap())
}

#[handler(App)]
async fn nested_right(num: usize) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("nested right {}", num))
        .unwrap())
}

#[handler(App)]
async fn numbered(num: usize) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("ID = {}", num))
        .unwrap())
}

#[handler(App)]
async fn named(name: &str) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("Hello, {}", name))
        .unwrap())
}

#[handler(App)]
async fn hello() -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body("Hello, world".into())
        .unwrap())
}

#[derive(Debug)]
enum Error {
    Client(ClientError),
}

impl From<ClientError> for Error {
    fn from(e: ClientError) -> Self {
        Error::Client(e)
    }
}
