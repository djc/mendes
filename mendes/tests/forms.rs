#![cfg(feature = "forms")]

use std::borrow::Cow;

use mendes::forms::{form, ToField, ToForm};
use serde::{Deserialize, Serialize};

#[test]
fn test_generate() {
    let form = SomeForm::to_form();
    let form = form.set("name", "hi").unwrap();
    let _ = form.to_string();
}

#[allow(dead_code)]
#[form(action = "/assets/new", submit = "Create")]
#[derive(Deserialize)]
struct SomeForm<'a> {
    name: Cow<'a, str>,
    amount: u32,
    rate: f32,
    byte: u8,
    #[form(item = "Group")]
    test: bool,
    #[form(item = "Group")]
    options: Options,
    #[cfg(feature = "chrono")]
    outstanding_principal_date: chrono::NaiveDate,
}

#[derive(Deserialize, Serialize, ToField)]
enum Options {
    Straight,
    #[option(label = "Relabeled")]
    Labeled,
}
