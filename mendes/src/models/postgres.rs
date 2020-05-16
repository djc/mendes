use std::borrow::Cow;

use super::{Column, EnumType, Serial, System, ToColumn};

pub struct PostgreSQL {}

impl System for PostgreSQL {}

impl<T: EnumType> ToColumn<PostgreSQL> for T {
    fn to_column(name: Cow<'static, str>, _: &[(&str, &str)]) -> Column {
        let ty_name = T::NAME;

        let variants = T::VARIANTS;
        let mut variant_str = String::new();
        for (i, variant) in variants.iter().enumerate() {
            variant_str.push('\'');
            variant_str.push_str(variant);
            variant_str.push('\'');
            if i != variants.len() - 1 {
                variant_str.push_str(", ");
            }
        }

        Column {
            name,
            ty: ty_name.into(),
            null: false,
            default: None,
            type_def: Some(format!("CREATE TYPE {} AS ENUM({})", ty_name, variant_str).into()),
        }
    }
}

impl ToColumn<PostgreSQL> for bool {
    fn to_column(name: Cow<'static, str>, _: &[(&str, &str)]) -> Column {
        Column {
            name,
            ty: "boolean".into(),
            null: false,
            default: None,
            type_def: None,
        }
    }
}

impl ToColumn<PostgreSQL> for Serial<i32> {
    fn to_column(name: Cow<'static, str>, _: &[(&str, &str)]) -> Column {
        Column {
            name,
            ty: "serial".into(),
            null: false,
            default: None,
            type_def: None,
        }
    }
}

impl ToColumn<PostgreSQL> for i32 {
    fn to_column(name: Cow<'static, str>, _: &[(&str, &str)]) -> Column {
        Column {
            name,
            ty: "integer".into(),
            null: false,
            default: None,
            type_def: None,
        }
    }
}

impl ToColumn<PostgreSQL> for i64 {
    fn to_column(name: Cow<'static, str>, _: &[(&str, &str)]) -> Column {
        Column {
            name,
            ty: "bigint".into(),
            null: false,
            default: None,
            type_def: None,
        }
    }
}

impl ToColumn<PostgreSQL> for Vec<u8> {
    fn to_column(name: Cow<'static, str>, _: &[(&str, &str)]) -> Column {
        Column {
            name,
            ty: "bytea".into(),
            null: false,
            default: None,
            type_def: None,
        }
    }
}

impl ToColumn<PostgreSQL> for Cow<'_, str> {
    fn to_column(name: Cow<'static, str>, _: &[(&str, &str)]) -> Column {
        Column {
            name,
            ty: "text".into(),
            null: false,
            default: None,
            type_def: None,
        }
    }
}

impl ToColumn<PostgreSQL> for String {
    fn to_column(name: Cow<'static, str>, _: &[(&str, &str)]) -> Column {
        Column {
            name,
            ty: "text".into(),
            null: false,
            default: None,
            type_def: None,
        }
    }
}

#[cfg(feature = "chrono")]
impl ToColumn<PostgreSQL> for chrono::NaiveDate {
    fn to_column(name: Cow<'static, str>, _: &[(&str, &str)]) -> Column {
        Column {
            name,
            ty: "date".into(),
            null: false,
            default: None,
            type_def: None,
        }
    }
}

#[cfg(feature = "chrono")]
impl ToColumn<PostgreSQL> for chrono::DateTime<chrono::FixedOffset> {
    fn to_column(name: Cow<'static, str>, _: &[(&str, &str)]) -> Column {
        Column {
            name,
            ty: "timestamp with time zone".into(),
            null: false,
            default: None,
            type_def: None,
        }
    }
}
