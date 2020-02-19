use std::mem;

use proc_macro::TokenStream;
use proc_macro2::{Ident, Punct, Spacing, Span};
use quote::{quote, TokenStreamExt};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

pub fn handler(app_type: &syn::Type, ast: &mut syn::ItemFn) {
    let new = syn::parse::<MethodArgs>(quote!(cx: Context<#app_type>).into()).unwrap();
    let old = mem::replace(&mut ast.sig.inputs, new.args);

    let (mut app, mut req, mut rest, mut complete) = ("__app", "__req", vec![], false);
    for arg in old {
        if complete {
            panic!("more arguments after #[raw] not allowed");
        }

        let ty = match &arg {
            syn::FnArg::Typed(ty) => ty,
            _ => panic!("did not expect receiver argument in handler"),
        };

        if let Some(attr) = ty.attrs.first() {
            if attr.path.is_ident("raw") {
                complete = true;
                continue;
            }
        }

        use syn::Pat::*;
        match ty.pat.as_ref() {
            Ident(id) => {
                if id.ident == "app" {
                    app = "app";
                } else if id.ident == "application" {
                    app = "application";
                } else if id.ident == "req" {
                    req = "req";
                } else if id.ident == "request" {
                    req = "request";
                } else {
                    rest.push(arg);
                }
            }
            Wild(_) => continue,
            _ => {
                rest.push(arg);
            }
        }
    }

    let mut block = Vec::with_capacity(ast.block.stmts.len() + rest.len() + 6);
    block.push(Statement::get(
        quote!(
            let Context { app, req, path } = cx;
        )
        .into(),
    ));

    let app_name = Ident::new(app, Span::call_site());
    block.push(Statement::get(quote!(let #app_name = app;).into()));
    let req_name = Ident::new(req, Span::call_site());
    block.push(Statement::get(quote!(let #req_name = req;).into()));
    block.push(Statement::get(quote!(let mut __path = path;).into()));

    for arg in rest {
        let typed = match &arg {
            syn::FnArg::Typed(typed) => typed,
            _ => panic!("did not expect receiver argument in handler"),
        };

        let pat = &typed.pat;
        if let Some(attr) = typed.attrs.first() {
            if attr.path.is_ident("rest") {
                block.push(Statement::get(
                    quote!(
                        let #pat = __path.rest(#req_name.uri().path());
                    )
                    .into(),
                ));
                break;
            }
        }

        let ty = &typed.ty;
        // Handle &str arguments
        if let syn::Type::Reference(type_ref) = ty.as_ref() {
            if let syn::Type::Path(path) = type_ref.elem.as_ref() {
                if path.qself.is_none() && path.path.is_ident("str") {
                    block.push(Statement::get(
                        quote!(
                            let #pat: #ty = __path.next(#req_name.uri().path())
                                .ok_or(::mendes::ClientError::NotFound)?;
                        )
                        .into(),
                    ));
                    continue;
                }
            }
        }

        block.push(Statement::get(
            quote!(
                let #pat: #ty = __path.next(#req_name.uri().path())
                    .ok_or(::mendes::ClientError::NotFound)?
                    .parse()
                    .map_err(|_| ::mendes::ClientError::NotFound)?;
            )
            .into(),
        ));
    }

    let old = mem::replace(&mut ast.block.stmts, block);
    ast.block.stmts.extend(old);
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

pub struct AppType {
    pub ty: syn::Type,
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

pub fn dispatch(ast: &mut syn::ItemFn) {
    let (block, routes) = match ast.block.stmts.get_mut(0) {
        Some(syn::Stmt::Item(syn::Item::Macro(expr))) => {
            if !expr.mac.path.is_ident("path") {
                panic!("dispatch function does not call the path!() macro")
            } else {
                let map = expr.mac.parse_body::<Map>().unwrap();
                (&mut ast.block, map)
            }
        }
        Some(syn::Stmt::Item(syn::Item::Fn(inner))) => {
            if let Some(syn::Stmt::Item(syn::Item::Macro(expr))) = inner.block.stmts.get(0) {
                if !expr.mac.path.is_ident("path") {
                    panic!("dispatch function does not call the path!() macro")
                } else {
                    let map = expr.mac.parse_body::<Map>().unwrap();
                    (&mut inner.block, map)
                }
            } else {
                panic!("did not find expression statement in nested function block");
            }
        }
        _ => panic!("did not find expression statement in block"),
    };

    let new = quote!({
        let app = cx.app.clone();
        #routes
    });

    mem::replace(
        block,
        Box::new(syn::parse::<syn::Block>(new.into()).unwrap()),
    );
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

impl quote::ToTokens for Map {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut route_tokens = proc_macro2::TokenStream::new();
        let mut wildcard = false;
        for route in self.routes.iter() {
            let mut rewind = false;
            if let syn::Pat::Wild(_) = route.component {
                wildcard = true;
                rewind = true;
            }

            route.component.to_tokens(&mut route_tokens);
            route_tokens.append(Punct::new('=', Spacing::Joint));
            route_tokens.append(Punct::new('>', Spacing::Alone));

            let nested = match &route.target {
                Target::Direct(expr) => quote!(#expr(cx).await.unwrap_or_else(|e| app.error(e))),
                Target::Routes(routes) => quote!(#routes),
            };

            if rewind {
                route_tokens.append_all(quote!({ let mut cx = cx.rewind(); #nested }));
            } else {
                route_tokens.append_all(nested);
            }
            route_tokens.append(Punct::new(',', Spacing::Alone));
        }

        if !wildcard {
            route_tokens.extend(quote!(
                _ => app.error(::mendes::ClientError::NotFound.into()),
            ));
        }

        tokens.extend(quote!(match cx.path() {
            #route_tokens
        }));
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
                Target::Routes(mac.mac.parse_body().unwrap())
            } else {
                Target::Direct(expr)
            }
        } else {
            Target::Direct(expr)
        };

        Ok(Route { component, target })
    }
}

#[allow(clippy::large_enum_variant)]
enum Target {
    Direct(syn::Expr),
    Routes(Map),
}
