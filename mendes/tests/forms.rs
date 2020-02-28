#![cfg(feature = "forms")]

use std::borrow::Cow;

use mendes::forms::{form, ToField, ToForm};
use serde::{Deserialize, Serialize};

#[test]
fn test_generate() {
    let form = SomeForm::to_form();
    let form = form.set("name", "hi").unwrap();
    let html = form.to_string();
    assert!(!html.contains("skipped"));
}

#[test]
fn test_roundtrip() {
    let obj = SomeForm {
        skipped: 0,
        name: "name".into(),
        amount: 1,
        rate: 2.0,
        byte: 3,
        test: true,
        options: Options::Straight,
        #[cfg(feature = "chrono")]
        date: chrono::Utc::today().naive_utc(),
    };
    let s = serde_urlencoded::to_string(&obj).unwrap();
    let decoded = serde_urlencoded::from_bytes(s.as_bytes()).unwrap();
    assert_eq!(obj, decoded);
}

#[allow(dead_code)]
#[form(action = "/assets/new", submit = "Create")]
#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct SomeForm<'a> {
    #[form(skip)]
    skipped: u8,
    name: Cow<'a, str>,
    amount: u32,
    rate: f32,
    byte: u8,
    #[form(item = "Group")]
    test: bool,
    #[form(item = "Group")]
    options: Options,
    #[cfg(feature = "chrono")]
    date: chrono::NaiveDate,
}

#[derive(Debug, Deserialize, Serialize, ToField, PartialEq)]
enum Options {
    Straight,
    #[option(label = "Relabeled")]
    Labeled,
}
