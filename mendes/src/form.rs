use std::borrow::Cow;
use std::{fmt, str};

pub use mendes_macros::{form, ToField};

pub trait ToForm {
    fn to_form() -> Form;
}

pub struct Form {
    pub action: Option<Cow<'static, str>>,
    pub enctype: Option<Cow<'static, str>>,
    pub method: Option<Cow<'static, str>>,
    pub sets: Vec<FieldSet>,
}

impl Form {
    pub fn prepare(mut self) -> Self {
        let multipart = self
            .sets
            .iter()
            .flat_map(|s| &s.items)
            .map(|i| &i.field)
            .find(|f| if let Field::File(_) = f { true } else { false });
        if multipart.is_some() {
            self.enctype = Some("multipart/form-data".into());
        }
        self
    }

    pub fn set<T: fmt::Display>(mut self, name: &str, value: T) -> Result<Self, ()> {
        let field = self
            .sets
            .iter_mut()
            .flat_map(|s| &mut s.items)
            .map(|i| &mut i.field)
            .find(|f| f.name() == name);
        if let Some(Field::Hidden(field)) = field {
            field.value = Some(value.to_string().into());
            Ok(self)
        } else {
            Err(())
        }
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
    pub field: Field,
}

impl fmt::Display for Item {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(s) = &self.label {
            write!(fmt, r#"<label for="{}">{}</label>"#, self.field.name(), s)?;
        }
        write!(fmt, "{}", self.field)
    }
}

pub enum Field {
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

pub struct Date {
    pub name: Cow<'static, str>,
}

impl fmt::Display for Date {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<input type="date" name="{}">"#, self.name)
    }
}

pub struct Email {
    pub name: Cow<'static, str>,
}

impl fmt::Display for Email {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<input type="email" name="{}">"#, self.name)
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
}

impl fmt::Display for Number {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<input type="number" name="{}">"#, self.name)
    }
}

pub struct Password {
    pub name: Cow<'static, str>,
}

impl fmt::Display for Password {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<input type="password" name="{}">"#, self.name)
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
    pub label: Option<Cow<'static, str>>,
    pub value: Option<Cow<'static, str>>,
    pub disabled: bool,
    pub selected: bool,
    pub text: Option<Cow<'static, str>>,
}

impl fmt::Display for SelectOption {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "<option")?;
        if let Some(s) = &self.label {
            write!(fmt, r#" label="{}""#, s)?;
        }
        if let Some(s) = &self.value {
            write!(fmt, r#" value="{}""#, s)?;
        }
        if self.disabled {
            write!(fmt, " disabled")?;
        }
        if self.selected {
            write!(fmt, " selected")?;
        }
        if let Some(s) = &self.text {
            write!(fmt, ">{}</option>", s)
        } else {
            write!(fmt, "></option>")
        }
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
}

impl fmt::Display for Text {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, r#"<input type="text" name="{}">"#, self.name)
    }
}

pub trait ToField {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field;
}

impl ToField for Cow<'_, str> {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field {
        for (key, value) in params {
            if *key == "type" {
                if *value == "email" {
                    return Field::Email(Email { name });
                } else if *value == "password" {
                    return Field::Password(Password { name });
                }
            }
        }
        Field::Text(Text { name })
    }
}

impl ToField for u8 {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field {
        for (key, value) in params {
            if *key == "type" && *value == "hidden" {
                return Field::Hidden(Hidden::from_params(name, params));
            }
        }
        Field::Number(Number { name })
    }
}

impl ToField for u16 {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field {
        for (key, value) in params {
            if *key == "type" && *value == "hidden" {
                return Field::Hidden(Hidden::from_params(name, params));
            }
        }
        Field::Number(Number { name })
    }
}

impl ToField for u32 {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field {
        for (key, value) in params {
            if *key == "type" && *value == "hidden" {
                return Field::Hidden(Hidden::from_params(name, params));
            }
        }
        Field::Number(Number { name })
    }
}

impl ToField for i32 {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field {
        for (key, value) in params {
            if *key == "type" && *value == "hidden" {
                return Field::Hidden(Hidden::from_params(name, params));
            }
        }
        Field::Number(Number { name })
    }
}

impl ToField for f32 {
    fn to_field(name: Cow<'static, str>, params: &[(&str, &str)]) -> Field {
        for (key, value) in params {
            if *key == "type" && *value == "hidden" {
                return Field::Hidden(Hidden::from_params(name, params));
            }
        }
        Field::Number(Number { name })
    }
}

#[cfg(feature = "chrono")]
impl ToField for chrono::NaiveDate {
    fn to_field(name: Cow<'static, str>, _: &[(&str, &str)]) -> Field {
        Field::Date(Date { name })
    }
}
