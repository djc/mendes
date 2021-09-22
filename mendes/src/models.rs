use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

pub use mendes_macros::{model, model_type};

#[cfg(feature = "postgres")]
pub mod postgres;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
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
                    write!(fmt, "{};\n\n", def)?;
                }
            }
        }

        write!(fmt, "CREATE TABLE \"{}\" (", self.name)?;
        for (i, col) in self.columns.iter().enumerate() {
            if i > 0 {
                write!(fmt, ",")?;
            }
            write!(fmt, "\n    {}", col)?;
        }
        for constraint in self.constraints.iter() {
            write!(fmt, ",\n    {}", constraint)?;
        }
        write!(fmt, "\n)")
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Column {
    pub name: Cow<'static, str>,
    pub ty: Cow<'static, str>,
    pub null: bool,
    pub default: Option<Cow<'static, str>>,
    pub type_def: Option<Cow<'static, str>>,
}

impl fmt::Display for Column {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "\"{}\" {}", self.name, self.ty)?;
        if !self.null {
            write!(fmt, " NOT NULL")?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum Constraint {
    ForeignKey {
        name: Cow<'static, str>,
        columns: Cow<'static, [Cow<'static, str>]>,
        ref_table: Cow<'static, str>,
        ref_columns: Cow<'static, [Cow<'static, str>]>,
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
                write!(fmt, "CONSTRAINT \"{}\" FOREIGN KEY (", name)?;
                for (i, col) in columns.iter().enumerate() {
                    if i > 0 {
                        write!(fmt, ", ")?;
                    }
                    write!(fmt, "\"{}\"", col)?;
                }
                write!(fmt, ") REFERENCES \"{}\" (", ref_table)?;
                for (i, col) in ref_columns.iter().enumerate() {
                    if i > 0 {
                        write!(fmt, ", ")?;
                    }
                    write!(fmt, "\"{}\"", col)?;
                }
                write!(fmt, ")")
            }
            Constraint::PrimaryKey { name, columns } => {
                write!(fmt, "CONSTRAINT \"{}\" PRIMARY KEY (", name)?;
                for (i, col) in columns.iter().enumerate() {
                    if i > 0 {
                        write!(fmt, ", ")?;
                    }
                    write!(fmt, "\"{}\"", col)?;
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

    fn query() -> QueryBuilder<Sys, Sources<Self>> {
        QueryBuilder {
            sys: PhantomData,
            state: Sources(PhantomData),
        }
    }
}

pub struct QueryBuilder<Sys: System, State: QueryState> {
    sys: PhantomData<Sys>,
    state: State,
}

impl<Sys: System, T: Source> QueryBuilder<Sys, Sources<T>> {
    pub fn sort<F, S: SortStrategy>(self, f: F) -> QueryBuilder<Sys, Sorted<T, S>>
    where
        F: FnOnce(&'static T::Expression) -> S,
    {
        QueryBuilder {
            sys: PhantomData,
            state: Sorted {
                source: self.state.0,
                sort: f(T::expr()),
            },
        }
    }
}

pub struct Sorted<T: Source + ?Sized, S: SortStrategy> {
    source: PhantomData<T>,
    #[allow(dead_code)]
    sort: S,
}

impl<T: Source + ?Sized, S: SortStrategy> QueryState for Sorted<T, S> {}

pub trait SortStrategy {}

pub struct Sources<T: Source + ?Sized>(PhantomData<T>);

impl<T: Source + ?Sized> QueryState for Sources<T> {}

impl<T: ModelMeta + ?Sized> Source for T {
    type Expression = T::Expression;

    fn expr() -> &'static Self::Expression {
        Self::EXPRESSION
    }
}

pub trait QueryState {}

pub trait Source {
    type Expression: 'static;

    fn expr() -> &'static Self::Expression;
}

pub struct ColumnExpr<Table: ModelMeta, Type> {
    pub table: PhantomData<Table>,
    pub ty: PhantomData<Type>,
    pub name: &'static str,
}

pub trait ModelMeta {
    type PrimaryKey;
    type Expression: 'static;

    const TABLE_NAME: &'static str;
    const PRIMARY_KEY_COLUMNS: &'static [Cow<'static, str>];
    const EXPRESSION: &'static Self::Expression;
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
