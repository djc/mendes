use std::collections::HashSet;
use std::mem;
use std::str::FromStr;

use proc_macro2::Span;
use quote::quote;
use syn::ext::IdentExt;

pub fn model(ast: &mut syn::ItemStruct) -> proc_macro2::TokenStream {
    let fields = match &mut ast.fields {
        syn::Fields::Named(fields) => fields,
        _ => panic!("only structs with named fields are supported"),
    };

    let name = &ast.ident;
    let mut table_name = name.to_string().to_lowercase();
    if table_name.ends_with('s') {
        table_name.push_str("es");
    } else {
        table_name.push('s');
    }

    let mut pkey_ty = None;
    let mut bounds = HashSet::new();
    let mut columns = proc_macro2::TokenStream::new();
    let mut constraints = proc_macro2::TokenStream::new();
    for field in fields.named.iter_mut() {
        let name = field.ident.as_ref().unwrap().unraw().to_string();
        if name == "id" {
            let cname = format!("{}_pkey", table_name);
            constraints.extend(quote!(
                mendes::models::Constraint::PrimaryKey {
                    name: #cname.into(),
                    columns: vec!["id".into()],
                },
            ));
        }

        let ty = match &field.ty {
            syn::Type::Path(ty) => ty,
            _ => panic!("unsupported type"),
        };

        if name == "id" {
            let segment = ty.path.segments.last().unwrap();
            pkey_ty = if segment.ident == "Serial" {
                match &segment.arguments {
                    syn::PathArguments::AngleBracketed(args) => match args.args.first() {
                        Some(syn::GenericArgument::Type(syn::Type::Path(ty))) => Some(ty),
                        _ => panic!("unsupported Serial argument type"),
                    },
                    _ => panic!("unsupported Serial argument type"),
                }
            } else {
                Some(ty)
            };
            bounds.insert(quote!(#pkey_ty: mendes::models::ToColumn<Sys>).to_string());
        }

        if ty.path.segments.last().unwrap().ident == "PrimaryKey" {
            let mut ref_table = ty.clone();
            let last = ref_table.path.segments.last_mut().unwrap();
            mem::replace(
                &mut last.ident,
                syn::Ident::new("TABLE_NAME", Span::call_site()),
            );
            constraints.extend(quote!(
                mendes::models::Constraint::ForeignKey {
                    name: #name.into(),
                    columns: vec![#name.into()],
                    ref_table: #ref_table.into(),
                    ref_columns: vec!["id".into()],
                },
            ));
        }

        bounds.insert(quote!(#ty: mendes::models::ToColumn<Sys>).to_string());
        columns.extend(quote!(
            <#ty as mendes::models::ToColumn<Sys>>::to_column(#name.into(), &[]),
        ));
    }

    let system = ast.generics.params.iter().any(|param| {
        // TODO: make this more robust
        match param {
            syn::GenericParam::Type(ty_param) => ty_param.ident == "Sys",
            _ => false,
        }
    });

    let mut generics = ast.generics.clone();
    let (impl_generics, type_generics, where_clause) = if !system {
        bounds.insert(quote!(Sys: mendes::models::System).to_string());
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

        let (_, type_generics, where_clause) = ast.generics.split_for_impl();
        let (impl_generics, _, _) = generics.split_for_impl();
        (impl_generics, type_generics, where_clause)
    } else {
        ast.generics.split_for_impl()
    };

    let bounds = bounds.iter().enumerate().fold(
        if where_clause.is_none() {
            quote!(where)
        } else {
            quote!(,)
        },
        |mut tokens, (i, bound)| {
            if i > 0 {
                tokens.extend(quote!(,));
            }
            tokens.extend(proc_macro2::TokenStream::from_str(bound).unwrap());
            tokens
        },
    );

    let orig_impl_generics = ast.generics.split_for_impl().0;
    let impls = quote!(
        impl#orig_impl_generics mendes::models::ModelMeta for #name#type_generics #where_clause {
            type PrimaryKey = #pkey_ty;
            const TABLE_NAME: &'static str = #table_name;
        }

        impl#impl_generics mendes::models::Model<Sys> for #name#type_generics #where_clause #bounds {
            fn table() -> mendes::models::Table {
                mendes::models::Table {
                    name: #table_name.into(),
                    columns: vec![#columns],
                    constraints: vec![#constraints],
                }
            }
        }
    );

    impls
}

pub fn model_type(ast: &mut syn::Item) -> proc_macro2::TokenStream {
    match ast {
        syn::Item::Enum(e) => enum_type(e),
        syn::Item::Struct(s) => match &s.fields {
            syn::Fields::Unnamed(f) if f.unnamed.len() == 1 => newtype_type(s),
            _ => panic!("unsupported type for model type"),
        },
        _ => panic!("unsupported type for model type"),
    }
}

fn enum_type(ty: &syn::ItemEnum) -> proc_macro2::TokenStream {
    let mut variants = proc_macro2::TokenStream::new();
    for variant in &ty.variants {
        let name = variant.ident.to_string();
        variants.extend(quote!(#name, ));
    }

    let name = &ty.ident;
    let name_str = name.to_string();
    let (impl_generics, type_generics, where_clause) = ty.generics.split_for_impl();

    quote!(
        impl#impl_generics mendes::models::EnumType for #name#type_generics #where_clause {
            const NAME: &'static str = #name_str;
            const VARIANTS: &'static [&'static str] = &[#variants];
        }
    )
}

fn newtype_type(ty: &syn::ItemStruct) -> proc_macro2::TokenStream {
    let name = &ty.ident;
    let wrapped = if let syn::Fields::Unnamed(fu) = &ty.fields {
        &fu.unnamed.first().unwrap().ty
    } else {
        panic!("invalid");
    };

    quote!(
        impl<Sys> mendes::models::ToColumn<Sys> for #name
        where
            Sys: mendes::models::System,
            #wrapped: mendes::models::ToColumn<Sys>,
        {
            fn to_column(name: std::borrow::Cow<'static, str>, data: &[(&str, &str)]) -> mendes::models::Column {
                #wrapped::to_column(name, data)
            }
        }
    )
}
