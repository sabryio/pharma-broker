//! Error types for the AI client

use thiserror::Error;

/// AI client error types
#[derive(Debug, Error)]
pub enum Error {
    /// Network errors (connection failed, timeout)
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// API errors (non-2xx status codes)
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    /// JSON parsing errors
    #[error("Parse error: {0}")]
    Parse(#[from] serde_json::Error),

    /// Schema validation errors
    #[error("Schema error: {0}")]
    Schema(String),

    /// Retry exhausted
    #[error("Retry exhausted after {attempts} attempts: {last_error}")]
    RetryExhausted { attempts: u32, last_error: String },

    /// Circuit breaker open
    #[error("Circuit breaker open")]
    CircuitOpen,

    /// Empty response from model
    #[error("Empty response from model")]
    EmptyResponse,

    /// Incomplete JSON response (truncated output from model)
    #[error("Incomplete JSON response: {0}")]
    IncompleteJson(String),
}

impl Error {
    /// Check if a serde_json error indicates incomplete/truncated JSON
    ///
    /// Returns true for errors like "EOF while parsing" or "unexpected end of input"
    /// which indicate the model returned truncated output.
    pub fn is_incomplete_json(err: &serde_json::Error) -> bool {
        let msg = err.to_string().to_lowercase();
        msg.contains("eof while parsing") || msg.contains("unexpected end of input")
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::Network(_) => true,
            Error::Api { status, .. } => *status >= 500 || *status == 429,
            Error::Parse(_) => false,
            Error::IncompleteJson(_) => true,
            Error::Schema(_) => false,
            Error::RetryExhausted { .. } => false,
            Error::CircuitOpen => false,
            Error::EmptyResponse => true,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Helper to create a serde_json error from truncated JSON
    fn make_truncated_json_error(json: &str) -> serde_json::Error {
        serde_json::from_str::<serde_json::Value>(json).unwrap_err()
    }

    // Feature: ai-client-incomplete-json-fix, Property 1: Truncation Error Classification
    // For any JSON parse error with "eof while parsing" or "unexpected end of input",
    // is_incomplete_json SHALL return true.
    // **Validates: Requirements 1.1, 1.2**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_truncated_string_detected(s in "[a-zA-Z0-9 ]{1,50}") {
            // Truncated string (missing closing quote)
            let json = format!(r#"{{"key": "{}"#, s);
            let err = make_truncated_json_error(&json);
            prop_assert!(
                Error::is_incomplete_json(&err),
                "Expected truncated string to be detected as incomplete: {}",
                err
            );
        }

        #[test]
        fn prop_truncated_object_detected(s in "[a-zA-Z0-9]{1,20}") {
            // Truncated object (missing closing brace)
            let json = format!(r#"{{"key": "{}""#, s);
            let err = make_truncated_json_error(&json);
            prop_assert!(
                Error::is_incomplete_json(&err),
                "Expected truncated object to be detected as incomplete: {}",
                err
            );
        }

        #[test]
        fn prop_truncated_array_detected(n in 1..100i32) {
            // Truncated array (missing closing bracket)
            let json = format!(r#"[{}, {}"#, n, n + 1);
            let err = make_truncated_json_error(&json);
            prop_assert!(
                Error::is_incomplete_json(&err),
                "Expected truncated array to be detected as incomplete: {}",
                err
            );
        }
    }

    // Feature: ai-client-incomplete-json-fix, Property 2: Non-Truncation Error Classification
    // For any JSON parse error that does NOT contain truncation indicators,
    // is_incomplete_json SHALL return false.
    // **Validates: Requirements 1.3**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_invalid_syntax_not_incomplete(s in "[a-zA-Z0-9]{1,20}") {
            // Invalid syntax (missing colon)
            let json = format!(r#"{{"key" "{}"}}"#, s);
            let err = make_truncated_json_error(&json);
            prop_assert!(
                !Error::is_incomplete_json(&err),
                "Expected invalid syntax to NOT be detected as incomplete: {}",
                err
            );
        }

        #[test]
        fn prop_type_mismatch_not_incomplete(n in 1..1000i32) {
            // Extra comma (syntax error, not truncation)
            let json = format!(r#"{{"key": {},}}"#, n);
            let err = make_truncated_json_error(&json);
            prop_assert!(
                !Error::is_incomplete_json(&err),
                "Expected trailing comma error to NOT be detected as incomplete: {}",
                err
            );
        }

        #[test]
        fn prop_invalid_token_not_incomplete(s in "[a-zA-Z]{1,10}") {
            // Unquoted string value (invalid token)
            let json = format!(r#"{{"key": {}}}"#, s);
            let err = make_truncated_json_error(&json);
            prop_assert!(
                !Error::is_incomplete_json(&err),
                "Expected invalid token to NOT be detected as incomplete: {}",
                err
            );
        }
    }
}
