//! Token-Aware Batching for AI Messages
//!
//! Ported from: legacy/parsing/token_batcher.go
//!
//! Splits message batches based on token limits to prevent exceeding LLM context windows.

use std::sync::atomic::{AtomicU64, Ordering};

// =============================================================================
// Configuration Constants (matching legacy)
// =============================================================================

/// Default maximum tokens per batch (leaves room for response)
pub const DEFAULT_MAX_TOKENS_PER_BATCH: usize = 6000;

/// Default estimated tokens for system prompt + mappings
pub const DEFAULT_PROMPT_OVERHEAD: usize = 2000;

/// Default overhead per message structure (labels, formatting)
pub const DEFAULT_TOKENS_PER_MESSAGE: usize = 50;

/// Default hard limit on messages per batch
pub const DEFAULT_MAX_MESSAGES_PER_BATCH: usize = 10;

// =============================================================================
// Configuration
// =============================================================================

/// Token batching configuration
#[derive(Debug, Clone)]
pub struct TokenBatchConfig {
    /// Maximum tokens allowed per batch (default: 6000)
    pub max_tokens_per_batch: usize,
    /// Estimated tokens for system prompt + mappings (default: 2000)
    pub prompt_overhead: usize,
    /// Estimated overhead per message structure (default: 50)
    pub tokens_per_message: usize,
    /// Hard limit on messages per batch (default: 10)
    pub max_messages_per_batch: usize,
}

impl Default for TokenBatchConfig {
    fn default() -> Self {
        Self {
            max_tokens_per_batch: DEFAULT_MAX_TOKENS_PER_BATCH,
            prompt_overhead: DEFAULT_PROMPT_OVERHEAD,
            tokens_per_message: DEFAULT_TOKENS_PER_MESSAGE,
            max_messages_per_batch: DEFAULT_MAX_MESSAGES_PER_BATCH,
        }
    }
}

// =============================================================================
// Message Structure
// =============================================================================

/// A message to be parsed by AI
#[derive(Debug, Clone)]
pub struct BatchMessage {
    /// Unique identifier for tracking
    pub id: String,
    /// Message content text
    pub content: String,
    /// Sender name
    pub sender_name: Option<String>,
    /// Group name
    pub group_name: Option<String>,
    /// Reply context (truncated to 200 chars in prompt)
    pub reply_to: Option<String>,
}

impl BatchMessage {
    /// Create a new batch message
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            sender_name: None,
            group_name: None,
            reply_to: None,
        }
    }

    /// Set sender name
    pub fn with_sender(mut self, sender: impl Into<String>) -> Self {
        self.sender_name = Some(sender.into());
        self
    }

    /// Set group name
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group_name = Some(group.into());
        self
    }

    /// Set reply context
    pub fn with_reply(mut self, reply: impl Into<String>) -> Self {
        self.reply_to = Some(reply.into());
        self
    }
}

// =============================================================================
// Statistics
// =============================================================================

/// Token batching statistics (thread-safe)
#[derive(Debug, Default)]
pub struct TokenBatchStats {
    /// Total batches created
    pub total_batches: AtomicU64,
    /// Total messages processed
    pub total_messages: AtomicU64,
    /// Total tokens estimated
    pub total_tokens: AtomicU64,
    /// Batches that were split due to token limits
    pub split_batches: AtomicU64,
    /// Messages that exceeded single-message token limit
    pub oversized_messages: AtomicU64,
}

impl TokenBatchStats {
    /// Get a snapshot of current statistics
    pub fn snapshot(&self) -> TokenBatchStatsSnapshot {
        TokenBatchStatsSnapshot {
            total_batches: self.total_batches.load(Ordering::Relaxed),
            total_messages: self.total_messages.load(Ordering::Relaxed),
            total_tokens: self.total_tokens.load(Ordering::Relaxed),
            split_batches: self.split_batches.load(Ordering::Relaxed),
            oversized_messages: self.oversized_messages.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of token batch statistics
#[derive(Debug, Clone)]
pub struct TokenBatchStatsSnapshot {
    pub total_batches: u64,
    pub total_messages: u64,
    pub total_tokens: u64,
    pub split_batches: u64,
    pub oversized_messages: u64,
}

// =============================================================================
// Token Batcher
// =============================================================================

/// Token-aware message batcher
///
/// Splits message batches based on token limits to prevent exceeding LLM context windows.
/// Ported from legacy/parsing/token_batcher.go
pub struct TokenBatcher {
    config: TokenBatchConfig,
    stats: TokenBatchStats,
}

impl TokenBatcher {
    /// Create a new token batcher with the given configuration
    pub fn new(config: TokenBatchConfig) -> Self {
        Self {
            config,
            stats: TokenBatchStats::default(),
        }
    }

    /// Create a new token batcher with default configuration
    pub fn with_defaults() -> Self {
        Self::new(TokenBatchConfig::default())
    }

    /// Estimate token count for a text string
    ///
    /// Uses simple heuristic: ~4 characters per token (cl100k_base-like)
    /// This matches the legacy fallback behavior
    pub fn estimate_tokens(text: &str) -> usize {
        // Simple estimation: ~4 chars per token
        // This is a reasonable approximation for cl100k_base encoding
        // More accurate would require a proper tokenizer like tiktoken-rs
        text.len().saturating_div(4).max(1)
    }

    /// Estimate token count for a single message
    ///
    /// Accounts for:
    /// - Base overhead for message structure (From:, Group:, Content: labels)
    /// - Content tokens
    /// - Reply context tokens (capped at 60 tokens)
    pub fn estimate_message_tokens(&self, msg: &BatchMessage) -> usize {
        // Base overhead for message structure
        let mut tokens = self.config.tokens_per_message;

        // Count content tokens
        tokens += Self::estimate_tokens(&msg.content);

        // Add reply context tokens if present (capped at 60 as in legacy)
        if let Some(ref reply) = msg.reply_to {
            let reply_tokens = Self::estimate_tokens(reply);
            // Reply is truncated to 200 chars in prompt, so cap tokens
            tokens += reply_tokens.min(60);
        }

        tokens
    }

    /// Estimate total tokens for a batch of messages
    pub fn estimate_batch_tokens(&self, messages: &[BatchMessage]) -> usize {
        let mut total = self.config.prompt_overhead;
        for msg in messages {
            total += self.estimate_message_tokens(msg);
        }
        total
    }

    /// Split messages into token-aware batches
    ///
    /// Each batch will not exceed MaxTokensPerBatch (accounting for prompt overhead).
    /// Oversized messages (exceeding available tokens) are processed alone.
    pub fn split_into_batches(&self, messages: Vec<BatchMessage>) -> Vec<Vec<BatchMessage>> {
        if messages.is_empty() {
            return Vec::new();
        }

        let available_tokens = self
            .config
            .max_tokens_per_batch
            .saturating_sub(self.config.prompt_overhead)
            .max(self.config.max_tokens_per_batch / 2); // Safety fallback

        let mut batches: Vec<Vec<BatchMessage>> = Vec::new();
        let mut current_batch: Vec<BatchMessage> = Vec::new();
        let mut current_tokens: usize = 0;
        let mut total_tokens: usize = 0;

        for msg in messages.iter() {
            let msg_tokens = self.estimate_message_tokens(msg);
            total_tokens += msg_tokens;

            // Check if single message exceeds limit (oversized)
            if msg_tokens > available_tokens {
                self.stats
                    .oversized_messages
                    .fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    msg_id = %msg.id,
                    tokens = msg_tokens,
                    limit = available_tokens,
                    "⚠️ Oversized message exceeds token limit, processing alone"
                );

                // Flush current batch if not empty
                if !current_batch.is_empty() {
                    batches.push(std::mem::take(&mut current_batch));
                    current_tokens = 0;
                }

                // Add oversized message as its own batch
                batches.push(vec![msg.clone()]);
                continue;
            }

            // Check if adding this message would exceed limits
            let would_exceed_tokens = current_tokens + msg_tokens > available_tokens;
            let would_exceed_count = current_batch.len() >= self.config.max_messages_per_batch;

            if would_exceed_tokens || would_exceed_count {
                // Flush current batch
                if !current_batch.is_empty() {
                    batches.push(std::mem::take(&mut current_batch));
                    self.stats.split_batches.fetch_add(1, Ordering::Relaxed);
                }
                current_tokens = 0;
            }

            // Add message to current batch
            current_batch.push(msg.clone());
            current_tokens += msg_tokens;
        }

        // Don't forget the last batch
        if !current_batch.is_empty() {
            batches.push(current_batch);
        }

        // Update stats
        self.stats
            .total_batches
            .fetch_add(batches.len() as u64, Ordering::Relaxed);
        self.stats
            .total_messages
            .fetch_add(messages.len() as u64, Ordering::Relaxed);
        self.stats
            .total_tokens
            .fetch_add(total_tokens as u64, Ordering::Relaxed);

        // Log if batches were split
        if batches.len() > 1 {
            tracing::info!(
                original_count = messages.len(),
                batch_count = batches.len(),
                total_tokens = total_tokens,
                max_tokens = self.config.max_tokens_per_batch,
                "📦 Split messages into token-aware batches"
            );
        }

        batches
    }

    /// Get current configuration
    pub fn config(&self) -> &TokenBatchConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: TokenBatchConfig) {
        tracing::info!(
            max_tokens = config.max_tokens_per_batch,
            prompt_overhead = config.prompt_overhead,
            max_messages = config.max_messages_per_batch,
            "Token batcher configuration updated"
        );
        self.config = config;
    }

    /// Get statistics snapshot
    pub fn stats(&self) -> TokenBatchStatsSnapshot {
        self.stats.snapshot()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(TokenBatcher::estimate_tokens(""), 1); // min 1
    }

    #[test]
    fn test_estimate_tokens_short() {
        // "hello" = 5 chars / 4 = 1 token
        assert_eq!(TokenBatcher::estimate_tokens("hello"), 1);
    }

    #[test]
    fn test_estimate_tokens_medium() {
        // 100 chars / 4 = 25 tokens
        let text = "a".repeat(100);
        assert_eq!(TokenBatcher::estimate_tokens(&text), 25);
    }

    #[test]
    fn test_estimate_tokens_arabic() {
        // Arabic text: 40 chars / 4 = 10 tokens (bytes may vary but we use chars)
        let arabic = "مرحبا بكم في الصيدلية"; // ~21 chars
        let tokens = TokenBatcher::estimate_tokens(arabic);
        assert!((5..=20).contains(&tokens));
    }

    #[test]
    fn test_estimate_message_tokens() {
        let batcher = TokenBatcher::with_defaults();
        let msg = BatchMessage::new("1", "hello world");

        let tokens = batcher.estimate_message_tokens(&msg);
        // 50 (overhead) + "hello world" (11 chars) / 4 = 50 + 2 = 52
        assert!(tokens >= 50);
    }

    #[test]
    fn test_estimate_message_tokens_with_reply() {
        let batcher = TokenBatcher::with_defaults();
        let msg = BatchMessage::new("1", "response text")
            .with_reply("original message that was quite long");

        let tokens = batcher.estimate_message_tokens(&msg);
        // Should be > 50 (overhead) + content + reply (capped)
        assert!(tokens > 50);
    }

    #[test]
    fn test_split_empty() {
        let batcher = TokenBatcher::with_defaults();
        let batches = batcher.split_into_batches(vec![]);
        assert!(batches.is_empty());
    }

    #[test]
    fn test_split_single_message() {
        let batcher = TokenBatcher::with_defaults();
        let messages = vec![BatchMessage::new("1", "short message")];

        let batches = batcher.split_into_batches(messages);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 1);
    }

    #[test]
    fn test_split_within_limits() {
        let batcher = TokenBatcher::with_defaults();
        let messages: Vec<_> = (0..5)
            .map(|i| BatchMessage::new(i.to_string(), format!("message {}", i)))
            .collect();

        let batches = batcher.split_into_batches(messages);
        // 5 small messages should fit in one batch
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 5);
    }

    #[test]
    fn test_split_exceeds_message_count() {
        // Config with max 3 messages per batch
        let config = TokenBatchConfig {
            max_messages_per_batch: 3,
            ..Default::default()
        };
        let batcher = TokenBatcher::new(config);

        let messages: Vec<_> = (0..7)
            .map(|i| BatchMessage::new(i.to_string(), format!("message {}", i)))
            .collect();

        let batches = batcher.split_into_batches(messages);
        // 7 messages with max 3 per batch = 3 batches (3 + 3 + 1)
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 3);
        assert_eq!(batches[1].len(), 3);
        assert_eq!(batches[2].len(), 1);
    }

    #[test]
    fn test_split_exceeds_token_limit() {
        // Config with very low token limit
        let config = TokenBatchConfig {
            max_tokens_per_batch: 200,
            prompt_overhead: 100,
            tokens_per_message: 10,
            max_messages_per_batch: 100, // High count limit
        };
        let batcher = TokenBatcher::new(config);

        // Create messages that will exceed token limit
        // Available: 200 - 100 = 100 tokens
        // Each message: 10 (overhead) + content
        let messages: Vec<_> = (0..5)
            .map(|i| BatchMessage::new(i.to_string(), "a".repeat(100))) // ~25 tokens each
            .collect();

        let batches = batcher.split_into_batches(messages);
        // Should split into multiple batches
        assert!(batches.len() > 1);
    }

    #[test]
    fn test_oversized_message() {
        // Config where a large message won't fit
        let config = TokenBatchConfig {
            max_tokens_per_batch: 200,
            prompt_overhead: 100,
            tokens_per_message: 10,
            max_messages_per_batch: 10,
        };
        let batcher = TokenBatcher::new(config);

        // Large message: 1000 chars / 4 = 250 tokens > available (100)
        let messages = vec![
            BatchMessage::new("1", "short"),
            BatchMessage::new("2", "a".repeat(1000)), // Oversized
            BatchMessage::new("3", "short"),
        ];

        let batches = batcher.split_into_batches(messages);
        // Oversized message should be in its own batch
        assert!(batches.len() >= 2);

        let stats = batcher.stats();
        assert_eq!(stats.oversized_messages, 1);
    }

    #[test]
    fn test_stats_tracking() {
        let batcher = TokenBatcher::with_defaults();
        let messages: Vec<_> = (0..3)
            .map(|i| BatchMessage::new(i.to_string(), format!("message {}", i)))
            .collect();

        batcher.split_into_batches(messages);

        let stats = batcher.stats();
        assert_eq!(stats.total_batches, 1);
        assert_eq!(stats.total_messages, 3);
        assert!(stats.total_tokens > 0);
    }
}
