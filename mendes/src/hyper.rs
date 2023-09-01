use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

use async_trait::async_trait;
use futures_util::future::{ready, CatchUnwind, FutureExt, Map, Ready};
use http::request::Parts;
use http::{Request, Response, StatusCode};
use hyper::server::conn::AddrStream;
use hyper::service::Service;

use super::Application;
use crate::application::{Context, FromContext, PathState};

pub use hyper::Body;

/// Extension trait for creating a `hyper::Server`
#[async_trait]
pub trait HyperApplicationExt: Application {
    /// Returns a `hyper::service::MakeService` compatible type limited to `AddrStream` targets
    ///
    /// This can be used to start a hyper server using `hyper::server::Builder::serve`.
    ///
    /// # Example
    /// ```
    /// # async fn run() {
    /// # use hyper::Body;
    /// # use mendes::hyper::HyperApplicationExt;
    /// # use async_trait::async_trait;
    /// # use std::net::ToSocketAddrs;
    /// struct App;

    /// #[async_trait]
    /// impl mendes::Application for App {
    ///     type RequestBody = Body;
    ///     type ResponseBody = Body;
    ///     type Error = mendes::Error;

    ///     async fn handle(_ctx: mendes::Context<Self>) -> http::Response<Self::ResponseBody> {
    ///         http::Response::new(hyper::Body::empty())
    ///     }
    /// }
    ///
    /// hyper::Server::bind(&"127.0.0.1:0".to_socket_addrs().unwrap().next().unwrap())
    ///   .serve(App.into_hyper_service());
    /// # }
    /// # fn main() {
    /// # tokio::runtime::Builder::new_current_thread().enable_io().build().unwrap().block_on(async {run().await});
    /// # }
    /// ```
    ///
    /// mendes previously provided serve_with_graceful_shutdown.
    /// To construct a server with graceful shutdown, use the following pattern:
    ///
    ///  ```
    /// # async fn run() {
    /// # use hyper::Body;
    /// # use mendes::hyper::HyperApplicationExt;
    /// # use async_trait::async_trait;
    /// # use std::net::ToSocketAddrs;
    /// struct App;

    /// #[async_trait]
    /// impl mendes::Application for App {
    ///     type RequestBody = Body;
    ///     type ResponseBody = Body;
    ///     type Error = mendes::Error;

    ///     async fn handle(_ctx: mendes::Context<Self>) -> http::Response<Self::ResponseBody> {
    ///         http::Response::new(hyper::Body::empty())
    ///     }
    /// }
    /// let (shutdown, signal) = tokio::sync::oneshot::channel();
    /// hyper::Server::bind(&"127.0.0.1:0".to_socket_addrs().unwrap().next().unwrap())
    ///   .serve(App.into_hyper_service()).with_graceful_shutdown(async {
    ////       signal.await.ok();
    ///    });
    /// shutdown.send(()).unwrap()
    /// # }
    /// # fn main() {
    /// # tokio::runtime::Builder::new_current_thread().enable_io().build().unwrap().block_on(async {run().await});
    /// # }
    /// ```
    fn into_hyper_service(self) -> MakeServiceFn<Self>;
}

impl<T: Application> HyperApplicationExt for T {
    fn into_hyper_service(self) -> MakeServiceFn<Self> {
        MakeServiceFn {
            app: Arc::new(self),
        }
    }
}

#[derive(Clone)]
pub struct MakeServiceFn<A> {
    app: Arc<A>,
}

impl<'t, A> Service<&'t AddrStream> for MakeServiceFn<A> {
    type Response = ConnectionService<A>;
    type Error = Infallible;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, target: &'t AddrStream) -> Self::Future {
        ready(Ok(ConnectionService {
            app: self.app.clone(),
            addr: target.remote_addr(),
        }))
    }
}

pub struct ConnectionService<A> {
    app: Arc<A>,
    addr: SocketAddr,
}

impl<A> Service<Request<Body>> for ConnectionService<A>
where
    A: Application<RequestBody = Body, ResponseBody = Body> + 'static,
{
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = UnwindSafeHandlerFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        req.extensions_mut().insert(ClientAddr(self.addr));
        let cx = Context::new(self.app.clone(), req);
        AssertUnwindSafe(A::handle(cx))
            .catch_unwind()
            .map(panic_response)
    }
}

type UnwindSafeHandlerFuture<T, E> = Map<
    CatchUnwind<AssertUnwindSafe<Pin<Box<dyn Future<Output = T> + Send>>>>,
    fn(Result<T, Box<(dyn std::any::Any + std::marker::Send + 'static)>>) -> Result<T, E>,
>;

fn panic_response(
    result: Result<Response<Body>, Box<dyn std::any::Any + std::marker::Send + 'static>>,
) -> Result<Response<Body>, Infallible> {
    let error = match result {
        Ok(rsp) => return Ok(rsp),
        Err(e) => e,
    };

    #[cfg(feature = "tracing")]
    {
        let panic_str = if let Some(s) = error.downcast_ref::<String>() {
            Some(s.as_str())
        } else if let Some(s) = error.downcast_ref::<&'static str>() {
            Some(*s)
        } else {
            Some("no error")
        };

        tracing::error!("caught panic from request handler: {:?}", panic_str);
    }

    Ok(Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body("Caught panic".into())
        .unwrap())
}

impl<'a, A: Application> FromContext<'a, A> for Body
where
    A: Application<RequestBody = Body>,
{
    fn from_context(
        _: &'a Arc<A>,
        _: &'a Parts,
        _: &mut PathState,
        body: &mut Option<Body>,
    ) -> Result<Self, A::Error> {
        match body.take() {
            Some(body) => Ok(body),
            None => panic!("attempted to retrieve body twice"),
        }
    }
}

impl<'a, A: Application> FromContext<'a, A> for ClientAddr
where
    A: Application<RequestBody = Body>,
{
    fn from_context(
        _: &'a Arc<A>,
        req: &'a Parts,
        _: &mut PathState,
        _: &mut Option<Body>,
    ) -> Result<Self, A::Error> {
        // This is safe because we insert ClientAddr into the request extensions
        // unconditionally in the ConnectionService::call method.
        Ok(req.extensions.get::<ClientAddr>().copied().unwrap())
    }
}

#[cfg(feature = "compression")]
#[cfg_attr(docsrs, doc(cfg(feature = "compression")))]
mod encoding {
    use std::str::FromStr;
    use std::{io, mem};

    #[cfg(feature = "brotli")]
    use async_compression::tokio::bufread::BrotliEncoder;
    #[cfg(feature = "deflate")]
    use async_compression::tokio::bufread::DeflateEncoder;
    #[cfg(feature = "gzip")]
    use async_compression::tokio::bufread::GzipEncoder;
    use futures_util::stream::TryStreamExt;
    use http::header::{HeaderValue, ACCEPT_ENCODING, CONTENT_ENCODING};
    use http::Response;
    use tokio_util::codec::{BytesCodec, FramedRead};
    use tokio_util::io::StreamReader;

    use super::*;

    pub fn encode_content(req: &Parts, mut rsp: Response<Body>) -> Response<Body> {
        let accept = match req.headers.get(ACCEPT_ENCODING).map(|hv| hv.to_str()) {
            Some(Ok(accept)) => accept,
            _ => return rsp,
        };

        let mut encodings = accept
            .split(',')
            .filter_map(|s| {
                let mut parts = s.splitn(2, ';');
                let alg = match Encoding::from_str(parts.next()?.trim()) {
                    Ok(encoding) => encoding,
                    Err(()) => return None,
                };

                let qual = parts
                    .next()
                    .and_then(|s| {
                        let mut parts = s.splitn(2, '=');
                        if parts.next()?.trim() != "q" {
                            return None;
                        }

                        let value = parts.next()?;
                        f64::from_str(value).ok()
                    })
                    .unwrap_or(1.0);

                Some((alg, (qual * 100.0) as u64))
            })
            .collect::<Vec<_>>();
        encodings.sort_by_key(|(algo, qual)| (-(*qual as i64), *algo));

        match encodings.first().map(|v| v.0) {
            #[cfg(feature = "brotli")]
            Some(Encoding::Brotli) => {
                let orig = mem::replace(rsp.body_mut(), Body::empty());
                rsp.headers_mut()
                    .insert(CONTENT_ENCODING, HeaderValue::from_static("br"));
                *rsp.body_mut() = Body::wrap_stream(FramedRead::new(
                    BrotliEncoder::new(StreamReader::new(
                        orig.map_err(|e| io::Error::new(io::ErrorKind::Other, e)),
                    )),
                    BytesCodec::new(),
                ));
                rsp
            }
            #[cfg(feature = "gzip")]
            Some(Encoding::Gzip) => {
                rsp.headers_mut()
                    .insert(CONTENT_ENCODING, HeaderValue::from_static("gzip"));
                let orig = mem::replace(rsp.body_mut(), Body::empty());
                *rsp.body_mut() = Body::wrap_stream(FramedRead::new(
                    GzipEncoder::new(StreamReader::new(
                        orig.map_err(|e| io::Error::new(io::ErrorKind::Other, e)),
                    )),
                    BytesCodec::new(),
                ));
                rsp
            }
            #[cfg(feature = "deflate")]
            Some(Encoding::Deflate) => {
                rsp.headers_mut()
                    .insert(CONTENT_ENCODING, HeaderValue::from_static("deflate"));
                let orig = mem::replace(rsp.body_mut(), Body::empty());
                *rsp.body_mut() = Body::wrap_stream(FramedRead::new(
                    DeflateEncoder::new(StreamReader::new(
                        orig.map_err(|e| io::Error::new(io::ErrorKind::Other, e)),
                    )),
                    BytesCodec::new(),
                ));
                rsp
            }
            Some(Encoding::Identity) | None => rsp,
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
    enum Encoding {
        #[cfg(feature = "brotli")]
        Brotli,
        #[cfg(feature = "gzip")]
        Gzip,
        #[cfg(feature = "deflate")]
        Deflate,
        Identity,
    }

    impl FromStr for Encoding {
        type Err = ();

        fn from_str(s: &str) -> Result<Encoding, ()> {
            Ok(match s {
                "identity" => Encoding::Identity,
                #[cfg(feature = "gzip")]
                "gzip" => Encoding::Gzip,
                #[cfg(feature = "deflate")]
                "deflate" => Encoding::Deflate,
                #[cfg(feature = "brotli")]
                "br" => Encoding::Brotli,
                _ => return Err(()),
            })
        }
    }
}

#[cfg(feature = "compression")]
#[cfg_attr(docsrs, doc(cfg(feature = "application")))]
pub use encoding::encode_content;

#[derive(Debug, Clone, Copy)]
pub struct ClientAddr(SocketAddr);

impl std::ops::Deref for ClientAddr {
    type Target = SocketAddr;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<SocketAddr> for ClientAddr {
    fn from(addr: SocketAddr) -> Self {
        Self(addr)
    }
}
