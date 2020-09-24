#![cfg(feature = "askama")]

use async_trait::async_trait;
use hyper::Body;
use mendes::application::Responder;
use mendes::askama::Template;
use mendes::http::{Response, StatusCode};
use mendes::{get, route, Application, Context};

#[get]
async fn hello(_: &App) -> Result<HelloTemplate<'static>, Error> {
    Ok(HelloTemplate { name: "world" })
}

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate<'a> {
    name: &'a str,
}

struct App {}

#[async_trait]
impl Application for App {
    type RequestBody = ();
    type ResponseBody = Body;
    type Error = Error;

    #[route]
    async fn handle(mut cx: Context<Self>) -> Response<Body> {
        path! {
            _ => hello,
        }
    }
}

#[derive(Debug)]
enum Error {
    Askama(askama::Error),
    Mendes(mendes::Error),
}

impl From<askama::Error> for Error {
    fn from(e: askama::Error) -> Error {
        Error::Askama(e)
    }
}

impl From<mendes::Error> for Error {
    fn from(e: mendes::Error) -> Error {
        Error::Mendes(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Askama(e) => write!(f, "{}", e),
            Error::Mendes(e) => write!(f, "{}", e),
        }
    }
}

impl Responder<App> for Error {
    fn into_response(self, _: &App) -> Response<Body> {
        Response::builder()
            .status(StatusCode::from(&self))
            .body(self.to_string().into())
            .unwrap()
    }
}

impl From<&Error> for StatusCode {
    fn from(e: &Error) -> StatusCode {
        match e {
            Error::Mendes(e) => StatusCode::from(e),
            Error::Askama(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
