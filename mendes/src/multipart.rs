use std::fmt::{self, Display};
use std::str::{self, FromStr};

use http::HeaderMap;
use httparse;
use serde::de::{
    DeserializeSeed, EnumAccess, Error as ErrorTrait, MapAccess, VariantAccess, Visitor,
};
use serde::Deserialize;
use twoway::find_bytes;

pub fn from_form_data<'a, T: Deserialize<'a>>(
    headers: &HeaderMap,
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
            Some((State::Data, part)) => match part {
                Part::Blob { .. } => visitor.visit_borrowed_str("data"),
                Part::Text { .. } => self.deserialize_str(visitor),
            },
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
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(Enum { de: self })
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

struct Enum<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'de, 'a> EnumAccess<'de> for Enum<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        Ok((seed.deserialize(&mut *self.de)?, self))
    }
}

impl<'de, 'a> VariantAccess<'de> for Enum<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
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

#[derive(Deserialize)]
pub struct File<'a> {
    #[serde(rename = "type")]
    pub ctype: Option<&'a str>,
    pub filename: Option<&'a str>,
    pub data: &'a [u8],
}

impl super::forms::ToField for File<'_> {
    fn to_field(name: std::borrow::Cow<'static, str>, _: &[(&str, &str)]) -> super::forms::Field {
        super::forms::Field::File(super::forms::FileInput { name })
    }
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

    #[test]
    fn enum_field() {
        let ctype = "multipart/form-data; boundary=---------------------------345106847831590504122057183932";
        let body = "-----------------------------345106847831590504122057183932\r
Content-Disposition: form-data; name=\"foo\"\r
\r
Foo\r
-----------------------------345106847831590504122057183932--";

        let mut headers = HeaderMap::new();
        headers.insert("content-type", ctype.try_into().unwrap());
        let form = from_form_data::<EnumForm>(&headers, body.as_bytes()).unwrap();
        assert_eq!(form.foo, FooBar::Foo);
    }

    #[derive(Deserialize)]
    struct EnumForm {
        foo: FooBar,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    enum FooBar {
        Foo,
        Bar,
    }
}
