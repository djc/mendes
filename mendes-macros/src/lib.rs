extern crate proc_macro;

use proc_macro::TokenStream;
use quote::ToTokens;

mod cookies;
mod forms;
mod models;
mod route;
mod util;

#[proc_macro_attribute]
pub fn cookie(_: TokenStream, item: TokenStream) -> TokenStream {
    let ast = syn::parse::<syn::ItemStruct>(item).unwrap();

    let cookie = cookies::cookie(&ast);

    let mut tokens = ast.to_token_stream();
    tokens.extend(cookie);
    TokenStream::from(tokens)
}

#[proc_macro_attribute]
pub fn form(meta: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast = syn::parse::<syn::ItemStruct>(item).unwrap();
    let meta = syn::parse::<forms::FormMeta>(meta).unwrap();

    let display = forms::form(&meta, &mut ast);

    let mut tokens = ast.to_token_stream();
    tokens.extend(display);
    TokenStream::from(tokens)
}

#[proc_macro_attribute]
pub fn handler(meta: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast = syn::parse::<syn::ItemFn>(item).unwrap();
    let methods = syn::parse::<route::HandlerMethods>(meta).unwrap().methods;
    route::handler(&methods, &mut ast);
    TokenStream::from(ast.to_token_stream())
}

#[proc_macro_attribute]
pub fn get(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast = syn::parse::<syn::ItemFn>(item).unwrap();
    route::handler(&["get"], &mut ast);
    TokenStream::from(ast.to_token_stream())
}

#[proc_macro_attribute]
pub fn post(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast = syn::parse::<syn::ItemFn>(item).unwrap();
    route::handler(&["post"], &mut ast);
    TokenStream::from(ast.to_token_stream())
}

#[proc_macro_attribute]
pub fn route(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast: syn::ItemFn = syn::parse::<syn::ItemFn>(item).unwrap();
    route::route(&mut ast);
    TokenStream::from(ast.to_token_stream())
}

#[proc_macro_derive(ToField, attributes(option))]
pub fn derive_to_field(item: TokenStream) -> TokenStream {
    let ast = syn::parse::<syn::DeriveInput>(item).unwrap();
    TokenStream::from(forms::to_field(ast))
}

#[proc_macro_attribute]
pub fn model(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast = syn::parse::<syn::ItemStruct>(item).unwrap();

    let impls = models::model(&mut ast);

    let mut tokens = ast.to_token_stream();
    tokens.extend(impls);
    TokenStream::from(tokens)
}

#[proc_macro_attribute]
pub fn model_type(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast = syn::parse::<syn::Item>(item).unwrap();

    let impls = models::model_type(&mut ast);

    let mut tokens = ast.to_token_stream();
    tokens.extend(impls);
    TokenStream::from(tokens)
}
