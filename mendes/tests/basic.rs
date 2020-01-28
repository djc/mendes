use std::sync::Arc;

use async_trait::async_trait;
use http::{Request, Response, StatusCode};
use hyper::Body;
use mendes::{dispatch, handler, Application, ClientError, Context};

#[tokio::test]
async fn basic() {
    let req = Request::builder()
        .uri("https://example.com/hello")
        .body(())
        .unwrap();

    let app = Arc::new(App {});
    let cx = Context::new(app, req);
    let rsp = App::handle(cx).await;
    assert_eq!(rsp.status(), StatusCode::OK);
}

#[handler(App)]
async fn hello(_: &App, _: Request<()>) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body("Hello, world".into())
        .unwrap())
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
            _ => hello,
        }
    }

    fn error(&self, _: Error) -> Response<Body> {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("ERROR".into())
            .unwrap()
    }
}

#[derive(Debug)]
enum Error {
    Default,
}

impl From<ClientError> for Error {
    fn from(_: ClientError) -> Self {
        Error::Default
    }
}
