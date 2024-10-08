#![cfg(all(feature = "application", feature = "hyper", feature = "body-util"))]

use std::sync::Arc;

#[cfg(all(feature = "compression", feature = "deflate"))]
use async_compression::tokio::write::ZlibDecoder;
use async_trait::async_trait;
use http::header::{ACCEPT_ENCODING, CONTENT_TYPE};
use http_body_util::BodyExt;
#[cfg(all(feature = "compression", feature = "deflate"))]
use tokio::io::AsyncWriteExt;

use mendes::application::IntoResponse;
#[cfg(feature = "compression")]
use mendes::body::EncodeResponse;
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

#[cfg(all(feature = "compression", feature = "deflate"))]
#[tokio::test]
async fn test_deflate_compression() {
    let rsp = handle(path_request("/echo", "hello world", Some("deflate"))).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    let body = rsp.into_body().collect().await.unwrap().to_bytes();
    // If the lower half of the first byte is 0x08, then the stream is
    // a zlib stream, otherwise it's a
    // raw deflate stream.
    assert_eq!(body[0] & 0x0F, 0x8);

    // Decode as Zlib container
    let mut decoder = ZlibDecoder::new(Vec::new());
    decoder.write_all(&body).await.unwrap();
    decoder.shutdown().await.unwrap();
    assert_eq!(
        String::from_utf8_lossy(&decoder.into_inner()),
        "hello world"
    );
}

fn path_request(path: &str, body: &str, compression: Option<&'static str>) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(format!("https://example.com{path}"))
        .header(CONTENT_TYPE, "application/json; charset=utf-8");
    if let Some(compression) = compression {
        request = request.header(ACCEPT_ENCODING, compression);
    }
    request.body(body.to_owned().into()).unwrap()
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
        let response = route!(match cx.path() {
            #[cfg(feature = "json")]
            Some("sum") => sum,
            Some("echo") => echo,
        });

        #[cfg(feature = "compression")]
        let response = response.encoded(&cx.req);

        response
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

#[handler(POST)]
async fn echo(_: &App, _req: &Parts, body: Body) -> Result<Response<Body>, Error> {
    let content = App::body_bytes(body, 100).await.unwrap();
    Ok(Response::builder().body(content.into()).unwrap())
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
