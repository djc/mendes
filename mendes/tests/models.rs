#![cfg(all(feature = "models", feature = "postgres"))]
#![allow(clippy::blacklisted_name)]

use mendes::models::postgres::{types, PostgreSql};
use mendes::models::{model, model_type, Model, ModelMeta, Serial, System};

#[test]
fn test_model() {
    let table = PostgreSql::table::<Named>();
    let sql = table.to_string();
    assert_eq!(
        sql,
        r#"CREATE TYPE "Foo" AS ENUM('Bar', 'Baz');

CREATE TABLE "named" (
    "id" serial NOT NULL,
    "name" text NOT NULL,
    "num" bigint NOT NULL,
    "maybe" boolean,
    "foo" "Foo" NOT NULL,
    "wrap" integer NOT NULL,
    "answer" integer NOT NULL DEFAULT 42,
    CONSTRAINT "named_pkey" PRIMARY KEY ("id")
)"#
    );

    let new = Named::builder()
        .name("name".into())
        .num(12)
        .maybe(false)
        .foo(Foo::Bar)
        .wrap(Wrap(14));

    assert_eq!(
        Named::insert(&new).0,
        r#"INSERT INTO "named" (
    "name", "num", "maybe", "foo", "wrap"
) VALUES (
    $1, $2, $3, $4, $5
)"#
    );

    assert_eq!(
        PostgreSql::table::<Dependent>().to_string(),
        r#"CREATE TABLE "dependent" (
    "dep_id" serial NOT NULL,
    "named" integer NOT NULL,
    CONSTRAINT "named" FOREIGN KEY ("named") REFERENCES "named" ("id"),
    CONSTRAINT "dependent_pkey" PRIMARY KEY ("dep_id")
)"#
    )
}

#[allow(dead_code)]
#[model]
struct Named {
    id: Serial<i32>,
    name: String,
    num: i64,
    maybe: Option<bool>,
    foo: Foo,
    wrap: Wrap,
    #[model(default = 42)]
    answer: i32,
}

#[allow(dead_code)]
#[model_type]
#[derive(Debug, types::ToSql)]
enum Foo {
    Bar,
    Baz,
}

#[model_type]
#[derive(Debug, types::ToSql)]
struct Wrap(i32);

#[allow(dead_code)]
#[model]
struct Dependent {
    #[model(primary_key)]
    dep_id: Serial<i32>,
    named: <Named as ModelMeta>::PrimaryKey,
}
