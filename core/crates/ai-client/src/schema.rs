//! JSON Schema generation helpers

use schemars::{JsonSchema, schema_for};
use serde_json::Value;

/// Generate a JSON schema for the given type
///
/// This produces an OpenAI-compatible JSON schema suitable for structured output.
pub fn generate_schema<T: JsonSchema>() -> Value {
    let schema = schema_for!(T);
    let mut value = serde_json::to_value(&schema).unwrap_or_default();

    // Post-process for OpenAI compatibility
    sanitize_for_openai(&mut value);

    value
}

/// Sanitize schema for OpenAI compatibility
///
/// OpenAI structured outputs have specific requirements:
/// - No `format` field on strings
/// - `additionalProperties: false` on objects
/// - All properties should be required
fn sanitize_for_openai(value: &mut Value) {
    if let Value::Object(obj) = value {
        // Remove unsupported fields
        obj.remove("$schema");
        obj.remove("title");
        obj.remove("format");

        // Set additionalProperties to false for objects
        if obj.get("type") == Some(&Value::String("object".to_string())) {
            obj.insert("additionalProperties".to_string(), Value::Bool(false));

            // Make all properties required
            if let Some(Value::Object(props)) = obj.get("properties") {
                let required: Vec<Value> = props.keys().map(|k| Value::String(k.clone())).collect();
                if !required.is_empty() {
                    obj.insert("required".to_string(), Value::Array(required));
                }
            }
        }

        // Recurse into nested schemas
        for (_, v) in obj.iter_mut() {
            sanitize_for_openai(v);
        }
    } else if let Value::Array(arr) = value {
        for v in arr.iter_mut() {
            sanitize_for_openai(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::JsonSchema;
    use serde::Deserialize;

    #[derive(JsonSchema, Deserialize)]
    struct TestOutput {
        items: Vec<String>,
        count: i32,
    }

    #[test]
    fn test_generate_schema() {
        let schema = generate_schema::<TestOutput>();
        assert!(schema.is_object());

        let obj = schema.as_object().unwrap();
        assert_eq!(obj.get("type"), Some(&Value::String("object".to_string())));
        assert_eq!(obj.get("additionalProperties"), Some(&Value::Bool(false)));
    }
}
