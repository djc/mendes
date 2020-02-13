use proc_macro2::Span;
use quote::quote;

pub fn cookie(ast: &syn::ItemStruct) -> proc_macro2::TokenStream {
    let ident = &ast.ident;
    let name = syn::LitStr::new(&ident.to_string(), Span::call_site());
    let cookie = quote!(
        impl mendes::cookies::CookieData for #ident {
            fn expires() -> Option<std::time::Duration> {
                Some(std::time::Duration::new(60 * 60 * 6, 0))
            }
            fn from_header<B>(key: &mendes::cookies::Key, req: &mendes::http::Request<B>) -> Option<Self> {
                mendes::cookies::extract(#name, key, req)
            }
            fn to_string(self, key: &mendes::cookies::Key) -> Result<mendes::http::HeaderValue, ()> {
                mendes::cookies::store(#name, key, self)
            }
            fn tombstone() -> Result<mendes::http::HeaderValue, ()> {
                mendes::cookies::tombstone(#name)
            }
        }
    );

    cookie
}
