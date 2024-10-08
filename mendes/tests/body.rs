#![cfg(all(feature = "application", feature = "hyper", feature = "body-util"))]

use std::sync::Arc;

use http_body_util::BodyExt;
use async_trait::async_trait;

use mendes::application::IntoResponse;
use mendes::http::request::Parts;
use mendes::http::{Method, Request, Response, StatusCode};
use mendes::{handler, route, Application, Body, Context};

#[cfg(feature = "json")]
#[tokio::test]
async fn test_json_decode() {
    let rsp = handle(path_request("/sum", "[1, 2, 3]", None)).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    let body = rsp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(String::from_utf8_lossy(&body), "6");
}
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(rsp.into_body(), "6");
}

fn path_request(path: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(format!("https://example.com{path}"))
        .header("Content-Type", "application/json; charset=utf-8")
        .body(body.to_owned().into())
        .unwrap()
}

async fn handle(req: Request<Body>) -> Response<Body> {
    App::handle(Context::new(Arc::new(App {}), req)).await
}

struct App {}

#[async_trait]
impl Application for App {
    type RequestBody = Body;
    type ResponseBody = Body;
    type Error = Error;

    async fn handle(mut cx: Context<Self>) -> Response<Self::ResponseBody> {
        route!(match cx.path() {
            #[cfg(feature = "json")]
            Some("sum") => sum,
        })
    }
}

#[cfg(feature = "json")]
#[handler(POST)]
async fn sum(_: &App, req: &Parts, body: Body) -> Result<Response<Body>, Error> {
    let numbers = App::from_body::<Vec<f32>>(req, body, 16).await.unwrap();
    Ok(Response::builder()
        .body(numbers.iter().sum::<f32>().to_string().into())
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

impl IntoResponse<App> for Error {
    fn into_response(self, _: &App, _: &Parts) -> Response<Body> {
        let Error::Mendes(err) = self;
        Response::builder()
            .status(StatusCode::from(&err))
            .body(err.to_string().into())
            .unwrap()
    }
}
