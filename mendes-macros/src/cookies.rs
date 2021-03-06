use proc_macro2::Span;
use quote::quote;

/// Derive the `mendes::cookies::CookieData` trait For the given struct
///
/// Defaults to an expiry time of 6 hours.
pub fn cookie(ast: &syn::ItemStruct) -> proc_macro2::TokenStream {
    let ident = &ast.ident;
    let name = syn::LitStr::new(&ident.to_string(), Span::call_site());
    quote!(
        impl mendes::cookies::CookieData for #ident {
            const NAME: &'static str = #name;

            fn expires() -> Option<std::time::Duration> {
                Some(std::time::Duration::new(60 * 60 * 6, 0))
            }
        }
    )
}
