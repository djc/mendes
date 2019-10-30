use std::sync::Arc;

use http::{Request, Response, StatusCode};
use hyper::Body;
use mendes::Application;
use mendes_derive::dispatch;
use tokio::runtime::current_thread::Runtime;

#[test]
fn basic() {
    let req = Request::builder()
        .uri("https://example.com/hello")
        .body(())
        .unwrap();
    let mut rt = Runtime::new().unwrap();
    let rsp = rt
        .block_on(async { route(Arc::new(App {}), req).await })
        .unwrap();
    assert_eq!(rsp.status(), StatusCode::OK);
}

#[dispatch]
async fn route(app: Arc<App>, req: Request<()>) -> Result<Response<Body>, Error> {
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
