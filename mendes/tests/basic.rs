use std::sync::Arc;

use async_trait::async_trait;
use http::{Request, Response, StatusCode};
use hyper::body::to_bytes;
use hyper::Body;
use mendes::{dispatch, handler, Application, ClientError, Context};

#[tokio::test]
async fn test_numbered_invalid() {
    let req = Request::builder()
        .uri("https://example.com/numbered/Foo")
        .body(())
        .unwrap();
    let rsp = handle(req).await;
    assert_eq!(rsp.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        &to_bytes(rsp.into_body()).await.unwrap(),
        &b"404 Not Found"[..]
    );
}

#[tokio::test]
async fn test_numbered() {
    let req = Request::builder()
        .uri("https://example.com/numbered/2016")
        .body(())
        .unwrap();
    let rsp = handle(req).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(&to_bytes(rsp.into_body()).await.unwrap(), &b"ID = 2016"[..]);
}

#[tokio::test]
async fn test_named() {
    let req = Request::builder()
        .uri("https://example.com/named/Foo")
        .body(())
        .unwrap();
    let rsp = handle(req).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(
        &to_bytes(rsp.into_body()).await.unwrap(),
        &b"Hello, Foo"[..]
    );
}

#[tokio::test]
async fn test_named_no_arg() {
    let req = Request::builder()
        .uri("https://example.com/named")
        .body(())
        .unwrap();
    let rsp = handle(req).await;
    assert_eq!(rsp.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        &to_bytes(rsp.into_body()).await.unwrap(),
        &b"404 Not Found"[..]
    );
}

#[tokio::test]
async fn test_magic_404() {
    let req = Request::builder()
        .uri("https://example.com/foo")
        .body(())
        .unwrap();
    let rsp = handle(req).await;
    assert_eq!(rsp.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        &to_bytes(rsp.into_body()).await.unwrap(),
        &b"404 Not Found"[..]
    );
}

#[tokio::test]
async fn basic() {
    let req = Request::builder()
        .uri("https://example.com/hello")
        .body(())
        .unwrap();
    let rsp = handle(req).await;
    assert_eq!(rsp.status(), StatusCode::OK);
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
