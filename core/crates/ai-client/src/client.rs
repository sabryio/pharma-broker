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
    /// Default max tokens
    pub max_tokens: u32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:12434/engines/llama.cpp/v1".to_string(),
            api_key: None,
            model: "ai/qwen3-vl:latest".to_string(),
            timeout: Duration::from_secs(60),
            retry: RetryConfig::default(),
            temperature: 0.1,
            max_tokens: 1000,
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
                    .unwrap_or(60),
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
            temperature: Some(self.config.temperature),
            max_tokens: Some(self.config.max_tokens),
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
            max_tokens: Some(self.config.max_tokens),
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

                    match serde_json::from_str::<T>(content) {
                        Ok(parsed) => {
                            info!(attempt, "Structured output parsed successfully");
                            return Ok(parsed);
                        }
                        Err(e) => {
                            warn!(attempt, error = %e, "Failed to parse structured output");
                            last_error = Some(Error::Parse(e));
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
        let url = format!("{}/chat/completions", self.config.base_url);

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
}
