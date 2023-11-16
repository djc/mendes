use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::task::ready;
use std::task::Poll;
use std::{io, mem, str};

#[cfg(feature = "brotli")]
use async_compression::tokio::bufread::BrotliEncoder;
#[cfg(feature = "deflate")]
use async_compression::tokio::bufread::DeflateEncoder;
#[cfg(feature = "gzip")]
use async_compression::tokio::bufread::GzipEncoder;
use bytes::{Buf, Bytes, BytesMut};
#[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
use http::header::{ACCEPT_ENCODING, CONTENT_ENCODING};
#[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
use http::HeaderMap;
#[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
use http::{request, HeaderValue, Response};
use http_body::{Frame, SizeHint};
use pin_project::pin_project;
#[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
use tokio::io::{AsyncBufRead, AsyncRead, ReadBuf};
#[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
use tokio_util::io::poll_read_buf;

#[pin_project]
pub struct Body {
    #[pin]
    inner: InnerBody,
    full_size: u64,
    done: bool,
}

impl Body {
    pub fn empty() -> Self {
        Self {
            inner: InnerBody::Bytes(Bytes::new()),
            full_size: 0,
            done: true,
        }
    }

    pub fn lazy(future: impl Future<Output = io::Result<Bytes>> + Send + 'static) -> Self {
        Self {
            inner: InnerBody::Lazy {
                future: Box::pin(future),
                encoding: Encoding::Identity,
            },
            full_size: 0,
            done: false,
        }
    }

    pub fn stream(
        stream: impl http_body::Body<Data = Bytes, Error = io::Error> + Send + 'static,
    ) -> Self {
        Self {
            inner: InnerBody::Streaming(Box::pin(stream)),
            full_size: 0,
            done: false,
        }
    }
}

impl http_body::Body for Body {
    type Data = Bytes;
    type Error = io::Error;

    #[allow(unused_variables)] // Depends on features
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        if *this.done {
            return Poll::Ready(None);
        }

        #[allow(unused_mut)] // Depends on features
        let mut buf = BytesMut::new();
        let result = match this.inner.project() {
            #[cfg(feature = "brotli")]
            PinnedBody::Brotli(encoder) => poll_read_buf(encoder, cx, &mut buf),
            #[cfg(feature = "deflate")]
            PinnedBody::Deflate(encoder) => poll_read_buf(encoder, cx, &mut buf),
            #[cfg(feature = "gzip")]
            PinnedBody::Gzip(encoder) => poll_read_buf(encoder, cx, &mut buf),
            PinnedBody::Bytes(bytes) => {
                *this.done = true;
                let bytes = mem::take(bytes.get_mut());
                return Poll::Ready(match bytes.has_remaining() {
                    true => Some(Ok(Frame::data(bytes))),
                    false => None,
                });
            }
            PinnedBody::Streaming(inner) => match ready!(inner.as_mut().poll_frame(cx)) {
                Some(item) => return Poll::Ready(Some(item)),
                None => {
                    *this.done = true;
                    return Poll::Ready(None);
                }
            },
            PinnedBody::Lazy { future, encoding } => {
                let bytes = match ready!(future.as_mut().poll(cx)) {
                    Ok(bytes) => bytes,
                    Err(error) => return Poll::Ready(Some(Err(error))),
                };

                let len = bytes.len();
                let mut inner = InnerBody::wrap(bytes, *encoding);
                *this.full_size = len as u64;
                // The duplication here is pretty ugly, but I couldn't come up with anything better.
                match &mut inner {
                    #[cfg(feature = "brotli")]
                    InnerBody::Brotli(encoder) => poll_read_buf(Pin::new(encoder), cx, &mut buf),
                    #[cfg(feature = "deflate")]
                    InnerBody::Deflate(encoder) => poll_read_buf(Pin::new(encoder), cx, &mut buf),
                    #[cfg(feature = "gzip")]
                    InnerBody::Gzip(encoder) => poll_read_buf(Pin::new(encoder), cx, &mut buf),
                    InnerBody::Bytes(bytes) => {
                        *this.done = true;
                        let bytes = mem::take(bytes);
                        return Poll::Ready(match bytes.has_remaining() {
                            true => Some(Ok(Frame::data(bytes))),
                            false => None,
                        });
                    }
                    InnerBody::Lazy { .. } | InnerBody::Streaming(_) => unreachable!(),
                }
            }
        };

        #[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
        match ready!(result) {
            Ok(0) => {
                *this.done = true;
                Poll::Ready(None)
            }
            Ok(n) => {
                *this.full_size = this.full_size.saturating_sub(n as u64);
                Poll::Ready(Some(Ok(Frame::data(buf.freeze()))))
            }
            Err(error) => Poll::Ready(Some(Err(error))),
        }
    }

    fn is_end_stream(&self) -> bool {
        self.done
    }

    fn size_hint(&self) -> http_body::SizeHint {
        match (self.done, &self.inner) {
            (true, _) => SizeHint::with_exact(0),
            (false, InnerBody::Bytes(body)) => SizeHint::with_exact(body.len() as u64),
            (false, InnerBody::Lazy { .. } | InnerBody::Streaming(_)) => SizeHint::default(),
            #[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
            (false, InnerBody::Brotli(_) | InnerBody::Deflate(_) | InnerBody::Gzip(_)) => {
                let mut hint = SizeHint::default();
                hint.set_lower(1);
                hint.set_upper(self.full_size + 256);
                hint
            }
        }
    }
}

impl From<Vec<u8>> for Body {
    fn from(data: Vec<u8>) -> Self {
        Self::from(Bytes::from(data))
    }
}

impl From<String> for Body {
    fn from(data: String) -> Self {
        Self::from(Bytes::from(data))
    }
}

impl From<&'static str> for Body {
    fn from(data: &'static str) -> Self {
        Self::from(Bytes::from(data))
    }
}

impl From<Bytes> for Body {
    fn from(data: Bytes) -> Self {
        Self {
            done: !data.has_remaining(),
            full_size: data.len() as u64,
            inner: InnerBody::Bytes(data),
        }
    }
}

#[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
impl EncodeResponse for Response<Body> {
    fn encoded(mut self, req: &request::Parts) -> Response<Body> {
        let buf = match self.body_mut() {
            Body { done: true, .. } => return self,
            Body {
                inner: InnerBody::Bytes(buf),
                ..
            } => mem::take(buf),
            Body {
                inner:
                    InnerBody::Lazy {
                        encoding: enc @ Encoding::Identity,
                        ..
                    },
                ..
            } => {
                let new = Encoding::from_accept(&req.headers).unwrap_or(Encoding::Identity);
                *enc = new;
                return self;
            }
            Body {
                inner:
                    InnerBody::Brotli(_)
                    | InnerBody::Deflate(_)
                    | InnerBody::Gzip(_)
                    | InnerBody::Lazy { .. }
                    | InnerBody::Streaming(_),
                ..
            } => return self,
        };

        let len = buf.len();
        let encoding = Encoding::from_accept(&req.headers).unwrap_or(Encoding::Identity);
        let inner = InnerBody::wrap(buf, encoding);
        if let Some(encoding) = encoding.as_str() {
            self.headers_mut()
                .insert(CONTENT_ENCODING, HeaderValue::from_static(encoding));
        }

        let body = self.body_mut();
        body.full_size = len as u64;
        body.inner = inner;
        self
    }
}

#[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
pub trait EncodeResponse {
    fn encoded(self, req: &request::Parts) -> Self;
}

#[pin_project(project = PinnedBody)]
enum InnerBody {
    #[cfg(feature = "brotli")]
    Brotli(#[pin] BrotliEncoder<BufReader>),
    #[cfg(feature = "deflate")]
    Deflate(#[pin] DeflateEncoder<BufReader>),
    #[cfg(feature = "gzip")]
    Gzip(#[pin] GzipEncoder<BufReader>),
    Bytes(#[pin] Bytes),
    Lazy {
        future: Pin<Box<dyn Future<Output = io::Result<Bytes>> + Send>>,
        encoding: Encoding,
    },
    Streaming(Pin<Box<dyn http_body::Body<Data = Bytes, Error = io::Error> + Send>>),
}

impl InnerBody {
    fn wrap(buf: Bytes, encoding: Encoding) -> Self {
        match encoding {
            #[cfg(feature = "brotli")]
            Encoding::Brotli => Self::Brotli(BrotliEncoder::new(BufReader { buf })),
            #[cfg(feature = "deflate")]
            Encoding::Deflate => Self::Deflate(DeflateEncoder::new(BufReader { buf })),
            #[cfg(feature = "gzip")]
            Encoding::Gzip => Self::Gzip(GzipEncoder::new(BufReader { buf })),
            Encoding::Identity => Self::Bytes(buf),
        }
    }
}

#[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
struct BufReader {
    pub(crate) buf: Bytes,
}

#[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
impl AsyncBufRead for BufReader {
    fn poll_fill_buf(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> Poll<io::Result<&[u8]>> {
        Poll::Ready(Ok(self.get_mut().buf.chunk()))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        self.get_mut().buf.advance(amt);
    }
}

#[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
impl AsyncRead for BufReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let len = Ord::min(self.buf.remaining(), buf.remaining());
        self.get_mut()
            .buf
            .copy_to_slice(buf.initialize_unfilled_to(len));
        Poll::Ready(Ok(()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
enum Encoding {
    #[cfg(feature = "brotli")]
    Brotli,
    #[cfg(feature = "deflate")]
    Deflate,
    #[cfg(feature = "gzip")]
    Gzip,
    Identity,
}

impl Encoding {
    #[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
    fn from_accept(headers: &HeaderMap) -> Option<Self> {
        let accept = match headers.get(ACCEPT_ENCODING).map(|hv| hv.to_str()) {
            Some(Ok(accept)) => accept,
            _ => return None,
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

        encodings.into_iter().next().map(|(algo, _)| algo)
    }
}

impl Encoding {
    #[cfg(any(feature = "brotli", feature = "deflate", feature = "gzip"))]
    pub fn as_str(self) -> Option<&'static str> {
        match self {
            #[cfg(feature = "brotli")]
            Self::Brotli => Some("br"),
            #[cfg(feature = "deflate")]
            Self::Deflate => Some("deflate"),
            #[cfg(feature = "gzip")]
            Self::Gzip => Some("gzip"),
            Self::Identity => None,
        }
    }
}

impl FromStr for Encoding {
    type Err = ();

    fn from_str(s: &str) -> Result<Encoding, ()> {
        Ok(match s {
            #[cfg(feature = "brotli")]
            "br" => Encoding::Brotli,
            #[cfg(feature = "deflate")]
            "deflate" => Encoding::Deflate,
            #[cfg(feature = "gzip")]
            "gzip" => Encoding::Gzip,
            "identity" => Encoding::Identity,
            _ => return Err(()),
        })
    }
}
