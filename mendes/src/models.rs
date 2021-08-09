use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

pub use mendes_macros::{model, model_type};

#[cfg(feature = "postgres")]
pub mod postgres;

#[derive(Deserialize, Serialize)]
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

#[derive(Deserialize, Serialize)]
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

#[derive(Deserialize, Serialize)]
pub enum Constraint {
    ForeignKey {
        name: Cow<'static, str>,
        columns: Vec<Cow<'static, str>>,
        ref_table: Cow<'static, str>,
        ref_columns: Vec<Cow<'static, str>>,
    },
    PrimaryKey {
        name: Cow<'static, str>,
        columns: Vec<Cow<'static, str>>,
    },
}

impl fmt::Display for Constraint {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constraint::ForeignKey {
                name,
                columns,
                ref_table,
                ref_columns,
            } => {
                write!(fmt, "CONSTRAINT {} FOREIGN KEY (", name)?;
                for (i, col) in columns.iter().enumerate() {
                    if i > 0 {
                        write!(fmt, ", ")?;
                    }
                    write!(fmt, "{}", col)?;
                }
                write!(fmt, ") REFERENCES {} (", ref_table)?;
                for (i, col) in ref_columns.iter().enumerate() {
                    if i > 0 {
                        write!(fmt, ", ")?;
                    }
                    write!(fmt, "{}", col)?;
                }
                write!(fmt, ")")
            }
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Serial<T>(T);

impl<T> From<T> for Serial<T> {
    fn from(t: T) -> Self {
        Serial(t)
    }
}

pub trait EnumType {
    const NAME: &'static str;
    const VARIANTS: &'static [&'static str];
}

pub trait Model<Sys: System>: ModelMeta {
    fn table() -> Table;
    // TODO: don't use a Vec for this (needs const generics?)
    fn insert(&self) -> (&str, Vec<&Sys::Parameter>);
}

pub trait ModelMeta {
    type PrimaryKey;
    const TABLE_NAME: &'static str;
}

pub trait ModelType<Sys: System> {
    fn value(&self) -> &Sys::Parameter;

    #[allow(clippy::wrong_self_convention)]
    fn to_column(name: Cow<'static, str>, params: &[(&str, &str)]) -> Column;
}

pub trait System: Sized {
    type Parameter: ?Sized;
    type StatementReturn;

    fn table<M: Model<Self>>() -> Table {
        M::table()
    }
}

pub struct Store<Sys: System> {
    tables: HashMap<&'static str, Table>,
    system: PhantomData<Sys>,
}

impl<Sys: System> Store<Sys> {
    pub fn set<M: Model<Sys>>(&mut self) -> &mut Self {
        self.tables.insert(M::TABLE_NAME, M::table());
        self
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &'_ Table)> {
        self.tables.iter().map(|(name, def)| (*name, def))
    }
}

impl<Sys: System> Default for Store<Sys> {
    fn default() -> Self {
        Self {
            tables: HashMap::default(),
            system: PhantomData::default(),
        }
    }
}
