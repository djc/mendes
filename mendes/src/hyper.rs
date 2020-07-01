use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::future::FutureExt;
use http::request::Parts;
use hyper::service::{make_service_fn, service_fn};
use hyper::server::conn::AddrStream;

use super::{Application, Context};
use crate::application::{FromContext, PathState, Server};

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
                        req.extensions_mut().insert(ClientAddr(addr.clone()));
                        let cx = Context::new(app.clone(), req);
                        A::handle(cx).map(Ok::<_, Infallible>)
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
