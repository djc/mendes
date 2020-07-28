#![cfg(feature = "application")]

use async_trait::async_trait;
use hyper::Body;
use mendes::application::Responder;
use mendes::http::{Response, StatusCode};
use mendes::{get, route, Application, ClientError, Context};

#[get]
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

    #[route]
    async fn handle(mut cx: Context<Self>) -> Response<Body> {
        path! {
            _ => hello,
        }
    }
}

#[derive(Debug)]
enum Error {
    Client(ClientError),
}

impl From<ClientError> for Error {
    fn from(e: ClientError) -> Error {
        Error::Client(e)
    }
}

impl Responder<App> for Error {
    fn into_response(self, _: &App) -> Response<Body> {
        let Error::Client(err) = self;
        Response::builder()
            .status(StatusCode::from(err))
            .body(err.to_string().into())
            .unwrap()
    }
}
