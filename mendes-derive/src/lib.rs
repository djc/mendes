extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Punct, Spacing};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::parse::{Parse, ParseStream};

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
        route_tokens.append_all(quote!(#handler(&app, req).await));
        route_tokens.append(Punct::new(',', Spacing::Alone));
    }

    let block = quote!({
        let path = req.uri().path();
        let component = match path[1..].find('/') {
            Some(pos) => &path[1..1 + pos],
            None => &path[1..],
        };

        let result = match component { #route_tokens };

        match result {
            Ok(rsp) => Ok(rsp),
            Err(e) => Ok(app.error(e)),
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
