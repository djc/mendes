use std::collections::HashSet;
use std::fmt::Write;
use std::str::FromStr;

use darling::FromMeta;
use proc_macro2::Span;
use quote::quote;
use syn::ext::IdentExt;

pub fn model(ast: &mut syn::ItemStruct) -> proc_macro2::TokenStream {
    let fields = match &mut ast.fields {
        syn::Fields::Named(fields) => fields,
        _ => panic!("only structs with named fields are supported"),
    };

    let name = &ast.ident;
    let visibility = &ast.vis;
    let table_name = name.to_string().to_lowercase();

    let mut id_type = None;
    let mut pkey = None;
    let mut bounds = HashSet::new();
    let mut columns = proc_macro2::TokenStream::new();
    let mut constraints = proc_macro2::TokenStream::new();
    let mut column_names = Vec::with_capacity(fields.named.len());
    let mut expr_type_fields = proc_macro2::TokenStream::new();
    let mut expr_instance_fields = proc_macro2::TokenStream::new();
    let mut builder_fields = vec![];
    let mut required_fields = 0;
    for field in fields.named.iter_mut() {
        let col_name = field.ident.as_ref().unwrap().unraw().to_string();
        column_names.push(col_name.clone());

        let ty = match &field.ty {
            syn::Type::Path(ty) => ty,
            _ => panic!("unsupported type"),
        };

        if col_name == "id" {
            id_type = Some(ty);
        }

        let mut attr = None;
        field.attrs.retain(|a| {
            let meta = a.parse_meta().ok();
            match meta.and_then(|meta| FieldAttribute::from_meta(&meta).ok()) {
                Some(a) => {
                    attr = Some(a);
                    false
                }
                None => true,
            }
        });

        let attrs = attr.unwrap_or_default();
        if attrs.primary_key {
            pkey = Some((&field.ident, ty));
        }

        let field_name = field.ident.as_ref().unwrap();
        let outer_type_segment = &ty.path.segments.last().unwrap();
        let outer_type_name = &outer_type_segment.ident;
        let mut column_params = proc_macro2::TokenStream::new();
        if attrs.unique {
            column_params.extend(quote!(("unique", "")));
        }

        if let Some(val) = attrs.default {
            if outer_type_name == "Option" {
                panic!("default values not allowed on Option-typed fields");
            }

            let str_val = match val {
                syn::Lit::Str(inner) => {
                    let value = inner.value();
                    match value.ends_with(')') {
                        true => value,
                        false => format!("'{}'", value),
                    }
                }
                syn::Lit::ByteStr(_) => todo!(),
                syn::Lit::Byte(_) => todo!(),
                syn::Lit::Char(_) => todo!(),
                syn::Lit::Int(inner) => inner.base10_digits().to_string(),
                syn::Lit::Float(inner) => inner.base10_digits().to_string(),
                syn::Lit::Bool(inner) => format!("{}", inner.value()),
                syn::Lit::Verbatim(_) => todo!(),
            };
            column_params.extend(quote!(("default", #str_val),));
            bounds.insert(
                quote!(
                    ::mendes::models::Defaulted<#ty>: mendes::models::ModelType<Sys>
                )
                .to_string(),
            );

            builder_fields.push((field_name, ty, false));
        } else if outer_type_name == "Serial" || outer_type_name == "Option" {
            let inner_ty = match &outer_type_segment.arguments {
                syn::PathArguments::AngleBracketed(args) => match args.args.first() {
                    Some(syn::GenericArgument::Type(syn::Type::Path(ty))) => ty,
                    _ => panic!("unsupported Serial argument type"),
                },
                _ => panic!("unsupported Serial argument type"),
            };

            bounds.insert(
                quote!(
                    #inner_ty: mendes::models::ModelType<Sys>
                )
                .to_string(),
            );

            builder_fields.push((field_name, inner_ty, false));
        } else {
            builder_fields.push((field_name, ty, true));
            required_fields += 1;
        }

        if ty.path.segments.last().unwrap().ident == "PrimaryKey" {
            let mut ref_table = ty.clone();
            let last = ref_table.path.segments.last_mut().unwrap();
            last.ident = syn::Ident::new("TABLE_NAME", Span::call_site());

            let mut ref_columns = ty.clone();
            let last = ref_columns.path.segments.last_mut().unwrap();
            last.ident = syn::Ident::new("PRIMARY_KEY_COLUMNS", Span::call_site());
            constraints.extend(quote!(
                mendes::models::Constraint::ForeignKey {
                    name: #col_name.into(),
                    columns: ::std::borrow::Cow::Borrowed(&[::std::borrow::Cow::Borrowed(#col_name)]),
                    ref_table: #ref_table.into(),
                    ref_columns: ::std::borrow::Cow::Borrowed(#ref_columns),
                },
            ));
        }

        bounds.insert(quote!(#ty: mendes::models::ModelType<Sys>).to_string());
        columns.extend(quote!(
            <#ty as mendes::models::ModelType<Sys>>::to_column(#col_name.into(), &[#column_params]),
        ));

        expr_type_fields.extend(quote!(
            #visibility #field_name: ::mendes::models::ColumnExpr<#name, #ty>,
        ));
        expr_instance_fields.extend(quote!(#field_name: ::mendes::models::ColumnExpr {
            table: ::std::marker::PhantomData,
            ty: ::std::marker::PhantomData,
            name: #col_name,
        },));
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

    let pkey_ty = if let Some((name, ty)) = pkey {
        let cname = format!("{}_pkey", table_name);
        let name = format!("{}", name.as_ref().unwrap());
        constraints.extend(quote!(
            mendes::models::Constraint::PrimaryKey {
                name: #cname.into(),
                columns: vec![#name.into()],
            },
        ));

        let segment = ty.path.segments.last().unwrap();
        let pkey_ty = if segment.ident == "Serial" {
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
        bounds.insert(quote!(#pkey_ty: mendes::models::ModelType<Sys>).to_string());
        pkey_ty
    } else if let Some(ty) = id_type {
        let cname = format!("{}_pkey", table_name);
        constraints.extend(quote!(
            mendes::models::Constraint::PrimaryKey {
                name: #cname.into(),
                columns: vec!["id".into()],
            },
        ));

        let segment = ty.path.segments.last().unwrap();
        let pkey_ty = if segment.ident == "Serial" {
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
        bounds.insert(quote!(#pkey_ty: mendes::models::ModelType<Sys>).to_string());
        pkey_ty
    } else {
        panic!("no primary key found for type {:?}", name);
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

    let mut placeholders = String::with_capacity(column_names.len() * 4);
    for i in 0..column_names.len() {
        placeholders
            .write_fmt(format_args!("${}, ", i + 1))
            .unwrap();
    }
    placeholders.pop();
    placeholders.pop();

    let mut query_fmt = proc_macro2::TokenStream::new();
    query_fmt.extend(quote!(
        use std::fmt::Write;
        use ::mendes::models::ModelType;
        let mut num_name_params = 0;
    ));
    let mut query_values = proc_macro2::TokenStream::new();
    query_values.extend(quote!(let mut num_value_params = 0;));
    for (fname, _, req) in &builder_fields {
        let name_str = fname.to_string();
        if *req {
            query_fmt.extend(quote!(
                if num_name_params > 0 {
                    sql.push_str(", ");
                }
                sql.write_fmt(format_args!("\"{}\"", #name_str)).unwrap();
                num_name_params += 1;
            ));
        } else {
            query_fmt.extend(quote!(if new.#fname.is_some() {
                if num_name_params > 0 {
                    sql.push_str(", ");
                }
                sql.write_fmt(format_args!("\"{}\"", #name_str)).unwrap();
                num_name_params += 1;
            }));
        }

        if *req {
            query_values.extend(quote!(
                if num_value_params > 0 {
                    sql.push_str(", ");
                }
                sql.write_fmt(format_args!("${}", num_value_params + 1)).unwrap();
                num_value_params += 1;
                params.push(new.#fname.value());
            ));
        } else {
            query_values.extend(quote!(
                if let Some(val) = &new.#fname {
                    if num_value_params > 0 {
                        sql.push_str(", ");
                    }
                    sql.write_fmt(format_args!("${}", num_value_params + 1)).unwrap();
                    num_value_params += 1;
                    params.push(val.value());
                }
            ));
        }
    }

    let builder_state_start = syn::Ident::new(&format!("{}State0", name), Span::call_site());
    let expr_type_name = syn::Ident::new(&format!("{}Expression", name), Span::call_site());
    let orig_impl_generics = ast.generics.split_for_impl().0;
    let insert_state_name = syn::Ident::new(
        &format!("{}State{}", name, required_fields),
        Span::call_site(),
    );
    let mut impls = quote!(
        impl#orig_impl_generics mendes::models::ModelMeta for #name#type_generics #where_clause {
            type PrimaryKey = #pkey_ty;
            type Expression = #expr_type_name;
            type Builder = #builder_state_start;
            type Insert = #insert_state_name;

            const TABLE_NAME: &'static str = #table_name;
            const PRIMARY_KEY_COLUMNS: &'static [::std::borrow::Cow<'static, str>] = &[
                ::std::borrow::Cow::Borrowed("id"),
            ];
            const EXPRESSION: &'static #expr_type_name = &#expr_type_name {
                #expr_instance_fields
            };

        }

        impl#impl_generics mendes::models::Model<Sys> for #name#type_generics #where_clause #bounds {
            fn table() -> mendes::models::Table {
                mendes::models::Table {
                    name: #table_name.into(),
                    columns: vec![#columns],
                    constraints: vec![#constraints],
                }
            }

            fn builder() -> Self::Builder {
                <Self::Builder as Default>::default()
            }

            fn insert(new: &Self::Insert) -> (String, Vec<&Sys::Parameter>) {
                let mut sql = String::with_capacity(64);
                let mut params = Vec::with_capacity(8);
                sql.push_str(concat!("INSERT INTO \"", #table_name, "\" (\n    "));
                #query_fmt
                sql.push_str("\n) VALUES (\n    ");
                #query_values
                sql.push_str("\n)");
                (sql, params)
            }
        }

        #visibility struct #expr_type_name { #expr_type_fields }

    );

    let mut seen = 0;
    for i in 0..(required_fields + 1) {
        let state_name = syn::Ident::new(&format!("{}State{}", name, i), Span::call_site());
        let required = builder_fields[seen..]
            .iter()
            .position(|(_, _, required)| *required);

        let required = match required {
            Some(val) => val + seen,
            None => builder_fields.len(),
        };

        let mut state_fields = proc_macro2::TokenStream::new();
        let mut transition_fields = proc_macro2::TokenStream::new();
        let mut optional_methods = proc_macro2::TokenStream::new();
        for (i, (fname, ty, req)) in builder_fields.iter().enumerate() {
            if *req && i > required {
                break;
            }

            if !*req && i >= required {
                transition_fields.extend(quote!(
                    #fname: None,
                ));
            } else if i != required {
                transition_fields.extend(quote!(
                    #fname: self.#fname,
                ));
            }

            if i < required && i >= seen {
                optional_methods.extend(quote!(
                    #visibility fn #fname(mut self, #fname: #ty) -> Self {
                        self.#fname = Some(#fname);
                        self
                    }
                ));
            }

            if i == required {
                transition_fields.extend(quote!(
                    #fname,
                ));
            }

            if i >= required {
                continue;
            }

            match req {
                true => state_fields.extend(quote!(#fname: #ty,)),
                false => state_fields.extend(quote!(#fname: ::core::option::Option<#ty>,)),
            }
        }

        if required < builder_fields.len() {
            if i == 0 {
                impls.extend(quote!(#[derive(Default)]));
            }

            let transition_name = &builder_fields[required].0;
            let transition_type = &builder_fields[required].1;
            let next_state = syn::Ident::new(&format!("{}State{}", name, i + 1), Span::call_site());

            impls.extend(quote!(
                #visibility struct #state_name { #state_fields }

                impl #state_name {
                    #optional_methods
                    #visibility fn #transition_name(self, #transition_name: #transition_type) -> #next_state {
                        #next_state { #transition_fields }
                    }
                }
            ));
        } else {
            impls.extend(quote!(
                #visibility struct #state_name { #state_fields }
                impl #state_name {
                    #optional_methods
                }
            ));
        }

        seen = required + 1;
    }

    impls
}

#[derive(Debug, Default, FromMeta)]
struct FieldAttribute {
    #[darling(default)]
    primary_key: bool,
    #[darling(default)]
    unique: bool,
    #[darling(default)]
    default: Option<syn::Lit>,
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
        impl<Sys> mendes::models::ModelType<Sys> for #name
        where
            Sys: mendes::models::System,
            #wrapped: mendes::models::ModelType<Sys>,
        {
            fn value(&self) -> &Sys::Parameter { self.0.value() }

            fn to_column(name: std::borrow::Cow<'static, str>, data: &[(&str, &'static str)]) -> mendes::models::Column {
                #wrapped::to_column(name, data)
            }
        }
    )
}
