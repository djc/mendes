use proc_macro2::Span;
use quote::{format_ident, quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

/// Derive the `mendes::cookies::CookieData` trait For the given struct
///
/// Defaults to an expiry time of 6 hours.
pub fn cookie(meta: &CookieMeta, ast: &syn::ItemStruct) -> proc_macro2::TokenStream {
    let ident = &ast.ident;
    let name = syn::LitStr::new(&ident.to_string(), Span::call_site());

    let (http_only, max_age, path, secure) =
        (meta.http_only, meta.max_age, &meta.path, meta.secure);
    let domain = match &meta.domain {
        Some(v) => quote!(Some(#v)),
        None => quote!(None),
    };
    let same_site = match &meta.same_site {
        Some(v) => {
            let variant = format_ident!("{}", v);
            quote!(Some(mendes::cookies::SameSite::#variant))
        }
        None => quote!(None),
    };

    quote!(
        impl mendes::cookies::CookieData for #ident {
            fn meta() -> mendes::cookies::CookieMeta<'static> {
                mendes::cookies::CookieMeta {
                    domain: #domain,
                    http_only: #http_only,
                    max_age: #max_age,
                    path: #path,
                    same_site: #same_site,
                    secure: #secure,
                }
            }

            const NAME: &'static str = #name;
        }
    )
}

pub struct CookieMeta {
    domain: Option<String>,
    http_only: bool,
    max_age: u32,
    path: String,
    same_site: Option<String>,
    secure: bool,
}

impl Parse for CookieMeta {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut new = CookieMeta::default();
        for field in Punctuated::<syn::MetaNameValue, Comma>::parse_terminated(input)? {
            let value = match field.value {
                syn::Expr::Lit(v) => v,
                _ => panic!(
                    "expected literal value for key {:?}",
                    field.path.to_token_stream()
                ),
            };

            if field.path.is_ident("domain") {
                match value.lit {
                    syn::Lit::Str(v) => new.domain = Some(v.value()),
                    _ => panic!("expected string value for key 'domain'"),
                }
            } else if field.path.is_ident("http_only") {
                match value.lit {
                    syn::Lit::Bool(v) => {
                        new.http_only = v.value();
                    }
                    _ => panic!("expected string value for key 'http_only'"),
                }
            } else if field.path.is_ident("max_age") {
                match value.lit {
                    syn::Lit::Int(v) => {
                        new.max_age = v
                            .base10_parse::<u32>()
                            .expect("expected u32 value for key 'max_age'");
                    }
                    _ => panic!("expected string value for key 'max_age'"),
                }
            } else if field.path.is_ident("path") {
                match value.lit {
                    syn::Lit::Str(v) => new.path = v.value(),
                    _ => panic!("expected string value for key 'path'"),
                }
            } else if field.path.is_ident("same_site") {
                match value.lit {
                    syn::Lit::Str(v) => {
                        let value = v.value();
                        new.same_site = Some(match value.as_str() {
                            "Strict" => value,
                            "Lax" => value,
                            "None" => value,
                            _ => panic!("expected 'Strict', 'Lax' or 'None' for key 'same_site'"),
                        });
                    }
                    _ => panic!("expected string value for key 'same_site'"),
                }
            } else if field.path.is_ident("secure") {
                match value.lit {
                    syn::Lit::Bool(v) => {
                        new.secure = v.value();
                    }
                    _ => panic!("expected string value for key 'secure'"),
                }
            } else {
                panic!("unexpected key {:?}", field.path.to_token_stream());
            }
        }

        if new.same_site.as_deref() == Some("Strict") && !new.secure {
            panic!("'same_site' is 'Strict' but 'secure' is false");
        }

        Ok(new)
    }
}

impl Default for CookieMeta {
    fn default() -> Self {
        Self {
            domain: None,
            http_only: false,
            max_age: 6 * 60 * 60,
            path: "/".to_owned(),
            same_site: Some("None".to_owned()),
            secure: true,
        }
    }
}
