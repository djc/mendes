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
    block.push(Statement::get(extract.into()));
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

impl Statement {
    fn get(tokens: TokenStream) -> syn::Stmt {
        syn::parse::<Statement>(tokens).unwrap().stmt
    }
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

    if !expr.path.is_ident("path") {
        panic!("dispatch function does not call the route!() macro")
    }

    let routes = path_routes(&expr.parse_body::<Map>().unwrap().routes);
    let block = quote!({
        let app = cx.app.clone();
        #routes
    });

    ast.block = Box::new(syn::parse::<syn::Block>(block.into()).unwrap());
    TokenStream::from(ast.to_token_stream())
}

fn path_routes(routes: &[Route]) -> proc_macro2::TokenStream {
    let mut route_tokens = proc_macro2::TokenStream::new();
    for route in routes.iter() {
        route.component.to_tokens(&mut route_tokens);
        route_tokens.append(Punct::new('=', Spacing::Joint));
        route_tokens.append(Punct::new('>', Spacing::Alone));

        let nested = match &route.target {
            Target::Direct(expr) => quote!(#expr(cx).await.unwrap_or_else(|e| app.error(e))),
            Target::Routes(routes) => path_routes(routes)
        };

        route_tokens.append_all(nested);
        route_tokens.append(Punct::new(',', Spacing::Alone));
    }

    quote!(match cx.path() {
        #route_tokens
    })
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
    target: Target,
}

impl Parse for Route {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let component = input.parse()?;
        input.parse::<syn::Token![=>]>()?;
        let expr = input.parse::<syn::Expr>()?;
        let target = if let syn::Expr::Macro(mac) = &expr {
            if mac.mac.path.is_ident("path") {
                let Map { routes } = mac.mac.parse_body().unwrap();
                Target::Routes(routes)
            } else {
                Target::Direct(expr)
            }
        } else {
            Target::Direct(expr)
        };

        Ok(Route { component, target })
    }
}

enum Target {
    Direct(syn::Expr),
    Routes(Vec<Route>),
}
