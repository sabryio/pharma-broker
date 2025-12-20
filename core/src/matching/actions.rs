//! Confidence-Based Actions
//!
//! Automatic actions based on match confidence scores.
//! Implements Task 5.1: AutoActionHandler
//!
//! Action bands:
//! - AUTO (≥0.9): Auto-confirm and notify
//! - SUGGEST (0.7-0.9): Suggest to operator
//! - REVIEW (0.5-0.7): Queue for review
//! - IGNORE (<0.5): Low confidence, ignore

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::domain::ConfidenceBand;

// ============================================================================
// Match Action Types
// ============================================================================

/// Actions that can be taken based on match confidence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchAction {
    /// Automatically confirm the match and notify parties
    AutoConfirm,
    /// Suggest the match to an operator for approval
    SuggestToOperator,
    /// Queue for human review (low-medium confidence)
    QueueForReview,
    /// Ignore the match (too low confidence)
    Ignore,
}

impl std::fmt::Display for MatchAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AutoConfirm => write!(f, "auto_confirm"),
            Self::SuggestToOperator => write!(f, "suggest"),
            Self::QueueForReview => write!(f, "review"),
            Self::Ignore => write!(f, "ignore"),
        }
    }
}

impl MatchAction {
    /// Check if this action requires human intervention
    pub fn requires_human(&self) -> bool {
        matches!(self, Self::SuggestToOperator | Self::QueueForReview)
    }

    /// Check if this is an automatic action
    pub fn is_automatic(&self) -> bool {
        matches!(self, Self::AutoConfirm | Self::Ignore)
    }
}

// ============================================================================
// AI Parse Action (for review queue)
// ============================================================================

/// Action to take for AI parse results based on confidence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseAction {
    /// Accept the parse result and create offers/requests
    Accept,
    /// Queue for human review
    QueueForReview,
    /// Reject the parse result (too low confidence)
    Reject,
}

impl ParseAction {
    /// Determine parse action based on average confidence
    pub fn from_confidence(avg_confidence: f64, config: &AutoActionConfig) -> Self {
        if avg_confidence >= config.accept_threshold {
            Self::Accept
        } else if avg_confidence >= config.review_threshold {
            Self::QueueForReview
        } else {
            Self::Reject
        }
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for automatic action thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoActionConfig {
    /// Minimum score for auto-confirm (default: 0.90)
    pub auto_confirm_threshold: f64,
    /// Minimum score to suggest to operator (default: 0.70)
    pub suggest_threshold: f64,
    /// Minimum score for review queue (default: 0.50)
    pub review_threshold: f64,
    /// Minimum confidence for AI parse acceptance (default: 0.60)
    pub accept_threshold: f64,
    /// Whether auto-confirm is enabled
    pub auto_confirm_enabled: bool,
    /// Whether to queue low-confidence parses for review
    pub queue_low_confidence: bool,
}

impl Default for AutoActionConfig {
    fn default() -> Self {
        Self {
            auto_confirm_threshold: 0.90,
            suggest_threshold: 0.70,
            review_threshold: 0.50,
            accept_threshold: 0.60,
            auto_confirm_enabled: false, // Conservative default
            queue_low_confidence: true,
        }
    }
}

impl AutoActionConfig {
    /// Create a permissive configuration (more auto-confirms)
    pub fn permissive() -> Self {
        Self {
            auto_confirm_threshold: 0.85,
            suggest_threshold: 0.65,
            review_threshold: 0.45,
            accept_threshold: 0.55,
            auto_confirm_enabled: true,
            queue_low_confidence: true,
        }
    }

    /// Create a strict configuration (more human review)
    pub fn strict() -> Self {
        Self {
            auto_confirm_threshold: 0.95,
            suggest_threshold: 0.80,
            review_threshold: 0.60,
            accept_threshold: 0.70,
            auto_confirm_enabled: false,
            queue_low_confidence: true,
        }
    }

    /// Load from environment variables
    pub fn from_env() -> Self {
        Self {
            auto_confirm_threshold: std::env::var("AUTO_CONFIRM_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.90),
            suggest_threshold: std::env::var("SUGGEST_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.70),
            review_threshold: std::env::var("REVIEW_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.50),
            accept_threshold: std::env::var("ACCEPT_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.60),
            auto_confirm_enabled: std::env::var("AUTO_CONFIRM_ENABLED")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(false),
            queue_low_confidence: std::env::var("QUEUE_LOW_CONFIDENCE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
        }
    }
}

// ============================================================================
// Action Handler
// ============================================================================

use super::thresholds::{SmoothThresholdCalculator, SmoothThresholdConfig};

/// Handler for determining and executing confidence-based actions
#[derive(Clone)]
pub struct AutoActionHandler {
    config: AutoActionConfig,
    calculator: Arc<SmoothThresholdCalculator>,
}

impl AutoActionHandler {
    /// Create a new handler with the given config
    pub fn new(config: AutoActionConfig) -> Self {
        let smooth_config = SmoothThresholdConfig {
            auto_threshold: config.auto_confirm_threshold,
            suggest_threshold: config.suggest_threshold,
            review_threshold: config.review_threshold,
            ..Default::default()
        };
        Self {
            config,
            calculator: Arc::new(SmoothThresholdCalculator::new(smooth_config)),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(AutoActionConfig::default())
    }

    /// Create from environment variables
    pub fn from_env() -> Self {
        Self::new(AutoActionConfig::from_env())
    }

    /// Get the current configuration
    pub fn config(&self) -> &AutoActionConfig {
        &self.config
    }

    /// Determine the action to take for a match based on its score
    pub async fn determine_action(&self, score: f64) -> MatchAction {
        let result = self.calculator.calculate(score).await;

        match result.primary_band {
            ConfidenceBand::Auto if self.config.auto_confirm_enabled => MatchAction::AutoConfirm,
            ConfidenceBand::Auto | ConfidenceBand::Suggest => MatchAction::SuggestToOperator,
            ConfidenceBand::Review => MatchAction::QueueForReview,
            ConfidenceBand::None => MatchAction::Ignore,
        }
    }

    /// Determine action from a ConfidenceBand
    pub fn action_for_band(&self, band: ConfidenceBand) -> MatchAction {
        match band {
            ConfidenceBand::Auto if self.config.auto_confirm_enabled => MatchAction::AutoConfirm,
            ConfidenceBand::Auto | ConfidenceBand::Suggest => MatchAction::SuggestToOperator,
            ConfidenceBand::Review => MatchAction::QueueForReview,
            ConfidenceBand::None => MatchAction::Ignore,
        }
    }

    /// Determine parse action based on average confidence
    pub fn determine_parse_action(&self, avg_confidence: f64) -> ParseAction {
        ParseAction::from_confidence(avg_confidence, &self.config)
    }

    /// Check if a score should trigger auto-confirm
    pub fn should_auto_confirm(&self, score: f64) -> bool {
        self.config.auto_confirm_enabled && score >= self.config.auto_confirm_threshold
    }

    /// Check if a parse result should be queued for review
    pub fn should_queue_for_review(&self, avg_confidence: f64) -> bool {
        self.config.queue_low_confidence
            && avg_confidence < self.config.accept_threshold
            && avg_confidence >= self.config.review_threshold
    }

    /// Get the internal calculator for adjustment
    pub fn calculator(&self) -> Arc<SmoothThresholdCalculator> {
        self.calculator.clone()
    }
}

// ============================================================================
// Action Result
// ============================================================================

/// Result of taking an action
#[derive(Debug, Clone, Serialize)]
pub struct ActionResult {
    pub action: MatchAction,
    pub executed: bool,
    pub message: String,
}

impl ActionResult {
    pub fn success(action: MatchAction, message: impl Into<String>) -> Self {
        Self {
            action,
            executed: true,
            message: message.into(),
        }
    }

    pub fn skipped(action: MatchAction, message: impl Into<String>) -> Self {
        Self {
            action,
            executed: false,
            message: message.into(),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_action_display() {
        assert_eq!(MatchAction::AutoConfirm.to_string(), "auto_confirm");
        assert_eq!(MatchAction::SuggestToOperator.to_string(), "suggest");
        assert_eq!(MatchAction::QueueForReview.to_string(), "review");
        assert_eq!(MatchAction::Ignore.to_string(), "ignore");
    }

    #[test]
    fn test_match_action_requires_human() {
        assert!(!MatchAction::AutoConfirm.requires_human());
        assert!(MatchAction::SuggestToOperator.requires_human());
        assert!(MatchAction::QueueForReview.requires_human());
        assert!(!MatchAction::Ignore.requires_human());
    }

    #[test]
    fn test_auto_action_config_default() {
        let config = AutoActionConfig::default();
        assert_eq!(config.auto_confirm_threshold, 0.90);
        assert_eq!(config.suggest_threshold, 0.70);
        assert_eq!(config.review_threshold, 0.50);
        assert!(!config.auto_confirm_enabled);
    }

    #[tokio::test]
    async fn test_determine_action_with_auto_confirm_disabled() {
        let handler = AutoActionHandler::with_defaults();

        // Even high scores don't auto-confirm when disabled
        assert_eq!(
            handler.determine_action(0.95).await,
            MatchAction::SuggestToOperator
        );
        assert_eq!(
            handler.determine_action(0.75).await,
            MatchAction::SuggestToOperator
        );
        assert_eq!(
            handler.determine_action(0.55).await,
            MatchAction::QueueForReview
        );
        assert_eq!(handler.determine_action(0.35).await, MatchAction::Ignore);
    }

    #[tokio::test]
    async fn test_determine_action_with_auto_confirm_enabled() {
        let config = AutoActionConfig {
            auto_confirm_enabled: true,
            ..Default::default()
        };
        let handler = AutoActionHandler::new(config);

        assert_eq!(
            handler.determine_action(0.95).await,
            MatchAction::AutoConfirm
        );
        assert_eq!(
            handler.determine_action(0.90).await,
            MatchAction::AutoConfirm
        );
        assert_eq!(
            handler.determine_action(0.89).await,
            MatchAction::SuggestToOperator
        );
    }

    #[tokio::test]
    async fn test_boundary_cases() {
        let handler = AutoActionHandler::with_defaults();

        // Test boundary at 0.70
        assert_eq!(
            handler.determine_action(0.70).await,
            MatchAction::SuggestToOperator
        );
        assert_eq!(
            handler.determine_action(0.699).await,
            MatchAction::QueueForReview
        );

        // Test boundary at 0.50
        assert_eq!(
            handler.determine_action(0.50).await,
            MatchAction::QueueForReview
        );
        assert_eq!(handler.determine_action(0.499).await, MatchAction::Ignore);
    }

    #[test]
    fn test_parse_action_from_confidence() {
        let config = AutoActionConfig::default();

        assert_eq!(
            ParseAction::from_confidence(0.70, &config),
            ParseAction::Accept
        );
        assert_eq!(
            ParseAction::from_confidence(0.55, &config),
            ParseAction::QueueForReview
        );
        assert_eq!(
            ParseAction::from_confidence(0.40, &config),
            ParseAction::Reject
        );
    }

    #[test]
    fn test_should_queue_for_review() {
        let handler = AutoActionHandler::with_defaults();

        // Below accept but above review threshold
        assert!(handler.should_queue_for_review(0.55));

        // Above accept threshold - no need for review
        assert!(!handler.should_queue_for_review(0.70));

        // Below review threshold - reject instead
        assert!(!handler.should_queue_for_review(0.40));
    }
}
