#![cfg(all(feature = "application", feature = "hyper"))]

use async_trait::async_trait;
use hyper::Body;
use mendes::application::Responder;
use mendes::http::request::Parts;
use mendes::http::{Response, StatusCode};
use mendes::{handler, route, Application, Context};

#[handler(GET)]
async fn hello(_: &App) -> Result<Response<Body>, Error> {
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

    async fn handle(mut cx: Context<Self>) -> Response<Body> {
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

impl Responder<App> for Error {
    fn into_response(self, _: &App, _: &Parts) -> Response<Body> {
        let Error::Mendes(err) = self;
        Response::builder()
            .status(StatusCode::from(&err))
            .body(err.to_string().into())
            .unwrap()
    }
}
