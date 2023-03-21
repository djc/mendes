extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::parse_macro_input;

mod cookies;
mod forms;
mod route;
mod util;

#[proc_macro_attribute]
pub fn cookie(meta: TokenStream, item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as syn::ItemStruct);
    let meta = parse_macro_input!(meta as cookies::CookieMeta);
    let cookie = cookies::cookie(&meta, &ast);
    let mut tokens = ast.to_token_stream();
    tokens.extend(cookie);
    TokenStream::from(tokens)
}

#[proc_macro_attribute]
pub fn form(meta: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(item as syn::ItemStruct);
    let meta = parse_macro_input!(meta as forms::FormMeta);
    let display = forms::form(&meta, &mut ast);
    let mut tokens = ast.to_token_stream();
    tokens.extend(display);
    TokenStream::from(tokens)
}

/// Implement a request handler wrapper for the annotated function
///
/// The attribute takes allowed methods as its arguments:
///
/// ```ignore
/// /// This handler will immediately return a `405 Method not allowed`
/// /// error for all request methods other than `GET`
/// #[handler(GET)]
/// fn hello(_: &App) -> Result<Response<String>, Error> {
///     Ok(Response::builder()
///         .status(StatusCode::OK)
///         .body("Hello, world".into())
///         .unwrap())
/// }
/// ```
///
/// The first argument of the function must be a reference to an implementer of
/// the `Application` trait (the implementor may also be wrapped in an `Arc`).
/// All unannotated arguments must be of types that implement the `FromContext`
/// trait for the `Application` type used in the first argument. This includes
/// `&http::request::Parts`, the `Request`'s headers and any number of types
/// that can represent a path component from the URI:
///
/// * `&[u8]` for the bytes representation of the path component
/// * `Cow<'_, str>`
/// * `String`
/// * Numeric types (`i8`, `u8`, `i16`, `u16`, ..., `isize`, `usize`, `f32`, `f64`)
/// * `bool` and `char`
/// * If the `hyper` feature is enabled, `hyper::body::Body`
///   (only if `Application::RequestBody` is also `Body`)
///
/// Each of these types can be wrapped in `Option` for optional path components.
/// Additionally, there are two attributes that may be used on handler arguments:
///
/// * `#[rest]`: a `&str` representing the part of the request path not yet consumed by routing
/// * `#[query]`: a type that implements `Deserialize`, and will be used to deserialize the URI query
///
/// This macro will generate a module that contains a `call()` function mirroring
/// the original function, and you may rely on this behavior (for example, for testing).
///
/// ```ignore
/// mod hello {
///    use super::*;
///    /// ... some internals hidden ...
///    pub(super) async fn call(_: &App) -> Result<Response<Body>, Error> {
///        Ok(Response::builder()
///            .status(StatusCode::OK)
///            .body("Hello, world".into())
///            .unwrap())
///    }
/// }
/// ```
#[proc_macro_attribute]
pub fn handler(meta: TokenStream, item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as syn::ItemFn);
    let methods = parse_macro_input!(meta as route::HandlerMethods).methods;
    route::handler(&methods, ast)
}

#[proc_macro_attribute]
pub fn scope(_: TokenStream, item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as syn::ItemFn);
    route::scope(ast)
}

#[proc_macro]
pub fn route(item: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(item as syn::ExprMatch);
    route::route(&mut ast);
    quote!(#ast).into()
}

#[proc_macro_derive(ToField, attributes(option))]
pub fn derive_to_field(item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as syn::DeriveInput);
    TokenStream::from(forms::to_field(ast))
}
