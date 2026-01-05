use crate::json::JsonValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapeTokenType {
    Null,
    True,
    False,
    NumberI64,
    NumberU64,
    NumberF64,
    String,
    ObjectStart,
    ObjectEnd,
    ArrayStart,
    ArrayEnd,
}

impl TapeTokenType {
    pub fn as_str(self) -> &'static str {
        match self {
            TapeTokenType::Null => "null",
            TapeTokenType::True => "true",
            TapeTokenType::False => "false",
            TapeTokenType::NumberI64 => "number_i64",
            TapeTokenType::NumberU64 => "number_u64",
            TapeTokenType::NumberF64 => "number_f64",
            TapeTokenType::String => "string",
            TapeTokenType::ObjectStart => "object_start",
            TapeTokenType::ObjectEnd => "object_end",
            TapeTokenType::ArrayStart => "array_start",
            TapeTokenType::ArrayEnd => "array_end",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TapeEntry {
    pub token_type: TapeTokenType,
    pub offset: usize,
    pub length: usize,
    pub payload: u64, // jump index for containers; numeric payload for numbers
}

impl TapeEntry {
    pub fn new(token_type: TapeTokenType, offset: usize, length: usize) -> Self {
        Self {
            token_type,
            offset,
            length,
            payload: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tape {
    pub root_index: usize,
    pub data_span: (usize, usize), // absolute offsets in original data
    pub entries: Vec<TapeEntry>,
}

impl Tape {
    pub fn to_json_value(&self, max_entries: Option<usize>) -> JsonValue {
        let mut obj = vec![
            (
                "root_index".to_string(),
                JsonValue::NumberU64(self.root_index as u64),
            ),
            (
                "data_span".to_string(),
                JsonValue::Array(vec![
                    JsonValue::NumberU64(self.data_span.0 as u64),
                    JsonValue::NumberU64(self.data_span.1 as u64),
                ]),
            ),
            (
                "entry_count".to_string(),
                JsonValue::NumberU64(self.entries.len() as u64),
            ),
        ];

        if let Some(max_n) = max_entries {
            let n = std::cmp::min(max_n, self.entries.len());
            let mut entries_v = Vec::with_capacity(n);
            for e in self.entries.iter().take(n) {
                entries_v.push(JsonValue::Object(vec![
                    (
                        "t".to_string(),
                        JsonValue::String(e.token_type.as_str().to_string()),
                    ),
                    ("offset".to_string(), JsonValue::NumberU64(e.offset as u64)),
                    ("length".to_string(), JsonValue::NumberU64(e.length as u64)),
                    ("payload".to_string(), JsonValue::NumberU64(e.payload)),
                ]));
            }
            obj.push(("entries".to_string(), JsonValue::Array(entries_v)));
            obj.push((
                "entries_truncated".to_string(),
                JsonValue::Bool(n < self.entries.len()),
            ));
        }

        JsonValue::Object(obj)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TapeError {
    pub message: String,
    pub pos: usize,
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\r' | b'\t')
}

fn skip_ws(bytes: &[u8], i: &mut usize) {
    while *i < bytes.len() && is_ws(bytes[*i]) {
        *i += 1;
    }
}

fn err(message: &str, base_offset: usize, pos: usize) -> TapeError {
    TapeError {
        message: message.to_string(),
        pos: base_offset + pos,
    }
}

fn is_hex(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

fn parse_literal(
    bytes: &[u8],
    base_offset: usize,
    i: &mut usize,
    lit: &[u8],
) -> Result<(), TapeError> {
    if bytes.len().saturating_sub(*i) < lit.len() {
        return Err(err("unexpected EOF", base_offset, *i));
    }
    if &bytes[*i..*i + lit.len()] != lit {
        return Err(err("invalid literal", base_offset, *i));
    }
    *i += lit.len();
    Ok(())
}

fn parse_string(
    bytes: &[u8],
    base_offset: usize,
    i: &mut usize,
    entries: &mut Vec<TapeEntry>,
) -> Result<usize, TapeError> {
    let start = *i;
    if bytes.get(*i) != Some(&b'"') {
        return Err(err("expected string", base_offset, *i));
    }
    *i += 1;
    while *i < bytes.len() {
        let ch = bytes[*i];
        if ch == b'"' {
            *i += 1;
            let idx = entries.len();
            entries.push(TapeEntry::new(
                TapeTokenType::String,
                base_offset + start,
                *i - start,
            ));
            return Ok(idx);
        }
        if ch == b'\\' {
            *i += 1;
            if *i >= bytes.len() {
                return Err(err("unexpected EOF in string escape", base_offset, *i));
            }
            match bytes[*i] {
                b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {
                    *i += 1;
                }
                b'u' => {
                    *i += 1;
                    if bytes.len().saturating_sub(*i) < 4 {
                        return Err(err("invalid unicode escape", base_offset, *i));
                    }
                    if !is_hex(bytes[*i])
                        || !is_hex(bytes[*i + 1])
                        || !is_hex(bytes[*i + 2])
                        || !is_hex(bytes[*i + 3])
                    {
                        return Err(err("invalid unicode escape", base_offset, *i));
                    }
                    *i += 4;
                }
                _ => return Err(err("invalid escape", base_offset, *i)),
            }
            continue;
        }
        if ch < 0x20 {
            return Err(err("control character in string", base_offset, *i));
        }
        *i += 1;
    }
    Err(err("unterminated string", base_offset, start))
}

fn parse_number(
    bytes: &[u8],
    base_offset: usize,
    i: &mut usize,
    entries: &mut Vec<TapeEntry>,
) -> Result<usize, TapeError> {
    let start = *i;
    if bytes.get(*i) == Some(&b'-') {
        *i += 1;
    }
    if *i >= bytes.len() {
        return Err(err("invalid number", base_offset, start));
    }
    if bytes[*i] == b'0' {
        *i += 1;
    } else if matches!(bytes[*i], b'1'..=b'9') {
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    } else {
        return Err(err("invalid number", base_offset, start));
    }

    let mut is_float = false;
    if bytes.get(*i) == Some(&b'.') {
        is_float = true;
        *i += 1;
        if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
            return Err(err("invalid number", base_offset, start));
        }
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    }

    if let Some(b'e') | Some(b'E') = bytes.get(*i) {
        is_float = true;
        *i += 1;
        if let Some(b'+') | Some(b'-') = bytes.get(*i) {
            *i += 1;
        }
        if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
            return Err(err("invalid number", base_offset, start));
        }
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    }

    let s = std::str::from_utf8(&bytes[start..*i])
        .map_err(|_| err("invalid number utf-8", base_offset, start))?;

    let idx = entries.len();
    if is_float {
        let v: f64 = s
            .parse()
            .map_err(|_| err("invalid number", base_offset, start))?;
        let mut e = TapeEntry::new(TapeTokenType::NumberF64, base_offset + start, *i - start);
        e.payload = v.to_bits();
        entries.push(e);
        return Ok(idx);
    }

    if s.starts_with('-') {
        let v: i64 = s
            .parse()
            .map_err(|_| err("invalid number", base_offset, start))?;
        let mut e = TapeEntry::new(TapeTokenType::NumberI64, base_offset + start, *i - start);
        e.payload = v as u64;
        entries.push(e);
        return Ok(idx);
    }

    let v: u64 = s
        .parse()
        .map_err(|_| err("invalid number", base_offset, start))?;
    let mut e = TapeEntry::new(TapeTokenType::NumberU64, base_offset + start, *i - start);
    e.payload = v;
    entries.push(e);
    Ok(idx)
}

fn parse_array(
    bytes: &[u8],
    base_offset: usize,
    i: &mut usize,
    entries: &mut Vec<TapeEntry>,
) -> Result<usize, TapeError> {
    let start = *i;
    if bytes.get(*i) != Some(&b'[') {
        return Err(err("expected '['", base_offset, *i));
    }
    let start_idx = entries.len();
    entries.push(TapeEntry::new(
        TapeTokenType::ArrayStart,
        base_offset + start,
        1,
    ));
    *i += 1;
    skip_ws(bytes, i);
    if bytes.get(*i) == Some(&b']') {
        let end_idx = entries.len();
        entries.push(TapeEntry::new(TapeTokenType::ArrayEnd, base_offset + *i, 1));
        entries[start_idx].payload = end_idx as u64;
        *i += 1;
        return Ok(start_idx);
    }
    loop {
        parse_value(bytes, base_offset, i, entries)?;
        skip_ws(bytes, i);
        match bytes.get(*i) {
            Some(b',') => {
                *i += 1;
                skip_ws(bytes, i);
            }
            Some(b']') => {
                let end_idx = entries.len();
                entries.push(TapeEntry::new(TapeTokenType::ArrayEnd, base_offset + *i, 1));
                entries[start_idx].payload = end_idx as u64;
                *i += 1;
                return Ok(start_idx);
            }
            Some(_) => return Err(err("expected ',' or ']'", base_offset, *i)),
            None => return Err(err("unexpected EOF", base_offset, *i)),
        }
    }
}

fn parse_object(
    bytes: &[u8],
    base_offset: usize,
    i: &mut usize,
    entries: &mut Vec<TapeEntry>,
) -> Result<usize, TapeError> {
    let start = *i;
    if bytes.get(*i) != Some(&b'{') {
        return Err(err("expected '{'", base_offset, *i));
    }
    let start_idx = entries.len();
    entries.push(TapeEntry::new(
        TapeTokenType::ObjectStart,
        base_offset + start,
        1,
    ));
    *i += 1;
    skip_ws(bytes, i);
    if bytes.get(*i) == Some(&b'}') {
        let end_idx = entries.len();
        entries.push(TapeEntry::new(
            TapeTokenType::ObjectEnd,
            base_offset + *i,
            1,
        ));
        entries[start_idx].payload = end_idx as u64;
        *i += 1;
        return Ok(start_idx);
    }
    loop {
        skip_ws(bytes, i);
        parse_string(bytes, base_offset, i, entries)?;
        skip_ws(bytes, i);
        if bytes.get(*i) != Some(&b':') {
            return Err(err("expected ':'", base_offset, *i));
        }
        *i += 1;
        parse_value(bytes, base_offset, i, entries)?;
        skip_ws(bytes, i);
        match bytes.get(*i) {
            Some(b',') => {
                *i += 1;
            }
            Some(b'}') => {
                let end_idx = entries.len();
                entries.push(TapeEntry::new(
                    TapeTokenType::ObjectEnd,
                    base_offset + *i,
                    1,
                ));
                entries[start_idx].payload = end_idx as u64;
                *i += 1;
                return Ok(start_idx);
            }
            Some(_) => return Err(err("expected ',' or '}'", base_offset, *i)),
            None => return Err(err("unexpected EOF", base_offset, *i)),
        }
    }
}

fn parse_value(
    bytes: &[u8],
    base_offset: usize,
    i: &mut usize,
    entries: &mut Vec<TapeEntry>,
) -> Result<usize, TapeError> {
    skip_ws(bytes, i);
    let Some(&ch) = bytes.get(*i) else {
        return Err(err("unexpected EOF", base_offset, *i));
    };
    match ch {
        b'n' => {
            let idx = entries.len();
            parse_literal(bytes, base_offset, i, b"null")?;
            entries.push(TapeEntry::new(TapeTokenType::Null, base_offset + *i - 4, 4));
            Ok(idx)
        }
        b't' => {
            let idx = entries.len();
            parse_literal(bytes, base_offset, i, b"true")?;
            entries.push(TapeEntry::new(TapeTokenType::True, base_offset + *i - 4, 4));
            Ok(idx)
        }
        b'f' => {
            let idx = entries.len();
            parse_literal(bytes, base_offset, i, b"false")?;
            entries.push(TapeEntry::new(
                TapeTokenType::False,
                base_offset + *i - 5,
                5,
            ));
            Ok(idx)
        }
        b'"' => parse_string(bytes, base_offset, i, entries),
        b'{' => parse_object(bytes, base_offset, i, entries),
        b'[' => parse_array(bytes, base_offset, i, entries),
        b'-' | b'0'..=b'9' => parse_number(bytes, base_offset, i, entries),
        _ => Err(err("unexpected character", base_offset, *i)),
    }
}

pub fn parse_strict_tape(bytes: &[u8], base_offset: usize) -> Result<Tape, TapeError> {
    let mut i: usize = 0;
    let mut entries: Vec<TapeEntry> = Vec::new();
    skip_ws(bytes, &mut i);
    let root_index = parse_value(bytes, base_offset, &mut i, &mut entries)?;
    skip_ws(bytes, &mut i);
    if i != bytes.len() {
        return Err(err("trailing characters", base_offset, i));
    }
    Ok(Tape {
        root_index,
        data_span: (base_offset, base_offset + bytes.len()),
        entries,
    })
}

pub fn parse_object_pair_segment(
    bytes: &[u8],
    base_offset: usize,
) -> Result<Vec<TapeEntry>, TapeError> {
    let mut i: usize = 0;
    let mut entries: Vec<TapeEntry> = Vec::new();
    skip_ws(bytes, &mut i);
    parse_string(bytes, base_offset, &mut i, &mut entries)?;
    skip_ws(bytes, &mut i);
    if bytes.get(i) != Some(&b':') {
        return Err(err("expected ':'", base_offset, i));
    }
    i += 1;
    parse_value(bytes, base_offset, &mut i, &mut entries)?;
    skip_ws(bytes, &mut i);
    if i != bytes.len() {
        return Err(err("trailing characters", base_offset, i));
    }
    Ok(entries)
}

pub fn append_segment(dst: &mut Vec<TapeEntry>, seg: &[TapeEntry]) {
    let base = dst.len();
    for e in seg {
        let mut ee = *e;
        if matches!(
            ee.token_type,
            TapeTokenType::ObjectStart | TapeTokenType::ArrayStart
        ) {
            ee.payload = ee.payload.saturating_add(base as u64);
        }
        dst.push(ee);
    }
}
