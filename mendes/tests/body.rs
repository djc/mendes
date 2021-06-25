#![cfg(all(feature = "application", feature = "http-body", feature = "hyper"))]

use std::sync::Arc;

use async_trait::async_trait;
use mendes::application::Responder;
use mendes::http::request::Parts;
use mendes::http::{Method, Request, Response, StatusCode};
use mendes::hyper::Body;
use mendes::{handler, route, Application};

#[cfg(feature = "serde-derive")]
#[tokio::test]
async fn test_json_decode() {
    let rsp = handle(path_request("/sum", "[1, 2, 3]")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(rsp.into_body(), "6");
}

fn path_request(path: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(format!("https://example.com{}", path))
        .header("Content-Type", "application/json; charset=utf-8")
        .body(body.to_owned().into())
        .unwrap()
}

async fn handle(req: Request<Body>) -> Response<String> {
    Arc::new(App {}).handle(req).await
}

struct App {}

#[async_trait]
impl Application for App {
    type RequestBody = Body;
    type ResponseBody = String;
    type Error = Error;

    #[route]
    async fn handle(
        self: Arc<App>,
        req: Request<Self::RequestBody>,
    ) -> Response<Self::ResponseBody> {
        path! {
            #[cfg(feature = "json")]
            Some("sum") => sum,
        }
    }
}

#[cfg(feature = "json")]
#[handler(POST)]
async fn sum(_: &App, req: &Parts, body: Body) -> Result<Response<String>, Error> {
    let numbers = App::from_body::<Vec<f32>>(req, body).await.unwrap();
    Ok(Response::builder()
        .body(numbers.iter().sum::<f32>().to_string())
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
    fn into_response(self, _: &App, _: &Parts) -> Response<String> {
        let Error::Mendes(err) = self;
        Response::builder()
            .status(StatusCode::from(&err))
            .body(err.to_string())
            .unwrap()
    }
}
