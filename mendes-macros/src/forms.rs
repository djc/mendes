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

    let mut item_state = None;
    let mut new = proc_macro2::TokenStream::new();
    for field in fields.named.iter_mut() {
        let name = field.ident.as_ref().unwrap().to_string();
        let mut label = {
            let label = syn::LitStr::new(&label(&name), Span::call_site());
            quote!(Some(#label.into()))
        };
        let mut item = None;

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
                    label = quote!(None);
                } else if key == "label" {
                    label = quote!(Some(#value.into()));
                } else if key == "item" {
                    item = Some(value.clone());
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

        let ty = &field.ty;
        let tokens = quote!(
            mendes::forms::Item {
                label: #label,
                contents: mendes::forms::ItemContents::Single(
                    <#ty as mendes::forms::ToField>::to_field(#name.into(), &[#params])
                ),
            },
        );

        item_state = match item_state {
            None if item.is_none() => {
                new.extend(tokens);
                None
            }
            None => Some((item.unwrap(), tokens)),
            Some((name, mut items)) => match item {
                Some(cur) if cur == name => {
                    items.extend(tokens);
                    Some((name, items))
                }
                Some(cur) => {
                    let label = syn::LitStr::new(&name, Span::call_site());
                    new.extend(quote!(
                        mendes::forms::Item {
                            label: Some(#label.into()),
                            contents: mendes::forms::ItemContents::Multi(vec![#items]),
                        },
                    ));
                    Some((cur, tokens))
                }
                None => {
                    let label = syn::LitStr::new(&name, Span::call_site());
                    new.extend(quote!(
                        mendes::forms::Item {
                            label: Some(#label.into()),
                            contents: mendes::forms::ItemContents::Multi(vec![#items]),
                        },
                    ));
                    None
                }
            },
        }
    }

    let submit = syn::LitStr::new(&meta.submit, Span::call_site());
    new.extend(quote!(
        mendes::forms::Item {
            label: None,
            contents: mendes::forms::ItemContents::Single(
                mendes::forms::Field::Submit(mendes::forms::Submit {
                    name: "submit".into(),
                    value: #submit.into(),
                })
            ),
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

pub fn to_field(mut ast: syn::DeriveInput) -> proc_macro2::TokenStream {
    let item = match &mut ast.data {
        syn::Data::Enum(item) => item,
        _ => panic!("only enums can derive ToField for now"),
    };

    let mut options = proc_macro2::TokenStream::new();
    for variant in item.variants.iter_mut() {
        match variant.fields {
            syn::Fields::Unit => {}
            _ => panic!("only unit variants are supported for now"),
        };

        let params = if let Some((i, attr)) = variant
            .attrs
            .iter_mut()
            .enumerate()
            .find(|(_, a)| a.path.is_ident("option"))
        {
            let input = mem::replace(&mut attr.tokens, proc_macro2::TokenStream::new());
            let params = syn::parse2::<FieldParams>(input).unwrap().params;
            variant.attrs.remove(i);
            params
        } else {
            vec![]
        };

        let name = variant.ident.to_string();
        let label = params
            .iter()
            .find_map(|(key, value)| {
                if key == "label" {
                    Some(quote!(#value.into()))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| quote!(#name));

        options.extend(quote!(
            mendes::forms::SelectOption {
                label: #label,
                value: #name.into(),
                disabled: false,
                selected: false,
            },
        ));
    }

    let ident = &ast.ident;
    quote!(
        impl ToField for #ident {
            fn to_field(name: std::borrow::Cow<'static, str>, _: &[(&str, &str)]) -> mendes::forms::Field {
                mendes::forms::Field::Select(mendes::forms::Select {
                    name,
                    options: vec![#options],
                })
            }
        }
    )
}

fn label(s: &str) -> String {
    let mut new = String::with_capacity(s.len());
    for (i, c) in s.chars().enumerate() {
        if i == 0 {
            new.extend(c.to_uppercase());
        } else if c == '_' {
            new.push(' ');
        } else {
            new.push(c);
        }
    }
    new
}
