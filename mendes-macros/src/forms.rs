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
        let mut skip = false;

        let params = if let Some((i, attr)) = field
            .attrs
            .iter_mut()
            .enumerate()
            .find(|(_, a)| a.path().is_ident("form"))
        {
            let input = match &mut attr.meta {
                syn::Meta::List(list) => {
                    mem::replace(&mut list.tokens, proc_macro2::TokenStream::new())
                }
                _ => panic!("expected list in form attribute"),
            };

            let mut tokens = proc_macro2::TokenStream::new();
            for (key, value) in syn::parse2::<FieldParams>(input).unwrap().params {
                if key == "type" && value == "hidden" {
                    label = quote!(None);
                } else if key == "label" {
                    label = quote!(Some(#value.into()));
                } else if key == "item" {
                    item = Some(value.clone());
                } else if key == "skip" {
                    skip = true;
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

        if skip {
            continue;
        }

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
                    new.extend(tokens);
                    None
                }
            },
        }
    }

    let FormMeta {
        action,
        classes,
        submit,
    } = &meta;
    let submit = match submit {
        Some(s) => quote!(Some(#s.into())),
        None => quote!(None),
    };

    new.extend(quote!(
        mendes::forms::Item {
            label: None,
            contents: mendes::forms::ItemContents::Single(
                mendes::forms::Field::Submit(mendes::forms::Submit {
                    value: #submit,
                })
            ),
        },
    ));

    let action = match action {
        Some(s) => quote!(Some(#s.into())),
        None => quote!(None),
    };

    let name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();
    let display = quote!(
        impl #impl_generics mendes::forms::ToForm for #name #type_generics #where_clause {
            fn to_form() -> mendes::forms::Form {
                mendes::forms::Form {
                    action: #action,
                    enctype: None,
                    method: Some("post".into()),
                    classes: #classes,
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
    action: Option<String>,
    submit: Option<String>,
    classes: proc_macro2::TokenStream,
}

impl Parse for FormMeta {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (mut action, mut submit, mut classes) = (None, None, quote!(vec![]));
        for field in Punctuated::<syn::MetaNameValue, Comma>::parse_terminated(input)? {
            let value = match field.value {
                syn::Expr::Lit(v) => v,
                _ => panic!(
                    "expected literal value for key {:?}",
                    field.path.to_token_stream()
                ),
            };

            if field.path.is_ident("action") {
                match value.lit {
                    syn::Lit::Str(v) => {
                        action = Some(v.value());
                    }
                    _ => panic!("expected string value for key 'action'"),
                }
            } else if field.path.is_ident("submit") {
                match value.lit {
                    syn::Lit::Str(v) => {
                        submit = Some(v.value());
                    }
                    _ => panic!("expected string value for key 'submit'"),
                }
            } else if field.path.is_ident("class") {
                match value.lit {
                    syn::Lit::Str(v) => {
                        let val = v.value();
                        let iter = val.split(' ');
                        classes = quote!(vec![#(#iter.into()),*]);
                    }
                    _ => panic!("expected string value for key 'class'"),
                }
            } else {
                panic!("unexpected field {:?}", field.path.to_token_stream());
            }
        }

        Ok(Self {
            action,
            submit,
            classes,
        })
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
            .find(|(_, a)| a.path().is_ident("option"))
        {
            let input = match &mut attr.meta {
                syn::Meta::List(list) => {
                    mem::replace(&mut list.tokens, proc_macro2::TokenStream::new())
                }
                _ => panic!("expected list in form attribute"),
            };

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
            .unwrap_or_else(|| quote!(#name.into()));

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

pub struct FieldParams {
    pub params: Vec<(String, String)>,
}

impl Parse for FieldParams {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            params: Punctuated::<syn::Meta, Comma>::parse_terminated(input)?
                .into_iter()
                .map(|meta| match meta {
                    syn::Meta::NameValue(meta) => {
                        let key = meta.path.get_ident().unwrap().to_string();
                        let value = meta.value.into_token_stream().to_string();
                        let value = value.trim_matches('"').to_string();
                        (key, value)
                    }
                    syn::Meta::Path(path) => {
                        let key = path.get_ident().unwrap().to_string();
                        (key, "true".into())
                    }
                    _ => unimplemented!(),
                })
                .collect(),
        })
    }
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
