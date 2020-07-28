use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use http::request::Parts;
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};

use super::{Application, Context};
use crate::application::{FromContext, PathState, Responder, Server};

pub use hyper::Body;

#[async_trait]
impl<A> Server for A
where
    A: Application<RequestBody = Body, ResponseBody = Body> + Send + Sync + 'static,
{
    type ServerError = hyper::Error;

    async fn serve(self: Self, addr: &SocketAddr) -> Result<(), hyper::Error> {
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
                            let mut cx = Context::new(app, req);
                            Ok::<_, Infallible>(match cx.app.prepare(&mut cx.req).await {
                                Ok(()) => A::handle(cx).await,
                                Err(e) => e.into_response(&cx.app),
                            })
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

pub struct ClientAddr(SocketAddr);

impl std::ops::Deref for ClientAddr {
    type Target = SocketAddr;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
