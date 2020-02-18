use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

pub fn form(meta: &FormMeta, ast: &syn::ItemStruct) -> proc_macro2::TokenStream {
    let fields = match &ast.fields {
        syn::Fields::Named(fields) => fields,
        _ => panic!("only structs with named fields are supported"),
    };

    let mut new = proc_macro2::TokenStream::new();
    for field in &fields.named {
        let name = field.ident.as_ref().unwrap().to_string();
        let label = syn::LitStr::new(&capitalize(&name), Span::call_site());
        let ty = &field.ty;
        new.extend(quote!(
            mendes::forms::Item {
                label: Some(#label),
                field: <#ty as mendes::forms::ToField>::to_field(#name.into(), &[]),
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
                    action: Some(#action),
                    enctype: None,
                    method: Some("post"),
                    sets: vec![
                        mendes::forms::FieldSet {
                            legend: None,
                            items: vec![
                                #new
                            ],
                        }
                    ],
                }
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

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
