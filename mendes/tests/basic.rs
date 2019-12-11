use std::sync::Arc;

use http::{Request, Response, StatusCode};
use hyper::Body;
use mendes::Application;
use mendes_derive::dispatch;

#[tokio::test]
async fn basic() {
    let req = Request::builder()
        .uri("https://example.com/hello")
        .body(())
        .unwrap();
    let rsp = route(Arc::new(App {}), req).await;
    assert_eq!(rsp.status(), StatusCode::OK);
}

#[dispatch]
async fn route(app: Arc<App>, req: Request<()>) -> Response<Body> {
    route! {
        _ => hello,
    }
}

async fn hello(_: &App, _: Request<()>) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body("Hello, world".into())
        .unwrap())
}

struct App {}

impl Application for App {
    type ResponseBody = Body;
    type Error = Error;

    fn error(&self, _: Error) -> Response<Body> {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("ERROR".into())
            .unwrap()
    }
}

#[derive(Debug)]
enum Error {}
