#![cfg(feature = "models")]
use mendes::models::{model, model_type, PostgreSQL, Serial, System};

#[test]
fn test_model() {
    let table = PostgreSQL::table::<Named>();
    let sql = table.to_string();
    assert_eq!(
        sql,
        "CREATE TYPE Foo AS ENUM('Bar', 'Baz'); \
         CREATE TABLE nameds (\
             id serial NOT NULL, \
             name text NOT NULL, \
             num bigint NOT NULL, \
             foo Foo NOT NULL, \
             wrap integer NOT NULL, \
             CONSTRAINT nameds_pkey PRIMARY KEY (id)\
         )"
    );
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
enum Foo {
    Bar,
    Baz,
}

#[model_type]
struct Wrap(i32);
