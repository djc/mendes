use std::sync::Arc;

use http::{Request, Response, StatusCode};
use hyper::Body;
use mendes::{Application, Context};
use mendes_derive::{dispatch, handler};

#[tokio::test]
async fn basic() {
    let req = Request::builder()
        .uri("https://example.com/hello")
        .body(())
        .unwrap();
    let rsp = route(Context::new(Arc::new(App {}), req)).await;
    assert_eq!(rsp.status(), StatusCode::OK);
}

#[dispatch]
async fn route(mut cx: Context<App>) -> Response<Body> {
    route! {
        _ => hello,
    }
}

#[handler(App)]
async fn hello(_: &App, _: Request<()>) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body("Hello, world".into())
        .unwrap())
}

struct App {}

impl Application for App {
    type RequestBody = ();
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
