use std::fmt::Display;

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

pub fn handler<T>(methods: &[T], mut ast: syn::ItemFn) -> TokenStream
where
    T: Display,
{
    let app_type = match ast.sig.inputs.first() {
        Some(syn::FnArg::Typed(syn::PatType { ty, .. })) => match **ty {
            syn::Type::Reference(ref reffed) => (*reffed.elem).clone(),
            _ => panic!("handler's first argument must be a reference"),
        },
        _ => panic!("handler argument lists must have &App as their first type"),
    };

    let app_type = match &app_type {
        syn::Type::Path(syn::TypePath { path, .. }) => Some(path),
        _ => None,
    }
    .and_then(|path| path.segments.first())
    .and_then(|segment| match &segment.ident {
        id if id == "Arc" => Some(&segment.arguments),
        _ => None,
    })
    .and_then(|args| match args {
        syn::PathArguments::AngleBracketed(inner) => Some(inner),
        _ => None,
    })
    .and_then(|args| match args.args.first() {
        Some(syn::GenericArgument::Type(ty)) => Some(ty.clone()),
        _ => None,
    })
    .unwrap_or(app_type);

    let mut method_patterns = proc_macro2::TokenStream::new();
    for (i, method) in methods.iter().enumerate() {
        let method = Ident::new(&method.to_string().to_ascii_uppercase(), Span::call_site());
        method_patterns.extend(if i > 0 {
            quote!( | &mendes::http::Method::#method)
        } else {
            quote!(&mendes::http::Method::#method)
        });
    }

    let mut done = false;
    let mut prefix = proc_macro2::TokenStream::new();
    let mut args = proc_macro2::TokenStream::new();
    for (i, arg) in ast.sig.inputs.iter_mut().enumerate() {
        let typed = match arg {
            syn::FnArg::Typed(typed) => typed,
            _ => panic!("did not expect receiver argument in handler"),
        };

        let mut special = false;
        let (pat, ty) = (&*typed.pat, &typed.ty);
        typed.attrs.retain(|attr| {
            if attr.path.is_ident("rest") {
                prefix.extend(quote!(
                    let #pat = cx.path.rest(&cx.req.uri.path());
                ));
                args.extend(quote!(#pat,));
                done = true;
                special = true;
                false
            } else if attr.path.is_ident("query") {
                prefix.extend(quote!(let #pat = cx.query::<#ty>()?;));
                args.extend(quote!(#pat,));
                special = true;
                false
            } else {
                true
            }
        });

        if special {
            continue;
        } else if done {
            panic!("more arguments after #[rest] not allowed");
        }

        let name = match pat {
            syn::Pat::Wild(_) => syn::Pat::Ident(syn::PatIdent {
                ident: Ident::new(&format!("_{}", i), Span::call_site()),
                attrs: Vec::new(),
                mutability: None,
                subpat: None,
                by_ref: None,
            }),
            _ => pat.clone(),
        };

        prefix.extend(quote!(
            let #name = <#ty as mendes::FromContext<#app_type>>::from_context(
                &cx.app, &cx.req, &mut cx.path, &mut cx.body,
            )?;
        ));
        args.extend(quote!(#name,));
    }

    let name = ast.sig.ident.clone();
    let orig_vis = ast.vis.clone();
    ast.vis = nested_visibility(ast.vis);

    let handler = {
        let nested_vis = &ast.vis;
        let generics = &ast.sig.generics;
        let rtype = &ast.sig.output;
        let where_clause = &ast.sig.generics.where_clause;
        quote!(
            #nested_vis async fn handler#generics(
                cx: &mut mendes::application::Context<#app_type>
            ) #rtype #where_clause {
                match &cx.req.method {
                    #method_patterns => {}
                    _ => return Err(mendes::Error::MethodNotAllowed.into()),
                }
                #prefix
                call(#args).await
            }
        )
    };

    let call = {
        ast.sig.ident = Ident::new("call", Span::call_site());
        quote!(#ast)
    };

    quote!(#orig_vis mod #name {
        use super::*;
        #handler
        #call
    })
    .into()
}

fn nested_visibility(vis: syn::Visibility) -> syn::Visibility {
    match vis {
        cur @ syn::Visibility::Crate(_) | cur @ syn::Visibility::Public(_) => cur,
        syn::Visibility::Inherited => visibility("super"),
        cur @ syn::Visibility::Restricted(_) => {
            let inner = match &cur {
                syn::Visibility::Restricted(inner) => inner,
                _ => unreachable!(),
            };

            if inner.path.is_ident("self") {
                visibility("super")
            } else if inner.path.is_ident("super") {
                visibility("super::super")
            } else {
                cur
            }
        }
    }
}

fn visibility(path: &str) -> syn::Visibility {
    syn::Visibility::Restricted(syn::VisRestricted {
        pub_token: syn::Token![pub](Span::call_site()),
        paren_token: syn::token::Paren {
            span: Span::call_site(),
        },
        in_token: match path {
            "self" | "crate" | "super" => None,
            _ => Some(syn::Token![in](Span::call_site())),
        },
        path: Box::new(Ident::new(path, Span::call_site()).into()),
    })
}

pub fn route(mut ast: syn::ItemFn, root: bool) -> TokenStream {
    let (block, routes, self_name, req_name) = match ast.block.stmts.get_mut(0) {
        Some(syn::Stmt::Item(syn::Item::Macro(expr))) => {
            let target = Target::from_item(expr);
            let self_name = argument_name(&ast.sig, 0);
            let req_name = argument_name(&ast.sig, 1);
            (&mut ast.block, target, self_name, req_name)
        }
        Some(syn::Stmt::Item(syn::Item::Fn(inner))) => {
            if let Some(syn::Stmt::Item(syn::Item::Macro(expr))) = inner.block.stmts.get(0) {
                let target = Target::from_item(expr);
                let self_name = argument_name(&inner.sig, 0);
                let req_name = argument_name(&inner.sig, 1);
                (&mut inner.block, target, self_name, req_name)
            } else {
                panic!("did not find expression statement in nested function block")
            }
        }
        _ => panic!("did not find expression statement in block"),
    };

    if root {
        let self_name = self_name.unwrap();
        let req_name = req_name.unwrap();

        let new = quote!({
            use mendes::Application;
            use mendes::application::Responder;
            let app = #self_name.clone();
            let mut cx = mendes::Context::new(#self_name, #req_name);
            let rsp = #routes;
            let mendes::Context { app, req, .. } = cx;
            app.respond(&req, rsp).await
        });

        *block = Box::new(syn::parse::<syn::Block>(new.into()).unwrap());
        return ast.to_token_stream().into();
    }

    let cx_name = self_name.unwrap();
    let new = quote!({
        use mendes::Application;
        use mendes::application::Responder;
        let mut cx = #cx_name;
        let app = cx.app.clone();
        #routes
    });
    *block = Box::new(syn::parse::<syn::Block>(new.into()).unwrap());

    let name = ast.sig.ident.clone();
    let orig_vis = ast.vis.clone();
    ast.vis = nested_visibility(ast.vis);

    ast.sig.ident = Ident::new("handler", Span::call_site());
    quote!(#orig_vis mod #name {
        use super::*;
        #ast
    })
    .into()
}

fn argument_name(sig: &syn::Signature, i: usize) -> Option<&syn::Ident> {
    let pat = match sig.inputs.iter().nth(i)? {
        syn::FnArg::Typed(arg) => &arg.pat,
        _ => return None,
    };

    match &**pat {
        syn::Pat::Ident(ident) => Some(&ident.ident),
        _ => None,
    }
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
                #expr::handler(&mut cx).await.into_response(&*app)
            )
            .to_tokens(tokens),
            Target::MethodMap(map) => map.to_tokens(tokens),
            Target::PathMap(map) => map.to_tokens(tokens),
        }
    }
}

struct PathMap {
    routes: Vec<(Vec<syn::Attribute>, syn::Pat, Target)>,
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

            let attrs = input.call(syn::Attribute::parse_outer)?;
            let component = input.parse()?;
            input.parse::<syn::Token![=>]>()?;
            let target = input.parse()?;
            routes.push((attrs, component, target));
        }
        Ok(PathMap { routes })
    }
}

impl quote::ToTokens for PathMap {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut route_tokens = proc_macro2::TokenStream::new();
        let mut wildcard = false;
        for (attrs, component, target) in self.routes.iter() {
            let mut rewind = false;
            if let syn::Pat::Wild(_) = component {
                wildcard = true;
                rewind = true;
            }

            attrs
                .iter()
                .for_each(|attr| attr.to_tokens(&mut route_tokens));
            if rewind {
                quote!(
                    #component => {
                        cx.rewind();
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
                _ => ::mendes::Error::PathNotFound.into_response(&*app),
            ));
        }

        tokens.extend(quote!(match cx.next_path().as_deref() {
            #route_tokens
        }));
    }
}

struct MethodMap {
    routes: Vec<(Vec<syn::Attribute>, syn::Ident, Target)>,
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

            let attrs = input.call(syn::Attribute::parse_outer)?;
            let component = input.parse()?;
            input.parse::<syn::Token![=>]>()?;
            let target = input.parse()?;
            routes.push((attrs, component, target));
        }
        Ok(MethodMap { routes })
    }
}

impl quote::ToTokens for MethodMap {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut route_tokens = proc_macro2::TokenStream::new();
        let mut wildcard = false;
        for (attrs, component, target) in self.routes.iter() {
            if component == "_" {
                wildcard = true;
            }

            attrs
                .iter()
                .for_each(|attr| attr.to_tokens(&mut route_tokens));
            quote!(mendes::http::Method::#component => #target,).to_tokens(&mut route_tokens);
        }

        if !wildcard {
            route_tokens.extend(quote!(
                _ => ::mendes::Error::MethodNotAllowed.into_response(&*app),
            ));
        }

        tokens.extend(quote!(match cx.req.method {
            #route_tokens
        }));
    }
}

pub struct HandlerMethods {
    pub methods: Vec<syn::Ident>,
}

impl Parse for HandlerMethods {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let methods = Punctuated::<syn::Ident, Comma>::parse_terminated(input)?;
        Ok(Self {
            methods: methods.into_iter().collect(),
        })
    }
}
