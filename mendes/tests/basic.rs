use std::sync::Arc;

use async_trait::async_trait;
use http::{Request, Response, StatusCode};
use hyper::body::to_bytes;
use hyper::Body;
use mendes::{dispatch, handler, Application, ClientError, Context};

#[tokio::test]
async fn test_nested_rest() {
    let rsp = handle(path_request("/nested/some/more")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(
        &to_bytes(rsp.into_body()).await.unwrap(),
        &b"nested rest some/more"[..]
    );
}

#[tokio::test]
async fn test_nested_right() {
    let rsp = handle(path_request("/nested/right/2018")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(
        &to_bytes(rsp.into_body()).await.unwrap(),
        &b"nested right 2018"[..]
    );
}

#[tokio::test]
async fn test_numbered_invalid() {
    let rsp = handle(path_request("/numbered/Foo")).await;
    assert_eq!(rsp.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        &to_bytes(rsp.into_body()).await.unwrap(),
        &b"404 Not Found"[..]
    );
}

#[tokio::test]
async fn test_numbered() {
    let rsp = handle(path_request("/numbered/2016")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(&to_bytes(rsp.into_body()).await.unwrap(), &b"ID = 2016"[..]);
}

#[tokio::test]
async fn test_named() {
    let rsp = handle(path_request("/named/Foo")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(
        &to_bytes(rsp.into_body()).await.unwrap(),
        &b"Hello, Foo"[..]
    );
}

#[tokio::test]
async fn test_named_no_arg() {
    let rsp = handle(path_request("/named")).await;
    assert_eq!(rsp.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        &to_bytes(rsp.into_body()).await.unwrap(),
        &b"404 Not Found"[..]
    );
}

#[tokio::test]
async fn test_magic_404() {
    let rsp = handle(path_request("/foo")).await;
    assert_eq!(rsp.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        &to_bytes(rsp.into_body()).await.unwrap(),
        &b"404 Not Found"[..]
    );
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

async fn handle(req: Request<()>) -> Response<Body> {
    let app = Arc::new(App {});
    let cx = Context::new(app, req);
    App::handle(cx).await
}

struct App {}

#[async_trait]
impl Application for App {
    type RequestBody = ();
    type ResponseBody = Body;
    type Error = Error;

    #[dispatch]
    async fn handle(mut cx: Context<Self>) -> Response<Body> {
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

    fn error(&self, err: Error) -> Response<Body> {
        let err = match err {
            Error::Client(err) => err,
        };

        Response::builder()
            .status(StatusCode::from(err))
            .body(err.to_string().into())
            .unwrap()
    }
}

#[handler(App)]
async fn nested_rest(#[rest] path: &str) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("nested rest {}", path).into())
        .unwrap())
}

#[handler(App)]
async fn nested_right(num: usize) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("nested right {}", num).into())
        .unwrap())
}

#[handler(App)]
async fn numbered(num: usize) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("ID = {}", num).into())
        .unwrap())
}

#[handler(App)]
async fn named(name: &str) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("Hello, {}", name).into())
        .unwrap())
}

#[handler(App)]
async fn hello() -> Result<Response<Body>, Error> {
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
