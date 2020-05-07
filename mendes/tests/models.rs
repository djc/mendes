#![cfg(feature = "models")]
use mendes::models::{model, PostgreSQL, Serial, System};

#[test]
fn test_model() {
    let table = PostgreSQL::table::<Named>();
    let sql = table.to_string();
    assert_eq!(
        sql,
        "CREATE TABLE nameds (id serial NOT NULL, name text NOT NULL, num bigint NOT NULL, CONSTRAINT nameds_pkey PRIMARY KEY (id))"
    );
}

#[allow(dead_code)]
#[model]
struct Named {
    id: Serial<i32>,
    name: String,
    num: i64,
}
