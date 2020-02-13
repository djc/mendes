#[cfg(feature = "uploads")]
use std::collections::HashMap;
use std::str;

#[cfg(feature = "uploads")]
use http::header::{HeaderMap, HeaderValue};
#[cfg(feature = "httparse")]
use httparse;
pub use mendes_macros::form;
#[cfg(feature = "twoway")]
use twoway::find_bytes;

#[cfg(feature = "uploads")]
pub fn multipart_form_data<'b, 'r>(
    headers: &'r HeaderMap<HeaderValue>,
    body: &'b [u8],
) -> HashMap<&'b str, (HashMap<&'b str, &'b str>, &'b [u8])> {
    let ctype = headers.get("content-type").unwrap().as_bytes();
    let split = find_bytes(ctype, b"; boundary=").unwrap();
    let value = &ctype[split + 11..];

    let mut offset = value.len() + 4;
    let mut haystack = &body[offset..];
    let mut parts = Vec::new();
    while let Some(pos) = find_bytes(haystack, value) {
        parts.push(&body[offset..offset + pos - 4]);
        offset += pos + value.len() + 2;
        haystack = &body[offset..];
    }

    let mut headers: HashMap<String, &'b str> = HashMap::new();
    let mut part_data = HashMap::new();
    for part in parts {
        let mut header_buf = [httparse::EMPTY_HEADER; 4];
        let (len, parsed) = match httparse::parse_headers(part, &mut header_buf).unwrap() {
            httparse::Status::Complete((len, headers)) => (len, headers),
            _ => panic!("expected complete headers"),
        };
        let body = &part[len..];

        headers.clear();
        for header in parsed {
            let name = header.name.to_string().to_ascii_lowercase();
            headers.insert(name, str::from_utf8(header.value).unwrap());
        }

        let mut meta = HashMap::new();
        let disposition = headers.get("content-disposition").unwrap();
        for (i, val) in disposition.split(';').enumerate() {
            if i == 0 {
                assert_eq!(val.trim(), "form-data");
                continue;
            }

            let mut parts = val.splitn(2, '=');
            let key = parts.next().unwrap().trim();
            let value = parts.next().unwrap().trim_matches('"');
            meta.insert(key, value);
        }

        let name = meta.remove("name").unwrap();
        if name == "file" {
            meta.insert("type", headers.get("content-type").unwrap());
        }
        part_data.insert(name, (meta, body));
    }

    part_data
}

pub trait Form {
    fn form() -> &'static str;
}
