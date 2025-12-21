//! AI module for parsing messages
//!
//! Provides the PharmaParser for parsing pharmaceutical messages using AI,
//! along with utilities like circuit breaker, token batching, and feedback loop.

mod circuit_breaker;
mod feedback_loop;
mod pharma_parser;
mod pharma_prompts;
mod pharma_types;
mod token_batcher;

// Circuit breaker for resilient network calls
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitOpenError, CircuitState};

// LLM Feedback Loop for continuous improvement
pub use feedback_loop::{
    AIExtraction, CorrectExtraction, ExampleCategory, ExampleSource, ExtractionFeedback,
    FeedbackLoopConfig, FeedbackLoopStats, FeedbackType, FewShotExample, LLMFeedbackLoop, Language,
    MedicationCorrection,
};

// Token batching for efficient AI calls
pub use token_batcher::{
    BatchMessage, TokenBatchConfig, TokenBatchStats, TokenBatchStatsSnapshot, TokenBatcher,
};

// New direct AI client
pub use pharma_parser::{BatchParseResult, ParseError, PharmaParser, PharmaParserConfig};
pub use pharma_prompts::{SYSTEM_PROMPT, build_user_prompt_with_mappings};
pub use pharma_types::{Intent, ParseResult, ParsedItem, UrgencyLevel};

// Re-export ai-client crate for advanced usage
pub use ai_client::{Client as GenericClient, ClientConfig, generate_schema};
