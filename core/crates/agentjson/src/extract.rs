use crate::types::RepairAction;

#[derive(Debug, Clone)]
pub struct Extraction {
    pub extracted: String,
    pub span: (usize, usize), // byte offsets in original text
    pub truncated: bool,
    pub method: String,
    pub repairs: Vec<RepairAction>,
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\r' | b'\t')
}

fn find_code_fence(text: &str) -> Option<(usize, usize, usize, usize)> {
    // Returns (fence_start, inner_start, inner_end, fence_end)
    // Looks for ```json ... ``` or ``` ... ``` (json optional).
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == b'`' && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
            let fence_start = i;
            i += 3;
            // optional "json"
            while i < bytes.len() && is_ws(bytes[i]) {
                i += 1;
            }
            if i + 4 <= bytes.len() {
                let maybe = &bytes[i..i + 4];
                if maybe.eq_ignore_ascii_case(b"json") {
                    i += 4;
                }
            }
            // skip whitespace/newline after language
            while i < bytes.len() && (is_ws(bytes[i]) || bytes[i] == b'\n') {
                i += 1;
            }
            let inner_start = i;
            // find closing fence
            while i + 2 < bytes.len() {
                if bytes[i] == b'`' && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
                    let inner_end = i;
                    let fence_end = i + 3;
                    return Some((fence_start, inner_start, inner_end, fence_end));
                }
                i += 1;
            }
            return None;
        }
        i += 1;
    }
    None
}

fn brace_scan_extract(text: &str) -> Extraction {
    let bytes = text.as_bytes();
    let start_obj = text.find('{');
    let start_arr = text.find('[');
    let start = match (start_obj, start_arr) {
        (None, None) => {
            return Extraction {
                extracted: text.to_string(),
                span: (0, text.len()),
                truncated: true,
                method: "no_json_found".to_string(),
                repairs: vec![],
            };
        }
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (Some(a), Some(b)) => a.min(b),
    };

    let mut in_string = false;
    let mut escape = false;
    let mut depth_brace: i64 = 0;
    let mut depth_bracket: i64 = 0;
    let mut end = bytes.len();
    let mut truncated = true;

    let mut i = start;
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

        if depth_brace == 0 && depth_bracket == 0 && i >= start {
            end = i + 1;
            truncated = false;
            break;
        }
        i += 1;
    }

    let extracted = &text[start..end];
    let mut repairs = Vec::new();
    if start > 0 {
        let mut a = RepairAction::new("strip_prefix_text", 0.3);
        a.span = Some((0, start));
        repairs.push(a);
    }
    if end < text.len() {
        let mut a = RepairAction::new("strip_suffix_text", 0.3);
        a.span = Some((end, text.len()));
        repairs.push(a);
    }
    Extraction {
        extracted: extracted.to_string(),
        span: (start, end),
        truncated,
        method: "brace_scan".to_string(),
        repairs,
    }
}

pub fn extract_json_candidate(text: &str) -> Extraction {
    if let Some((fence_start, inner_start, inner_end, fence_end)) = find_code_fence(text) {
        let inner = text[inner_start..inner_end].trim();
        if inner.starts_with('{') || inner.starts_with('[') {
            let mut repairs = Vec::new();
            if inner_start > 0 {
                let mut a = RepairAction::new("strip_prefix_text", 0.3);
                a.span = Some((0, inner_start));
                repairs.push(a);
            }
            if inner_end < text.len() {
                let mut a = RepairAction::new("strip_suffix_text", 0.3);
                a.span = Some((inner_end, text.len()));
                repairs.push(a);
            }
            let mut a = RepairAction::new("strip_code_fence", 0.2);
            a.span = Some((fence_start, fence_end));
            repairs.push(a);
            return Extraction {
                extracted: inner.to_string(),
                span: (inner_start, inner_end),
                truncated: false,
                method: "code_fence".to_string(),
                repairs,
            };
        }
    }
    brace_scan_extract(text)
}
