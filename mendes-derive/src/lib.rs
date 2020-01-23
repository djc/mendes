extern crate proc_macro;

use std::mem;

use proc_macro::TokenStream;
use proc_macro2::{Punct, Spacing};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

#[proc_macro_attribute]
pub fn handler(meta: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast = syn::parse::<syn::ItemFn>(item).unwrap();

    let app_type = syn::parse::<AppType>(meta).unwrap().ty;
    let new = syn::parse::<MethodArgs>(quote!(cx: Context<#app_type>).into()).unwrap();
    let _ = mem::replace(&mut ast.sig.inputs, new.args);

    let mut block = Vec::with_capacity(ast.block.stmts.len() + 1);
    let extract = quote!(let Context { app, req, .. } = cx;);
    block.push(syn::parse::<Statement>(extract.into()).unwrap().stmt);
    let old = mem::replace(&mut ast.block.stmts, block);
    ast.block.stmts.extend(old);

    TokenStream::from(ast.to_token_stream())
}

struct MethodArgs {
    args: Punctuated<syn::FnArg, Comma>,
}

impl Parse for MethodArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            args: Punctuated::parse_terminated(input)?,
        })
    }
}

struct AppType {
    ty: syn::Type,
}

impl Parse for AppType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self { ty: input.parse()? })
    }
}

struct Statement {
    stmt: syn::Stmt,
}

impl Parse for Statement {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            stmt: input.parse()?,
        })
    }
}

#[proc_macro_attribute]
pub fn dispatch(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast: syn::ItemFn = syn::parse(item).unwrap();

    let expr = match ast.block.stmts.get(0) {
        Some(syn::Stmt::Item(syn::Item::Macro(expr))) => &expr.mac,
        _ => panic!("did not find expression statement in block"),
    };

    if !expr.path.is_ident("route") {
        panic!("dispatch function does not call the route!() macro")
    }

    let Map { routes } = expr.parse_body().unwrap();
    let mut route_tokens = proc_macro2::TokenStream::new();
    for route in routes.iter() {
        route.component.to_tokens(&mut route_tokens);
        route_tokens.append(Punct::new('=', Spacing::Joint));
        route_tokens.append(Punct::new('>', Spacing::Alone));

        let handler = &route.handler;
        route_tokens.append_all(quote!(#handler(cx).await.unwrap_or_else(|e| app.error(e))));
        route_tokens.append(Punct::new(',', Spacing::Alone));
    }

    let block = quote!({
        let app = cx.app.clone();
        match cx.path() {
            #route_tokens
        }
    });

    let block: syn::Block = syn::parse(TokenStream::from(block)).unwrap();
    ast.block = Box::new(block);
    TokenStream::from(ast.to_token_stream())
}

struct Map {
    routes: Vec<Route>,
}

impl Parse for Map {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut routes = vec![];
        while !input.is_empty() {
            if !routes.is_empty() {
                let _ = input.parse::<syn::Token![,]>();
                if input.is_empty() {
                    break;
                }
            }
            routes.push(Route::parse(input)?);
        }
        Ok(Map { routes })
    }
}

struct Route {
    component: syn::Pat,
    handler: syn::Expr,
}

impl Parse for Route {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let component = input.parse()?;
        input.parse::<syn::Token![=>]>()?;
        let handler = input.parse()?;
        Ok(Route { component, handler })
    }
}
