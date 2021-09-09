#![cfg(all(feature = "models", feature = "postgres"))]

use mendes::models::postgres::{types, PostgreSql};
use mendes::models::{model, model_type, ModelMeta, Serial, System};

#[test]
fn test_model() {
    let table = PostgreSql::table::<Named>();
    let sql = table.to_string();
    assert_eq!(
        sql,
        "CREATE TYPE \"Foo\" AS ENUM('Bar', 'Baz'); \
         CREATE TABLE named (\
             id serial NOT NULL, \
             name text NOT NULL, \
             num bigint NOT NULL, \
             foo \"Foo\" NOT NULL, \
             wrap integer NOT NULL, \
             CONSTRAINT named_pkey PRIMARY KEY (id)\
         )"
    );

    assert_eq!(
        PostgreSql::table::<Dependent>().to_string(),
        "CREATE TABLE dependent (\
             dep_id serial NOT NULL, \
             named integer NOT NULL, \
             CONSTRAINT named FOREIGN KEY (named) REFERENCES named (id), \
             CONSTRAINT dependent_pkey PRIMARY KEY (dep_id)\
         )"
    )
}

#[allow(dead_code)]
#[model]
struct Named {
    id: Serial<i32>,
    name: String,
    num: i64,
    foo: Foo,
    wrap: Wrap,
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
