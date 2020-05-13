use quote::ToTokens;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

pub struct FieldParams {
    pub params: Vec<(String, String)>,
}

impl Parse for FieldParams {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        let _ = syn::parenthesized!(content in input);
        let metas = Punctuated::<syn::NestedMeta, Comma>::parse_terminated(&content)?;

        let mut params = vec![];
        for meta in metas {
            match meta {
                syn::NestedMeta::Meta(syn::Meta::NameValue(pair)) => {
                    let key = pair.path.get_ident().unwrap().to_string();
                    let value = pair.lit.into_token_stream().to_string();
                    let value = value.trim_matches('"').to_string();
                    params.push((key, value))
                }
                syn::NestedMeta::Meta(syn::Meta::Path(path)) => {
                    let key = path.get_ident().unwrap().to_string();
                    params.push((key, "true".into()));
                }
                _ => unimplemented!(),
            }
        }

        Ok(Self { params })
    }
}
