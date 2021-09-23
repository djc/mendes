#![allow(clippy::wrong_self_convention)] // https://github.com/rust-lang/rust-clippy/issues/7374

use std::borrow::Cow;
use std::error::Error as StdError;
use std::fmt;
use std::ops::Deref;

use bytes::BytesMut;
use futures_util::FutureExt;
pub use postgres_types as types;
pub use tokio_postgres::{Error, Row};
use types::{FromSql, ToSql};

use super::{
    Column, ColumnExpr, Defaulted, EnumType, Model, ModelMeta, ModelType, Query, Serial, Source,
    System, Values,
};

impl<M: ModelMeta, Type: for<'a> FromSql<'a>> Values<PostgreSql> for ColumnExpr<M, Type> {
    type Output = Type;

    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt(fmt)
    }

    fn build(row: Row) -> Result<Self::Output, Error> {
        row.try_get(0)
    }
}

pub struct PostgreSql;

impl System for PostgreSql {
    type Parameter = Parameter;
    type StatementReturn = Result<u64, tokio_postgres::Error>;
    type Row = tokio_postgres::Row;
    type Error = Error;
}

impl<T> types::ToSql for Serial<T>
where
    T: types::ToSql,
{
    fn to_sql(
        &self,
        ty: &types::Type,
        out: &mut BytesMut,
    ) -> Result<types::IsNull, Box<dyn StdError + Sync + Send>> {
        T::to_sql(&self.0, ty, out)
    }
    fn accepts(ty: &types::Type) -> bool {
        T::accepts(ty)
    }
    types::to_sql_checked!();
}

impl<T: EnumType> ModelType<PostgreSql> for T
where
    Self: types::ToSql + Sync + 'static,
{
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(name: Cow<'static, str>, params: &[(&str, &'static str)]) -> Column {
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

        let mut default = None;
        for (key, val) in params {
            if *key == "default" {
                default = Some(Cow::from(*val));
            }
        }

        Column {
            name,
            ty: format!("\"{}\"", ty_name).into(),
            null: false,
            default,
            type_def: Some(format!("CREATE TYPE \"{}\" AS ENUM({})", ty_name, variant_str).into()),
        }
    }
}

impl<T: types::ToSql> types::ToSql for Defaulted<T> {
    fn to_sql(
        &self,
        ty: &types::Type,
        out: &mut BytesMut,
    ) -> Result<types::IsNull, Box<dyn StdError + Sync + Send>>
    where
        Self: Sized,
    {
        match self {
            Self::Value(val) => val.to_sql(ty, out),
            Self::Default => "DEFAULT".to_sql(ty, out),
        }
    }

    fn accepts(ty: &types::Type) -> bool
    where
        Self: Sized,
    {
        T::accepts(ty)
    }

    types::to_sql_checked!();
}

impl<T: ModelType<PostgreSql> + types::ToSql + Sync + 'static> ModelType<PostgreSql>
    for Defaulted<T>
{
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(_: Cow<'static, str>, _: &[(&str, &'static str)]) -> Column {
        unreachable!()
    }
}

impl<T: ModelType<PostgreSql> + types::ToSql + Sync + 'static> ModelType<PostgreSql> for Option<T> {
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(name: Cow<'static, str>, params: &[(&str, &'static str)]) -> Column {
        let mut column = T::to_column(name, params);
        column.null = true;
        column
    }
}

impl ModelType<PostgreSql> for bool
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(name: Cow<'static, str>, params: &[(&str, &'static str)]) -> Column {
        let mut default = None;
        for (key, val) in params {
            if *key == "default" {
                default = Some(Cow::from(*val));
            }
        }

        Column {
            name,
            ty: "boolean".into(),
            null: false,
            default,
            type_def: None,
        }
    }
}

impl ModelType<PostgreSql> for Serial<i32>
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(name: Cow<'static, str>, params: &[(&str, &'static str)]) -> Column {
        let mut default = None;
        for (key, val) in params {
            if *key == "default" {
                default = Some(Cow::from(*val));
            }
        }

        Column {
            name,
            ty: "serial".into(),
            null: false,
            default,
            type_def: None,
        }
    }
}

impl ModelType<PostgreSql> for i32
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(name: Cow<'static, str>, params: &[(&str, &'static str)]) -> Column {
        let mut default = None;
        for (key, val) in params {
            if *key == "default" {
                default = Some(Cow::from(*val));
            }
        }

        Column {
            name,
            ty: "integer".into(),
            null: false,
            default,
            type_def: None,
        }
    }
}

impl ModelType<PostgreSql> for i64
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(name: Cow<'static, str>, params: &[(&str, &'static str)]) -> Column {
        let mut default = None;
        for (key, val) in params {
            if *key == "default" {
                default = Some(Cow::from(*val));
            }
        }

        Column {
            name,
            ty: "bigint".into(),
            null: false,
            default,
            type_def: None,
        }
    }
}

impl ModelType<PostgreSql> for Serial<i64>
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(name: Cow<'static, str>, params: &[(&str, &'static str)]) -> Column {
        let mut default = None;
        for (key, val) in params {
            if *key == "default" {
                default = Some(Cow::from(*val));
            }
        }

        Column {
            name,
            ty: "bigserial".into(),
            null: false,
            default,
            type_def: None,
        }
    }
}

impl ModelType<PostgreSql> for Vec<u8>
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(name: Cow<'static, str>, params: &[(&str, &'static str)]) -> Column {
        let mut default = None;
        for (key, val) in params {
            if *key == "default" {
                default = Some(Cow::from(*val));
            }
        }

        Column {
            name,
            ty: "bytea".into(),
            null: false,
            default,
            type_def: None,
        }
    }
}

impl ModelType<PostgreSql> for String
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(name: Cow<'static, str>, params: &[(&str, &'static str)]) -> Column {
        let mut default = None;
        for (key, val) in params {
            if *key == "default" {
                default = Some(Cow::from(*val));
            }
        }

        Column {
            name,
            ty: "text".into(),
            null: false,
            default,
            type_def: None,
        }
    }
}

#[cfg(feature = "chrono")]
impl ModelType<PostgreSql> for chrono::NaiveDate
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(name: Cow<'static, str>, params: &[(&str, &'static str)]) -> Column {
        let mut default = None;
        for (key, val) in params {
            if *key == "default" {
                default = Some(Cow::from(*val));
            }
        }

        Column {
            name,
            ty: "date".into(),
            null: false,
            default,
            type_def: None,
        }
    }
}

#[cfg(feature = "chrono")]
impl ModelType<PostgreSql> for chrono::DateTime<chrono::FixedOffset>
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(name: Cow<'static, str>, params: &[(&str, &'static str)]) -> Column {
        let mut default = None;
        for (key, val) in params {
            if *key == "default" {
                default = Some(Cow::from(*val));
            }
        }

        Column {
            name,
            ty: "timestamp with time zone".into(),
            null: false,
            default,
            type_def: None,
        }
    }
}

#[cfg(feature = "chrono")]
impl ModelType<PostgreSql> for chrono::DateTime<chrono::Utc>
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(name: Cow<'static, str>, params: &[(&str, &'static str)]) -> Column {
        let mut default = None;
        for (key, val) in params {
            if *key == "default" {
                default = Some(Cow::from(*val));
            }
        }

        Column {
            name,
            ty: "timestamp with time zone".into(),
            null: false,
            default,
            type_def: None,
        }
    }
}

pub struct Client<C: Deref<Target = tokio_postgres::Client>>(C);

impl<C: Deref<Target = tokio_postgres::Client>> Client<C> {
    pub async fn query_one<S: Source, V: Values<PostgreSql>>(
        &self,
        query: Query<PostgreSql, S, V>,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<V::Output, Error> {
        self.0
            .query_one(query.to_string().as_str(), params)
            .map(|result| result.and_then(V::build))
            .await
    }

    pub async fn insert<M: Model<PostgreSql>>(
        &self,
        data: &M::Insert,
    ) -> Result<u64, tokio_postgres::Error> {
        let (statement, params) = M::insert(data);
        self.0.execute(statement, &params).await
    }

    pub async fn exists<M: Model<PostgreSql>>(&self) -> Result<bool, Error> {
        self.0
            .query_one(
                "SELECT EXISTS (
            SELECT 1
            FROM information_schema.tables
            WHERE table_schema = 'public' AND table_name = $1
        )",
                &[&M::TABLE_NAME],
            )
            .map(|result| result.map(|row| row.get(0)))
            .await
    }
}

impl<C: Deref<Target = tokio_postgres::Client>> Deref for Client<C> {
    type Target = tokio_postgres::Client;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C: Deref<Target = tokio_postgres::Client>> From<C> for Client<C> {
    fn from(inner: C) -> Self {
        Client(inner)
    }
}

type Parameter = dyn tokio_postgres::types::ToSql + Sync;
