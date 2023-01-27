use std::fmt::Display;

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::parse_quote;
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
                    let #pat = <mendes::application::Rest<#ty> as mendes::FromContext<#app_type>>::from_context(
                        &cx.app, &cx.req, &mut cx.path, &mut cx.body,
                    )?.0;
                ));
                args.extend(quote!(#pat,));
                done = true;
                special = true;
                false
            } else if attr.path.is_ident("query") {
                prefix.extend(quote!(
                    let #pat = <mendes::application::Query<#ty> as mendes::FromContext<#app_type>>::from_context(
                        &cx.app, &cx.req, &mut cx.path, &mut cx.body,
                    )?.0;
                ));
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
                ident: Ident::new(&format!("_{i}"), Span::call_site()),
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
            #nested_vis async fn handler #generics(
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

pub fn scope(mut ast: syn::ItemFn) -> TokenStream {
    let orig_ident = ast.sig.ident.clone();
    let orig_vis = ast.vis.clone();

    ast.vis = nested_visibility(ast.vis);
    ast.sig.ident = Ident::new("handler", Span::call_site());

    quote!(#orig_vis mod #orig_ident {
        use super::*;
        #ast
    })
    .into()
}

pub fn route(ast: &mut syn::ExprMatch) {
    let (cx, ty) = match &*ast.expr {
        syn::Expr::MethodCall(call) => {
            let ty = match &call.method {
                id if id == "path" => RouteType::Path,
                id if id == "method" => RouteType::Method,
                m => panic!("unroutable method {m:?}"),
            };

            let cx = match &*call.receiver {
                syn::Expr::Path(p) if p.path.get_ident().is_some() => {
                    p.path.get_ident().unwrap().clone()
                }
                _ => panic!("inner expression must method call on identifier"),
            };

            match ty {
                RouteType::Path => {
                    let expr = &*ast.expr;
                    *ast.expr = parse_quote!(#expr.as_deref());
                }
                RouteType::Method => {
                    let expr = &*ast.expr;
                    *ast.expr = parse_quote!(*#expr);
                }
            }

            (cx, ty)
        }
        _ => panic!("expected method call in match expression"),
    };

    let mut wildcard = false;
    for arm in ast.arms.iter_mut() {
        let mut rewind = false;
        if let syn::Pat::Wild(_) = arm.pat {
            wildcard = true;
            rewind = true;
        }

        if let RouteType::Method = ty {
            match &mut arm.pat {
                syn::Pat::Ident(method) => {
                    arm.pat = parse_quote!(mendes::http::Method::#method);
                }
                _ => panic!("method pattern must be an identifier"),
            }
        }

        match &mut *arm.body {
            syn::Expr::Path(path) => {
                let rewind = rewind.then(|| quote!(#cx.rewind();));
                *arm.body = parse_quote!({
                    #rewind
                    let rsp = #path::handler(#cx.as_mut()).await;
                    ::mendes::application::IntoResponse::into_response(rsp, &*#cx.app, &cx.req)
                });
            }
            syn::Expr::Match(inner) => route(inner),
            _ => panic!("only identifiers, paths and match expressions allowed"),
        }
    }

    if !wildcard {
        let variant = match ty {
            RouteType::Path => quote!(PathNotFound),
            RouteType::Method => quote!(MethodNotAllowed),
        };

        ast.arms.push(parse_quote!(
            _ => {
                let e = ::mendes::Error::#variant;
                ::mendes::application::IntoResponse::into_response(e, &*#cx.app, &cx.req)
            }
        ));
    }
}

enum RouteType {
    Path,
    Method,
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
