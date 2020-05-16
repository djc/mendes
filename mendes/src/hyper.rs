use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::future::FutureExt;
use hyper::service::{make_service_fn, service_fn};

use super::{Application, Context};
use crate::application::Server;

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
            .serve(make_service_fn(move |_| {
                let app = app.clone();
                async {
                    Ok::<_, Infallible>(service_fn(move |req| {
                        let cx = Context::new(app.clone(), req);
                        A::handle(cx).map(Ok::<_, Infallible>)
                    }))
                }
            }))
            .await
    }
}
