#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenType {
    Punct,
    String,
    Number,
    Literal,
    Ident,
    Garbage,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub typ: TokenType,
    pub value: String,
    pub start: usize,
    pub end: usize,
    pub quote: Option<char>,
    pub closed: bool,
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\r' | b'\t')
}

fn is_delim(b: u8) -> bool {
    matches!(b, b'{' | b'}' | b'[' | b']' | b',' | b':' | b'"' | b'\'')
}

fn read_string(bytes: &[u8], mut i: usize, quote: u8) -> (Token, usize) {
    let start = i;
    i += 1;
    let mut out = String::new();
    let mut escape = false;
    while i < bytes.len() {
        let ch = bytes[i];
        if escape {
            // best-effort escape handling
            match ch {
                b'n' => out.push('\n'),
                b't' => out.push('\t'),
                b'r' => out.push('\r'),
                b'b' => out.push('\u{08}'),
                b'f' => out.push('\u{0C}'),
                b'u' => {
                    if i + 4 < bytes.len() {
                        let hex = &bytes[i + 1..i + 5];
                        if let Ok(hs) = std::str::from_utf8(hex) {
                            if let Ok(v) = u16::from_str_radix(hs, 16) {
                                if let Some(c) = char::from_u32(v as u32) {
                                    out.push(c);
                                    i += 4;
                                }
                            }
                        }
                    }
                }
                b'\\' => out.push('\\'),
                b'"' => out.push('"'),
                b'\'' => out.push('\''),
                other => out.push(other as char),
            }
            escape = false;
            i += 1;
            continue;
        }

        if ch == b'\\' {
            escape = true;
            i += 1;
            continue;
        }

        if ch == quote {
            let tok = Token {
                typ: TokenType::String,
                value: out,
                start,
                end: i + 1,
                quote: Some(quote as char),
                closed: true,
            };
            return (tok, i + 1);
        }

        // read utf-8 char
        let slice = &bytes[i..];
        let s = std::str::from_utf8(slice).unwrap_or("");
        let mut it = s.chars();
        if let Some(c) = it.next() {
            out.push(c);
            i += c.len_utf8();
        } else {
            break;
        }
    }
    let tok = Token {
        typ: TokenType::String,
        value: out,
        start,
        end: bytes.len(),
        quote: Some(quote as char),
        closed: false,
    };
    (tok, bytes.len())
}

fn read_number(bytes: &[u8], mut i: usize) -> (Token, usize) {
    let start = i;
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }
    let value = std::str::from_utf8(&bytes[start..i])
        .unwrap_or("")
        .to_string();
    (
        Token {
            typ: TokenType::Number,
            value,
            start,
            end: i,
            quote: None,
            closed: true,
        },
        i,
    )
}

fn read_word(text: &str, bytes: &[u8], mut i: usize) -> (Token, usize) {
    let start = i;
    i += 1;
    while i < bytes.len() && ((bytes[i] as char).is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    let word = &text[start..i];
    let low = word.to_ascii_lowercase();
    let (typ, value) = if low == "true" || low == "false" || low == "null" {
        (TokenType::Literal, low)
    } else {
        (TokenType::Ident, word.to_string())
    };
    (
        Token {
            typ,
            value,
            start,
            end: i,
            quote: None,
            closed: true,
        },
        i,
    )
}

pub fn tolerant_lex(text: &str, allow_single_quotes: bool) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut tokens: Vec<Token> = Vec::new();
    let mut i: usize = 0;
    while i < bytes.len() {
        let ch = bytes[i];
        if is_ws(ch) {
            i += 1;
            continue;
        }
        if matches!(ch, b'{' | b'}' | b'[' | b']' | b',' | b':') {
            tokens.push(Token {
                typ: TokenType::Punct,
                value: (ch as char).to_string(),
                start: i,
                end: i + 1,
                quote: None,
                closed: true,
            });
            i += 1;
            continue;
        }
        if ch == b'"' {
            let (tok, ni) = read_string(bytes, i, b'"');
            tokens.push(tok);
            i = ni;
            continue;
        }
        if ch == b'\'' && allow_single_quotes {
            let (tok, ni) = read_string(bytes, i, b'\'');
            tokens.push(tok);
            i = ni;
            continue;
        }
        if ch.is_ascii_digit() || ch == b'-' {
            let (tok, ni) = read_number(bytes, i);
            tokens.push(tok);
            i = ni;
            continue;
        }
        if (ch as char).is_ascii_alphabetic() || ch == b'_' {
            let (tok, ni) = read_word(text, bytes, i);
            tokens.push(tok);
            i = ni;
            continue;
        }

        // garbage chunk: read until whitespace or delimiter
        let start = i;
        i += 1;
        while i < bytes.len() && !is_ws(bytes[i]) && !is_delim(bytes[i]) {
            i += 1;
        }
        tokens.push(Token {
            typ: TokenType::Garbage,
            value: text[start..i].to_string(),
            start,
            end: i,
            quote: None,
            closed: true,
        });
    }
    tokens.push(Token {
        typ: TokenType::Eof,
        value: "".to_string(),
        start: bytes.len(),
        end: bytes.len(),
        quote: None,
        closed: true,
    });
    tokens
}
