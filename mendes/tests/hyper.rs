#![cfg(feature = "hyper")]

use std::fmt::{self, Display};
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use mendes::application::IntoResponse;
use mendes::http::request::Parts;
use mendes::http::{Response, StatusCode};
use mendes::hyper::body::Incoming;
use mendes::hyper::{ClientAddr, Server};
use mendes::{handler, route, Application, Body, Context};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::sleep;

struct ServerRunner {
    handle: JoinHandle<Result<(), io::Error>>,
}

impl ServerRunner {
    async fn run(addr: SocketAddr) -> Self {
        let listener = TcpListener::bind(addr).await.unwrap();
        let handle = tokio::spawn(Server::new(listener, App::default()).serve());
        sleep(Duration::from_millis(10)).await;
        Self { handle }
    }

    fn stop(self) {
        self.handle.abort();
    }
}

impl Drop for ServerRunner {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

#[tokio::test]
async fn test_client_addr() {
    let addr = "127.0.0.1:12345".parse::<SocketAddr>().unwrap();
    let runner = ServerRunner::run(addr).await;

    let rsp = reqwest::get(format!("http://{addr}/client-addr"))
        .await
        .unwrap();
    assert_eq!(rsp.status(), StatusCode::OK);

    let body = rsp.text().await.unwrap();
    assert_eq!(body, "client_addr: 127.0.0.1");

    runner.stop();
}

#[derive(Default)]
struct App {}

#[async_trait]
impl Application for App {
    type RequestBody = Incoming;
    type ResponseBody = Body;
    type Error = Error;

    async fn handle(mut cx: Context<Self>) -> Response<Self::ResponseBody> {
        route!(match cx.path() {
            Some("client-addr") => client_addr,
        })
    }
}

#[handler(GET)]
async fn client_addr(_: &App, client_addr: ClientAddr) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(Bytes::from(format!(
            "client_addr: {}",
            client_addr.ip()
        ))))
        .unwrap())
}

#[derive(Debug)]
enum Error {
    Mendes(mendes::Error),
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Mendes(err) => err.fmt(formatter),
        }
    }
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
            .body(Body::from(Bytes::from(err.to_string())))
            .unwrap()
    }
}
