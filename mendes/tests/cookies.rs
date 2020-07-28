#![cfg(feature = "cookies")]

use std::convert::TryInto;
use std::sync::Arc;

use async_trait::async_trait;
use mendes::cookies::{cookie, AppWithAeadKey, AppWithCookies, Key};
use mendes::http::header::{COOKIE, SET_COOKIE};
use mendes::http::{Request, Response, StatusCode};
use mendes::{dispatch, get, Application, ClientError, Context};
use serde::{Deserialize, Serialize};

#[tokio::test]
async fn cookie() {
    let app = Arc::new(App {
        key: mendes::cookies::Key::new(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31,
        ]),
    });

    let rsp = handle(app.clone(), path_request("/store")).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    let set = rsp.headers().get(SET_COOKIE).unwrap();
    let value = set.to_str().unwrap().split(';').next().unwrap();

    let mut req = path_request("/extract");
    req.headers_mut().insert(COOKIE, value.try_into().unwrap());
    let rsp = handle(app.clone(), req).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(rsp.into_body(), "user = 37");
}

fn path_request(path: &str) -> Request<()> {
    Request::builder()
        .uri(format!("https://example.com{}", path))
        .body(())
        .unwrap()
}

async fn handle(app: Arc<App>, req: Request<()>) -> Response<String> {
    let cx = Context::new(app, req);
    App::handle(cx).await
}

struct App {
    key: mendes::cookies::Key,
}

impl AppWithAeadKey for App {
    fn key(&self) -> &Key {
        &self.key
    }
}

#[async_trait]
impl Application for App {
    type RequestBody = ();
    type ResponseBody = String;
    type Error = Error;

    #[dispatch]
    async fn handle(mut cx: Context<Self>) -> Response<Self::ResponseBody> {
        path! {
            Some("store") => store,
            Some("extract") => extract,
        }
    }

    fn error(&self, err: Error) -> Response<Self::ResponseBody> {
        let Error::Client(err) = err;
        Response::builder()
            .status(StatusCode::from(err))
            .body(err.to_string())
            .unwrap()
    }
}

#[get]
async fn extract(app: &App, req: &http::request::Parts) -> Result<Response<String>, Error> {
    let session = app.cookie::<Session>(&req.headers).unwrap();
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("user = {}", session.user))
        .unwrap())
}

#[get]
async fn store(app: &App) -> Result<Response<String>, Error> {
    let session = Session { user: 37 };
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(SET_COOKIE, app.set_cookie_header(Some(session)).unwrap())
        .body("Hello, world".into())
        .unwrap())
}

#[cookie]
#[derive(Deserialize, Serialize)]
struct Session {
    user: i32,
}

#[derive(Debug)]
enum Error {
    Client(ClientError),
}

impl From<ClientError> for Error {
    fn from(e: ClientError) -> Self {
        Error::Client(e)
    }
}
