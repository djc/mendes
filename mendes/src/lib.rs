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

mod form;

#[cfg(feature = "uploads")]
mod multipart;

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

    use futures_util::future::FutureExt;
    use http::{header::LOCATION, Response, StatusCode};
    use hyper::service::{make_service_fn, service_fn};
    use hyper::{Body, Server};

    use super::{Application, Context};

    pub async fn run<A>(addr: &SocketAddr, app: A) -> Result<(), hyper::Error>
    where
        A: Application<RequestBody = Body, ResponseBody = Body> + Send + Sync + 'static,
    {
        let app = Arc::new(app);
        Server::bind(addr)
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

    pub fn redirect(status: StatusCode, path: &str) -> Response<Body> {
        http::Response::builder()
            .status(status)
            .header(LOCATION, path)
            .body(Body::empty())
            .unwrap()
    }
}

pub mod types {
    pub const HTML: &str = "text/html";
    pub const JSON: &str = "application/json";
}
