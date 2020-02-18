use std::mem;

use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

pub fn form(meta: &FormMeta, ast: &mut syn::ItemStruct) -> proc_macro2::TokenStream {
    let fields = match &mut ast.fields {
        syn::Fields::Named(fields) => fields,
        _ => panic!("only structs with named fields are supported"),
    };

    let mut new = proc_macro2::TokenStream::new();
    for field in fields.named.iter_mut() {
        let mut hidden = false;
        let params = if let Some((i, attr)) = field
            .attrs
            .iter_mut()
            .enumerate()
            .find(|(_, a)| a.path.is_ident("form"))
        {
            let input = mem::replace(&mut attr.tokens, proc_macro2::TokenStream::new());
            let mut tokens = proc_macro2::TokenStream::new();
            for (key, value) in syn::parse2::<FieldParams>(input).unwrap().params {
                if key == "type" && value == "hidden" {
                    hidden = true;
                }
                tokens.extend(quote!(
                    (#key, #value),
                ));
            }
            field.attrs.remove(i);
            tokens
        } else {
            quote!()
        };

        let name = field.ident.as_ref().unwrap().to_string();
        let label = if hidden {
            quote!(None)
        } else {
            let label = syn::LitStr::new(&capitalize(&name), Span::call_site());
            quote!(Some(#label.into()))
        };

        let ty = &field.ty;
        new.extend(quote!(
            mendes::forms::Item {
                label: #label,
                field: <#ty as mendes::forms::ToField>::to_field(#name.into(), &[#params]),
            },
        ));
    }

    let submit = syn::LitStr::new(&meta.submit, Span::call_site());
    new.extend(quote!(
        mendes::forms::Item {
            label: None,
            field: mendes::forms::Field::Submit(mendes::forms::Submit {
                name: "submit".into(),
                value: #submit.into(),
            }),
        },
    ));

    let name = &ast.ident;
    let action = &meta.action;
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();
    let display = quote!(
        impl#impl_generics mendes::forms::ToForm for #name#type_generics #where_clause {
            fn to_form() -> mendes::forms::Form {
                mendes::forms::Form {
                    action: Some(#action.into()),
                    enctype: None,
                    method: Some("post".into()),
                    sets: vec![
                        mendes::forms::FieldSet {
                            legend: None,
                            items: vec![
                                #new
                            ],
                        }
                    ],
                }.prepare()
            }
        }
    );

    display
}

pub struct FormMeta {
    action: String,
    submit: String,
}

impl Parse for FormMeta {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (mut action, mut submit) = (None, None);
        for field in Punctuated::<syn::MetaNameValue, Comma>::parse_terminated(input)? {
            if field.path.is_ident("action") {
                match field.lit {
                    syn::Lit::Str(v) => {
                        action = Some(v.value());
                    }
                    _ => panic!("expected string value for key 'action'"),
                }
            } else if field.path.is_ident("submit") {
                match field.lit {
                    syn::Lit::Str(v) => {
                        submit = Some(v.value());
                    }
                    _ => panic!("expected string value for key 'submit'"),
                }
            } else {
                panic!("unexpected field {:?}", field.path.to_token_stream());
            }
        }

        Ok(Self {
            action: action.unwrap(),
            submit: submit.unwrap(),
        })
    }
}

pub struct FieldParams {
    params: Vec<(String, String)>,
}

impl Parse for FieldParams {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        let _ = syn::parenthesized!(content in input);
        let metas = Punctuated::<syn::NestedMeta, Comma>::parse_terminated(&content)?;

        let mut params = vec![];
        for meta in metas {
            if let syn::NestedMeta::Meta(syn::Meta::NameValue(pair)) = meta {
                let key = pair.path.get_ident().unwrap().to_string();
                let value = pair.lit.into_token_stream().to_string();
                let value = value.trim_matches('"').to_string();
                params.push((key, value))
            } else {
                unreachable!()
            }
        }

        Ok(Self { params })
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
