extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Punct, Spacing};
use quote::{format_ident, quote, ToTokens, TokenStreamExt};
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

    let mod_name = format_ident!("{}__mod", ast.sig.ident);
    let Map { routes } = expr.parse_body().unwrap();
    let mut route_tokens = proc_macro2::TokenStream::new();
    for (i, route) in routes.iter().enumerate() {
        route.component.to_tokens(&mut route_tokens);
        route_tokens.append(Punct::new('=', Spacing::Joint));
        route_tokens.append(Punct::new('>', Spacing::Alone));

        let variant = format_ident!("V{}", i);
        let handler = &route.handler;
        route_tokens.append_all(quote!(#mod_name::Future::#variant(#handler(app, req))));
        route_tokens.append(Punct::new(',', Spacing::Alone));
    }

    let block = quote!({
        let path = req.uri().path();
        let component = match path[1..].find('/') {
            Some(pos) => &path[1..pos],
            None => "",
        };

        let future = match component { #route_tokens };

        match future.await {
            Ok(rsp) => Ok(rsp),
            Err(e) => Ok(app.error(e)),
        }
    });

    let mut type_args = proc_macro2::TokenStream::new();
    let mut variants = proc_macro2::TokenStream::new();
    let mut wheres = proc_macro2::TokenStream::new();
    let mut polls = proc_macro2::TokenStream::new();
    for i in 0..routes.len() {
        let var = format_ident!("T{}", i);
        type_args.append_all(quote!(#var));
        type_args.append(Punct::new(',', Spacing::Alone));

        let variant = format_ident!("V{}", i);
        variants.append_all(quote!(#variant(#var)));
        variants.append(Punct::new(',', Spacing::Alone));

        wheres.append_all(quote!(#var:));
        if i == 0 {
            wheres.append_all(quote!(::std::future::Future));
        } else {
            wheres.append_all(quote!(::std::future::Future<Output = T0::Output>));
        }
        wheres.append(Punct::new(',', Spacing::Alone));

        polls.append_all(quote!(
            Future::#variant(f) => ::std::pin::Pin::new_unchecked(f).poll(cx),
        ));
    }

    let future = quote!(pub enum Future<#type_args> { #variants });

    let future_impl = quote!(
        impl<#type_args> ::std::future::Future for Future<#type_args> where #wheres {
            type Output = T0::Output;

            fn poll(
                self: ::std::pin::Pin<&mut Self>,
                cx: &mut ::std::task::Context
            ) -> ::std::task::Poll<Self::Output> {
                unsafe {
                    match self.get_unchecked_mut() { #polls }
                }
            }
        }
    );

    let block: syn::Block = syn::parse(TokenStream::from(block)).unwrap();
    ast.block = Box::new(block);
    let mut tokens = ast.to_token_stream();
    tokens.append_all(quote!(
        #[allow(non_snake_case)]
        mod #mod_name {
            #future
            #future_impl
        }
    ));

    TokenStream::from(tokens)
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
