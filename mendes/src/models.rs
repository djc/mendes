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
    pub unique: bool,
    pub default: Option<Cow<'static, str>>,
    pub type_def: Option<Cow<'static, str>>,
}

impl fmt::Display for Column {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "\"{}\" {}", self.name, self.ty)?;
        if !self.null {
            write!(fmt, " NOT NULL")?;
        }
        if self.unique {
            write!(fmt, " UNIQUE")?;
        }
        if let Some(val) = &self.default {
            write!(fmt, " DEFAULT {}", val)?;
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
    fn insert(new: &Self::Insert) -> (String, Vec<&Sys::Parameter>);

    fn builder() -> Self::Builder;

    fn query() -> QueryBuilder<Sys, Sources<Self>> {
        QueryBuilder {
            sys: PhantomData,
            source: Sources(PhantomData),
        }
    }
}

pub struct QueryBuilder<Sys: System, S: Source> {
    sys: PhantomData<Sys>,
    #[allow(dead_code)]
    source: S,
}

impl<Sys: System, S: Source> QueryBuilder<Sys, S> {
    // TODO: rename according to std::slice naming (`sort_by_key`?)
    pub fn sort<F, SK: SortKey>(self, f: F) -> QueryBuilder<Sys, Sorted<S, SK>>
    where
        F: FnOnce(&'static S::Expression) -> SK,
    {
        QueryBuilder {
            sys: PhantomData,
            source: Sorted {
                source: self.source,
                sort_key: f(S::expr()),
            },
        }
    }
}

impl<Sys: System, S: Source> QueryBuilder<Sys, S> {
    pub fn limit(self, limit: u64) -> QueryBuilder<Sys, Paginated<S>> {
        QueryBuilder {
            sys: PhantomData,
            source: Paginated {
                source: self.source,
                limit,
            },
        }
    }
}

impl<Sys: System, S: Source> QueryBuilder<Sys, S> {
    pub fn select<F, V>(self, f: F) -> Query<Sys, S, V>
    where
        F: FnOnce(&'static S::Expression) -> V,
        V: Values<Sys>,
    {
        Query {
            sys: PhantomData,
            source: self.source,
            fields: f(S::expr()),
        }
    }
}

pub struct Query<Sys: System, S: Source, V: Values<Sys>> {
    sys: PhantomData<Sys>,
    #[allow(dead_code)]
    source: S,
    #[allow(dead_code)]
    fields: V,
}

impl<Sys: System, S: Source, V: Values<Sys>> fmt::Display for Query<Sys, S, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SELECT ")?;
        self.fields.fmt(f)?;
        f.write_str(" ")?;
        self.source.fmt(f)
    }
}

pub struct Paginated<S: Source> {
    source: S,
    #[allow(dead_code)]
    limit: u64,
}

impl<S: Source> Source for Paginated<S> {
    type Expression = S::Expression;

    fn expr() -> &'static Self::Expression {
        S::expr()
    }

    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(fmt)?;
        fmt.write_fmt(format_args!(" LIMIT {}", self.limit))
    }
}

pub struct Sorted<S: Source, SK: SortKey> {
    source: S,
    #[allow(dead_code)]
    sort_key: SK,
}

impl<T: Source, S: SortKey> QueryState for Sorted<T, S> {}

impl<S: Source, SK: SortKey> Source for Sorted<S, SK> {
    type Expression = S::Expression;

    fn expr() -> &'static Self::Expression {
        S::expr()
    }

    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(fmt)?;
        fmt.write_str(" ORDER BY ")?;
        self.sort_key.fmt(fmt)
    }
}

pub trait SortKey {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result;
}

#[cfg(feature = "chrono")]
impl<M: ModelMeta> SortKey for ColumnExpr<M, chrono::DateTime<chrono::Utc>> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt(fmt)
    }
}

pub trait QueryState {}

pub trait Source {
    type Expression: 'static;

    fn expr() -> &'static Self::Expression;

    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result;
}

pub struct ColumnExpr<M: ModelMeta, Type> {
    pub table: PhantomData<M>,
    pub ty: PhantomData<Type>,
    pub name: &'static str,
}

impl<M: ModelMeta, Type> ColumnExpr<M, Type> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_fmt(format_args!("{}.{}", M::TABLE_NAME, self.name))
    }
}

impl<M: ModelMeta, Type> Clone for ColumnExpr<M, Type> {
    fn clone(&self) -> Self {
        Self {
            table: self.table,
            ty: self.ty,
            name: self.name,
        }
    }
}

impl<M: ModelMeta, Type> Copy for ColumnExpr<M, Type> {}

pub struct Sources<M: ModelMeta + ?Sized>(PhantomData<M>);

impl<M: ModelMeta + ?Sized> Source for Sources<M> {
    type Expression = M::Expression;

    fn expr() -> &'static Self::Expression {
        M::EXPRESSION
    }

    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_fmt(format_args!("FROM {}", M::TABLE_NAME))
    }
}

pub trait Values<Sys: System> {
    type Output: Sized;

    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result;

    fn build(row: Sys::Row) -> Result<Self::Output, Sys::Error>;
}

pub trait ModelMeta {
    type PrimaryKey;
    type Expression: 'static;
    type Builder;
    type Insert;

    const TABLE_NAME: &'static str;
    const PRIMARY_KEY_COLUMNS: &'static [Cow<'static, str>];
    const EXPRESSION: &'static Self::Expression;
}

pub trait ModelType<Sys: System> {
    fn value(&self) -> &Sys::Parameter;

    #[allow(clippy::wrong_self_convention)]
    fn to_column(name: Cow<'static, str>, params: &[(&str, &'static str)]) -> Column;
}

pub trait System: Sized {
    type Parameter: ?Sized;
    type StatementReturn;
    type Row;
    type Error;

    fn table<M: Model<Self>>() -> Table {
        M::table()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Defaulted<T> {
    Value(T),
    Default,
}

impl<T> Defaulted<T> {
    pub fn unwrap_or<'a>(&'a self, alt: &'a T) -> &'a T {
        match self {
            Self::Value(val) => val,
            Self::Default => alt,
        }
    }
}

impl<T> Default for Defaulted<T> {
    fn default() -> Self {
        Defaulted::Default
    }
}

impl<T> From<T> for Defaulted<T> {
    fn from(val: T) -> Self {
        Defaulted::Value(val)
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
