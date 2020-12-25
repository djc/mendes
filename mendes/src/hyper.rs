use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use http::request::Parts;
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};

use super::Application;
use crate::application::{FromContext, PathState, Server};

pub use hyper::Body;

#[async_trait]
impl<A> Server for A
where
    A: Application<RequestBody = Body, ResponseBody = Body> + Send + Sync + 'static,
{
    type ServerError = hyper::Error;

    async fn serve(self, addr: &SocketAddr) -> Result<(), hyper::Error> {
        let app = Arc::new(self);
        hyper::Server::bind(addr)
            .serve(make_service_fn(move |addr: &AddrStream| {
                let addr = addr.remote_addr();
                let app = app.clone();
                async move {
                    Ok::<_, Infallible>(service_fn(move |mut req| {
                        let app = app.clone();
                        async move {
                            req.extensions_mut().insert(ClientAddr(addr));
                            Ok::<_, Infallible>(app.handle(req).await)
                        }
                    }))
                }
            }))
            .await
    }
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

#[cfg(feature = "compression")]
#[cfg_attr(docsrs, doc(cfg(feature = "compression")))]
mod encoding {
    use std::str::FromStr;
    use std::{io, mem};

    #[cfg(feature = "brotli")]
    use async_compression::stream::BrotliEncoder;
    #[cfg(feature = "deflate")]
    use async_compression::stream::DeflateEncoder;
    #[cfg(feature = "gzip")]
    use async_compression::stream::GzipEncoder;
    use futures_util::stream::TryStreamExt;
    use http::header::{HeaderValue, ACCEPT_ENCODING, CONTENT_ENCODING};
    use http::Response;

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
                        Some(f64::from_str(value).ok()?)
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
                *rsp.body_mut() = Body::wrap_stream(BrotliEncoder::new(
                    orig.map_err(|e| io::Error::new(io::ErrorKind::Other, e)),
                ));
                rsp
            }
            #[cfg(feature = "gzip")]
            Some(Encoding::Gzip) => {
                rsp.headers_mut()
                    .insert(CONTENT_ENCODING, HeaderValue::from_static("gzip"));
                let orig = mem::replace(rsp.body_mut(), Body::empty());
                *rsp.body_mut() = Body::wrap_stream(GzipEncoder::new(
                    orig.map_err(|e| io::Error::new(io::ErrorKind::Other, e)),
                ));
                rsp
            }
            #[cfg(feature = "deflate")]
            Some(Encoding::Deflate) => {
                rsp.headers_mut()
                    .insert(CONTENT_ENCODING, HeaderValue::from_static("deflate"));
                let orig = mem::replace(rsp.body_mut(), Body::empty());
                *rsp.body_mut() = Body::wrap_stream(DeflateEncoder::new(
                    orig.map_err(|e| io::Error::new(io::ErrorKind::Other, e)),
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

pub struct ClientAddr(SocketAddr);

impl std::ops::Deref for ClientAddr {
    type Target = SocketAddr;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
