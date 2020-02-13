extern crate proc_macro;

use proc_macro::TokenStream;
use quote::ToTokens;

mod forms;
mod route;

#[proc_macro_attribute]
pub fn form(meta: TokenStream, item: TokenStream) -> TokenStream {
    let ast = syn::parse::<syn::ItemStruct>(item).unwrap();
    let meta = syn::parse::<forms::FormMeta>(meta).unwrap();

    let display = forms::form(&meta, &ast);

    let mut tokens = ast.to_token_stream();
    tokens.extend(display);
    TokenStream::from(tokens)
}

#[proc_macro_attribute]
pub fn handler(meta: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast = syn::parse::<syn::ItemFn>(item).unwrap();
    let app_type = syn::parse::<route::AppType>(meta).unwrap().ty;
    route::handler(&app_type, &mut ast);
    TokenStream::from(ast.to_token_stream())
}

#[proc_macro_attribute]
pub fn dispatch(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast: syn::ItemFn = syn::parse::<syn::ItemFn>(item).unwrap();
    route::dispatch(&mut ast);
    TokenStream::from(ast.to_token_stream())
}
