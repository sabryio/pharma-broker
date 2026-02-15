//! Pharma AI Parser
//!
//! High-level wrapper around ai-client for pharma-specific parsing,
//! with circuit breaker, retry support, context size management,
//! and LLM feedback loop for continuous improvement.

use std::sync::Arc;

use ai_client::{AIContext, Client, ClientConfig, Error as ClientError};
use futures::future::join_all;
use tracing::{debug, error, info, warn};

use super::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use super::feedback_loop::{
    AIExtraction, CorrectExtraction, ExtractionFeedback, FeedbackLoopConfig, FeedbackLoopStats,
    FeedbackType, FewShotExample, LLMFeedbackLoop, MedicationCorrection,
};
use super::pharma_prompts::{SYSTEM_PROMPT, build_user_prompt_with_mappings};
use super::pharma_types::ParseResult;
use super::token_batcher::{BatchMessage, TokenBatchConfig, TokenBatcher};

// =============================================================================
// Context Size Constants
// =============================================================================

/// Default maximum context size for the model (intelligent default)
/// Most local LLMs have 4K-8K context windows. We use 6K as a safe default
/// that works with most models while leaving room for response.
const DEFAULT_MAX_CONTEXT_TOKENS: usize = 6000;

/// Reserved tokens for AI response output
/// Reserve 2000 tokens for the model's JSON response
const RESPONSE_RESERVED_TOKENS: usize = 2000;

/// Maximum lines per message chunk (matching legacy DefaultMaxMessageLines)
const MAX_MESSAGE_LINES: usize = 20;

/// Safety margin for token estimation (10% buffer for estimation errors)
const TOKEN_ESTIMATION_SAFETY_MARGIN: f64 = 0.10;

/// Maximum recursion depth for chunking to prevent infinite loops
const MAX_CHUNK_RECURSION_DEPTH: usize = 5;

/// Error prefix for permanent context failures (used for backoff detection)
pub const CONTEXT_EXCEEDED_PERMANENT_PREFIX: &str = "[PERMANENT] Context size exceeded";

/// Get maximum context tokens from environment or intelligent default
/// Set AI_MAX_CONTEXT_TOKENS env var to override, or leave unset for auto-detection
fn get_max_context_tokens() -> usize {
    std::env::var("AI_MAX_CONTEXT_TOKENS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_MAX_CONTEXT_TOKENS)
}

/// Calculate available tokens for content after accounting for overhead
fn calculate_available_tokens(overhead_tokens: usize) -> usize {
    let max_context = get_max_context_tokens();
    let safety_buffer = (max_context as f64 * TOKEN_ESTIMATION_SAFETY_MARGIN) as usize;

    max_context
        .saturating_sub(overhead_tokens)
        .saturating_sub(RESPONSE_RESERVED_TOKENS)
        .saturating_sub(safety_buffer)
        .max(512) // Minimum viable content size
}

/// Cached system prompt token count (computed once, reused)
fn get_cached_system_prompt_tokens() -> usize {
    static CACHED_TOKENS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *CACHED_TOKENS.get_or_init(|| {
        // Use the actual estimate_tokens function logic
        static BPE: std::sync::OnceLock<tiktoken_rs::CoreBPE> = std::sync::OnceLock::new();
        let bpe = BPE.get_or_init(|| tiktoken_rs::o200k_base().unwrap());
        bpe.encode_with_special_tokens(SYSTEM_PROMPT).len()
    })
}

/// Configuration for the PharmaParser
#[derive(Clone, Default, Debug)]
pub struct PharmaParserConfig {
    /// AI client configuration
    pub client: ClientConfig,
    /// Circuit breaker configuration
    pub circuit_breaker: CircuitBreakerConfig,
    /// Token batch configuration
    pub token_batch: TokenBatchConfig,
    /// Feedback loop configuration
    pub feedback_loop: FeedbackLoopConfig,
}

impl PharmaParserConfig {
    /// Create config from environment variables
    pub fn from_env() -> Self {
        Self {
            client: ClientConfig::from_env(),
            circuit_breaker: CircuitBreakerConfig::from_env(),
            token_batch: TokenBatchConfig::default(),
            feedback_loop: FeedbackLoopConfig::default(),
        }
    }
}

/// Error type for PharmaParser
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Circuit breaker open")]
    CircuitOpen,
    #[error("AI client error: {0}")]
    Client(#[from] ClientError),
    #[error("Parse error: {0}")]
    Parse(String),
}

impl ParseError {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            ParseError::CircuitOpen => false,
            ParseError::Client(e) => e.is_retryable(),
            ParseError::Parse(msg) => {
                // Permanent context failures are not retryable
                !msg.starts_with(CONTEXT_EXCEEDED_PERMANENT_PREFIX)
            }
        }
    }

    /// Check if this is a permanent failure that should not be retried
    pub fn is_permanent_failure(&self) -> bool {
        match self {
            ParseError::Parse(msg) => msg.starts_with(CONTEXT_EXCEEDED_PERMANENT_PREFIX),
            ParseError::Client(e) => e.is_context_error(),
            _ => false,
        }
    }
}

/// Pharma AI parser with circuit breaker, batch support, and feedback loop
pub struct PharmaParser {
    client: Client,
    circuit_breaker: Arc<CircuitBreaker>,
    token_batcher: TokenBatcher,
    feedback_loop: Arc<LLMFeedbackLoop>,
}

impl PharmaParser {
    /// Create a new parser with the given configuration
    pub fn new(config: PharmaParserConfig) -> Self {
        Self {
            client: Client::new(config.client),
            circuit_breaker: Arc::new(CircuitBreaker::new(config.circuit_breaker)),
            token_batcher: TokenBatcher::new(config.token_batch),
            feedback_loop: Arc::new(LLMFeedbackLoop::new(config.feedback_loop)),
        }
    }

    /// Create a parser from environment variables
    pub fn from_env() -> Self {
        Self::new(PharmaParserConfig::from_env())
    }

    /// Parse a single message with optional medication mappings
    /// Automatically enhances prompts with learned examples from feedback loop
    /// and pre-processes mixed offer/request messages for better accuracy
    pub async fn parse(
        &self,
        content: &str,
        sender_name: Option<&str>,
        group_name: &str,
        reply_to: Option<&str>,
        mappings: Option<&[String]>,
    ) -> Result<ParseResult, ParseError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_request() {
            warn!("Circuit breaker open, rejecting request");
            return Err(ParseError::CircuitOpen);
        }

        // Pre-process content for mixed intent messages
        let preprocessor = super::preprocessor::MessagePreprocessor::instance();
        let (processed_content, has_mixed_intents) = if preprocessor.has_mixed_intents(content) {
            debug!("Detected mixed offer/request message, adding intent hints");
            (preprocessor.preprocess(content), true)
        } else {
            (content.to_string(), false)
        };

        // Enhance system prompt with learned examples from feedback loop
        let enhanced_system_prompt = self
            .feedback_loop
            .build_enhanced_prompt(SYSTEM_PROMPT, content);

        // Calculate overhead tokens (system prompt + user prompt shell + mappings)
        let prompt_shell =
            build_user_prompt_with_mappings("", sender_name, group_name, reply_to, mappings);
        let overhead =
            Self::estimate_tokens(&enhanced_system_prompt) + Self::estimate_tokens(&prompt_shell);

        let available = calculate_available_tokens(overhead);

        // Check if content needs to be split due to context limits
        let content_tokens = Self::estimate_tokens(&processed_content);

        if content_tokens > available {
            // Content too large - split into chunks and merge results
            let preview: String = content.chars().take(100).collect();
            let preview = if content.chars().count() > 100 {
                format!("{}...", preview)
            } else {
                preview
            };

            warn!(
                content_tokens = content_tokens,
                available = available,
                overhead = overhead,
                max_context = get_max_context_tokens(),
                content = %preview,
                "Message exceeds dynamic context limit, splitting into chunks"
            );
            return self
                .parse_chunked_with_depth(content, sender_name, group_name, reply_to, mappings, 0)
                .await;
        }

        // Build user prompt with mappings (using processed content with intent hints)
        let user_prompt = build_user_prompt_with_mappings(
            &processed_content,
            sender_name,
            group_name,
            reply_to,
            mappings,
        );

        // Get learned medication mappings from feedback loop
        let learned_mappings = self.feedback_loop.get_medication_mappings();
        let all_mappings = if !learned_mappings.is_empty() {
            let mut combined = mappings.map(|m| m.to_vec()).unwrap_or_default();
            combined.extend(learned_mappings);
            Some(combined)
        } else {
            mappings.map(|m| m.to_vec())
        };

        // Rebuild user prompt with combined mappings if we have learned ones
        let final_user_prompt = if all_mappings.is_some() {
            build_user_prompt_with_mappings(
                &processed_content,
                sender_name,
                group_name,
                reply_to,
                all_mappings.as_deref(),
            )
        } else {
            user_prompt
        };

        debug!(
            learned_examples = self.feedback_loop.example_count(),
            learned_corrections = self.feedback_loop.correction_count(),
            has_mixed_intents = has_mixed_intents,
            "Parsing with feedback loop enhancements"
        );

        let result: Result<ParseResult, ClientError> = self
            .client
            .generate_object_with_context(
                &enhanced_system_prompt,
                &final_user_prompt,
                AIContext::Parsing,
            )
            .await;

        match result {
            Ok(parse_result) => {
                self.circuit_breaker.record_success();
                let items_count = parse_result.medications.len();
                if items_count > 0 {
                    info!(
                        items = items_count,
                        mixed_intents = has_mixed_intents,
                        "AI parsing complete"
                    );
                } else {
                    debug!("AI parsing complete (no items found)");
                }
                Ok(parse_result)
            }
            Err(e) => {
                // Handle context exceeded error with recursive split (Half-Chunk Retry)
                if let ClientError::Api {
                    status,
                    ref message,
                } = e
                    && status == 500
                    && (message.contains("Context size has been exceeded")
                        || message.contains("context_length_exceeded"))
                {
                    warn!("Context exceeded during single parse, attempting half-chunk split");
                    return self
                        .parse_chunked_with_depth(
                            content,
                            sender_name,
                            group_name,
                            reply_to,
                            mappings,
                            0,
                        )
                        .await;
                }

                self.circuit_breaker.record_failure();
                error!(error = %e, "AI parsing failed");
                Err(ParseError::Client(e))
            }
        }
    }

    /// Parse a large message by splitting into chunks
    /// Uses enhanced prompts from feedback loop for each chunk
    /// Tracks recursion depth to prevent infinite loops and mark permanent failures
    async fn parse_chunked_with_depth(
        &self,
        content: &str,
        sender_name: Option<&str>,
        group_name: &str,
        reply_to: Option<&str>,
        mappings: Option<&[String]>,
        depth: usize,
    ) -> Result<ParseResult, ParseError> {
        // Check recursion depth to prevent infinite loops
        if depth >= MAX_CHUNK_RECURSION_DEPTH {
            error!(
                depth = depth,
                max_depth = MAX_CHUNK_RECURSION_DEPTH,
                content_len = content.len(),
                "Maximum chunk recursion depth exceeded - marking as permanent failure"
            );
            return Err(ParseError::Parse(format!(
                "{}: Maximum chunking depth ({}) exceeded. Content may be too complex or model context too small.",
                CONTEXT_EXCEEDED_PERMANENT_PREFIX, MAX_CHUNK_RECURSION_DEPTH
            )));
        }

        let chunks = Self::split_content_into_chunks_smart(content, depth);
        info!(
            chunks = chunks.len(),
            depth = depth,
            "Split large message into {} chunks (depth {})",
            chunks.len(),
            depth
        );

        // Get learned medication mappings from feedback loop (once for all chunks)
        let learned_mappings = self.feedback_loop.get_medication_mappings();
        let all_mappings: Option<Vec<String>> = if !learned_mappings.is_empty() {
            let mut combined = mappings.map(|m| m.to_vec()).unwrap_or_default();
            combined.extend(learned_mappings);
            Some(combined)
        } else {
            mappings.map(|m| m.to_vec())
        };

        // Build enhanced system prompt once (shared across all chunks)
        // Use cached base tokens + estimate only the enhancement delta for performance
        let enhanced_system_prompt = self
            .feedback_loop
            .build_enhanced_prompt(SYSTEM_PROMPT, content);

        // Fast overhead estimation: cached base + delta for enhancements
        let base_tokens = get_cached_system_prompt_tokens();
        let enhancement_overhead = if enhanced_system_prompt.len() > SYSTEM_PROMPT.len() {
            Self::estimate_tokens(&enhanced_system_prompt[SYSTEM_PROMPT.len()..])
        } else {
            0
        };
        tracing::debug!(
            base_tokens = base_tokens,
            enhancement_overhead = enhancement_overhead,
            "System prompt token estimate (cached)"
        );

        // Create futures for parallel chunk processing
        let chunk_futures: Vec<_> = chunks
            .iter()
            .enumerate()
            .map(|(idx, chunk)| {
                let client = &self.client;
                let all_mappings_ref = all_mappings.as_deref();
                let enhanced_system_prompt_ref = &enhanced_system_prompt;
                let chunk_owned = chunk.clone(); // Clone to owned String before async

                async move {
                    let user_prompt = build_user_prompt_with_mappings(
                        &chunk_owned,
                        sender_name,
                        group_name,
                        reply_to,
                        all_mappings_ref,
                    );

                    let result: Result<ParseResult, ClientError> = client
                        .generate_object_with_context(
                            enhanced_system_prompt_ref,
                            &user_prompt,
                            AIContext::Parsing,
                        )
                        .await;

                    (idx, chunk_owned, result)
                }
            })
            .collect();

        // Execute all chunks in parallel
        let results = join_all(chunk_futures).await;

        let mut all_medications = Vec::new();
        let mut first_intent = None;
        let mut first_urgency = None;
        let mut first_reason = None;
        let mut errors = Vec::new();
        let mut permanent_failure = false;

        for (idx, chunk, result) in results {
            match result {
                Ok(parse_result) => {
                    self.circuit_breaker.record_success();

                    // Capture first chunk's metadata
                    if first_intent.is_none() {
                        first_intent = Some(parse_result.intent);
                        first_urgency = Some(parse_result.urgency);
                        first_reason = Some(parse_result.reason.clone());
                    }

                    let items_count = parse_result.medications.len();
                    if items_count > 0 {
                        info!(
                            chunk = idx + 1,
                            items = items_count,
                            depth = depth,
                            "Chunk parsed successfully (parallel)"
                        );
                    } else {
                        debug!(
                            chunk = idx + 1,
                            depth = depth,
                            "Chunk parsed successfully (no items)"
                        );
                    }
                    all_medications.extend(parse_result.medications);
                }
                Err(e) => {
                    // Check for context error (either direct API error or wrapped in RetryExhausted)
                    let is_context_error = match &e {
                        ClientError::Api { status, message } => {
                            *status == 500
                                && (message.contains("Context size has been exceeded")
                                    || message.contains("context_length_exceeded"))
                        }
                        ClientError::RetryExhausted { last_error, .. } => {
                            last_error.contains("Context size has been exceeded")
                                || last_error.contains("context_length_exceeded")
                        }
                        ClientError::ContextExceeded(_) => true,
                        _ => false,
                    };

                    if is_context_error {
                        warn!(
                            chunk = idx + 1,
                            depth = depth,
                            next_depth = depth + 1,
                            "Chunk still too large, attempting sub-split"
                        );

                        // Sub-split this specific chunk with increased depth
                        match Box::pin(self.parse_chunked_with_depth(
                            &chunk,
                            sender_name,
                            group_name,
                            reply_to,
                            mappings,
                            depth + 1,
                        ))
                        .await
                        {
                            Ok(sub_result) => {
                                all_medications.extend(sub_result.medications);
                                continue;
                            }
                            Err(ParseError::Parse(ref msg))
                                if msg.starts_with(CONTEXT_EXCEEDED_PERMANENT_PREFIX) =>
                            {
                                // Permanent failure from sub-chunk - propagate up
                                error!(
                                    chunk = idx + 1,
                                    depth = depth,
                                    "Sub-chunk hit permanent failure limit"
                                );
                                permanent_failure = true;
                                errors.push(msg.clone());
                            }
                            Err(sub_err) => {
                                errors.push(format!(
                                    "Chunk {} sub-split failed: {}",
                                    idx + 1,
                                    sub_err
                                ));
                            }
                        }
                    } else {
                        self.circuit_breaker.record_failure();
                        error!(chunk = idx + 1, depth = depth, error = %e, "Chunk parsing failed");
                        errors.push(format!("Chunk {}: {}", idx + 1, e));
                    }
                }
            }
        }

        // If we hit a permanent failure, propagate it
        if permanent_failure && all_medications.is_empty() {
            return Err(ParseError::Parse(format!(
                "{}: All chunks failed after maximum recursion. Errors: {}",
                CONTEXT_EXCEEDED_PERMANENT_PREFIX,
                errors.join("; ")
            )));
        }

        // If all chunks failed (but not permanent), return error
        if all_medications.is_empty() && !errors.is_empty() {
            return Err(ParseError::Parse(errors.join("; ")));
        }

        info!(
            total_items = all_medications.len(),
            chunks = chunks.len(),
            depth = depth,
            "Merged results from all chunks"
        );

        // Build merged ParseResult
        Ok(ParseResult {
            intent: first_intent.unwrap_or(crate::ai::Intent::Offer),
            urgency: first_urgency.unwrap_or(crate::ai::UrgencyLevel::Normal),
            reason: first_reason.unwrap_or_else(|| "Merged from chunks".to_string()),
            medications: all_medications,
        })
    }

    /// Legacy wrapper for backward compatibility
    #[allow(dead_code)]
    async fn parse_chunked(
        &self,
        content: &str,
        sender_name: Option<&str>,
        group_name: &str,
        reply_to: Option<&str>,
        mappings: Option<&[String]>,
    ) -> Result<ParseResult, ParseError> {
        self.parse_chunked_with_depth(content, sender_name, group_name, reply_to, mappings, 0)
            .await
    }

    /// Estimate token count for text using o200k_base
    fn estimate_tokens(text: &str) -> usize {
        static BPE: std::sync::OnceLock<tiktoken_rs::CoreBPE> = std::sync::OnceLock::new();
        let bpe = BPE.get_or_init(|| tiktoken_rs::o200k_base().unwrap());
        bpe.encode_with_special_tokens(text).len()
    }

    /// Split content into chunks that fit within context limits
    /// Uses depth to progressively reduce chunk sizes on retry
    fn split_content_into_chunks_smart(content: &str, depth: usize) -> Vec<String> {
        // Reduce max lines per chunk as depth increases (more aggressive splitting)
        let max_lines = MAX_MESSAGE_LINES.saturating_sub(depth * 5).max(3);

        let lines: Vec<&str> = content.lines().collect();

        // If few lines, split by token count instead
        if lines.len() <= max_lines {
            return Self::split_by_tokens_smart(content, depth);
        }

        // Split by lines with depth-adjusted chunk size
        let mut chunks = Vec::new();
        for chunk_lines in lines.chunks(max_lines) {
            chunks.push(chunk_lines.join("\n"));
        }
        chunks
    }

    /// Split content into chunks that fit within context limits (legacy)
    #[allow(dead_code)]
    fn split_content_into_chunks(content: &str) -> Vec<String> {
        Self::split_content_into_chunks_smart(content, 0)
    }

    /// Split content by estimated token count with depth-aware sizing
    fn split_by_tokens_smart(content: &str, depth: usize) -> Vec<String> {
        // Calculate available tokens based on current overhead
        let system_tokens = get_cached_system_prompt_tokens();
        let overhead = system_tokens + 500; // base + extra slack for user prompt
        let base_available = calculate_available_tokens(overhead);

        // Reduce available tokens as depth increases (more conservative)
        let depth_factor = 1.0 / (1.0 + depth as f64 * 0.5); // 1.0, 0.67, 0.5, 0.4, ...
        let available = ((base_available as f64) * depth_factor) as usize;
        let available = available.max(256); // Minimum viable chunk

        // Estimate chars per token (conservative: ~3 chars per token for mixed content)
        let chars_per_token = 3;
        let max_chars = available * chars_per_token;

        debug!(
            depth = depth,
            base_available = base_available,
            adjusted_available = available,
            max_chars = max_chars,
            content_len = content.len(),
            "Calculating chunk size for token-based split"
        );

        if content.len() <= max_chars {
            return vec![content.to_string()];
        }

        let mut chunks = Vec::new();
        let mut start = 0;

        while start < content.len() {
            let end = (start + max_chars).min(content.len());

            // Try to break at a newline or space for cleaner splits
            let actual_end = if end < content.len() {
                content[start..end]
                    .rfind('\n')
                    .or_else(|| content[start..end].rfind(' '))
                    .or_else(|| content[start..end].rfind('،')) // Arabic comma
                    .or_else(|| content[start..end].rfind('،')) // Arabic semicolon
                    .map(|pos| start + pos + 1)
                    .unwrap_or(end)
            } else {
                end
            };

            let chunk = content[start..actual_end].trim();
            if !chunk.is_empty() {
                chunks.push(chunk.to_string());
            }
            start = actual_end;
        }

        // Ensure we have at least one chunk
        if chunks.is_empty() && !content.is_empty() {
            chunks.push(content.to_string());
        }

        chunks
    }

    /// Split content by estimated token count (legacy)
    #[allow(dead_code)]
    fn split_by_tokens(content: &str) -> Vec<String> {
        Self::split_by_tokens_smart(content, 0)
    }

    /// Parse a batch of messages using token-aware batching
    pub async fn parse_batch(&self, messages: Vec<BatchMessage>) -> Vec<BatchParseResult> {
        // Split into token-aware batches
        let batches = self.token_batcher.split_into_batches(messages);
        let mut results = Vec::new();

        for batch in batches {
            for msg in batch {
                let result = self
                    .parse(&msg.content, None, &msg.group_name, None, None)
                    .await;
                results.push(BatchParseResult {
                    message_id: msg.id,
                    result,
                });
            }
        }

        results
    }

    /// Generate an embedding for text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, ParseError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_request() {
            warn!("Circuit breaker open, rejecting embed request");
            return Err(ParseError::CircuitOpen);
        }

        match self.client.generate_embedding(text).await {
            Ok(embedding) => {
                self.circuit_breaker.record_success();
                Ok(embedding)
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                error!(error = %e, "Embedding generation failed");
                Err(ParseError::Client(e))
            }
        }
    }

    /// Generate embeddings for multiple texts
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ParseError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_request() {
            warn!("Circuit breaker open, rejecting embed request");
            return Err(ParseError::CircuitOpen);
        }

        match self.client.generate_embeddings(texts).await {
            Ok(embeddings) => {
                self.circuit_breaker.record_success();
                Ok(embeddings)
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                error!(error = %e, "Embedding generation failed");
                Err(ParseError::Client(e))
            }
        }
    }

    /// Get circuit breaker state
    pub fn circuit_state(&self) -> super::circuit_breaker::CircuitState {
        self.circuit_breaker.state()
    }

    /// Get token batcher statistics
    pub fn batcher_stats(&self) -> super::token_batcher::TokenBatchStatsSnapshot {
        self.token_batcher.stats()
    }

    // =========================================================================
    // Feedback Loop Methods
    // =========================================================================

    /// Record extraction feedback for continuous improvement
    ///
    /// Call this when an operator confirms or corrects an AI extraction.
    /// The feedback loop will learn from this to improve future extractions.
    pub fn record_extraction_feedback(&self, feedback: ExtractionFeedback) {
        self.feedback_loop.record_feedback(feedback);
    }

    /// Record a correct extraction using structured params
    pub fn record_correct_extraction(&self, data: super::params::ExtractionData) {
        let feedback = ExtractionFeedback {
            message_id: data.message_id,
            message_content: data.message_content,
            ai_extraction: AIExtraction {
                medication: Some(data.medication.clone()),
                item_type: Some(data.item_type.clone()),
                quantity: Some(data.quantity),
                price: Some(data.price),
                confidence: data.ai_confidence,
            },
            correct_extraction: CorrectExtraction {
                medication: data.medication,
                item_type: data.item_type,
                quantity: data.quantity,
                price: data.price,
            },
            feedback_type: FeedbackType::Correct,
            feedback_source: "operator".to_string(),
            created_at: chrono::Utc::now(),
        };
        self.feedback_loop.record_feedback(feedback);
    }

    /// Record a medication correction (AI got the medication name wrong)
    pub fn record_medication_correction(&self, data: super::params::MedicationCorrectionData) {
        let feedback = ExtractionFeedback {
            message_id: data.message_id,
            message_content: data.message_content,
            ai_extraction: AIExtraction {
                medication: Some(data.ai_medication),
                item_type: Some(data.item_type.clone()),
                quantity: Some(data.quantity),
                price: Some(data.price),
                confidence: data.ai_confidence,
            },
            correct_extraction: CorrectExtraction {
                medication: data.correct_medication,
                item_type: data.item_type,
                quantity: data.quantity,
                price: data.price,
            },
            feedback_type: FeedbackType::WrongMedication,
            feedback_source: "operator".to_string(),
            created_at: chrono::Utc::now(),
        };
        self.feedback_loop.record_feedback(feedback);
    }

    /// Record a missed extraction (AI didn't find an item that exists)
    pub fn record_missed_extraction(&self, data: super::params::MissedExtractionData) {
        let feedback = ExtractionFeedback {
            message_id: data.message_id,
            message_content: data.message_content,
            ai_extraction: AIExtraction {
                medication: None,
                item_type: None,
                quantity: None,
                price: None,
                confidence: 0.0,
            },
            correct_extraction: CorrectExtraction {
                medication: data.medication,
                item_type: data.item_type,
                quantity: data.quantity,
                price: data.price,
            },
            feedback_type: FeedbackType::Missed,
            feedback_source: "operator".to_string(),
            created_at: chrono::Utc::now(),
        };
        self.feedback_loop.record_feedback(feedback);
    }

    /// Record a false positive (AI extracted something that wasn't a medication)
    pub fn record_false_positive(
        &self,
        message_id: &str,
        message_content: &str,
        ai_medication: &str,
        ai_confidence: f64,
    ) {
        let feedback = ExtractionFeedback {
            message_id: message_id.to_string(),
            message_content: message_content.to_string(),
            ai_extraction: AIExtraction {
                medication: Some(ai_medication.to_string()),
                item_type: Some("OFFER".to_string()),
                quantity: Some(1.0),
                price: None,
                confidence: ai_confidence,
            },
            correct_extraction: CorrectExtraction {
                medication: String::new(),
                item_type: String::new(),
                quantity: 0.0,
                price: 0.0,
            },
            feedback_type: FeedbackType::FalsePositive,
            feedback_source: "operator".to_string(),
            created_at: chrono::Utc::now(),
        };
        self.feedback_loop.record_feedback(feedback);
    }

    /// Get feedback loop statistics
    pub fn feedback_stats(&self) -> FeedbackLoopStats {
        self.feedback_loop.get_stats()
    }

    /// Get feedback loop configuration
    pub fn feedback_config(&self) -> FeedbackLoopConfig {
        self.feedback_loop.get_config()
    }

    /// Update feedback loop configuration
    pub fn set_feedback_config(&self, config: FeedbackLoopConfig) {
        self.feedback_loop.set_config(config);
    }

    /// Enable or disable feedback loop
    pub fn enable_feedback_loop(&self, enabled: bool) {
        self.feedback_loop.enable(enabled);
    }

    /// Check if feedback loop is enabled
    pub fn is_feedback_enabled(&self) -> bool {
        self.feedback_loop.is_enabled()
    }

    /// Get number of learned examples
    pub fn learned_example_count(&self) -> usize {
        self.feedback_loop.example_count()
    }

    /// Get number of learned medication corrections
    pub fn learned_correction_count(&self) -> usize {
        self.feedback_loop.correction_count()
    }

    /// Export learned examples for persistence
    pub fn export_learned_examples(&self) -> Vec<FewShotExample> {
        self.feedback_loop.export_examples()
    }

    /// Import learned examples from external source
    pub fn import_learned_examples(&self, examples: Vec<FewShotExample>) {
        self.feedback_loop.import_examples(examples);
    }

    /// Export medication corrections for persistence
    pub fn export_medication_corrections(&self) -> Vec<MedicationCorrection> {
        self.feedback_loop.export_corrections()
    }

    /// Import medication corrections from external source
    pub fn import_medication_corrections(&self, corrections: Vec<MedicationCorrection>) {
        self.feedback_loop.import_corrections(corrections);
    }

    /// Clear all learned data (examples and corrections)
    pub fn clear_learned_data(&self) {
        self.feedback_loop.clear();
    }
}

/// Result for a single message in a batch
pub struct BatchParseResult {
    pub message_id: String,
    pub result: Result<ParseResult, ParseError>,
}

#[cfg(test)]
mod tests {
    use super::super::params::{ExtractionData, MedicationCorrectionData, MissedExtractionData};
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_parser_creation() {
        let parser = PharmaParser::new(PharmaParserConfig::default());
        assert!(parser.is_feedback_enabled());
        assert_eq!(parser.learned_example_count(), 0);
        assert_eq!(parser.learned_correction_count(), 0);
    }

    #[test]
    fn test_parser_from_env() {
        let parser = PharmaParser::from_env();
        assert!(parser.is_feedback_enabled());
    }

    // =========================================================================
    // Feedback Recording Tests
    // =========================================================================

    #[test]
    fn test_record_correct_extraction() {
        let parser = PharmaParser::new(PharmaParserConfig::default());

        parser.record_correct_extraction(ExtractionData {
            message_id: "msg-1".to_string(),
            message_content: "متوفر اوجمنتين 1 جم".to_string(),
            medication: "Augmentin 1g".to_string(),
            item_type: "OFFER".to_string(),
            quantity: 1.0,
            price: 300.0,
            ai_confidence: 0.95,
        });

        let stats = parser.feedback_stats();
        assert_eq!(stats.total_feedback_received, 1);
        assert_eq!(stats.correct_extractions, 1);
    }

    #[test]
    fn test_record_medication_correction() {
        let parser = PharmaParser::new(PharmaParserConfig::default());

        parser.record_medication_correction(MedicationCorrectionData {
            message_id: "msg-1".to_string(),
            message_content: "متوفر بروفين".to_string(),
            ai_medication: "Brofen".to_string(), // AI got it wrong
            correct_medication: "Brufen".to_string(), // Correct name
            item_type: "OFFER".to_string(),
            quantity: 1.0,
            price: 100.0,
            ai_confidence: 0.8,
        });

        let stats = parser.feedback_stats();
        assert_eq!(stats.wrong_extractions, 1);
        assert!(parser.learned_correction_count() > 0);
    }

    #[test]
    fn test_record_missed_extraction() {
        let parser = PharmaParser::new(PharmaParserConfig::default());

        parser.record_missed_extraction(MissedExtractionData {
            message_id: "msg-1".to_string(),
            message_content: "محتاج فلاجيل".to_string(),
            medication: "Flagyl".to_string(),
            item_type: "REQUEST".to_string(),
            quantity: 1.0,
            price: 0.0,
        });

        let stats = parser.feedback_stats();
        assert_eq!(stats.missed_extractions, 1);
        assert!(parser.learned_example_count() > 0);
    }

    #[test]
    fn test_record_false_positive() {
        let parser = PharmaParser::new(PharmaParserConfig::default());

        parser.record_false_positive(
            "msg-1",
            "مرحبا كيف الحال",
            "مرحبا", // AI incorrectly extracted this as medication
            0.6,
        );

        let stats = parser.feedback_stats();
        assert_eq!(stats.false_positives, 1);
        assert!(parser.learned_example_count() > 0);
    }

    // =========================================================================
    // Feedback Loop Configuration Tests
    // =========================================================================

    #[test]
    fn test_enable_disable_feedback() {
        let parser = PharmaParser::new(PharmaParserConfig::default());

        assert!(parser.is_feedback_enabled());

        parser.enable_feedback_loop(false);
        assert!(!parser.is_feedback_enabled());

        parser.enable_feedback_loop(true);
        assert!(parser.is_feedback_enabled());
    }

    #[test]
    fn test_feedback_config_update() {
        let parser = PharmaParser::new(PharmaParserConfig::default());

        let mut config = parser.feedback_config();
        config.max_examples = 50;
        config.examples_per_prompt = 5;
        parser.set_feedback_config(config);

        let updated = parser.feedback_config();
        assert_eq!(updated.max_examples, 50);
        assert_eq!(updated.examples_per_prompt, 5);
    }

    // =========================================================================
    // Export/Import Tests
    // =========================================================================

    #[test]
    fn test_export_import_examples() {
        let parser1 = PharmaParser::new(PharmaParserConfig::default());

        // Record some feedback to generate examples
        parser1.record_missed_extraction(MissedExtractionData {
            message_id: "msg-1".to_string(),
            message_content: "متوفر دواء".to_string(),
            medication: "Medicine".to_string(),
            item_type: "OFFER".to_string(),
            quantity: 1.0,
            price: 100.0,
        });

        let examples = parser1.export_learned_examples();
        assert!(!examples.is_empty());

        // Import into new parser
        let parser2 = PharmaParser::new(PharmaParserConfig::default());
        parser2.import_learned_examples(examples);
        assert!(parser2.learned_example_count() > 0);
    }

    #[test]
    fn test_export_import_corrections() {
        let parser1 = PharmaParser::new(PharmaParserConfig::default());

        parser1.record_medication_correction(MedicationCorrectionData {
            message_id: "msg-1".to_string(),
            message_content: "Test".to_string(),
            ai_medication: "Wrong".to_string(),
            correct_medication: "Correct".to_string(),
            item_type: "OFFER".to_string(),
            quantity: 1.0,
            price: 100.0,
            ai_confidence: 0.8,
        });

        let corrections = parser1.export_medication_corrections();
        assert!(!corrections.is_empty());

        let parser2 = PharmaParser::new(PharmaParserConfig::default());
        parser2.import_medication_corrections(corrections);
        assert!(parser2.learned_correction_count() > 0);
    }

    #[test]
    fn test_clear_learned_data() {
        let parser = PharmaParser::new(PharmaParserConfig::default());

        parser.record_missed_extraction(MissedExtractionData {
            message_id: "msg-1".to_string(),
            message_content: "Test".to_string(),
            medication: "Med".to_string(),
            item_type: "OFFER".to_string(),
            quantity: 1.0,
            price: 100.0,
        });
        parser.record_medication_correction(MedicationCorrectionData {
            message_id: "msg-2".to_string(),
            message_content: "Test".to_string(),
            ai_medication: "Wrong".to_string(),
            correct_medication: "Correct".to_string(),
            item_type: "OFFER".to_string(),
            quantity: 1.0,
            price: 100.0,
            ai_confidence: 0.8,
        });

        assert!(parser.learned_example_count() > 0);
        assert!(parser.learned_correction_count() > 0);

        parser.clear_learned_data();

        assert_eq!(parser.learned_example_count(), 0);
        assert_eq!(parser.learned_correction_count(), 0);
    }

    // =========================================================================
    // Token Estimation Tests
    // =========================================================================

    #[rstest]
    #[case("Hello", 2)]
    #[case("Hello World", 3)]
    #[case("", 1)]
    fn test_estimate_tokens(#[case] text: &str, #[case] expected_min: usize) {
        let tokens = PharmaParser::estimate_tokens(text);
        assert!(tokens >= expected_min.saturating_sub(1));
    }

    // =========================================================================
    // Content Splitting Tests
    // =========================================================================

    #[test]
    fn test_split_content_short() {
        let content = "Short message";
        let chunks = PharmaParser::split_content_into_chunks(content);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], content);
    }

    #[test]
    fn test_split_content_many_lines() {
        let lines: Vec<String> = (0..50).map(|i| format!("Line {}", i)).collect();
        let content = lines.join("\n");

        let chunks = PharmaParser::split_content_into_chunks(&content);
        assert!(chunks.len() > 1);

        // Each chunk should have at most MAX_MESSAGE_LINES lines
        for chunk in &chunks {
            let line_count = chunk.lines().count();
            assert!(line_count <= MAX_MESSAGE_LINES);
        }
    }

    #[test]
    fn test_split_by_tokens() {
        let content = "Short content";
        let chunks = PharmaParser::split_by_tokens(content);
        assert_eq!(chunks.len(), 1);
    }

    // =========================================================================
    // Accuracy Rate Tests
    // =========================================================================

    #[test]
    fn test_accuracy_tracking() {
        let parser = PharmaParser::new(PharmaParserConfig::default());

        // Record 8 correct, 2 wrong
        for i in 0..8 {
            parser.record_correct_extraction(ExtractionData {
                message_id: format!("msg-{}", i),
                message_content: "Test".to_string(),
                medication: "Med".to_string(),
                item_type: "OFFER".to_string(),
                quantity: 1.0,
                price: 100.0,
                ai_confidence: 0.9,
            });
        }
        for i in 0..2 {
            parser.record_medication_correction(MedicationCorrectionData {
                message_id: format!("msg-wrong-{}", i),
                message_content: "Test".to_string(),
                ai_medication: "Wrong".to_string(),
                correct_medication: "Correct".to_string(),
                item_type: "OFFER".to_string(),
                quantity: 1.0,
                price: 100.0,
                ai_confidence: 0.8,
            });
        }

        let stats = parser.feedback_stats();
        assert!((stats.accuracy_rate - 0.8).abs() < 0.01);
    }

    // =========================================================================
    // Circuit Breaker Integration Tests
    // =========================================================================

    #[test]
    fn test_circuit_state_accessible() {
        let parser = PharmaParser::new(PharmaParserConfig::default());
        let state = parser.circuit_state();
        assert!(matches!(
            state,
            super::super::circuit_breaker::CircuitState::Closed
        ));
    }

    // =========================================================================
    // Batcher Stats Tests
    // =========================================================================

    #[test]
    fn test_batcher_stats_accessible() {
        let parser = PharmaParser::new(PharmaParserConfig::default());
        let stats = parser.batcher_stats();
        assert_eq!(stats.total_batches, 0);
    }
}


