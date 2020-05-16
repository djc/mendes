pub use http;
pub use mendes_macros::{dispatch, handler};

#[cfg(feature = "application")]
pub mod application;
#[cfg(feature = "application")]
pub use application::{from_body_bytes, Application, ClientError, Context, FromContext};

#[cfg(feature = "models")]
pub mod models;

#[cfg(feature = "cookies")]
pub mod cookies;

#[cfg(feature = "forms")]
mod form;

#[cfg(feature = "uploads")]
mod multipart;

#[cfg(feature = "forms")]
pub mod forms {
    pub use super::form::*;
    #[cfg(feature = "uploads")]
    pub use super::multipart::{from_form_data, File};
}

#[cfg(feature = "hyper")]
pub mod hyper {
    use std::convert::Infallible;
    use std::net::SocketAddr;
    use std::sync::Arc;

    use async_trait::async_trait;
    use futures_util::future::FutureExt;
    use hyper::service::{make_service_fn, service_fn};
    use hyper::Body;

    use super::{Application, Context};
    use crate::application::Server;

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
}

pub mod types {
    pub const HTML: &str = "text/html";
    pub const JSON: &str = "application/json";
}
