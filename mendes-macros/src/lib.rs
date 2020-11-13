extern crate proc_macro;

use proc_macro::TokenStream;
use quote::ToTokens;
use syn::parse_macro_input;

mod cookies;
mod forms;
mod models;
mod route;
mod util;

#[proc_macro_attribute]
pub fn cookie(_: TokenStream, item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as syn::ItemStruct);
    let cookie = cookies::cookie(&ast);
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

#[proc_macro_attribute]
pub fn handler(meta: TokenStream, item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as syn::ItemFn);
    let methods = parse_macro_input!(meta as route::HandlerMethods).methods;
    route::handler(&methods, ast)
}

#[proc_macro_attribute]
pub fn route(_: TokenStream, item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as syn::ItemFn);
    route::route(ast, true)
}

#[proc_macro_attribute]
pub fn scope(_: TokenStream, item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as syn::ItemFn);
    route::route(ast, false)
}

#[proc_macro_derive(ToField, attributes(option))]
pub fn derive_to_field(item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as syn::DeriveInput);
    TokenStream::from(forms::to_field(ast))
}

#[proc_macro_attribute]
pub fn model(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(item as syn::ItemStruct);
    let impls = models::model(&mut ast);
    let mut tokens = ast.to_token_stream();
    tokens.extend(impls);
    TokenStream::from(tokens)
}

#[proc_macro_attribute]
pub fn model_type(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(item as syn::Item);
    let impls = models::model_type(&mut ast);
    let mut tokens = ast.to_token_stream();
    tokens.extend(impls);
    TokenStream::from(tokens)
}
