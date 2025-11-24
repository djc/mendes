use std::any::Any;
use std::convert::Infallible;
use std::error::Error as StdError;
use std::future::{pending, Future, Pending};
use std::io;
use std::marker::Send;
use std::net::SocketAddr;
use std::panic::AssertUnwindSafe;
use std::pin::{pin, Pin};
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::{CatchUnwind, FutureExt, Map};
use http::request::Parts;
use http::{Request, Response, StatusCode};
use hyper::body::{Body, Incoming};
use hyper::service::Service;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::time::sleep;
use tracing::{debug, error, info};

use super::Application;
use crate::application::{Context, FromContext, PathState};

pub use hyper::body;

pub struct Server<A, F> {
    listener: TcpListener,
    app: Arc<A>,
    signal: Option<F>,
}

impl<A: Application> Server<A, Pending<()>> {
    pub async fn bind(address: SocketAddr, app: A) -> Result<Server<A, Pending<()>>, io::Error> {
        Ok(Self::new(TcpListener::bind(address).await?, app))
    }

    pub fn new(listener: TcpListener, app: A) -> Server<A, Pending<()>> {
        Server {
            listener,
            app: Arc::new(app),
            signal: None,
        }
    }
}

impl<A: Application> Server<A, Pending<()>> {
    pub fn with_graceful_shutdown<F: Future<Output = ()>>(self, signal: F) -> Server<A, F> {
        let Server { listener, app, .. } = self;
        Server {
            listener,
            app,
            signal: Some(signal),
        }
    }
}

impl<A, F> Server<A, F>
where
    A: Application + Sync + 'static,
    A::RequestBody: From<Incoming>,
    <<A as Application>::ResponseBody as Body>::Data: Send,
    <<A as Application>::ResponseBody as Body>::Error: StdError + Send + Sync,
    <A as Application>::ResponseBody: From<&'static str> + Send,
    F: Future<Output = ()> + Send + 'static,
{
    pub async fn serve(self) -> Result<(), io::Error> {
        let Server {
            listener,
            app,
            signal,
        } = self;

        let (listener_state, conn_state) = states(signal);
        let mut shutting_down = pin!(async move {
            match listener_state.shutting_down {
                Some(shutting_down) => shutting_down.closed().await,
                None => pending().await,
            }
        }
        .fuse());

        loop {
            let (stream, addr) = tokio::select! {
                res = listener.accept() => {
                    match res {
                        Ok((stream, addr)) => (stream, addr),
                        Err(error) => {
                            use io::ErrorKind::*;
                            if matches!(error.kind(), ConnectionRefused | ConnectionAborted | ConnectionReset) {
                                continue;
                            }

                            // Sleep for a bit to see if the error clears
                            error!(%error, "error accepting connection");
                            sleep(Duration::from_secs(1)).await;
                            continue;
                        }
                    }
                }
                _ = shutting_down.as_mut() => break,
            };

            debug!("connection accepted from {addr}");
            tokio::spawn(
                Connection {
                    stream,
                    addr,
                    state: conn_state.clone(),
                    app: app.clone(),
                }
                .run(),
            );
        }

        let ListenerState { task_monitor, .. } = listener_state;
        drop(conn_state);
        drop(listener);
        if let Some(task_monitor) = task_monitor {
            let tasks = task_monitor.receiver_count();
            if tasks > 0 {
                debug!("waiting for {tasks} task(s) to finish");
            }
            task_monitor.closed().await;
        }

        Ok(())
    }
}

fn states(
    future: Option<impl Future<Output = ()> + Send + 'static>,
) -> (ListenerState, ConnectionState) {
    let future = match future {
        Some(future) => future,
        None => return (ListenerState::default(), ConnectionState::default()),
    };

    let (shutting_down, signal) = watch::channel(()); // Axum: `signal_tx`, `signal_rx`
    let shutting_down = Arc::new(shutting_down);
    tokio::spawn(async move {
        future.await;
        info!("shutdown signal received, draining...");
        drop(signal);
    });

    let (task_monitor, task_done) = watch::channel(()); // Axum: `close_tx`, `close_rx`
    (
        ListenerState {
            shutting_down: Some(shutting_down.clone()),
            task_monitor: Some(task_monitor),
        },
        ConnectionState {
            shutting_down: Some(shutting_down),
            _task_done: Some(task_done),
        },
    )
}

#[derive(Default)]
struct ListenerState {
    /// If `Some` and `closed()`, the server is shutting down
    shutting_down: Option<Arc<watch::Sender<()>>>,
    /// If `Some`, `receiver_count()` can be used whether any connections are still going
    ///
    /// Call `closed().await` to wait for all connections to finish.
    task_monitor: Option<watch::Sender<()>>,
}

struct Connection<A> {
    stream: TcpStream,
    addr: SocketAddr,
    state: ConnectionState,
    app: Arc<A>,
}

impl<A: Application + 'static> Connection<A>
where
    A::RequestBody: From<Incoming>,
    A::ResponseBody: From<&'static str> + Send,
    <A::ResponseBody as Body>::Data: Send,
    <A::ResponseBody as Body>::Error: StdError + Send + Sync,
{
    async fn run(self) {
        let Connection {
            stream,
            addr,
            state,
            app,
        } = self;

        let service = ConnectionService { addr, app };

        let builder = Builder::new(TokioExecutor::new());
        let stream = TokioIo::new(stream);
        let mut conn = pin!(builder.serve_connection_with_upgrades(stream, service));
        let mut shutting_down = pin!(async move {
            match state.shutting_down {
                Some(shutting_down) => shutting_down.closed().await,
                None => pending().await,
            }
        }
        .fuse());

        loop {
            tokio::select! {
                result = conn.as_mut() => {
                    if let Err(error) = result {
                        error!(%addr, %error, "failed to serve connection");
                    }
                    break;
                }
                _ = shutting_down.as_mut() => {
                    debug!("shutting down connection to {addr}");
                    conn.as_mut().graceful_shutdown();
                }
            }
        }

        debug!("connection to {addr} closed");
    }
}

#[derive(Clone, Default)]
struct ConnectionState {
    /// If `Some` and `closed()`, the server is shutting down; don't accept new requests
    shutting_down: Option<Arc<watch::Sender<()>>>,
    /// Keeping this around will allow the server to wait for the connection to finish
    _task_done: Option<watch::Receiver<()>>,
}

pub struct ConnectionService<A> {
    addr: SocketAddr,
    app: Arc<A>,
}

impl<A: Application + 'static> Service<Request<Incoming>> for ConnectionService<A>
where
    A::RequestBody: From<Incoming>,
    A::ResponseBody: From<&'static str>,
{
    type Response = Response<A::ResponseBody>;
    type Error = Infallible;
    type Future = UnwindSafeHandlerFuture<Self::Response, Self::Error>;

    fn call(&self, mut req: Request<Incoming>) -> Self::Future {
        req.extensions_mut().insert(ClientAddr(self.addr));
        let cx = Context::new(self.app.clone(), req.map(|body| body.into()));
        AssertUnwindSafe(A::handle(cx))
            .catch_unwind()
            .map(panic_response)
    }
}

type UnwindSafeHandlerFuture<T, E> = Map<
    CatchUnwind<AssertUnwindSafe<Pin<Box<dyn Future<Output = T> + Send>>>>,
    fn(Result<T, Box<dyn Any + Send + 'static>>) -> Result<T, E>,
>;

fn panic_response<B: From<&'static str>>(
    result: Result<Response<B>, Box<dyn Any + Send + 'static>>,
) -> Result<Response<B>, Infallible> {
    #[allow(unused_variables)] // Depends on features
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

impl<'a, A: Application<RequestBody = Incoming>> FromContext<'a, A> for Incoming {
    fn from_context(
        _: &'a Arc<A>,
        _: &'a Parts,
        _: &mut PathState,
        body: &mut Option<Incoming>,
    ) -> Result<Self, A::Error> {
        match body.take() {
            Some(body) => Ok(body),
            None => panic!("attempted to retrieve body twice"),
        }
    }
}

impl<'a, A: Application> FromContext<'a, A> for ClientAddr {
    fn from_context(
        _: &'a Arc<A>,
        req: &'a Parts,
        _: &mut PathState,
        _: &mut Option<A::RequestBody>,
    ) -> Result<Self, A::Error> {
        // This is safe because we insert ClientAddr into the request extensions
        // unconditionally in the ConnectionService::call method.
        Ok(req.extensions.get::<ClientAddr>().copied().unwrap())
    }
}

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
