use std::borrow::Cow;
use std::{fmt, str};

pub use mendes_macros::form;
#[cfg(feature = "uploads")]
use serde::Deserialize;

#[cfg(feature = "uploads")]
pub use de::from_form_data;

#[cfg(feature = "uploads")]
mod de {
    use std::fmt::{self, Display};
    use std::str::{self, FromStr};

    use http::HeaderMap;
    use httparse;
    use serde::de::{Deserialize, DeserializeSeed, Error as ErrorTrait, MapAccess, Visitor};
    use twoway::find_bytes;

    #[cfg(feature = "uploads")]
    pub fn from_form_data<'a, T: Deserialize<'a>>(
        headers: &'a HeaderMap,
        input: &'a [u8],
    ) -> std::result::Result<T, Error> {
        let ctype = headers
            .get("content-type")
            .ok_or_else(|| Error::custom("content-type header not found"))?
            .as_bytes();
        let split =
            find_bytes(ctype, b"; boundary=").ok_or_else(|| Error::custom("boundary not found"))?;

        let mut boundary = Vec::with_capacity(2 + ctype.len() - split - 11);
        boundary.extend(b"--");
        boundary.extend(&ctype[split + 11..]);

        let mut deserializer = Deserializer {
            input,
            boundary,
            state: None,
        };
        T::deserialize(&mut deserializer)
    }

    pub struct Deserializer<'de> {
        input: &'de [u8],
        boundary: Vec<u8>,
        state: Option<(State, Part<'de>)>,
    }

    impl<'de, 'a> serde::de::Deserializer<'de> for &'a mut Deserializer<'de> {
        type Error = Error;

        fn deserialize_any<V>(self, _: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
        }

        fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            match &self.state {
                Some((State::Name, part)) => {
                    let name = match part {
                        Part::Blob { name, .. } => name,
                        Part::Text { name, .. } => name,
                    };
                    visitor.visit_borrowed_str(name)
                }
                Some((State::Filename, part)) => match part {
                    Part::Blob { .. } => visitor.visit_borrowed_str("filename"),
                    Part::Text { .. } => unreachable!(),
                },
                Some((State::Type, _)) => visitor.visit_borrowed_str("type"),
                Some((State::Data, _)) => visitor.visit_borrowed_str("data"),
                _ => unreachable!(),
            }
        }

        fn deserialize_ignored_any<V>(self, _: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
        }

        fn deserialize_bool<V>(self, _: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
            //visitor.visit_bool(self.parse_bool()?)
        }

        fn deserialize_i8<V>(self, _: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
            //visitor.visit_i8(self.parse_signed()?)
        }

        fn deserialize_i16<V>(self, _: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
            //visitor.visit_i16(self.parse_signed()?)
        }

        fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            if let Some((State::Data, Part::Text { data, .. })) = self.state {
                let s = str::from_utf8(data)
                    .map_err(|_| Error::custom("invalid input while UTF-8 decoding for i32"))?;
                visitor.visit_i32(
                    i32::from_str(s).map_err(|_| Error::custom("unable to convert str to i32"))?,
                )
            } else {
                unreachable!()
            }
        }

        fn deserialize_i64<V>(self, _: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
        }

        fn deserialize_u8<V>(self, _: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
        }

        fn deserialize_u16<V>(self, _: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
        }

        fn deserialize_u32<V>(self, _: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
        }

        fn deserialize_u64<V>(self, _: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
        }

        fn deserialize_f32<V>(self, _visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
        }

        fn deserialize_f64<V>(self, _visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
        }

        fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
        }

        fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            match self.state.as_ref() {
                Some((State::Name, _)) => unreachable!(),
                Some((State::Filename, Part::Blob { filename, .. })) => {
                    visitor.visit_borrowed_str(filename.as_ref().unwrap())
                }
                Some((State::Type, Part::Blob { ctype, .. })) => {
                    visitor.visit_borrowed_str(ctype.as_ref().unwrap())
                }
                Some((State::Data, part)) => {
                    let data = match part {
                        Part::Blob { data, .. } => data,
                        Part::Text { data, .. } => data,
                    };
                    let data = str::from_utf8(data)
                        .map_err(|_| Error::custom("error while decoding str from UTF-8"))?;
                    visitor.visit_borrowed_str(data)
                }
                _ => unreachable!(),
            }
        }

        fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            self.deserialize_str(visitor)
        }

        fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            let data = match self.state.as_ref() {
                Some((_, Part::Blob { data, .. })) => data,
                Some((_, Part::Text { data, .. })) => data,
                None => unreachable!(),
            };
            visitor.visit_borrowed_bytes(data)
        }

        fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
        }

        fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            match self.state.as_ref() {
                Some((State::Filename, part)) => {
                    if let Part::Blob {
                        filename: Some(_), ..
                    } = part
                    {
                        visitor.visit_some(self)
                    } else {
                        visitor.visit_none()
                    }
                }
                Some((State::Type, part)) => {
                    if let Part::Blob { ctype: Some(_), .. } = part {
                        visitor.visit_some(self)
                    } else {
                        visitor.visit_none()
                    }
                }
                _ => unreachable!(),
            }
        }

        fn deserialize_unit<V>(self, _: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!()
        }

        fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            self.deserialize_unit(visitor)
        }

        fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            visitor.visit_newtype_struct(self)
        }

        fn deserialize_seq<V>(self, _: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unreachable!()
        }

        fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            self.deserialize_seq(visitor)
        }

        // Tuple structs look just like sequences in JSON.
        fn deserialize_tuple_struct<V>(
            self,
            _name: &'static str,
            _len: usize,
            visitor: V,
        ) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            self.deserialize_seq(visitor)
        }

        fn deserialize_map<V>(mut self, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            visitor.visit_map(&mut self)
        }

        fn deserialize_struct<V>(
            self,
            _name: &'static str,
            _fields: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            self.deserialize_map(visitor)
        }

        fn deserialize_enum<V>(
            self,
            _name: &'static str,
            _variants: &'static [&'static str],
            _: V,
        ) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            unreachable!()
        }
    }

    impl<'de, 'a> MapAccess<'de> for &'a mut Deserializer<'de> {
        type Error = Error;

        fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
        where
            K: DeserializeSeed<'de>,
        {
            let split_len = self.boundary.len();
            if self.state.is_none() && self.input.starts_with(&self.boundary) {
                let (len, part) = Part::from_bytes(&self.input[split_len + 2..], &self.boundary)?;
                self.state = Some((State::Name, part));
                self.input = &self.input[split_len + 2 + len..];
                let res = seed.deserialize(&mut **self).map(Some);
                self.state = match self.state.take() {
                    Some((_, part @ Part::Blob { .. })) => Some((State::Filename, part)),
                    Some((_, part @ Part::Text { .. })) => Some((State::Data, part)),
                    None => unreachable!(),
                };
                res
            } else if let Some((state, part)) = &self.state {
                match state {
                    State::Name => seed.deserialize(&mut **self).map(Some),
                    State::Filename => match part {
                        Part::Blob { .. } => seed.deserialize(&mut **self).map(Some),
                        Part::Text { .. } => Ok(None),
                    },
                    State::Type => seed.deserialize(&mut **self).map(Some),
                    State::Data => seed.deserialize(&mut **self).map(Some),
                    State::End => {
                        self.state = None;
                        Ok(None)
                    }
                }
            } else {
                unreachable!()
            }
        }

        fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
        where
            V: DeserializeSeed<'de>,
        {
            let res = seed.deserialize(&mut **self);
            self.state = match self.state.take() {
                Some((State::Name, _)) => unreachable!(),
                Some((State::Filename, part)) => Some((State::Type, part)),
                Some((State::Type, part)) => Some((State::Data, part)),
                Some((State::Data, part)) => Some((State::End, part)),
                Some((State::End, _)) => unreachable!(),
                None => None,
            };
            res
        }
    }

    #[derive(Debug)]
    enum Part<'a> {
        Blob {
            name: &'a str,
            filename: Option<&'a str>,
            ctype: Option<&'a str>,
            data: &'a [u8],
        },
        Text {
            name: &'a str,
            data: &'a [u8],
        },
    }

    #[derive(Debug)]
    enum State {
        Name,
        Filename,
        Type,
        Data,
        End,
    }

    impl<'a> Part<'a> {
        fn from_bytes(bytes: &'a [u8], boundary: &[u8]) -> Result<(usize, Self)> {
            let mut header_buf = [httparse::EMPTY_HEADER; 4];
            let status = httparse::parse_headers(bytes, &mut header_buf)
                .map_err(|_| Error::custom("unable to parse part headers"))?;
            let (header_len, headers) = if let httparse::Status::Complete((len, headers)) = status {
                (len, headers)
            } else {
                unreachable!();
            };

            let (mut name, mut filename, mut ctype) = (None, None, None);
            for header in headers {
                let value = str::from_utf8(&header.value)
                    .map_err(|_| Error::custom("error while decoding UTF-8 from header value"))?;
                let header = header.name.to_string().to_ascii_lowercase();
                if header == "content-disposition" {
                    for param in value.split(';') {
                        if param.trim() == "form-data" {
                            continue;
                        }

                        let sep = param
                            .find('=')
                            .ok_or_else(|| Error::custom("parameter value not found"))?;
                        let pname = &param[..sep].trim();
                        let value = &param[sep + 2..param.len() - 1];
                        if *pname == "name" {
                            name = Some(value);
                        } else if *pname == "filename" {
                            filename = Some(value);
                        }
                    }
                } else if header == "content-type" {
                    ctype = Some(value);
                }
            }

            let (len, data) = if let Some(pos) = find_bytes(bytes, boundary) {
                (pos, &bytes[header_len..pos - 2])
            } else {
                (bytes.len(), &bytes[header_len..])
            };

            let name = name.ok_or_else(|| Error::custom("no name found"))?;
            let part = match &filename {
                Some(_) => Part::Blob {
                    name,
                    filename,
                    ctype,
                    data,
                },
                None => Part::Text { name, data },
            };
            Ok((len, part))
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub enum Error {
        Message(String),
    }

    impl serde::de::Error for Error {
        fn custom<T: Display>(msg: T) -> Self {
            Error::Message(msg.to_string())
        }
    }

    impl Display for Error {
        fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Error::Message(msg) => formatter.write_str(msg),
            }
        }
    }

    impl std::error::Error for Error {}

    type Result<T> = std::result::Result<T, Error>;
}

#[cfg(feature = "uploads")]
#[derive(Deserialize)]
pub struct File<'a> {
    #[serde(rename = "type")]
    pub ctype: Option<&'a str>,
    pub filename: Option<&'a str>,
    pub data: &'a [u8],
}

#[cfg(feature = "uploads")]
impl ToField for File<'_> {
    fn to_field(name: Cow<'static, str>, _: &[(&str, &str)]) -> Field {
        Field::File(FileInput { name })
    }
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
    //Date(Date),
    Email(Email),
    File(FileInput),
    Hidden(Hidden),
    Number(Number),
    Password(Password),
    Submit(Submit),
    Text(Text),
}

impl Field {
    pub fn name(&self) -> &str {
        use Field::*;
        match self {
            Email(f) => &f.name,
            File(f) => &f.name,
            Hidden(f) => &f.name,
            Number(f) => &f.name,
            Password(f) => &f.name,
            Submit(f) => &f.name,
            Text(f) => &f.name,
        }
    }
}

impl fmt::Display for Field {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Field::*;
        match self {
            Email(f) => write!(fmt, "{}", f),
            File(f) => write!(fmt, "{}", f),
            Hidden(f) => write!(fmt, "{}", f),
            Number(f) => write!(fmt, "{}", f),
            Password(f) => write!(fmt, "{}", f),
            Submit(f) => write!(fmt, "{}", f),
            Text(f) => write!(fmt, "{}", f),
        }
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

pub trait ToForm {
    fn to_form() -> Form;
}

#[cfg(feature = "uploads")]
#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderMap;
    use std::convert::TryInto;

    #[test]
    fn upload() {
        let ctype = "multipart/form-data; boundary=---------------------------200426345241597222021292378679";
        let body = [
            45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45,
            45, 45, 45, 45, 45, 45, 45, 50, 48, 48, 52, 50, 54, 51, 52, 53, 50, 52, 49, 53, 57, 55,
            50, 50, 50, 48, 50, 49, 50, 57, 50, 51, 55, 56, 54, 55, 57, 13, 10, 67, 111, 110, 116,
            101, 110, 116, 45, 68, 105, 115, 112, 111, 115, 105, 116, 105, 111, 110, 58, 32, 102,
            111, 114, 109, 45, 100, 97, 116, 97, 59, 32, 110, 97, 109, 101, 61, 34, 102, 105, 108,
            101, 34, 59, 32, 102, 105, 108, 101, 110, 97, 109, 101, 61, 34, 105, 49, 56, 110, 34,
            13, 10, 67, 111, 110, 116, 101, 110, 116, 45, 84, 121, 112, 101, 58, 32, 97, 112, 112,
            108, 105, 99, 97, 116, 105, 111, 110, 47, 111, 99, 116, 101, 116, 45, 115, 116, 114,
            101, 97, 109, 13, 10, 13, 10, 73, 195, 177, 116, 195, 171, 114, 110, 195, 162, 116,
            105, 195, 180, 110, 195, 160, 108, 105, 122, 195, 166, 116, 105, 195, 184, 110, 34, 10,
            13, 10, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45,
            45, 45, 45, 45, 45, 45, 45, 45, 45, 50, 48, 48, 52, 50, 54, 51, 52, 53, 50, 52, 49, 53,
            57, 55, 50, 50, 50, 48, 50, 49, 50, 57, 50, 51, 55, 56, 54, 55, 57, 13, 10, 67, 111,
            110, 116, 101, 110, 116, 45, 68, 105, 115, 112, 111, 115, 105, 116, 105, 111, 110, 58,
            32, 102, 111, 114, 109, 45, 100, 97, 116, 97, 59, 32, 110, 97, 109, 101, 61, 34, 97,
            115, 115, 101, 116, 34, 13, 10, 13, 10, 50, 13, 10, 45, 45, 45, 45, 45, 45, 45, 45, 45,
            45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 50, 48,
            48, 52, 50, 54, 51, 52, 53, 50, 52, 49, 53, 57, 55, 50, 50, 50, 48, 50, 49, 50, 57, 50,
            51, 55, 56, 54, 55, 57, 45, 45, 13, 10,
        ];

        let mut headers = HeaderMap::new();
        headers.insert("content-type", ctype.try_into().unwrap());
        let form = from_form_data::<Form>(&headers, &body).unwrap();
        assert_eq!(form.file.filename, Some("i18n"));
        assert_eq!(form.file.ctype, Some("application/octet-stream"));
        assert_eq!(
            form.file.data,
            b"I\xc3\xb1t\xc3\xabrn\xc3\xa2ti\xc3\xb4n\xc3\xa0liz\xc3\xa6ti\xc3\xb8n\"\n"
        );
        assert_eq!(form.asset, 2);
    }

    #[derive(Deserialize)]
    struct Form<'a> {
        #[serde(borrow)]
        file: File<'a>,
        asset: i32,
    }
}
