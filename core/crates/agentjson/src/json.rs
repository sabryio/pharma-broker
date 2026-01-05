#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    NumberI64(i64),
    NumberU64(u64),
    NumberF64(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    pub fn to_compact_string(&self) -> String {
        match self {
            JsonValue::Null => "null".to_string(),
            JsonValue::Bool(b) => {
                if *b {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            JsonValue::NumberI64(n) => n.to_string(),
            JsonValue::NumberU64(n) => n.to_string(),
            JsonValue::NumberF64(n) => {
                if n.is_finite() {
                    // JSON doesn't allow NaN/Infinity.
                    let mut s = format!("{n}");
                    if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                        s.push_str(".0");
                    }
                    s
                } else {
                    "null".to_string()
                }
            }
            JsonValue::String(s) => quote_json_string(s),
            JsonValue::Array(a) => {
                let mut out = String::from("[");
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&v.to_compact_string());
                }
                out.push(']');
                out
            }
            JsonValue::Object(obj) => {
                let mut out = String::from("{");
                for (i, (k, v)) in obj.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&quote_json_string(k));
                    out.push(':');
                    out.push_str(&v.to_compact_string());
                }
                out.push('}');
                out
            }
        }
    }

    pub fn as_object(&self) -> Option<&[(String, JsonValue)]> {
        match self {
            JsonValue::Object(v) => Some(v.as_slice()),
            _ => None,
        }
    }
}

pub fn quote_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if c < '\u{20}' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsonError {
    pub message: String,
    pub pos: usize,
}

pub fn parse_strict_json(input: &str) -> Result<JsonValue, JsonError> {
    let bytes = input.as_bytes();
    let mut i: usize = 0;

    skip_ws(bytes, &mut i);
    let v = parse_value(input, bytes, &mut i)?;
    skip_ws(bytes, &mut i);
    if i != bytes.len() {
        return Err(JsonError {
            message: "trailing characters".to_string(),
            pos: i,
        });
    }
    Ok(v)
}

fn skip_ws(bytes: &[u8], i: &mut usize) {
    while *i < bytes.len() {
        match bytes[*i] {
            b' ' | b'\n' | b'\r' | b'\t' => *i += 1,
            _ => break,
        }
    }
}

fn parse_value(input: &str, bytes: &[u8], i: &mut usize) -> Result<JsonValue, JsonError> {
    if *i >= bytes.len() {
        return Err(JsonError {
            message: "unexpected EOF".to_string(),
            pos: *i,
        });
    }
    match bytes[*i] {
        b'n' => parse_literal(bytes, i, b"null", JsonValue::Null),
        b't' => parse_literal(bytes, i, b"true", JsonValue::Bool(true)),
        b'f' => parse_literal(bytes, i, b"false", JsonValue::Bool(false)),
        b'"' => {
            let s = parse_string(input, bytes, i)?;
            Ok(JsonValue::String(s))
        }
        b'{' => parse_object(input, bytes, i),
        b'[' => parse_array(input, bytes, i),
        b'-' | b'0'..=b'9' => parse_number(bytes, i),
        _ => Err(JsonError {
            message: format!("unexpected byte: {}", bytes[*i]),
            pos: *i,
        }),
    }
}

fn parse_literal(
    bytes: &[u8],
    i: &mut usize,
    lit: &[u8],
    v: JsonValue,
) -> Result<JsonValue, JsonError> {
    if bytes.len().saturating_sub(*i) < lit.len() {
        return Err(JsonError {
            message: "unexpected EOF".to_string(),
            pos: *i,
        });
    }
    if &bytes[*i..*i + lit.len()] != lit {
        return Err(JsonError {
            message: "invalid literal".to_string(),
            pos: *i,
        });
    }
    *i += lit.len();
    Ok(v)
}

fn parse_string(input: &str, bytes: &[u8], i: &mut usize) -> Result<String, JsonError> {
    let start = *i;
    if bytes.get(*i) != Some(&b'"') {
        return Err(JsonError {
            message: "expected string".to_string(),
            pos: *i,
        });
    }
    *i += 1;
    let mut out = String::new();
    while *i < bytes.len() {
        let b = bytes[*i];
        if b == b'"' {
            *i += 1;
            return Ok(out);
        }
        if b == b'\\' {
            *i += 1;
            if *i >= bytes.len() {
                return Err(JsonError {
                    message: "unexpected EOF in escape".to_string(),
                    pos: *i,
                });
            }
            let e = bytes[*i];
            *i += 1;
            match e {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'/' => out.push('/'),
                b'b' => out.push('\u{08}'),
                b'f' => out.push('\u{0C}'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'u' => {
                    if bytes.len().saturating_sub(*i) < 4 {
                        return Err(JsonError {
                            message: "unexpected EOF in \\u escape".to_string(),
                            pos: *i,
                        });
                    }
                    let hex = &bytes[*i..*i + 4];
                    *i += 4;
                    let code = parse_hex4(hex).ok_or(JsonError {
                        message: "invalid \\u escape".to_string(),
                        pos: *i,
                    })?;

                    // Handle surrogate pairs.
                    if (0xD800..=0xDBFF).contains(&code) {
                        // expect \uXXXX
                        if bytes.len().saturating_sub(*i) >= 6
                            && bytes[*i] == b'\\'
                            && bytes[*i + 1] == b'u'
                        {
                            let hex2 = &bytes[*i + 2..*i + 6];
                            if let Some(code2) = parse_hex4(hex2)
                                && (0xDC00..=0xDFFF).contains(&code2)
                            {
                                *i += 6;
                                let full = 0x10000
                                    + (((code - 0xD800) as u32) << 10)
                                    + ((code2 - 0xDC00) as u32);
                                if let Some(ch) = char::from_u32(full) {
                                    out.push(ch);
                                    continue;
                                }
                            }
                        }
                        // fallback: emit replacement
                        out.push('\u{FFFD}');
                    } else if let Some(ch) = char::from_u32(code as u32) {
                        out.push(ch);
                    } else {
                        out.push('\u{FFFD}');
                    }
                }
                _ => {
                    return Err(JsonError {
                        message: "invalid escape".to_string(),
                        pos: *i - 1,
                    });
                }
            }
            continue;
        }
        if b < 0x20 {
            return Err(JsonError {
                message: "control character in string".to_string(),
                pos: *i,
            });
        }
        // Fast path ASCII.
        if b < 0x80 {
            out.push(b as char);
            *i += 1;
            continue;
        }
        // UTF-8 char (input is valid UTF-8 already; avoid re-validating the remainder).
        let s = input.get(*i..).ok_or(JsonError {
            message: "invalid utf-8 boundary".to_string(),
            pos: *i,
        })?;
        let mut it = s.chars();
        let ch = it.next().ok_or(JsonError {
            message: "unexpected EOF".to_string(),
            pos: *i,
        })?;
        out.push(ch);
        *i += ch.len_utf8();
    }

    Err(JsonError {
        message: "unterminated string".to_string(),
        pos: start,
    })
}

fn parse_hex4(hex: &[u8]) -> Option<u16> {
    let mut v: u16 = 0;
    for &b in hex {
        v <<= 4;
        match b {
            b'0'..=b'9' => v |= (b - b'0') as u16,
            b'a'..=b'f' => v |= (b - b'a' + 10) as u16,
            b'A'..=b'F' => v |= (b - b'A' + 10) as u16,
            _ => return None,
        }
    }
    Some(v)
}

fn parse_number(bytes: &[u8], i: &mut usize) -> Result<JsonValue, JsonError> {
    let start = *i;
    if bytes[*i] == b'-' {
        *i += 1;
        if *i >= bytes.len() {
            return Err(JsonError {
                message: "invalid number".to_string(),
                pos: start,
            });
        }
    }
    if bytes[*i] == b'0' {
        *i += 1;
    } else if matches!(bytes[*i], b'1'..=b'9') {
        *i += 1;
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    } else {
        return Err(JsonError {
            message: "invalid number".to_string(),
            pos: start,
        });
    }
    if *i < bytes.len() && bytes[*i] == b'.' {
        *i += 1;
        if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
            return Err(JsonError {
                message: "invalid number".to_string(),
                pos: start,
            });
        }
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    }
    if *i < bytes.len() && (bytes[*i] == b'e' || bytes[*i] == b'E') {
        *i += 1;
        if *i < bytes.len() && (bytes[*i] == b'+' || bytes[*i] == b'-') {
            *i += 1;
        }
        if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
            return Err(JsonError {
                message: "invalid number".to_string(),
                pos: start,
            });
        }
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    }

    let s = std::str::from_utf8(&bytes[start..*i]).map_err(|_| JsonError {
        message: "invalid utf-8".to_string(),
        pos: start,
    })?;
    if !s.contains(['.', 'e', 'E']) {
        if let Ok(n) = s.parse::<i64>() {
            return Ok(JsonValue::NumberI64(n));
        }
        if let Ok(n) = s.parse::<u64>() {
            return Ok(JsonValue::NumberU64(n));
        }
    }
    let n = s.parse::<f64>().map_err(|_| JsonError {
        message: "invalid number".to_string(),
        pos: start,
    })?;
    Ok(JsonValue::NumberF64(n))
}

fn parse_array(input: &str, bytes: &[u8], i: &mut usize) -> Result<JsonValue, JsonError> {
    if bytes.get(*i) != Some(&b'[') {
        return Err(JsonError {
            message: "expected array".to_string(),
            pos: *i,
        });
    }
    *i += 1;
    skip_ws(bytes, i);
    let mut out: Vec<JsonValue> = Vec::new();
    if bytes.get(*i) == Some(&b']') {
        *i += 1;
        return Ok(JsonValue::Array(out));
    }
    loop {
        skip_ws(bytes, i);
        let v = parse_value(input, bytes, i)?;
        out.push(v);
        skip_ws(bytes, i);
        match bytes.get(*i) {
            Some(b',') => {
                *i += 1;
                continue;
            }
            Some(b']') => {
                *i += 1;
                break;
            }
            Some(_) => {
                return Err(JsonError {
                    message: "expected ',' or ']'".to_string(),
                    pos: *i,
                });
            }
            None => {
                return Err(JsonError {
                    message: "unexpected EOF".to_string(),
                    pos: *i,
                });
            }
        }
    }
    Ok(JsonValue::Array(out))
}

fn parse_object(input: &str, bytes: &[u8], i: &mut usize) -> Result<JsonValue, JsonError> {
    if bytes.get(*i) != Some(&b'{') {
        return Err(JsonError {
            message: "expected object".to_string(),
            pos: *i,
        });
    }
    *i += 1;
    skip_ws(bytes, i);
    let mut out: Vec<(String, JsonValue)> = Vec::new();
    if bytes.get(*i) == Some(&b'}') {
        *i += 1;
        return Ok(JsonValue::Object(out));
    }
    loop {
        skip_ws(bytes, i);
        let key = match bytes.get(*i) {
            Some(b'"') => parse_string(input, bytes, i)?,
            _ => {
                return Err(JsonError {
                    message: "expected object key string".to_string(),
                    pos: *i,
                });
            }
        };
        skip_ws(bytes, i);
        if bytes.get(*i) != Some(&b':') {
            return Err(JsonError {
                message: "expected ':'".to_string(),
                pos: *i,
            });
        }
        *i += 1;
        skip_ws(bytes, i);
        let v = parse_value(input, bytes, i)?;
        out.push((key, v));
        skip_ws(bytes, i);
        match bytes.get(*i) {
            Some(b',') => {
                *i += 1;
                continue;
            }
            Some(b'}') => {
                *i += 1;
                break;
            }
            Some(_) => {
                return Err(JsonError {
                    message: "expected ',' or '}'".to_string(),
                    pos: *i,
                });
            }
            None => {
                return Err(JsonError {
                    message: "unexpected EOF".to_string(),
                    pos: *i,
                });
            }
        }
    }
    Ok(JsonValue::Object(out))
}

pub mod pretty {
    use super::{JsonValue, quote_json_string};

    pub fn to_pretty_json_string(v: &JsonValue, indent: usize) -> String {
        let mut out = String::new();
        write_value(&mut out, v, 0, indent);
        out
    }

    fn write_indent(out: &mut String, level: usize, indent: usize) {
        for _ in 0..(level * indent) {
            out.push(' ');
        }
    }

    fn write_value(out: &mut String, v: &JsonValue, level: usize, indent: usize) {
        match v {
            JsonValue::Null => out.push_str("null"),
            JsonValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            JsonValue::NumberI64(n) => out.push_str(&n.to_string()),
            JsonValue::NumberU64(n) => out.push_str(&n.to_string()),
            JsonValue::NumberF64(n) => out.push_str(&format!("{n}")),
            JsonValue::String(s) => out.push_str(&quote_json_string(s)),
            JsonValue::Array(a) => {
                if a.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push('[');
                out.push('\n');
                for (idx, item) in a.iter().enumerate() {
                    if idx > 0 {
                        out.push_str(",\n");
                    }
                    write_indent(out, level + 1, indent);
                    write_value(out, item, level + 1, indent);
                }
                out.push('\n');
                write_indent(out, level, indent);
                out.push(']');
            }
            JsonValue::Object(obj) => {
                if obj.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push('{');
                out.push('\n');
                for (idx, (k, v2)) in obj.iter().enumerate() {
                    if idx > 0 {
                        out.push_str(",\n");
                    }
                    write_indent(out, level + 1, indent);
                    out.push_str(&quote_json_string(k));
                    out.push_str(": ");
                    write_value(out, v2, level + 1, indent);
                }
                out.push('\n');
                write_indent(out, level, indent);
                out.push('}');
            }
        }
    }
}
