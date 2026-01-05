use crate::json::{JsonError, JsonValue, parse_strict_json};

pub fn strict_parse(text: &str) -> Result<JsonValue, JsonError> {
    parse_strict_json(text)
}
