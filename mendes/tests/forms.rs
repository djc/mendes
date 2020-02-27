#![cfg(feature = "forms")]

use std::borrow::Cow;

use mendes::forms::{form, ToField, ToForm};
use serde::{Deserialize, Serialize};

#[test]
fn test_form() {
    let form = SomeForm::to_form();
}

#[form(action = "/assets/new", submit = "Create")]
#[derive(serde::Deserialize)]
struct SomeForm<'a> {
    name: Cow<'a, str>,
    amount: u32,
    rate: f32,
    byte: u8,
    test: bool,
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
