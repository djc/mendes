#![allow(clippy::wrong_self_convention)] // https://github.com/rust-lang/rust-clippy/issues/7374

use std::borrow::Cow;
use std::error::Error as StdError;
use std::ops::Deref;

use bytes::BytesMut;
pub use postgres_types as types;
pub use tokio_postgres::Error;

use super::{Column, EnumType, Model, ModelType, Query, Serial, Source, System, Values};

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
            ty: format!("\"{}\"", ty_name).into(),
            null: false,
            default: None,
            type_def: Some(format!("CREATE TYPE \"{}\" AS ENUM({})", ty_name, variant_str).into()),
        }
    }
}

impl ModelType<PostgreSql> for bool
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

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

impl ModelType<PostgreSql> for Serial<i32>
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

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

impl ModelType<PostgreSql> for i32
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

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

impl ModelType<PostgreSql> for i64
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

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

impl ModelType<PostgreSql> for Serial<i64>
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

    fn to_column(name: Cow<'static, str>, _: &[(&str, &str)]) -> Column {
        Column {
            name,
            ty: "bigserial".into(),
            null: false,
            default: None,
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

impl ModelType<PostgreSql> for String
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

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
impl ModelType<PostgreSql> for chrono::NaiveDate
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

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
impl ModelType<PostgreSql> for chrono::DateTime<chrono::FixedOffset>
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

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

#[cfg(feature = "chrono")]
impl ModelType<PostgreSql> for chrono::DateTime<chrono::Utc>
where
    Self: types::ToSql,
{
    fn value(&self) -> &Parameter {
        self
    }

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

pub struct Client<C: Deref<Target = tokio_postgres::Client>>(C);

impl<C: Deref<Target = tokio_postgres::Client>> Client<C> {
    pub fn new(inner: C) -> Self {
        Self(inner)
    }

    pub async fn query_one<S: Source, V: Values<PostgreSql>>(
        &self,
        query: Query<PostgreSql, S, V>,
    ) -> Result<V, Error> {
    }

    pub async fn insert<M: Model<PostgreSql>>(
        &self,
        data: &M,
    ) -> Result<u64, tokio_postgres::Error> {
        let (statement, params) = data.insert();
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

type Parameter = dyn tokio_postgres::types::ToSql + Sync;
