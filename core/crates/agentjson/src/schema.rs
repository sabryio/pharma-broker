use crate::json::JsonValue;

fn type_ok(v: &JsonValue, t: &str) -> bool {
    match t {
        "int" => matches!(v, JsonValue::NumberI64(_) | JsonValue::NumberU64(_)),
        "float" => matches!(
            v,
            JsonValue::NumberI64(_) | JsonValue::NumberU64(_) | JsonValue::NumberF64(_)
        ),
        "str" => matches!(v, JsonValue::String(_)),
        "bool" => matches!(v, JsonValue::Bool(_)),
        "object" => matches!(v, JsonValue::Object(_)),
        "array" => matches!(v, JsonValue::Array(_)),
        "null" => matches!(v, JsonValue::Null),
        _ => true,
    }
}

pub fn schema_match_score(value: &JsonValue, schema: Option<&JsonValue>) -> Option<f64> {
    let schema = schema?;
    let obj = match value {
        JsonValue::Object(v) => v,
        _ => return Some(0.0),
    };

    let required: Vec<String> = match schema {
        JsonValue::Object(fields) => fields
            .iter()
            .find(|(k, _)| k == "required_keys")
            .and_then(|(_, v)| match v {
                JsonValue::Array(a) => Some(
                    a.iter()
                        .filter_map(|x| match x {
                            JsonValue::String(s) => Some(s.clone()),
                            _ => None,
                        })
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    let types: Vec<(String, String)> = match schema {
        JsonValue::Object(fields) => fields
            .iter()
            .find(|(k, _)| k == "types")
            .and_then(|(_, v)| match v {
                JsonValue::Object(map) => Some(
                    map.iter()
                        .filter_map(|(k, v2)| match v2 {
                            JsonValue::String(s) => Some((k.clone(), s.clone())),
                            _ => None,
                        })
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    let req_ok = if required.is_empty() {
        1.0
    } else {
        let present = required
            .iter()
            .filter(|k| obj.iter().any(|(kk, _)| kk == *k))
            .count();
        present as f64 / (required.len() as f64)
    };

    let type_ok_score = if types.is_empty() {
        1.0
    } else {
        let mut checks = 0usize;
        let mut good = 0usize;
        for (k, t) in types.iter() {
            checks += 1;
            if let Some((_, v2)) = obj.iter().find(|(kk, _)| kk == k)
                && type_ok(v2, t)
            {
                good += 1;
            }
        }
        if checks == 0 {
            1.0
        } else {
            good as f64 / (checks as f64)
        }
    };

    Some(0.5 * req_ok + 0.5 * type_ok_score)
}
