use std::collections::HashSet;
use std::str::FromStr;

use proc_macro2::Span;
use quote::quote;

pub fn model(ast: &mut syn::ItemStruct) -> proc_macro2::TokenStream {
    let fields = match &mut ast.fields {
        syn::Fields::Named(fields) => fields,
        _ => panic!("only structs with named fields are supported"),
    };

    let models_path = if cfg!(feature = "path-mendes") {
        quote!(mendes::models)
    } else {
        quote!(mendes_models)
    };

    let mut bounds = HashSet::new();
    bounds.insert(quote!(#models_path::System).to_string());
    let mut columns = proc_macro2::TokenStream::new();
    for field in fields.named.iter_mut() {
        let name = field.ident.as_ref().unwrap().to_string();
        let ty = &field.ty;
        bounds.insert(quote!(#models_path::ToColumn<#ty>).to_string());
        columns.extend(quote!(
            <Sys as #models_path::ToColumn<#ty>>::to_column(#name.into(), &[]),
        ));
    }

    let name = &ast.ident;
    let mut table_name = name.to_string().to_lowercase();
    if table_name.ends_with('s') {
        table_name.push_str("es");
    } else {
        table_name.push('s');
    }

    let mut generics = ast.generics.clone();
    generics.params.push(
        syn::TypeParam {
            attrs: vec![],
            ident: syn::Ident::new("Sys", Span::call_site()),
            colon_token: None,
            bounds: syn::punctuated::Punctuated::new(),
            eq_token: None,
            default: None,
        }
        .into(),
    );

    let (_, type_generics, where_clause) = &ast.generics.split_for_impl();
    let (impl_generics, _, _) = generics.split_for_impl();
    let bounds = bounds
        .iter()
        .enumerate()
        .fold(quote!(where Sys:), |mut tokens, (i, bound)| {
            if i > 0 {
                tokens.extend(quote!(+));
            }
            tokens.extend(proc_macro2::TokenStream::from_str(bound).unwrap());
            tokens
        });

    let impls = quote!(
        impl#impl_generics #models_path::Model<Sys> for #name#type_generics #where_clause #bounds {
            fn table() -> #models_path::Table {
                #models_path::Table {
                    name: #table_name.into(),
                    columns: vec![#columns],
                    constraints: vec![],
                }
            }
        }
    );

    impls
}
