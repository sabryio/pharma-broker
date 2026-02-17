//! Generic OpenAI-compatible HTTP client

use reqwest::Client as HttpClient;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::error::{Error, Result};
use crate::retry::RetryConfig;
use crate::schema::generate_schema;
use crate::types::*;

/// Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Base URL for the API (e.g., "http://localhost:12434/engines/llama.cpp/v1")
    pub base_url: String,
    /// API key (optional for local models)
    pub api_key: Option<String>,
    /// Model identifier (e.g., "ai/qwen3-vl:latest")
    pub model: String,
    /// Request timeout
    pub timeout: Duration,
    /// Retry configuration
    pub retry: RetryConfig,
    /// Default temperature
    pub temperature: f32,
    /// Default max tokens (None = unlimited/model default)
    pub max_tokens: Option<u32>,
    /// Top-p nucleus sampling (0.0-1.0)
    pub top_p: Option<f32>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:12434/engines/llama.cpp/v1".to_string(),
            api_key: None,
            model: "ai/qwen3-vl:latest".to_string(),
            timeout: Duration::from_secs(180), // Increased from 120 to 180 seconds
            retry: RetryConfig::default(),
            temperature: 0.1,
            max_tokens: None, // Unlimited by default - let the model decide
            top_p: Some(0.9),
        }
    }
}

/// Context-specific AI configuration presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AIContext {
    /// Parsing medication text (low temperature, deterministic)
    Parsing,
    /// Comparing medication names (zero temperature, exact)
    Comparison,
    /// General purpose (default settings)
    General,
}

impl AIContext {
    /// Get temperature for this context
    pub fn temperature(&self) -> f32 {
        match self {
            AIContext::Parsing => 0.2,
            AIContext::Comparison => 0.0,
            AIContext::General => 0.1,
        }
    }

    /// Get top_p for this context
    pub fn top_p(&self) -> f32 {
        match self {
            AIContext::Parsing => 0.9,
            AIContext::Comparison => 1.0, // No sampling for exact matching
            AIContext::General => 0.9,
        }
    }

    /// Get max tokens for this context
    pub fn max_tokens(&self) -> Option<u32> {
        match self {
            AIContext::Parsing => Some(2000),
            AIContext::Comparison => Some(500),
            AIContext::General => None,
        }
    }
}

impl ClientConfig {
    /// Create config from environment variables
    pub fn from_env() -> Self {
        Self {
            base_url: std::env::var("AI_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:12434/engines/llama.cpp/v1".to_string()),
            api_key: std::env::var("AI_API_KEY").ok(),
            model: std::env::var("AI_MODEL").unwrap_or_else(|_| "ai/qwen3-vl:latest".to_string()),
            timeout: Duration::from_secs(
                std::env::var("AI_TIMEOUT_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(180), // Increased default from 60 to 180 seconds
            ),
            ..Default::default()
        }
    }
}

/// Generic OpenAI-compatible AI client
pub struct Client {
    http: HttpClient,
    config: ClientConfig,
}

impl Client {
    /// Create a new client with the given configuration
    pub fn new(config: ClientConfig) -> Self {
        let http = HttpClient::builder()
            .timeout(config.timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self { http, config }
    }

    /// Create a client from environment variables
    pub fn from_env() -> Self {
        Self::new(ClientConfig::from_env())
    }

    /// Generate a structured object from the AI model
    ///
    /// Uses JSON schema to ensure the response matches the expected type.
    ///
    /// # Type Parameters
    /// - `T`: The output type, must implement `JsonSchema` and `DeserializeOwned`
    ///
    /// # Arguments
    /// - `prompt`: The user prompt to send
    ///
    /// # Example
    /// ```rust,ignore
    /// #[derive(JsonSchema, Deserialize)]
    /// struct Output { items: Vec<String> }
    ///
    /// let result: Output = client.generate_object("List 3 fruits").await?;
    /// ```
    pub async fn generate_object<T>(&self, prompt: &str) -> Result<T>
    where
        T: JsonSchema + DeserializeOwned,
    {
        self.generate_object_with_system::<T>("", prompt).await
    }

    /// Generate a structured object with a custom system prompt
    pub async fn generate_object_with_system<T>(&self, system: &str, prompt: &str) -> Result<T>
    where
        T: JsonSchema + DeserializeOwned,
    {
        self.generate_object_with_context::<T>(system, prompt, AIContext::General)
            .await
    }

    /// Generate a structured object with context-specific configuration
    ///
    /// Uses temperature and sampling parameters optimized for the given context:
    /// - `Parsing`: Low temperature (0.2) for deterministic parsing
    /// - `Comparison`: Zero temperature (0.0) for exact name matching
    /// - `General`: Default settings (0.1)
    pub async fn generate_object_with_context<T>(
        &self,
        system: &str,
        prompt: &str,
        context: AIContext,
    ) -> Result<T>
    where
        T: JsonSchema + DeserializeOwned,
    {
        let schema = generate_schema::<T>();
        let schema_name = std::any::type_name::<T>()
            .split("::")
            .last()
            .unwrap_or("Output");

        let mut messages = Vec::new();
        if !system.is_empty() {
            messages.push(ChatMessage::system(system));
        }
        messages.push(ChatMessage::user(prompt));

        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages,
            temperature: Some(context.temperature()),
            max_tokens: context.max_tokens().or(self.config.max_tokens),
            top_p: Some(context.top_p()),
            response_format: Some(ResponseFormat::JsonSchema {
                json_schema: JsonSchemaSpec {
                    name: schema_name.to_string(),
                    description: None,
                    schema,
                    strict: Some(true),
                },
            }),
        };

        self.execute_with_retry(request).await
    }

    /// Generate plain text from the AI model (no structured output)
    pub async fn generate_text(&self, prompt: &str) -> Result<String> {
        self.generate_text_with_system("", prompt).await
    }

    /// Generate plain text with a custom system prompt
    pub async fn generate_text_with_system(&self, system: &str, prompt: &str) -> Result<String> {
        let mut messages = Vec::new();
        if !system.is_empty() {
            messages.push(ChatMessage::system(system));
        }
        messages.push(ChatMessage::user(prompt));

        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages,
            temperature: Some(self.config.temperature),
            max_tokens: self.config.max_tokens,
            top_p: self.config.top_p,
            response_format: None,
        };

        let response = self.execute_request(&request).await?;
        response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or(Error::EmptyResponse)
    }

    /// Execute request with retry logic
    async fn execute_with_retry<T>(&self, request: ChatCompletionRequest) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let mut last_error = None;

        for attempt in 1..=self.config.retry.max_attempts {
            match self.execute_request(&request).await {
                Ok(response) => {
                    let content = response
                        .choices
                        .first()
                        .and_then(|c| c.message.content.as_ref())
                        .ok_or(Error::EmptyResponse)?;

                    // First try strict JSON parsing
                    match serde_json::from_str::<T>(content) {
                        Ok(parsed) => {
                            info!(attempt, "Structured output parsed successfully");
                            return Ok(parsed);
                        }
                        Err(e) => {
                            // Check if this is incomplete JSON (retryable after repair attempt)
                            if Error::is_incomplete_json(&e) {
                                warn!(
                                    attempt,
                                    error = %e,
                                    content_len = content.len(),
                                    content_preview = &content[..content.len().min(200)],
                                    "Incomplete JSON response from model, attempting repair"
                                );

                                // Try to repair the truncated JSON using agentjson
                                let repair_opts = json_prob_parser::types::RepairOptions::default();
                                let repair_result = json_prob_parser::parse(content, &repair_opts);

                                if let Some(repaired_json) = repair_result
                                    .best()
                                    .and_then(|b| b.normalized_json.as_ref())
                                {
                                    debug!(
                                        original_len = content.len(),
                                        repaired_len = repaired_json.len(),
                                        repair_status = %repair_result.status,
                                        "JSON repair attempted"
                                    );

                                    // Try parsing the repaired JSON
                                    match serde_json::from_str::<T>(repaired_json) {
                                        Ok(parsed) => {
                                            info!(
                                                attempt,
                                                repair_status = %repair_result.status,
                                                "Structured output parsed after JSON repair"
                                            );
                                            return Ok(parsed);
                                        }
                                        Err(repair_parse_err) => {
                                            warn!(
                                                attempt,
                                                error = %repair_parse_err,
                                                repair_status = %repair_result.status,
                                                "JSON repair did not produce valid output, will retry"
                                            );
                                        }
                                    }
                                } else {
                                    warn!(
                                        attempt,
                                        repair_status = %repair_result.status,
                                        "JSON repair produced no output, will retry"
                                    );
                                }

                                last_error = Some(Error::IncompleteJson(e.to_string()));
                            } else {
                                warn!(
                                    attempt,
                                    error = %e,
                                    content_preview = &content[..content.len().min(200)],
                                    "Failed to parse structured output (non-retryable)"
                                );
                                // Non-incomplete parse errors are not retryable
                                return Err(Error::Parse(e));
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(attempt, error = %e, "Request failed");
                    if !e.is_retryable() {
                        return Err(e);
                    }
                    last_error = Some(e);
                }
            }

            if attempt < self.config.retry.max_attempts {
                let delay = self.config.retry.delay_for_attempt(attempt);
                debug!(
                    attempt,
                    delay_ms = delay.as_millis(),
                    "Retrying after delay"
                );
                tokio::time::sleep(delay).await;
            }
        }

        Err(Error::RetryExhausted {
            attempts: self.config.retry.max_attempts,
            last_error: last_error.map(|e| e.to_string()).unwrap_or_default(),
        })
    }

    /// Execute a single HTTP request
    async fn execute_request(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        let base = self.config.base_url.trim_end_matches('/');
        let url = format!("{}/chat/completions", base);

        debug!(
            url = %url,
            model = %request.model,
            messages = request.messages.len(),
            "Sending request to AI API"
        );

        let mut http_request = self.http.post(&url).json(request);

        if let Some(ref api_key) = self.config.api_key {
            http_request = http_request.bearer_auth(api_key);
        }

        let response = http_request.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();

            // Check for context exceeded error and return specific error type
            if status == 500 && Error::is_context_exceeded(&message) {
                warn!(status, "Context size exceeded - input too large for model");
                return Err(Error::ContextExceeded(message));
            }

            return Err(Error::Api { status, message });
        }

        let completion: ChatCompletionResponse = response.json().await?;

        if let Some(ref usage) = completion.usage {
            debug!(
                prompt_tokens = usage.prompt_tokens,
                completion_tokens = usage.completion_tokens,
                total_tokens = usage.total_tokens,
                "Token usage"
            );
        }

        Ok(completion)
    }

    // =========================================================================
    // Embedding Methods
    // =========================================================================

    /// Generate an embedding for a single text
    ///
    /// # Arguments
    /// - `text`: The text to embed
    /// - `model`: Optional model override (defaults to embedding model from env)
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.generate_embeddings(&[text.to_string()]).await?;
        embeddings.into_iter().next().ok_or(Error::EmptyResponse)
    }

    /// Generate embeddings for multiple texts
    ///
    /// # Arguments
    /// - `texts`: The texts to embed
    pub async fn generate_embeddings(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let model = std::env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "ai/embeddinggemma:latest".to_string());

        let request = EmbeddingRequest {
            model,
            input: if texts.len() == 1 {
                EmbeddingInput::Single(texts[0].clone())
            } else {
                EmbeddingInput::Batch(texts.to_vec())
            },
        };

        let url = format!("{}/embeddings", self.config.base_url);

        debug!(url = %url, texts = texts.len(), "Sending embedding request");

        let mut http_request = self.http.post(&url).json(&request);

        if let Some(ref api_key) = self.config.api_key {
            http_request = http_request.bearer_auth(api_key);
        }

        let response = http_request.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(Error::Api { status, message });
        }

        let embedding_response: EmbeddingResponse = response.json().await?;

        // Sort by index and extract embeddings
        let mut data = embedding_response.data;
        data.sort_by_key(|d| d.index);

        Ok(data.into_iter().map(|d| d.embedding).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = ClientConfig::default();
        assert!(config.base_url.contains("localhost"));
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_incomplete_json_is_retryable() {
        let err = Error::IncompleteJson("EOF while parsing".to_string());
        assert!(err.is_retryable(), "IncompleteJson should be retryable");
    }

    #[test]
    fn test_parse_error_is_not_retryable() {
        // Create a real parse error from invalid JSON
        let parse_err =
            serde_json::from_str::<serde_json::Value>(r#"{"key": invalid}"#).unwrap_err();
        let err = Error::Parse(parse_err);
        assert!(!err.is_retryable(), "Parse error should not be retryable");
    }
}
