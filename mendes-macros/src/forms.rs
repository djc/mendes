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

    let mut html = String::new();
    html.push_str("<form action=\"");
    html.push_str(&meta.action);
    html.push_str("\" method=\"post\">");

    for field in &fields.named {
        let name = &field.ident.as_ref().unwrap().to_string();
        let mut ty_tokens = proc_macro2::TokenStream::new();
        field.ty.to_tokens(&mut ty_tokens);
        let ty_str = ty_tokens.to_string();

        let kind = if ty_str == "String"
            || ty_str.starts_with("& '") && ty_str.ends_with("str")
            || ty_str.starts_with("Cow < '") && ty_str.ends_with("str >")
        {
            FieldKind::String
        } else {
            panic!("unknown field type {}", ty_str);
        };

        html.push_str(&format!(
            "<label for=\"{}\">{}</label>",
            name,
            capitalize(name)
        ));
        let input = match kind {
            FieldKind::String => format!("<input type=\"text\" name=\"{}\">", name),
        };
        html.push_str(&input);
    }

    html.push_str(&format!(
        "<input type=\"submit\" value=\"{}\">",
        meta.submit
    ));
    html.push_str("</form>");

    let name = &ast.ident;
    let fmt = syn::LitStr::new(&html, Span::call_site());
    let display = quote!(
        impl mendes::Form for #name<'_> {
            fn form() -> &'static str {
                #fmt
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

enum FieldKind {
    String,
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
