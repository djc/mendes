# Mendes: web toolkit for impatient perfectionists

[![Documentation](https://docs.rs/mendes/badge.svg)](https://docs.rs/mendes/)
[![Crates.io](https://img.shields.io/crates/v/mendes.svg)](https://crates.io/crates/mendes)
[![Build status](https://github.com/djc/mendes/workflows/CI/badge.svg)](https://github.com/djc/mendes/actions?query=workflow%3ACI)
[![Coverage status](https://codecov.io/gh/djc/mendes/branch/master/graph/badge.svg)](https://codecov.io/gh/djc/mendes)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE-MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE-APACHE)

Mendes is a Rust web toolkit for impatient perfectionists (apologies to Django).
It aims to be:

* Modular: less framework, more library; pick and choose components
* Async: async/await from the start
* Low boilerplate: easy to get started, but with limited "magic"
* Type-safe: leverage the type system to make error handling low effort
* Secure: provide security by default; no unsafe code in this project
* Run on stable Rust (no promises on MSRV though)

Mendes is currently in an extremely early phase and probably not ready for anything
but experiments for those who are curious. Feedback is always welcome though!

## Minimal example

This should definitely become more minimal over time.

```rust
use async_trait::async_trait;
use hyper::Body;
use mendes::application::IntoResponse;
use mendes::http::request::Parts;
use mendes::http::{Response, StatusCode};
use mendes::{handler, route, Application, Context};

#[handler(GET)]
async fn hello(_: &App) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body("Hello, world".into())
        .unwrap())
}

struct App {}

#[async_trait]
impl Application for App {
    type RequestBody = ();
    type ResponseBody = Body;
    type Error = Error;

    async fn handle(mut cx: Context<Self>) -> Response<Body> {
        route!(match cx.path() {
            _ => hello,
        })
    }
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
```

All feedback welcome. Feel free to file bugs, requests for documentation and
any other feedback to the [issue tracker][issues].

Mendes was created and is maintained by Dirkjan Ochtman.

[issues]: https://github.com/djc/mendes/issues
