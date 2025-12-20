//! Telegram notification sender
//!
//! Sends match notifications via Telegram Bot API.

use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use tracing::{error, info, warn};

use crate::Result;
use crate::domain::Match;
use crate::matching::MatchAction;

use super::MatchNotifier;

/// Telegram Bot API configuration
#[derive(Debug, Clone)]
pub struct TelegramConfig {
    /// Bot token from @BotFather
    pub bot_token: String,
    /// Chat ID to send notifications to
    pub chat_id: String,
    /// Enable/disable notifications
    pub enabled: bool,
    /// Parse mode (HTML, Markdown, MarkdownV2)
    pub parse_mode: String,
}

impl TelegramConfig {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            bot_token: std::env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default(),
            chat_id: std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default(),
            enabled: std::env::var("TELEGRAM_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            parse_mode: std::env::var("TELEGRAM_PARSE_MODE").unwrap_or_else(|_| "HTML".to_string()),
        }
    }

    /// Check if properly configured
    pub fn is_valid(&self) -> bool {
        self.enabled && !self.bot_token.is_empty() && !self.chat_id.is_empty()
    }
}

/// Telegram message request body
#[derive(Debug, Serialize)]
struct SendMessageRequest<'a> {
    chat_id: &'a str,
    text: String,
    parse_mode: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    disable_notification: Option<bool>,
}

/// Telegram notifier implementation
pub struct TelegramNotifier {
    config: TelegramConfig,
    client: Client,
}

impl TelegramNotifier {
    /// Create a new Telegram notifier
    pub fn new(config: TelegramConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    /// Create from environment
    pub fn from_env() -> Self {
        Self::new(TelegramConfig::from_env())
    }

    /// Send a message via Telegram Bot API
    async fn send_message(&self, text: &str, silent: bool) -> Result<()> {
        if !self.config.is_valid() {
            warn!("Telegram not configured, skipping notification");
            return Ok(());
        }

        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.config.bot_token
        );

        let request = SendMessageRequest {
            chat_id: &self.config.chat_id,
            text: text.to_string(),
            parse_mode: &self.config.parse_mode,
            disable_notification: if silent { Some(true) } else { None },
        };

        match self.client.post(&url).json(&request).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("📱 Telegram notification sent");
                    Ok(())
                } else {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    error!(status = %status, body = %body, "Telegram API error");
                    Ok(()) // Don't fail the operation for notification failures
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to send Telegram notification");
                Ok(()) // Don't fail the operation for notification failures
            }
        }
    }

    /// Format a match notification message
    fn format_match_message(match_entity: &Match, action: &MatchAction) -> String {
        let action_emoji = match action {
            MatchAction::AutoConfirm => "✅",
            MatchAction::SuggestToOperator => "💡",
            MatchAction::QueueForReview => "👀",
            MatchAction::Ignore => "⏭️",
        };

        let action_text = match action {
            MatchAction::AutoConfirm => "Auto-Confirmed",
            MatchAction::SuggestToOperator => "Suggested",
            MatchAction::QueueForReview => "Queued for Review",
            MatchAction::Ignore => "Ignored",
        };

        format!(
            "{} <b>New Match {}</b>\n\n\
             📊 <b>Score:</b> {:.1}%\n\
             📝 <b>Reasoning:</b> {}\n\n\
             <b>Offer:</b> {}\n\
             <b>Request:</b> {}\n\n\
             <code>ID: {}</code>",
            action_emoji,
            action_text,
            match_entity.score * 100.0,
            match_entity.reasoning,
            match_entity.offer_id,
            match_entity.request_id,
            match_entity.id,
        )
    }
}

#[async_trait]
impl MatchNotifier for TelegramNotifier {
    async fn notify_new_match(&self, match_entity: &Match, action: MatchAction) -> Result<()> {
        let message = Self::format_match_message(match_entity, &action);
        let silent = matches!(action, MatchAction::Ignore);
        self.send_message(&message, silent).await
    }

    async fn notify_auto_confirmed(&self, match_id: &str, score: f64) -> Result<()> {
        let message = format!(
            "✅ <b>Match Auto-Confirmed</b>\n\n\
             📊 Score: {:.1}%\n\
             <code>ID: {}</code>",
            score * 100.0,
            match_id
        );
        self.send_message(&message, false).await
    }

    async fn notify_suggested(&self, match_entity: &Match) -> Result<()> {
        let message = format!(
            "💡 <b>Match Suggested</b>\n\n\
             � {}\n\
             📊 Score: {:.1}%\n\
             <code>ID: {}</code>",
            match_entity.reasoning,
            match_entity.score * 100.0,
            match_entity.id
        );
        self.send_message(&message, false).await
    }

    async fn notify_queued_for_review(&self, match_id: &str, reason: &str) -> Result<()> {
        let message = format!(
            "👀 <b>Match Queued for Review</b>\n\n\
             📝 Reason: {}\n\
             <code>ID: {}</code>",
            reason, match_id
        );
        self.send_message(&message, false).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    use crate::domain::MatchStatus;

    fn test_match() -> Match {
        Match {
            id: "match-123".to_string(),
            offer_id: "offer-456".to_string(),
            request_id: "req-789".to_string(),
            score: 0.85,
            reasoning: "High similarity".to_string(),
            matched_by: Some("system".to_string()),
            status: MatchStatus::Pending,
            created_at: Utc::now(),
            confirmed_at: None,
            notes: None,
        }
    }

    #[test]
    fn test_format_match_message() {
        let match_entity = test_match();
        let message =
            TelegramNotifier::format_match_message(&match_entity, &MatchAction::SuggestToOperator);

        assert!(message.contains("💡"));
        assert!(message.contains("High similarity")); // reasoning field
        assert!(message.contains("85.0%"));
        assert!(message.contains("match-123"));
    }

    #[test]
    fn test_config_validation() {
        let config = TelegramConfig {
            bot_token: "123:ABC".to_string(),
            chat_id: "-1001234567890".to_string(),
            enabled: true,
            parse_mode: "HTML".to_string(),
        };
        assert!(config.is_valid());

        let disabled = TelegramConfig {
            enabled: false,
            ..config.clone()
        };
        assert!(!disabled.is_valid());
    }
}
