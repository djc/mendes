use std::mem;

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

pub fn handler(app_type: &syn::Type, ast: &mut syn::ItemFn) {
    let new = syn::parse::<MethodArgs>(quote!(mut cx: Context<#app_type>).into()).unwrap();
    let old = mem::replace(&mut ast.sig.inputs, new.args);

    let (mut app, mut body, mut args, mut rest) = ("__app", None, vec![], None);
    for arg in old.iter() {
        let typed = match arg {
            syn::FnArg::Typed(typed) => typed,
            _ => panic!("did not expect receiver argument in handler"),
        };

        use syn::Pat::*;
        let (pat, ty) = (&typed.pat, &typed.ty);
        if let Ident(id) = pat.as_ref() {
            if id.ident == "app" {
                app = "app";
                continue;
            } else if id.ident == "application" {
                app = "application";
                continue;
            }
        }

        if let Some(attr) = typed.attrs.first() {
            if attr.path.is_ident("rest") {
                rest = Some(pat);
                continue;
            } else if attr.path.is_ident("body") {
                body = Some((pat, ty));
                continue;
            }
        }

        if rest.is_some() {
            panic!("more arguments after #[raw] not allowed");
        }

        args.push((pat, ty));
    }

    let mut block = Vec::with_capacity(ast.block.stmts.len());
    if body.is_some() {
        block.push(Statement::get(
            quote!(
                cx.retrieve_body().await?;
            )
            .into(),
        ));
    }

    for (pat, ty) in args {
        block.push(Statement::get(
            quote!(
                let #pat = <#ty as mendes::FromContext>::from_context::<#app_type>(&cx.req, &mut cx.path)?;
            )
            .into(),
        ));
    }

    if let Some(pat) = rest {
        block.push(Statement::get(
            quote!(
                let #pat = cx.path.rest(&cx.req.uri.path());
            )
            .into(),
        ));
    }

    if let Some((pat, ty)) = body {
        block.push(Statement::get(
            quote!(
                let #pat = cx.from_body::<#ty>()?;
            )
            .into(),
        ));
    }

    let app_name = Ident::new(app, Span::call_site());
    block.push(Statement::get(quote!(let #app_name = &cx.app;).into()));
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
            let target = Target::from_item(expr);
            (&mut ast.block, target)
        }
        Some(syn::Stmt::Item(syn::Item::Fn(inner))) => {
            if let Some(syn::Stmt::Item(syn::Item::Macro(expr))) = inner.block.stmts.get(0) {
                let target = Target::from_item(expr);
                (&mut inner.block, target)
            } else {
                panic!("did not find expression statement in nested function block")
            }
        }
        _ => panic!("did not find expression statement in block"),
    };

    let new = quote!({
        let app = cx.app().clone();
        #routes
    });

    mem::replace(
        block,
        Box::new(syn::parse::<syn::Block>(new.into()).unwrap()),
    );
}

#[allow(clippy::large_enum_variant)]
enum Target {
    Direct(syn::Expr),
    PathMap(PathMap),
    MethodMap(MethodMap),
}

impl Target {
    fn from_expr(expr: syn::Expr) -> Self {
        let mac = match expr {
            syn::Expr::Macro(mac) => mac,
            _ => return Target::Direct(expr),
        };

        Self::from_macro(&mac.mac)
    }

    fn from_item(expr: &syn::ItemMacro) -> Self {
        Self::from_macro(&expr.mac)
    }

    fn from_macro(mac: &syn::Macro) -> Self {
        if mac.path.is_ident("path") {
            Target::PathMap(mac.parse_body().unwrap())
        } else if mac.path.is_ident("method") {
            Target::MethodMap(mac.parse_body().unwrap())
        } else {
            panic!("unknown macro used as dispatch target")
        }
    }
}

impl Parse for Target {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Target::from_expr(input.parse::<syn::Expr>()?))
    }
}

impl quote::ToTokens for Target {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Target::Direct(expr) => quote!(
                #expr(cx).await.unwrap_or_else(|e| app.error(e))
            )
            .to_tokens(tokens),
            Target::MethodMap(map) => map.to_tokens(tokens),
            Target::PathMap(map) => map.to_tokens(tokens),
        }
    }
}

struct PathMap {
    routes: Vec<(syn::Pat, Target)>,
}

impl Parse for PathMap {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut routes = vec![];
        while !input.is_empty() {
            if !routes.is_empty() {
                let _ = input.parse::<syn::Token![,]>();
                if input.is_empty() {
                    break;
                }
            }

            let component = input.parse()?;
            input.parse::<syn::Token![=>]>()?;
            let target = input.parse()?;
            routes.push((component, target));
        }
        Ok(PathMap { routes })
    }
}

struct MethodMap {
    routes: Vec<(syn::Ident, Target)>,
}

impl Parse for MethodMap {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut routes = vec![];
        while !input.is_empty() {
            if !routes.is_empty() {
                let _ = input.parse::<syn::Token![,]>();
                if input.is_empty() {
                    break;
                }
            }

            let component = input.parse()?;
            input.parse::<syn::Token![=>]>()?;
            let target = input.parse()?;
            routes.push((component, target));
        }
        Ok(MethodMap { routes })
    }
}

impl quote::ToTokens for MethodMap {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut route_tokens = proc_macro2::TokenStream::new();
        let mut wildcard = false;
        for (component, target) in self.routes.iter() {
            if component == "_" {
                wildcard = true;
            }

            quote!(mendes::http::Method::#component => #target,).to_tokens(&mut route_tokens);
        }

        if !wildcard {
            route_tokens.extend(quote!(
                _ => app.error(::mendes::ClientError::MethodNotAllowed.into()),
            ));
        }

        tokens.extend(quote!(match cx.req.method {
            #route_tokens
        }));
    }
}

impl quote::ToTokens for PathMap {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut route_tokens = proc_macro2::TokenStream::new();
        let mut wildcard = false;
        for (component, target) in self.routes.iter() {
            let mut rewind = false;
            if let syn::Pat::Wild(_) = component {
                wildcard = true;
                rewind = true;
            }

            if rewind {
                quote!(
                    #component => {
                        let mut cx = cx.rewind();
                        #target
                    }
                )
                .to_tokens(&mut route_tokens);
            } else {
                quote!(#component => #target,).to_tokens(&mut route_tokens);
            }
        }

        if !wildcard {
            route_tokens.extend(quote!(
                _ => app.error(::mendes::ClientError::NotFound.into()),
            ));
        }

        tokens.extend(quote!(match cx.next_path() {
            #route_tokens
        }));
    }
}
