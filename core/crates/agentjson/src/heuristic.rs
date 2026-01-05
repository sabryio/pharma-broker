use crate::types::{RepairAction, RepairOptions};

// Cost constants for repair operations
const COST_FIX_SMART_QUOTES: f64 = 0.7;
const COST_STRIP_LINE_COMMENT: f64 = 0.4;
const COST_STRIP_BLOCK_COMMENT: f64 = 0.6;
const COST_MAP_PYTHON_LITERAL: f64 = 0.4;
const COST_REMOVE_TRAILING_COMMA: f64 = 0.2;
const COST_CLOSE_OPEN_STRING: f64 = 3.0;
const COST_CLOSE_CONTAINER: f64 = 0.5;
const COST_WRAP_UNQUOTED_KEY: f64 = 0.3;
const COST_CONVERT_SINGLE_QUOTES: f64 = 0.3;
const COST_WRAP_UNQUOTED_VALUE: f64 = 0.4;
const COST_INSERT_MISSING_COMMA: f64 = 0.5;

// JSON literal keywords
const JSON_LITERALS: &[&str] = &["true", "false", "null"];

/// Helper trait for building RepairAction fluently
trait RepairActionExt {
    fn with_span(self, span: (usize, usize)) -> Self;
    fn with_at(self, at: usize) -> Self;
    fn with_note(self, note: impl Into<String>) -> Self;
}

impl RepairActionExt for RepairAction {
    fn with_span(mut self, span: (usize, usize)) -> Self {
        self.span = Some(span);
        self
    }

    fn with_at(mut self, at: usize) -> Self {
        self.at = Some(at);
        self
    }

    fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

fn fix_smart_quotes(text: &str) -> (String, Vec<RepairAction>) {
    let mut out = String::with_capacity(text.len());
    let mut changed = false;

    for ch in text.chars() {
        let replacement = match ch {
            '\u{201C}' | '\u{201D}' => Some('"'),  // curly double quotes
            '\u{2018}' | '\u{2019}' => Some('\''), // curly single quotes
            _ => None,
        };

        if let Some(r) = replacement {
            out.push(r);
            changed = true;
        } else {
            out.push(ch);
        }
    }

    if changed {
        (
            out,
            vec![RepairAction::new("fix_smart_quotes", COST_FIX_SMART_QUOTES)],
        )
    } else {
        (text.to_string(), vec![])
    }
}

fn strip_comments(text: &str) -> (String, Vec<RepairAction>) {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut repairs = Vec::new();
    let mut i = 0;
    let mut in_string = false;
    let mut escape = false;

    while i < bytes.len() {
        let ch = bytes[i];

        // Inside a string - just copy and track escape state
        if in_string {
            out.push(ch);
            match (escape, ch) {
                (true, _) => escape = false,
                (false, b'\\') => escape = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
            i += 1;
            continue;
        }

        // Start of string
        if ch == b'"' {
            in_string = true;
            out.push(ch);
            i += 1;
            continue;
        }

        // Check for line comment: //
        if ch == b'/' && bytes.get(i + 1) == Some(&b'/') {
            let start = i;
            i += 2;
            while i < bytes.len() && !matches!(bytes[i], b'\n' | b'\r') {
                i += 1;
            }
            repairs.push(
                RepairAction::new("strip_line_comment", COST_STRIP_LINE_COMMENT)
                    .with_span((start, i)),
            );
            continue;
        }

        // Check for block comment: /* ... */
        if ch == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let start = i;
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            repairs.push(
                RepairAction::new("strip_block_comment", COST_STRIP_BLOCK_COMMENT)
                    .with_span((start, i)),
            );
            continue;
        }

        out.push(ch);
        i += 1;
    }

    (String::from_utf8_lossy(&out).into_owned(), repairs)
}

fn normalize_python_literals(text: &str) -> (String, Vec<RepairAction>) {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut repairs = Vec::new();
    let mut i = 0;
    let mut in_string = false;
    let mut escape = false;

    while i < bytes.len() {
        let ch = bytes[i];

        // Inside a string - just copy and track escape state
        if in_string {
            out.push(ch);
            match (escape, ch) {
                (true, _) => escape = false,
                (false, b'\\') => escape = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
            i += 1;
            continue;
        }

        // Start of string
        if ch == b'"' {
            in_string = true;
            out.push(ch);
            i += 1;
            continue;
        }

        // Check for identifier (potential Python literal)
        if is_ident_start(ch) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_char(bytes[i]) {
                i += 1;
            }
            let word = &text[start..i];

            // Map Python literals to JSON equivalents
            let mapped = match word {
                "True" => Some("true"),
                "False" => Some("false"),
                "None" | "undefined" => Some("null"),
                _ => None,
            };

            if let Some(json_literal) = mapped {
                out.extend_from_slice(json_literal.as_bytes());
                repairs.push(
                    RepairAction::new("map_python_literal", COST_MAP_PYTHON_LITERAL)
                        .with_span((start, i))
                        .with_note(format!("{word}->{json_literal}")),
                );
            } else {
                out.extend_from_slice(word.as_bytes());
            }
            continue;
        }

        out.push(ch);
        i += 1;
    }

    (String::from_utf8_lossy(&out).into_owned(), repairs)
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\r' | b'\t')
}

fn is_ident_start(ch: u8) -> bool {
    ch.is_ascii_alphabetic() || ch == b'_'
}

fn is_ident_char(ch: u8) -> bool {
    ch.is_ascii_alphanumeric() || ch == b'_'
}

/// Wraps unquoted keys with double quotes.
/// Detects patterns like `identifier:` and converts to `"identifier":`.
fn wrap_unquoted_keys(text: &str, opt: &RepairOptions) -> (String, Vec<RepairAction>) {
    if !opt.allow_unquoted_keys {
        return (text.to_string(), vec![]);
    }

    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len() + 256);
    let mut repairs = Vec::new();
    let mut i: usize = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut string_quote: u8 = b'"';

    while i < bytes.len() {
        let ch = bytes[i];

        // Handle string state
        if in_string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == b'\\' {
                escape = true;
            } else if ch == string_quote {
                in_string = false;
            }
            i += 1;
            continue;
        }

        // Start of string
        if ch == b'"' || ch == b'\'' {
            in_string = true;
            string_quote = ch;
            out.push(ch);
            i += 1;
            continue;
        }

        // Check for identifier that might be an unquoted key
        if is_ident_start(ch) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_char(bytes[i]) {
                i += 1;
            }
            let word = &text[start..i];

            // Skip whitespace to look for colon
            let mut j = i;
            while j < bytes.len() && is_ws(bytes[j]) {
                j += 1;
            }

            // If followed by colon, this is an unquoted key
            if j < bytes.len() && bytes[j] == b':' {
                let low = word.to_ascii_lowercase();
                let is_json_literal = JSON_LITERALS.contains(&low.as_str());

                if is_json_literal {
                    // Keep JSON literals as-is (they're valid values, not keys in practice)
                    out.extend_from_slice(word.as_bytes());
                } else {
                    // Wrap with quotes
                    out.push(b'"');
                    out.extend_from_slice(word.as_bytes());
                    out.push(b'"');
                    repairs.push(
                        RepairAction::new("wrap_unquoted_key", COST_WRAP_UNQUOTED_KEY)
                            .with_span((start, i))
                            .with_note(format!("{word} -> \"{word}\"")),
                    );
                }
            } else {
                // Not a key, just output as-is
                out.extend_from_slice(word.as_bytes());
            }
            continue;
        }

        out.push(ch);
        i += 1;
    }

    (String::from_utf8_lossy(&out).to_string(), repairs)
}

/// Converts single-quoted strings to double-quoted strings.
fn convert_single_quotes(text: &str, opt: &RepairOptions) -> (String, Vec<RepairAction>) {
    if !opt.allow_single_quotes {
        return (text.to_string(), vec![]);
    }

    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut repairs = Vec::new();
    let mut i: usize = 0;
    let mut in_double_string = false;
    let mut escape = false;

    while i < bytes.len() {
        let ch = bytes[i];

        // Handle double-quoted string state
        if in_double_string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == b'\\' {
                escape = true;
            } else if ch == b'"' {
                in_double_string = false;
            }
            i += 1;
            continue;
        }

        // Start of double-quoted string
        if ch == b'"' {
            in_double_string = true;
            out.push(ch);
            i += 1;
            continue;
        }

        // Handle single-quoted string - convert to double quotes
        if ch == b'\'' {
            let start = i;
            out.push(b'"'); // Replace opening single quote with double quote
            i += 1;
            let mut content_escape = false;

            while i < bytes.len() {
                let c = bytes[i];
                if content_escape {
                    // Handle escape sequences
                    if c == b'\'' {
                        out.push(b'\''); // Just output the single quote (no backslash needed)
                    } else if c == b'"' {
                        out.push(b'\\'); // Need to escape double quote in double-quoted string
                        out.push(b'"');
                    } else {
                        out.push(b'\\');
                        out.push(c);
                    }
                    content_escape = false;
                    i += 1;
                    continue;
                }

                if c == b'\\' {
                    content_escape = true;
                    i += 1;
                    continue;
                }

                if c == b'\'' {
                    out.push(b'"'); // Replace closing single quote with double quote
                    i += 1;
                    repairs.push(
                        RepairAction::new("convert_single_quotes", COST_CONVERT_SINGLE_QUOTES)
                            .with_span((start, i)),
                    );
                    break;
                }

                // Escape any unescaped double quotes in the content
                if c == b'"' {
                    out.push(b'\\');
                }
                out.push(c);
                i += 1;
            }
            continue;
        }

        out.push(ch);
        i += 1;
    }

    (String::from_utf8_lossy(&out).to_string(), repairs)
}

/// Wraps unquoted values in arrays with double quotes.
/// Detects patterns like `[admin, user]` and converts to `["admin", "user"]`.
fn wrap_unquoted_array_values(text: &str, opt: &RepairOptions) -> (String, Vec<RepairAction>) {
    if !opt.allow_unquoted_values {
        return (text.to_string(), vec![]);
    }

    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len() + 256);
    let mut repairs = Vec::new();
    let mut i: usize = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut array_depth: i32 = 0;
    let mut _object_depth: i32 = 0; // tracked for future use

    while i < bytes.len() {
        let ch = bytes[i];

        // Handle string state
        if in_string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == b'\\' {
                escape = true;
            } else if ch == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        // Start of string
        if ch == b'"' {
            in_string = true;
            out.push(ch);
            i += 1;
            continue;
        }

        // Track nesting
        match ch {
            b'[' => {
                array_depth += 1;
                out.push(ch);
                i += 1;
                continue;
            }
            b']' => {
                array_depth -= 1;
                out.push(ch);
                i += 1;
                continue;
            }
            b'{' => {
                _object_depth += 1;
                out.push(ch);
                i += 1;
                continue;
            }
            b'}' => {
                _object_depth -= 1;
                out.push(ch);
                i += 1;
                continue;
            }
            _ => {}
        }

        // Check for identifier that might be an unquoted array value
        // Only process if we're inside an array but not inside an object key position
        if array_depth > 0 && is_ident_start(ch) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_char(bytes[i]) {
                i += 1;
            }
            let word = &text[start..i];

            // Skip whitespace to look for what comes next
            let mut j = i;
            while j < bytes.len() && is_ws(bytes[j]) {
                j += 1;
            }

            // If followed by colon, this is a key (in a nested object), not an array value
            if j < bytes.len() && bytes[j] == b':' {
                // It's a key, output as-is (wrap_unquoted_keys should have handled it)
                out.extend_from_slice(word.as_bytes());
                continue;
            }

            // Check if it's a JSON literal
            let low = word.to_ascii_lowercase();
            let is_json_literal = JSON_LITERALS.contains(&low.as_str());

            if is_json_literal {
                out.extend_from_slice(low.as_bytes());
            } else {
                // Wrap with quotes - it's an unquoted array value
                out.push(b'"');
                out.extend_from_slice(word.as_bytes());
                out.push(b'"');
                repairs.push(
                    RepairAction::new("wrap_unquoted_value", COST_WRAP_UNQUOTED_VALUE)
                        .with_span((start, i))
                        .with_note(format!("{word} -> \"{word}\"")),
                );
            }
            continue;
        }

        out.push(ch);
        i += 1;
    }

    (String::from_utf8_lossy(&out).to_string(), repairs)
}

/// Inserts missing commas between adjacent values/key-value pairs.
/// Detects patterns like `"value1" "value2"` or `} {` or `] [` etc.
fn insert_missing_commas(text: &str) -> (String, Vec<RepairAction>) {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len() + 128);
    let mut repairs = Vec::new();
    let mut i: usize = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut last_value_end: Option<usize> = None;

    while i < bytes.len() {
        let ch = bytes[i];

        // Handle string state
        if in_string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == b'\\' {
                escape = true;
            } else if ch == b'"' {
                in_string = false;
                last_value_end = Some(out.len());
            }
            i += 1;
            continue;
        }

        // Skip whitespace
        if is_ws(ch) {
            out.push(ch);
            i += 1;
            continue;
        }

        // Check if we need to insert a comma before this token
        let needs_comma = if last_value_end.is_some() {
            match ch {
                // These could start a new value/key after a previous value
                b'"' => true,
                b'{' | b'[' => true,
                b'-' | b'0'..=b'9' => true,
                c if is_ident_start(c) => {
                    // Check if it's an identifier (could be unquoted key or literal)
                    true
                }
                _ => false,
            }
        } else {
            false
        };

        if needs_comma {
            // Insert comma before current position (after last value, before whitespace)
            let mut ws_start = out.len();
            while ws_start > 0 && is_ws(out[ws_start - 1]) {
                ws_start -= 1;
            }
            out.insert(ws_start, b',');
            repairs.push(
                RepairAction::new("insert_missing_comma", COST_INSERT_MISSING_COMMA).with_at(i),
            );
            last_value_end = None;
        }

        // Process current character
        match ch {
            b'"' => {
                in_string = true;
                out.push(ch);
                i += 1;
            }
            b'}' | b']' => {
                out.push(ch);
                last_value_end = Some(out.len());
                i += 1;
            }
            b'{' | b'[' => {
                out.push(ch);
                last_value_end = None;
                i += 1;
            }
            b',' | b':' => {
                out.push(ch);
                last_value_end = None;
                i += 1;
            }
            _ if ch.is_ascii_digit() || ch == b'-' => {
                // Read number
                let start = i;
                i += 1;
                while i < bytes.len()
                    && (bytes[i].is_ascii_digit()
                        || bytes[i] == b'.'
                        || bytes[i] == b'e'
                        || bytes[i] == b'E'
                        || bytes[i] == b'+'
                        || bytes[i] == b'-')
                {
                    i += 1;
                }
                out.extend_from_slice(&bytes[start..i]);
                last_value_end = Some(out.len());
            }
            _ if is_ident_start(ch) => {
                // Read identifier/literal
                let start = i;
                i += 1;
                while i < bytes.len() && is_ident_char(bytes[i]) {
                    i += 1;
                }
                out.extend_from_slice(&bytes[start..i]);
                // Check if followed by colon (it's a key, not a value)
                let mut j = i;
                while j < bytes.len() && is_ws(bytes[j]) {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b':' {
                    last_value_end = None;
                } else {
                    last_value_end = Some(out.len());
                }
            }
            _ => {
                out.push(ch);
                i += 1;
            }
        }
    }

    (String::from_utf8_lossy(&out).to_string(), repairs)
}

fn remove_trailing_commas(text: &str) -> (String, Vec<RepairAction>) {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut repairs = Vec::new();
    let mut i: usize = 0;
    let mut in_string = false;
    let mut escape = false;
    while i < bytes.len() {
        let ch = bytes[i];
        if in_string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == b'\\' {
                escape = true;
            } else if ch == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if ch == b'"' {
            in_string = true;
            out.push(ch);
            i += 1;
            continue;
        }

        if ch == b',' {
            let mut j = i + 1;
            while j < bytes.len() && is_ws(bytes[j]) {
                j += 1;
            }
            // Trailing comma: followed by ] or } or end of input
            if j >= bytes.len() || matches!(bytes[j], b'}' | b']') {
                repairs.push(
                    RepairAction::new("remove_trailing_comma", COST_REMOVE_TRAILING_COMMA)
                        .with_at(i),
                );
                i += 1;
                continue;
            }
        }

        out.push(ch);
        i += 1;
    }
    (String::from_utf8_lossy(&out).to_string(), repairs)
}

fn append_missing_closers(text: &str) -> (String, Vec<RepairAction>) {
    let bytes = text.as_bytes();
    let mut in_string = false;
    let mut escape = false;
    let mut depth_brace: i64 = 0;
    let mut depth_bracket: i64 = 0;
    let mut i: usize = 0;
    while i < bytes.len() {
        let ch = bytes[i];
        if in_string {
            if escape {
                escape = false;
            } else if ch == b'\\' {
                escape = true;
            } else if ch == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if ch == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        match ch {
            b'{' => depth_brace += 1,
            b'}' => depth_brace -= 1,
            b'[' => depth_bracket += 1,
            b']' => depth_bracket -= 1,
            _ => {}
        }
        i += 1;
    }

    let mut out = text.to_string();
    let mut repairs = Vec::new();

    // Close unclosed string
    if in_string {
        out.push('"');
        repairs.push(
            RepairAction::new("close_open_string", COST_CLOSE_OPEN_STRING).with_at(text.len()),
        );
    }

    // Close unclosed containers
    let unclosed_braces = depth_brace.max(0) as usize;
    let unclosed_brackets = depth_bracket.max(0) as usize;

    if unclosed_braces > 0 || unclosed_brackets > 0 {
        out.push_str(&"]".repeat(unclosed_brackets));
        out.push_str(&"}".repeat(unclosed_braces));

        let total_closers = unclosed_braces + unclosed_brackets;
        repairs.push(
            RepairAction::new(
                "close_containers",
                COST_CLOSE_CONTAINER * total_closers as f64,
            )
            .with_at(text.len())
            .with_note(format!(
                "brace={unclosed_braces}, bracket={unclosed_brackets}"
            )),
        );
    }

    (out, repairs)
}

pub fn heuristic_repair(extracted_text: &str, opt: &RepairOptions) -> (String, Vec<RepairAction>) {
    let mut text = extracted_text.to_string();
    let mut repairs: Vec<RepairAction> = Vec::new();

    // Step 1: Fix smart quotes (curly quotes -> straight quotes)
    let (t2, r2) = fix_smart_quotes(&text);
    if t2 != text {
        text = t2;
        repairs.extend(r2);
    }

    // Step 2: Strip comments (// and /* */)
    if opt.allow_comments {
        let (t2, r2) = strip_comments(&text);
        if t2 != text {
            text = t2;
            repairs.extend(r2);
        }
    }

    // Step 3: Wrap unquoted keys with quotes (identifier: -> "identifier":)
    // This MUST happen before convert_single_quotes to handle mixed cases
    if opt.allow_unquoted_keys {
        let (t2, r2) = wrap_unquoted_keys(&text, opt);
        if t2 != text {
            text = t2;
            repairs.extend(r2);
        }
    }

    // Step 4: Convert single quotes to double quotes
    if opt.allow_single_quotes {
        let (t2, r2) = convert_single_quotes(&text, opt);
        if t2 != text {
            text = t2;
            repairs.extend(r2);
        }
    }

    // Step 5: Wrap unquoted array values ([admin, user] -> ["admin", "user"])
    if opt.allow_unquoted_values {
        let (t2, r2) = wrap_unquoted_array_values(&text, opt);
        if t2 != text {
            text = t2;
            repairs.extend(r2);
        }
    }

    // Step 6: Normalize Python literals (True -> true, False -> false, None -> null)
    if opt.allow_python_literals {
        let (t2, r2) = normalize_python_literals(&text);
        if t2 != text {
            text = t2;
            repairs.extend(r2);
        }
    }

    // Step 7: Insert missing commas between adjacent values
    let (t2, r2) = insert_missing_commas(&text);
    if t2 != text {
        text = t2;
        repairs.extend(r2);
    }

    // Step 8: Remove trailing commas
    let (t2, r2) = remove_trailing_commas(&text);
    if t2 != text {
        text = t2;
        repairs.extend(r2);
    }

    // Step 9: Append missing closers (close unclosed strings, brackets, braces)
    let (t2, r2) = append_missing_closers(&text);
    if t2 != text {
        text = t2;
        repairs.extend(r2);
    }

    (text, repairs)
}
