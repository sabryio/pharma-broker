use crate::json::JsonValue;

fn clamp_char_boundary(text: &str, mut idx: usize) -> usize {
    if idx > text.len() {
        idx = text.len();
    }
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn make_snippet(text: &str, center: Option<usize>, window: usize) -> (String, (usize, usize)) {
    let len = text.len();
    let mut center = center.unwrap_or_else(|| std::cmp::min(len, len / 2));
    if center > len {
        center = len;
    }
    let half = std::cmp::max(1usize, window / 2);
    let mut start = center.saturating_sub(half);
    let mut end = std::cmp::min(len, center + half);
    start = clamp_char_boundary(text, start);
    end = std::cmp::max(start, clamp_char_boundary(text, end));
    (text[start..end].to_string(), (start, end))
}

fn as_object_or_empty(v: Option<&JsonValue>) -> JsonValue {
    match v {
        Some(JsonValue::Object(o)) => JsonValue::Object(o.clone()),
        _ => JsonValue::Object(Vec::new()),
    }
}

pub fn build_llm_payload_json(
    extracted_text: &str,
    mode: &str,
    error_pos: Option<usize>,
    schema_hint: Option<&JsonValue>,
    parser_state: Option<&JsonValue>,
    max_suggestions: usize,
    span_window: usize,
) -> JsonValue {
    let (snippet_text, (start, end)) = make_snippet(extracted_text, error_pos, span_window);
    JsonValue::Object(vec![
        (
            "task".to_string(),
            JsonValue::String("json_deep_repair".to_string()),
        ),
        ("mode".to_string(), JsonValue::String(mode.to_string())),
        (
            "snippet".to_string(),
            JsonValue::Object(vec![
                ("text".to_string(), JsonValue::String(snippet_text)),
                (
                    "encoding".to_string(),
                    JsonValue::String("utf-8".to_string()),
                ),
                (
                    "span_in_extracted".to_string(),
                    JsonValue::Array(vec![
                        JsonValue::NumberU64(start as u64),
                        JsonValue::NumberU64(end as u64),
                    ]),
                ),
            ]),
        ),
        ("parser_state".to_string(), as_object_or_empty(parser_state)),
        ("schema_hint".to_string(), as_object_or_empty(schema_hint)),
        (
            "constraints".to_string(),
            JsonValue::Object(vec![
                (
                    "max_suggestions".to_string(),
                    JsonValue::NumberU64(max_suggestions as u64),
                ),
                ("prefer_minimal_change".to_string(), JsonValue::Bool(true)),
                ("return_json_only".to_string(), JsonValue::Bool(true)),
            ]),
        ),
    ])
}

#[derive(Debug, Clone)]
enum PatchOp {
    Delete {
        start: usize,
        end: usize,
    },
    Replace {
        start: usize,
        end: usize,
        text: String,
    },
    Insert {
        at: usize,
        text: String,
    },
    TruncateAfter {
        at: usize,
    },
}

fn num_to_usize(v: &JsonValue) -> Option<usize> {
    match v {
        JsonValue::NumberU64(n) => Some(*n as usize),
        JsonValue::NumberI64(n) => {
            if *n < 0 {
                Some(0)
            } else {
                Some(*n as usize)
            }
        }
        JsonValue::NumberF64(n) => {
            if *n <= 0.0 {
                Some(0)
            } else {
                Some(*n as usize)
            }
        }
        _ => None,
    }
}

fn get_field<'a>(obj: &'a [(String, JsonValue)], key: &str) -> Option<&'a JsonValue> {
    obj.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn parse_patch_ops(ops: &[JsonValue]) -> Result<Vec<PatchOp>, String> {
    let mut out: Vec<PatchOp> = Vec::new();
    for op in ops {
        let obj = match op {
            JsonValue::Object(o) => o,
            _ => return Err("patch op must be an object".to_string()),
        };
        let kind = match get_field(obj, "op") {
            Some(JsonValue::String(s)) => s.as_str(),
            _ => return Err("patch op missing 'op' string".to_string()),
        };
        match kind {
            "delete" | "replace" => {
                let span =
                    get_field(obj, "span").ok_or_else(|| format!("invalid span for {kind}"))?;
                let (start, end) = match span {
                    JsonValue::Array(a) if a.len() == 2 => {
                        let s = num_to_usize(&a[0])
                            .ok_or_else(|| format!("invalid span start for {kind}"))?;
                        let e = num_to_usize(&a[1])
                            .ok_or_else(|| format!("invalid span end for {kind}"))?;
                        (s, e)
                    }
                    _ => return Err(format!("invalid span for {kind}")),
                };
                if kind == "delete" {
                    out.push(PatchOp::Delete { start, end });
                } else {
                    let text = match get_field(obj, "text") {
                        Some(JsonValue::String(s)) => s.clone(),
                        _ => "".to_string(),
                    };
                    out.push(PatchOp::Replace { start, end, text });
                }
            }
            "insert" => {
                let at = match get_field(obj, "at").and_then(num_to_usize) {
                    Some(v) => v,
                    None => return Err("invalid 'at' for insert".to_string()),
                };
                let text = match get_field(obj, "text") {
                    Some(JsonValue::String(s)) => s.clone(),
                    _ => "".to_string(),
                };
                out.push(PatchOp::Insert { at, text });
            }
            "truncate_after" => {
                let at = match get_field(obj, "at").and_then(num_to_usize) {
                    Some(v) => v,
                    None => return Err("invalid 'at' for truncate_after".to_string()),
                };
                out.push(PatchOp::TruncateAfter { at });
            }
            _ => return Err(format!("unsupported patch op: {kind:?}")),
        }
    }
    Ok(out)
}

pub fn apply_patch_ops_utf8(extracted_text: &str, ops: &[JsonValue]) -> Result<String, String> {
    let mut b: Vec<u8> = extracted_text.as_bytes().to_vec();

    let mut parsed = parse_patch_ops(ops)?;
    // Apply from back to front to keep offsets stable.
    parsed.sort_by(|a, b| {
        let (sa, ea) = match a {
            PatchOp::Delete { start, end } => (*start, *end),
            PatchOp::Replace { start, end, .. } => (*start, *end),
            PatchOp::Insert { at, .. } => (*at, *at),
            PatchOp::TruncateAfter { at } => (*at, *at),
        };
        let (sb, eb) = match b {
            PatchOp::Delete { start, end } => (*start, *end),
            PatchOp::Replace { start, end, .. } => (*start, *end),
            PatchOp::Insert { at, .. } => (*at, *at),
            PatchOp::TruncateAfter { at } => (*at, *at),
        };
        // reverse
        (sb, eb).cmp(&(sa, ea))
    });

    for op in parsed {
        match op {
            PatchOp::Delete { start, end } => {
                let s = start.min(b.len());
                let e = end.min(b.len());
                let mut out = Vec::with_capacity(b.len().saturating_sub(e - s));
                out.extend_from_slice(&b[..s]);
                out.extend_from_slice(&b[e..]);
                b = out;
            }
            PatchOp::Replace { start, end, text } => {
                let s = start.min(b.len());
                let e = end.min(b.len());
                let repl = text.as_bytes();
                let mut out = Vec::with_capacity(b.len().saturating_sub(e - s) + repl.len());
                out.extend_from_slice(&b[..s]);
                out.extend_from_slice(repl);
                out.extend_from_slice(&b[e..]);
                b = out;
            }
            PatchOp::Insert { at, text } => {
                let s = at.min(b.len());
                let ins = text.as_bytes();
                let mut out = Vec::with_capacity(b.len() + ins.len());
                out.extend_from_slice(&b[..s]);
                out.extend_from_slice(ins);
                out.extend_from_slice(&b[s..]);
                b = out;
            }
            PatchOp::TruncateAfter { at } => {
                let s = at.min(b.len());
                b.truncate(s);
            }
        }
    }

    Ok(String::from_utf8_lossy(&b).to_string())
}
