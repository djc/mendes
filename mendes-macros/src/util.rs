use quote::ToTokens;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

pub struct FieldParams {
    pub params: Vec<(String, String)>,
}

impl Parse for FieldParams {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let metas = Punctuated::<syn::Meta, Comma>::parse_terminated(input)?;

        let mut params = vec![];
        for meta in metas {
            match meta {
                syn::Meta::NameValue(meta) => {
                    let key = meta.path.get_ident().unwrap().to_string();
                    let value = meta.value.into_token_stream().to_string();
                    let value = value.trim_matches('"').to_string();
                    params.push((key, value))
                }
                syn::Meta::Path(path) => {
                    let key = path.get_ident().unwrap().to_string();
                    params.push((key, "true".into()));
                }
                _ => unimplemented!(),
            }
        }

        Ok(Self { params })
    }
}
