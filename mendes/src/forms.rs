use std::borrow::Cow;
use std::{fmt, str};

pub use mendes_macros::{form, ToField};

#[cfg(feature = "uploads")]
pub use crate::multipart::{from_form_data, File};

pub trait ToForm {
    fn to_form() -> Form;
}

pub struct Form {
    pub action: Option<Cow<'static, str>>,
    pub enctype: Option<Cow<'static, str>>,
    pub method: Option<Cow<'static, str>>,
    pub classes: Vec<Cow<'static, str>>,
    pub sets: Vec<FieldSet>,
}

impl Form {
    pub fn prepare(mut self) -> Self {
        let multipart = self
            .sets
            .iter()
            .flat_map(|s| &s.items)
            .any(|i| i.multipart());
        if multipart {
            self.enctype = Some("multipart/form-data".into());
        }
        self
    }

    pub fn set<T: fmt::Display>(mut self, name: &str, value: T) -> Result<Self, ()> {
        let res = self
            .sets
            .iter_mut()
            .flat_map(|s| &mut s.items)
            .fold(Err(()), |mut res, item| {
                if item.set(name, &value).is_ok() {
                    res = Ok(());
                }
                res
            });
        res.map(|_| self)
    }
}

impl fmt::Display for Form {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "<form")?;
        if let Some(s) = &self.action {
            write!(fmt, r#" action="{}""#, s)?;
        }
        if let Some(s) = &self.enctype {
            write!(fmt, r#" enctype="{}""#, s)?;
        }
        if let Some(s) = &self.method {
            write!(fmt, r#" method="{}""#, s)?;
        }
        if !self.classes.is_empty() {
            write!(fmt, r#" class=""#)?;
            for (i, s) in self.classes.iter().enumerate() {
                match i {
                    0 => write!(fmt, "{}", s)?,
                    _ => write!(fmt, " {}", s)?,
                }
            }
            write!(fmt, "\"")?;
        }
        write!(fmt, ">")?;
        for set in &self.sets {
            write!(fmt, "{}", set)?;
        }
        write!(fmt, "</form>")
    }
}

pub struct FieldSet {
    pub legend: Option<&'static str>,
    pub items: Vec<Item>,
}

impl fmt::Display for FieldSet {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "<fieldset>")?;
        if let Some(s) = self.legend {
            write!(fmt, "<legend>{}</legend>", s)?;
        }
        for item in &self.items {
            write!(fmt, "{}", item)?;
        }
        write!(fmt, "</fieldset>")
    }
}

pub struct Item {
    pub label: Option<Cow<'static, str>>,
    pub contents: ItemContents,
}

impl Item {
    fn set<T: fmt::Display>(&mut self, name: &str, value: &T) -> Result<(), ()> {
        match &mut self.contents {
            ItemContents::Single(f) => {
                if f.name() == name {
                    match f {
                        Field::Checkbox(f) => {
                            let s = value.to_string();
                            if s == "true" || s == "1" {
                                f.checked = true;
                                Ok(())
                            } else if s == "false" || s == "0" {
                                f.checked = false;
                                Ok(())
                            } else {
                                Err(())
                            }
                        }
                        Field::Date(f) => {
                            f.value = Some(value.to_string().into());
                            Ok(())
                        }
                        Field::Email(f) => {
                            f.value = Some(value.to_string().into());
                            Ok(())
                        }
                        Field::Hidden(f) => {
                            f.value = Some(value.to_string().into());
                            Ok(())
                        }
                        Field::Number(f) => {
                            f.value = Some(value.to_string().into());
                            Ok(())
                        }
                        Field::Password(f) => {
                            f.value = Some(value.to_string().into());
                            Ok(())
                        }
                        Field::Select(f) => {
                            let val = value.to_string();
                            for option in &mut f.options {
                                if option.value == val {
                                    option.selected = true;
                                    return Ok(());
                                }
                            }
                            Err(())
                        }
                        Field::Text(f) => {
                            f.value = Some(value.to_string().into());
                            Ok(())
                        }
                        Field::File(_) | Field::Submit(_) => Err(()),
                    }
                } else {
                    Err(())
                }
            }
            ItemContents::Multi(items) => {
                let mut found = Err(());
                for item in items {
                    if item.set(name, value).is_ok() {
                        found = Ok(());
                    }
                }
                found
            }
        }
    }

    fn multipart(&self) -> bool {
        match &self.contents {
            ItemContents::Single(f) => match f {
                Field::File(_) => true,
                _ => false,
            },
            ItemContents::Multi(items) => items.iter().any(|i| i.multipart()),
        }
    }
}

impl fmt::Display for Item {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "<label")?;
        if let ItemContents::Single(f) = &self.contents {
            write!(fmt, r#" for="{}""#, f.name())?;
        }
        if let Some(s) = &self.label {
            write!(fmt, r#">{}</label>{}"#, s, self.contents)
        } else {
            write!(fmt, r#"></label>{}"#, self.contents)
        }
    }
}

pub enum ItemContents {
    Single(Field),
    Multi(Vec<Item>),
}

impl fmt::Display for ItemContents {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ItemContents::Single(f) => write!(fmt, "{}", f),
            ItemContents::Multi(items) => {
                write!(fmt, r#"<div class="compound-item">"#)?;
                for item in items {
                    write!(fmt, "{}", item)?;
                }
                write!(fmt, "</div>")
            }
        }
    }
}

pub enum Field {
    Checkbox(Checkbox),
    Date(Date),
    Email(Email),
    File(FileInput),
    Hidden(Hidden),
    Number(Number),
    Password(Password),
    Select(Select),
    Submit(Submit),
    Text(Text),
}

impl Field {
    pub fn name(&self) -> &str {
        use Field::*;
        match self {
            Checkbox(f) => &f.name,
            Date(f) => &f.name,
            Email(f) => &f.name,
            File(f) => &f.name,
            Hidden(f) => &f.name,
            Number(f) => &f.name,
            Password(f) => &f.name,
            Select(f) => &f.name,
            Submit(f) => &f.name,
            Text(f) => &f.name,
        }
    }
}

impl fmt::Display for Field {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Field::*;
        match self {
            Checkbox(f) => write!(fmt, "{}", f),
            Date(f) => write!(fmt, "{}", f),
            Email(f) => write!(fmt, "{}", f),
            File(f) => write!(fmt, "{}", f),
            Hidden(f) => write!(fmt, "{}", f),
            Number(f) => write!(fmt, "{}", f),
            Password(f) => write!(fmt, "{}", f),
            Select(f) => write!(fmt, "{}", f),
            Submit(f) => write!(fmt, "{}", f),
            Text(f) => write!(fmt, "{}", f),
        }
    }
}

pub struct Checkbox {
    pub name: Cow<'static, str>,
    pub checked: bool,
}

impl fmt::Display for Checkbox {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<input type="checkbox" name="{}""#, self.name)?;
        if self.checked {
            write!(fmt, " checked")?;
        }
        write!(fmt, ">")
    }
}

pub struct Date {
    pub name: Cow<'static, str>,
    pub value: Option<Cow<'static, str>>,
}

impl fmt::Display for Date {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<input type="date" name="{}""#, self.name)?;
        if let Some(s) = &self.value {
            write!(fmt, r#" value="{}""#, s)?;
        }
        write!(fmt, ">")
    }
}

pub struct Email {
    pub name: Cow<'static, str>,
    pub value: Option<Cow<'static, str>>,
}

impl fmt::Display for Email {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<input type="email" name="{}""#, self.name)?;
        if let Some(s) = &self.value {
            write!(fmt, r#" value="{}""#, s)?;
        }
        write!(fmt, ">")
    }
}

pub struct FileInput {
    pub name: Cow<'static, str>,
}

impl fmt::Display for FileInput {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<input type="file" name="{}">"#, self.name)
    }
}

pub struct Hidden {
    pub name: Cow<'static, str>,
    pub value: Option<Cow<'static, str>>,
}

impl Hidden {
    fn from_params(name: Cow<'static, str>, _: &[(&str, &str)]) -> Self {
        Self { name, value: None }
    }
}

impl fmt::Display for Hidden {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<input type="hidden" name="{}""#, self.name)?;
        if let Some(s) = &self.value {
            write!(fmt, r#" value="{}""#, s)?;
        }
        write!(fmt, ">")
    }
}

pub struct Number {
    pub name: Cow<'static, str>,
    pub value: Option<Cow<'static, str>>,
}

impl fmt::Display for Number {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<input type="number" name="{}""#, self.name)?;
        if let Some(s) = &self.value {
            write!(fmt, r#" value="{}""#, s)?;
        }
        write!(fmt, ">")
    }
}

pub struct Password {
    pub name: Cow<'static, str>,
    pub value: Option<Cow<'static, str>>,
}

impl fmt::Display for Password {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<input type="password" name="{}""#, self.name)?;
        if let Some(s) = &self.value {
            write!(fmt, r#" value="{}""#, s)?;
        }
        write!(fmt, ">")
    }
}

pub struct Select {
    pub name: Cow<'static, str>,
    pub options: Vec<SelectOption>,
}

impl fmt::Display for Select {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<select name="{}">"#, &self.name)?;
        for opt in &self.options {
            write!(fmt, "{}", opt)?;
        }
        write!(fmt, "</select>")
    }
}

pub struct SelectOption {
    pub label: Cow<'static, str>,
    pub value: Cow<'static, str>,
    pub disabled: bool,
    pub selected: bool,
}

impl fmt::Display for SelectOption {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<option value="{}""#, self.value)?;
        if self.disabled {
            write!(fmt, " disabled")?;
        }
        if self.selected {
            write!(fmt, " selected")?;
        }
        write!(fmt, ">{}</option>", self.label)
    }
}

pub struct Submit {
    pub name: Cow<'static, str>,
    pub value: Cow<'static, str>,
}

impl fmt::Display for Submit {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            fmt,
            r#"<input type="submit" name="{}" value="{}">"#,
            self.name, self.value
        )
    }
}

pub struct Text {
    pub name: Cow<'static, str>,
    pub value: Option<Cow<'static, str>>,
}

impl fmt::Display for Text {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<input type="text" name="{}""#, self.name)?;
        if let Some(s) = &self.value {
            write!(fmt, r#" value="{}""#, s)?;
        }
        write!(fmt, ">")
    }
}

pub trait ToField {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field;
}

impl ToField for bool {
    fn to_field(name: Cow<'static, str>, _: &[(&str, &str)]) -> Field {
        Field::Checkbox(Checkbox {
            name,
            checked: false,
        })
    }
}

impl ToField for String {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field {
        for (key, value) in params {
            if *key == "type" {
                if *value == "email" {
                    return Field::Email(Email { name, value: None });
                } else if *value == "password" {
                    return Field::Password(Password { name, value: None });
                }
            }
        }
        Field::Text(Text { name, value: None })
    }
}

impl ToField for Cow<'_, str> {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field {
        for (key, value) in params {
            if *key == "type" {
                if *value == "email" {
                    return Field::Email(Email { name, value: None });
                } else if *value == "password" {
                    return Field::Password(Password { name, value: None });
                }
            }
        }
        Field::Text(Text { name, value: None })
    }
}

impl ToField for u8 {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field {
        for (key, value) in params {
            if *key == "type" && *value == "hidden" {
                return Field::Hidden(Hidden::from_params(name, params));
            }
        }
        Field::Number(Number { name, value: None })
    }
}

impl ToField for u16 {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field {
        for (key, value) in params {
            if *key == "type" && *value == "hidden" {
                return Field::Hidden(Hidden::from_params(name, params));
            }
        }
        Field::Number(Number { name, value: None })
    }
}

impl ToField for u32 {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field {
        for (key, value) in params {
            if *key == "type" && *value == "hidden" {
                return Field::Hidden(Hidden::from_params(name, params));
            }
        }
        Field::Number(Number { name, value: None })
    }
}

impl ToField for i32 {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field {
        for (key, value) in params {
            if *key == "type" && *value == "hidden" {
                return Field::Hidden(Hidden::from_params(name, params));
            }
        }
        Field::Number(Number { name, value: None })
    }
}

impl ToField for f32 {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field {
        for (key, value) in params {
            if *key == "type" && *value == "hidden" {
                return Field::Hidden(Hidden::from_params(name, params));
            }
        }
        Field::Number(Number { name, value: None })
    }
}

#[cfg(feature = "chrono")]
impl ToField for chrono::NaiveDate {
    fn to_field(name: Cow<'static, str>, _: &[(&str, &str)]) -> Field {
        Field::Date(Date { name, value: None })
    }
}
