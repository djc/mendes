use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};

pub use mendes_macros::{model, model_type};

pub struct Table {
    pub name: Cow<'static, str>,
    pub columns: Vec<Column>,
    pub constraints: Vec<Constraint>,
}

impl fmt::Display for Table {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut defined = HashSet::new();
        for col in self.columns.iter() {
            if let Some(def) = &col.type_def {
                if defined.insert(&col.ty) {
                    write!(fmt, "{}; ", def)?;
                }
            }
        }

        write!(fmt, "CREATE TABLE {} (", self.name)?;
        for (i, col) in self.columns.iter().enumerate() {
            if i > 0 {
                write!(fmt, ", ")?;
            }
            write!(fmt, "{}", col)?;
        }
        for constraint in self.constraints.iter() {
            write!(fmt, ", {}", constraint)?;
        }
        write!(fmt, ")")
    }
}

pub struct Column {
    pub name: Cow<'static, str>,
    pub ty: Cow<'static, str>,
    pub null: bool,
    pub default: Option<Cow<'static, str>>,
    pub type_def: Option<Cow<'static, str>>,
}

impl fmt::Display for Column {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{} {}", self.name, self.ty)?;
        if !self.null {
            write!(fmt, " NOT NULL")?;
        }
        Ok(())
    }
}

pub enum Constraint {
    PrimaryKey {
        name: Cow<'static, str>,
        columns: Vec<Cow<'static, str>>,
    },
}

impl fmt::Display for Constraint {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constraint::PrimaryKey { name, columns } => {
                write!(fmt, "CONSTRAINT {} PRIMARY KEY (", name)?;
                for (i, col) in columns.iter().enumerate() {
                    if i > 0 {
                        write!(fmt, ", ")?;
                    }
                    write!(fmt, "{}", col)?;
                }
                write!(fmt, ")")
            }
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Serial<T>(T);

pub trait EnumType {
    const NAME: &'static str;
    const VARIANTS: &'static [&'static str];
}

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

pub trait Model<S: System> {
    fn table() -> Table;
}

pub trait ToColumn<Sys: System> {
    fn to_column(name: Cow<'static, str>, params: &[(&str, &str)]) -> Column;
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

pub struct PostgreSQL {}

impl System for PostgreSQL {}

pub trait System: Sized {
    fn table<M: Model<Self>>() -> Table {
        M::table()
    }
}
