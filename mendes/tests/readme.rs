#![cfg(all(feature = "application", feature = "hyper", feature = "body-util"))]

use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::Full;
use mendes::application::IntoResponse;
use mendes::http::request::Parts;
use mendes::http::{Response, StatusCode};
use mendes::hyper::body::Incoming;
use mendes::{handler, route, Application, Context};

#[handler(GET)]
async fn hello(_: &App) -> Result<Response<Full<Bytes>>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body("Hello, world".into())
        .unwrap())
}

struct App {}

#[async_trait]
impl Application for App {
    type RequestBody = Incoming;
    type ResponseBody = Full<Bytes>;
    type Error = Error;

    async fn handle(mut cx: Context<Self>) -> Response<Full<Bytes>> {
        route!(match cx.path() {
            _ => hello,
        })
    }
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
    fn into_response(self, _: &App, _: &Parts) -> Response<Full<Bytes>> {
        let Error::Mendes(err) = self;
        Response::builder()
            .status(StatusCode::from(&err))
            .body(Full::new(Bytes::from(err.to_string())))
            .unwrap()
    }
}
