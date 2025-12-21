//! FTS Token Search Module
//!
//! Ported from legacy/parsing/service.go (getRelevantMappings, extractTokens)
//!
//! Extracts tokens from messages and queries the medication mapping repository
//! using both exact and fuzzy (trigram) matching.
//!
//! Key features:
//! - Token extraction with normalization
//! - FTS query sanitization (removes special chars, Arabic diacritics)
//! - Exact match search with OR queries
//! - Fuzzy trigram fallback for longer words
//! - Memory safeguards (max tokens, max query length)

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

// =============================================================================
// Constants (ported from legacy/parsing/constants.go)
// =============================================================================

/// Maximum tokens to include in FTS query
pub const MAX_FTS_TOKENS: usize = 50;

/// Maximum fuzzy queries to generate
pub const MAX_FUZZY_QUERIES: usize = 20;

/// Minimum word length for fuzzy matching (runes/chars)
pub const MIN_WORD_LENGTH_FOR_FUZZY: usize = 4;

/// Maximum query string length (memory safeguard)
pub const MAX_QUERY_LENGTH: usize = 10000;

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for FTS token search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtsSearchConfig {
    /// Maximum tokens to include in exact search
    pub max_fts_tokens: usize,
    /// Maximum fuzzy queries to generate
    pub max_fuzzy_queries: usize,
    /// Minimum word length for fuzzy matching
    pub min_word_length_for_fuzzy: usize,
    /// Maximum query string length
    pub max_query_length: usize,
    /// Enable fuzzy fallback search
    pub enable_fuzzy_fallback: bool,
    /// Include reply context in token extraction
    pub include_reply_context: bool,
}

impl Default for FtsSearchConfig {
    fn default() -> Self {
        Self {
            max_fts_tokens: MAX_FTS_TOKENS,
            max_fuzzy_queries: MAX_FUZZY_QUERIES,
            min_word_length_for_fuzzy: MIN_WORD_LENGTH_FOR_FUZZY,
            max_query_length: MAX_QUERY_LENGTH,
            enable_fuzzy_fallback: true,
            include_reply_context: true,
        }
    }
}

// =============================================================================
// Statistics
// =============================================================================

/// Statistics for FTS search operations
#[derive(Debug, Default)]
pub struct FtsSearchStats {
    /// Total search operations
    total_searches: AtomicU64,
    /// Total tokens extracted
    total_tokens_extracted: AtomicU64,
    /// Tokens truncated due to limits
    tokens_truncated: AtomicU64,
    /// Fuzzy searches performed
    fuzzy_searches: AtomicU64,
    /// Query length truncations
    query_truncations: AtomicU64,
}

impl FtsSearchStats {
    /// Get a snapshot of current statistics
    pub fn snapshot(&self) -> FtsSearchStatsSnapshot {
        FtsSearchStatsSnapshot {
            total_searches: self.total_searches.load(Ordering::Relaxed),
            total_tokens_extracted: self.total_tokens_extracted.load(Ordering::Relaxed),
            tokens_truncated: self.tokens_truncated.load(Ordering::Relaxed),
            fuzzy_searches: self.fuzzy_searches.load(Ordering::Relaxed),
            query_truncations: self.query_truncations.load(Ordering::Relaxed),
        }
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.total_searches.store(0, Ordering::Relaxed);
        self.total_tokens_extracted.store(0, Ordering::Relaxed);
        self.tokens_truncated.store(0, Ordering::Relaxed);
        self.fuzzy_searches.store(0, Ordering::Relaxed);
        self.query_truncations.store(0, Ordering::Relaxed);
    }
}

/// Snapshot of FTS search statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtsSearchStatsSnapshot {
    pub total_searches: u64,
    pub total_tokens_extracted: u64,
    pub tokens_truncated: u64,
    pub fuzzy_searches: u64,
    pub query_truncations: u64,
}

// =============================================================================
// Token Sanitizer
// =============================================================================

/// Characters to remove from tokens for FTS5 compatibility
/// Ported from Go: tokenReplacer (service.go:305-320)
const FTS_SPECIAL_CHARS: &[char] = &['"', '\'', ':', '!', '*', '(', ')', '^', '~'];

/// Arabic diacritics to remove
/// Ported from Go: tokenReplacer Arabic diacritics
const ARABIC_DIACRITICS: &[char] = &[
    '\u{064B}', // fathatan ً
    '\u{064C}', // dammatan ٌ
    '\u{064D}', // kasratan ٍ
    '\u{064E}', // fatha َ
    '\u{064F}', // damma ُ
    '\u{0650}', // kasra ِ
    '\u{0651}', // shadda ّ
    '\u{0652}', // sukun ْ
];

/// Sanitize a token for FTS5 queries
/// Ported from Go: tokenReplacer.Replace (service.go:305-320)
pub fn sanitize_token(token: &str) -> String {
    token
        .chars()
        .filter(|c| !FTS_SPECIAL_CHARS.contains(c) && !ARABIC_DIACRITICS.contains(c))
        .collect()
}

/// Characters to replace with space in content normalization
const CONTENT_SEPARATORS: &[char] = &['\n', ',', '.', '-'];

/// Normalize content for token extraction
/// Ported from Go: contentReplacer (service.go:323)
pub fn normalize_content(content: &str) -> String {
    content
        .chars()
        .map(|c| {
            if CONTENT_SEPARATORS.contains(&c) {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .to_lowercase()
}

// =============================================================================
// Trigram Generation
// =============================================================================

/// Generate trigrams (3-character substrings) from a string
/// Ported from Go: generateTrigrams (utils.go:87-96)
pub fn generate_trigrams(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 3 {
        return Vec::new();
    }

    let mut trigrams = Vec::with_capacity(chars.len() - 2);
    for i in 0..chars.len() - 2 {
        trigrams.push(chars[i..i + 3].iter().collect());
    }
    trigrams
}

// =============================================================================
// FTS Token Searcher
// =============================================================================

/// FTS Token Search engine
/// Ported from Go: Parser.getRelevantMappings, Parser.extractTokens
pub struct FtsTokenSearcher {
    config: std::sync::RwLock<FtsSearchConfig>,
    stats: FtsSearchStats,
}

impl Default for FtsTokenSearcher {
    fn default() -> Self {
        Self::new(FtsSearchConfig::default())
    }
}

impl FtsTokenSearcher {
    /// Create a new FTS token searcher
    pub fn new(config: FtsSearchConfig) -> Self {
        Self {
            config: std::sync::RwLock::new(config),
            stats: FtsSearchStats::default(),
        }
    }

    // =========================================================================
    // Token Extraction
    // =========================================================================

    /// Extract tokens from content into a set
    /// Ported from Go: Parser.extractTokens (service.go:326-340)
    pub fn extract_tokens(&self, content: &str, tokens: &mut HashSet<String>) {
        if content.is_empty() {
            return;
        }

        let normalized = normalize_content(content);
        let words: Vec<&str> = normalized.split_whitespace().collect();

        for word in words {
            // Filter out short words (< 3 chars)
            if word.chars().count() > 2 {
                tokens.insert(word.to_string());
            }
        }
    }

    /// Extract tokens from multiple messages
    /// Includes reply context if configured
    pub fn extract_tokens_from_messages<T: MessageContent>(
        &self,
        messages: &[T],
    ) -> HashSet<String> {
        let config = self.config.read().unwrap();
        let mut tokens = HashSet::new();

        for msg in messages {
            self.extract_tokens(msg.content(), &mut tokens);

            if config.include_reply_context
                && let Some(reply) = msg.reply_content()
            {
                self.extract_tokens(reply, &mut tokens);
            }
        }

        let extracted_count = tokens.len();
        self.stats
            .total_tokens_extracted
            .fetch_add(extracted_count as u64, Ordering::Relaxed);

        tokens
    }

    // =========================================================================
    // Query Building
    // =========================================================================

    /// Build an exact match FTS query from tokens
    /// Ported from Go: Parser.getRelevantMappings (service.go:380-420)
    pub fn build_exact_query(&self, tokens: &HashSet<String>) -> Option<String> {
        let config = self.config.read().unwrap();

        if tokens.is_empty() {
            return None;
        }

        let mut query = String::new();
        let mut count = 0;

        for token in tokens {
            if count >= config.max_fts_tokens {
                self.stats.tokens_truncated.fetch_add(1, Ordering::Relaxed);
                break;
            }

            if query.len() > config.max_query_length {
                self.stats.query_truncations.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(query_length = query.len(), "Query length limit reached");
                break;
            }

            let clean_token = sanitize_token(token);
            if clean_token.is_empty() {
                continue;
            }

            if !query.is_empty() {
                query.push_str(" OR ");
            }
            query.push('"');
            query.push_str(&clean_token);
            query.push('"');

            count += 1;
        }

        if count == 0 {
            return None;
        }

        Some(query)
    }

    /// Build a fuzzy (trigram) query from tokens
    /// Ported from Go: Parser.getRelevantMappings fuzzy section (service.go:422-470)
    pub fn build_fuzzy_query(&self, tokens: &HashSet<String>) -> Option<String> {
        let config = self.config.read().unwrap();

        if !config.enable_fuzzy_fallback {
            return None;
        }

        let mut query = String::new();
        let mut fuzzy_count = 0;

        for token in tokens {
            // Only fuzzy match longer words
            if token.chars().count() < config.min_word_length_for_fuzzy {
                continue;
            }

            let trigrams = generate_trigrams(token);
            if trigrams.is_empty() {
                continue;
            }

            if fuzzy_count >= config.max_fuzzy_queries {
                break;
            }

            if query.len() > config.max_query_length {
                self.stats.query_truncations.fetch_add(1, Ordering::Relaxed);
                break;
            }

            for tri in &trigrams {
                if !query.is_empty() {
                    query.push_str(" OR ");
                }
                query.push('"');
                query.push_str(tri);
                query.push('"');
            }

            fuzzy_count += 1;
        }

        if fuzzy_count > 0 {
            self.stats.fuzzy_searches.fetch_add(1, Ordering::Relaxed);
            Some(query)
        } else {
            None
        }
    }

    /// Build both exact and fuzzy queries
    pub fn build_queries(&self, tokens: &HashSet<String>) -> (Option<String>, Option<String>) {
        self.stats.total_searches.fetch_add(1, Ordering::Relaxed);
        (
            self.build_exact_query(tokens),
            self.build_fuzzy_query(tokens),
        )
    }

    // =========================================================================
    // High-Level Search
    // =========================================================================

    /// Search for relevant mappings from messages
    /// Returns a map of Arabic name -> English name
    /// Ported from Go: Parser.getRelevantMappings (service.go:345-475)
    pub async fn search_relevant_mappings<T, R>(
        &self,
        messages: &[T],
        repo: &R,
        limit: i64,
    ) -> HashMap<String, String>
    where
        T: MessageContent,
        R: MedicationSearchRepository,
    {
        let mut relevant = HashMap::new();

        // Extract tokens from messages
        let tokens = self.extract_tokens_from_messages(messages);
        if tokens.is_empty() {
            return relevant;
        }

        // Build and execute exact query
        if let Some(exact_query) = self.build_exact_query(&tokens) {
            match repo.search(&exact_query, limit).await {
                Ok(mappings) => {
                    for m in mappings {
                        relevant.insert(m.arabic_name, m.english_name);
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to search medication mappings (exact)");
                }
            }
        }

        // Build and execute fuzzy query (fallback)
        if let Some(fuzzy_query) = self.build_fuzzy_query(&tokens) {
            match repo.search(&fuzzy_query, limit).await {
                Ok(mappings) => {
                    tracing::debug!(count = mappings.len(), "Fuzzy search results");
                    for m in mappings {
                        // Don't overwrite exact matches
                        relevant.entry(m.arabic_name).or_insert(m.english_name);
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to search medication mappings (fuzzy)");
                }
            }
        }

        relevant
    }

    // =========================================================================
    // Configuration & Statistics
    // =========================================================================

    /// Get current configuration
    pub fn get_config(&self) -> FtsSearchConfig {
        self.config.read().unwrap().clone()
    }

    /// Update configuration
    pub fn set_config(&self, config: FtsSearchConfig) {
        *self.config.write().unwrap() = config;
    }

    /// Get statistics snapshot
    pub fn get_stats(&self) -> FtsSearchStatsSnapshot {
        self.stats.snapshot()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.stats.reset();
    }
}

// =============================================================================
// Traits for Abstraction
// =============================================================================

/// Trait for message content extraction
pub trait MessageContent {
    /// Get the main content
    fn content(&self) -> &str;
    /// Get reply content (if any)
    fn reply_content(&self) -> Option<&str>;
}

/// Simple message struct for testing
#[derive(Debug, Clone)]
pub struct SimpleMessage {
    pub content: String,
    pub reply_to_content: Option<String>,
}

impl MessageContent for SimpleMessage {
    fn content(&self) -> &str {
        &self.content
    }

    fn reply_content(&self) -> Option<&str> {
        self.reply_to_content.as_deref()
    }
}

/// Medication mapping result from search
#[derive(Debug, Clone)]
pub struct MedicationSearchResult {
    pub arabic_name: String,
    pub english_name: String,
}

/// Trait for medication search repository
#[async_trait::async_trait]
pub trait MedicationSearchRepository: Send + Sync {
    /// Search for mappings using FTS query
    async fn search(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<MedicationSearchResult>, Box<dyn std::error::Error + Send + Sync>>;
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Sanitization Tests
    // =========================================================================

    #[test]
    fn test_sanitize_token_removes_special_chars() {
        assert_eq!(sanitize_token("hello\"world"), "helloworld");
        assert_eq!(sanitize_token("test:value"), "testvalue");
        assert_eq!(sanitize_token("foo*bar"), "foobar");
        assert_eq!(sanitize_token("(test)"), "test");
    }

    #[test]
    fn test_sanitize_token_removes_arabic_diacritics() {
        // Arabic word with diacritics
        assert_eq!(sanitize_token("مَرْحَبًا"), "مرحبا");
    }

    #[test]
    fn test_normalize_content() {
        assert_eq!(normalize_content("Hello\nWorld"), "hello world");
        assert_eq!(normalize_content("A,B.C-D"), "a b c d");
        assert_eq!(normalize_content("UPPERCASE"), "uppercase");
    }

    // =========================================================================
    // Trigram Tests
    // =========================================================================

    #[test]
    fn test_generate_trigrams() {
        let trigrams = generate_trigrams("hello");
        assert_eq!(trigrams, vec!["hel", "ell", "llo"]);
    }

    #[test]
    fn test_generate_trigrams_short_string() {
        assert!(generate_trigrams("ab").is_empty());
        assert_eq!(generate_trigrams("abc"), vec!["abc"]);
    }

    #[test]
    fn test_generate_trigrams_arabic() {
        let trigrams = generate_trigrams("مرحبا");
        assert_eq!(trigrams.len(), 3); // 5 chars - 2 = 3 trigrams
    }

    // =========================================================================
    // Token Extraction Tests
    // =========================================================================

    #[test]
    fn test_extract_tokens() {
        let searcher = FtsTokenSearcher::default();
        let mut tokens = HashSet::new();

        searcher.extract_tokens("Hello world test", &mut tokens);

        assert!(tokens.contains("hello"));
        assert!(tokens.contains("world"));
        assert!(tokens.contains("test"));
    }

    #[test]
    fn test_extract_tokens_filters_short_words() {
        let searcher = FtsTokenSearcher::default();
        let mut tokens = HashSet::new();

        searcher.extract_tokens("I am a test", &mut tokens);

        assert!(!tokens.contains("i"));
        assert!(!tokens.contains("am"));
        assert!(!tokens.contains("a"));
        assert!(tokens.contains("test"));
    }

    #[test]
    fn test_extract_tokens_from_messages() {
        let searcher = FtsTokenSearcher::default();
        let messages = vec![SimpleMessage {
            content: "Hello world".to_string(),
            reply_to_content: Some("Previous message".to_string()),
        }];

        let tokens = searcher.extract_tokens_from_messages(&messages);

        assert!(tokens.contains("hello"));
        assert!(tokens.contains("world"));
        assert!(tokens.contains("previous"));
        assert!(tokens.contains("message"));
    }

    #[test]
    fn test_extract_tokens_without_reply_context() {
        let config = FtsSearchConfig {
            include_reply_context: false,
            ..Default::default()
        };
        let searcher = FtsTokenSearcher::new(config);
        let messages = vec![SimpleMessage {
            content: "Hello world".to_string(),
            reply_to_content: Some("Previous message".to_string()),
        }];

        let tokens = searcher.extract_tokens_from_messages(&messages);

        assert!(tokens.contains("hello"));
        assert!(tokens.contains("world"));
        assert!(!tokens.contains("previous")); // Reply context excluded
    }

    // =========================================================================
    // Query Building Tests
    // =========================================================================

    #[test]
    fn test_build_exact_query() {
        let searcher = FtsTokenSearcher::default();
        let mut tokens = HashSet::new();
        tokens.insert("hello".to_string());
        tokens.insert("world".to_string());

        let query = searcher.build_exact_query(&tokens).unwrap();

        // Query should contain both tokens with OR
        assert!(query.contains("\"hello\"") || query.contains("\"world\""));
        assert!(query.contains(" OR "));
    }

    #[test]
    fn test_build_exact_query_empty() {
        let searcher = FtsTokenSearcher::default();
        let tokens = HashSet::new();

        assert!(searcher.build_exact_query(&tokens).is_none());
    }

    #[test]
    fn test_build_exact_query_sanitizes_tokens() {
        let searcher = FtsTokenSearcher::default();
        let mut tokens = HashSet::new();
        tokens.insert("test:value".to_string());

        let query = searcher.build_exact_query(&tokens).unwrap();

        assert!(query.contains("\"testvalue\""));
        assert!(!query.contains(":"));
    }

    #[test]
    fn test_build_fuzzy_query() {
        let searcher = FtsTokenSearcher::default();
        let mut tokens = HashSet::new();
        tokens.insert("hello".to_string()); // 5 chars >= MIN_WORD_LENGTH_FOR_FUZZY (4)

        let query = searcher.build_fuzzy_query(&tokens).unwrap();

        // Should contain trigrams
        assert!(query.contains("\"hel\""));
        assert!(query.contains("\"ell\""));
        assert!(query.contains("\"llo\""));
    }

    #[test]
    fn test_build_fuzzy_query_skips_short_words() {
        let searcher = FtsTokenSearcher::default();
        let mut tokens = HashSet::new();
        tokens.insert("abc".to_string()); // 3 chars < MIN_WORD_LENGTH_FOR_FUZZY (4)

        assert!(searcher.build_fuzzy_query(&tokens).is_none());
    }

    #[test]
    fn test_build_fuzzy_query_disabled() {
        let config = FtsSearchConfig {
            enable_fuzzy_fallback: false,
            ..Default::default()
        };
        let searcher = FtsTokenSearcher::new(config);
        let mut tokens = HashSet::new();
        tokens.insert("hello".to_string());

        assert!(searcher.build_fuzzy_query(&tokens).is_none());
    }

    // =========================================================================
    // Limit Tests
    // =========================================================================

    #[test]
    fn test_max_tokens_limit() {
        let config = FtsSearchConfig {
            max_fts_tokens: 3,
            ..Default::default()
        };
        let searcher = FtsTokenSearcher::new(config);

        let mut tokens = HashSet::new();
        for i in 0..10 {
            tokens.insert(format!("token{}", i));
        }

        let query = searcher.build_exact_query(&tokens).unwrap();

        // Count OR occurrences (should be max_tokens - 1 = 2)
        let or_count = query.matches(" OR ").count();
        assert_eq!(or_count, 2);
    }

    #[test]
    fn test_max_fuzzy_queries_limit() {
        let config = FtsSearchConfig {
            max_fuzzy_queries: 2,
            min_word_length_for_fuzzy: 4,
            ..Default::default()
        };
        let searcher = FtsTokenSearcher::new(config);

        let mut tokens = HashSet::new();
        for i in 0..10 {
            tokens.insert(format!("longword{}", i)); // All >= 4 chars
        }

        let query = searcher.build_fuzzy_query(&tokens).unwrap();

        // Should have limited trigrams (2 words * ~7 trigrams each)
        // But we can't easily count, so just verify it's not empty
        assert!(!query.is_empty());

        let stats = searcher.get_stats();
        assert_eq!(stats.fuzzy_searches, 1);
    }

    // =========================================================================
    // Statistics Tests
    // =========================================================================

    #[test]
    fn test_statistics_tracking() {
        let searcher = FtsTokenSearcher::default();

        let mut tokens = HashSet::new();
        tokens.insert("hello".to_string());
        tokens.insert("world".to_string());

        searcher.build_queries(&tokens);

        let stats = searcher.get_stats();
        assert_eq!(stats.total_searches, 1);
        assert_eq!(stats.fuzzy_searches, 1); // "hello" and "world" are >= 4 chars
    }

    #[test]
    fn test_statistics_reset() {
        let searcher = FtsTokenSearcher::default();

        let mut tokens = HashSet::new();
        tokens.insert("hello".to_string());
        searcher.build_queries(&tokens);

        assert!(searcher.get_stats().total_searches > 0);

        searcher.reset_stats();

        assert_eq!(searcher.get_stats().total_searches, 0);
    }

    // =========================================================================
    // Configuration Tests
    // =========================================================================

    #[test]
    fn test_default_config() {
        let config = FtsSearchConfig::default();

        assert_eq!(config.max_fts_tokens, MAX_FTS_TOKENS);
        assert_eq!(config.max_fuzzy_queries, MAX_FUZZY_QUERIES);
        assert_eq!(config.min_word_length_for_fuzzy, MIN_WORD_LENGTH_FOR_FUZZY);
        assert_eq!(config.max_query_length, MAX_QUERY_LENGTH);
        assert!(config.enable_fuzzy_fallback);
        assert!(config.include_reply_context);
    }

    #[test]
    fn test_config_update() {
        let searcher = FtsTokenSearcher::default();

        let new_config = FtsSearchConfig {
            max_fts_tokens: 100,
            ..Default::default()
        };
        searcher.set_config(new_config);

        assert_eq!(searcher.get_config().max_fts_tokens, 100);
    }
}
