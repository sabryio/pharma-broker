//! Email notification sender
//!
//! Sends match notifications and reports via email.

use async_trait::async_trait;
use tracing::{info, warn};
use uuid::Uuid;

use crate::Result;
use crate::domain::Match;
use crate::matching::MatchAction;

use super::MatchNotifier;

/// Email configuration
#[derive(Debug, Clone)]
pub struct EmailConfig {
    /// SMTP server host
    pub smtp_host: String,
    /// SMTP server port
    pub smtp_port: u16,
    /// SMTP username
    pub username: String,
    /// SMTP password
    pub password: String,
    /// From email address
    pub from_address: String,
    /// From display name
    pub from_name: String,
    /// Recipients for notifications
    pub recipients: Vec<String>,
    /// Enable/disable notifications
    pub enabled: bool,
    /// Use TLS
    pub use_tls: bool,
}

impl EmailConfig {
    /// Create from environment variables
    pub fn from_env() -> Self {
        let recipients = std::env::var("EMAIL_RECIPIENTS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Self {
            smtp_host: std::env::var("SMTP_HOST").unwrap_or_else(|_| "localhost".to_string()),
            smtp_port: std::env::var("SMTP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(587),
            username: std::env::var("SMTP_USERNAME").unwrap_or_default(),
            password: std::env::var("SMTP_PASSWORD").unwrap_or_default(),
            from_address: std::env::var("EMAIL_FROM_ADDRESS")
                .unwrap_or_else(|_| "noreply@pharmabroker.local".to_string()),
            from_name: std::env::var("EMAIL_FROM_NAME")
                .unwrap_or_else(|_| "PharmaBroker".to_string()),
            recipients,
            enabled: std::env::var("EMAIL_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            use_tls: std::env::var("SMTP_USE_TLS")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),
        }
    }

    /// Check if properly configured
    pub fn is_valid(&self) -> bool {
        self.enabled && !self.smtp_host.is_empty() && !self.recipients.is_empty()
    }
}

/// Email notifier implementation
///
/// Note: For production, integrate with lettre crate for SMTP.
/// This implementation logs emails for development/testing.
pub struct EmailNotifier {
    config: EmailConfig,
}

impl EmailNotifier {
    /// Create a new Email notifier
    pub fn new(config: EmailConfig) -> Self {
        Self { config }
    }

    /// Create from environment
    pub fn from_env() -> Self {
        Self::new(EmailConfig::from_env())
    }

    /// Send an email (stub - logs for now)
    ///
    /// TODO: Integrate with lettre crate for actual SMTP sending
    async fn send_email(&self, subject: &str, html_body: &str) -> Result<()> {
        if !self.config.is_valid() {
            warn!("Email not configured, skipping notification");
            return Ok(());
        }

        // For now, just log the email
        // In production, use lettre crate:
        // let mailer = SmtpTransport::relay(&self.config.smtp_host)?
        //     .credentials(Credentials::new(username, password))
        //     .build();

        info!(
            subject = %subject,
            recipients = ?self.config.recipients,
            "📧 Email notification (stub): {}",
            subject
        );

        // Log body preview
        let preview = if html_body.len() > 100 {
            format!("{}...", &html_body[..100])
        } else {
            html_body.to_string()
        };
        info!(body_preview = %preview, "Email body preview");

        Ok(())
    }

    /// Format match notification as HTML email
    fn format_match_html(match_entity: &Match, action: &MatchAction) -> String {
        let action_color = match action {
            MatchAction::AutoConfirm => "#28a745",
            MatchAction::SuggestToOperator => "#17a2b8",
            MatchAction::QueueForReview => "#ffc107",
            MatchAction::Ignore => "#6c757d",
        };

        let action_text = match action {
            MatchAction::AutoConfirm => "Auto-Confirmed",
            MatchAction::SuggestToOperator => "Suggested",
            MatchAction::QueueForReview => "Queued for Review",
            MatchAction::Ignore => "Ignored",
        };

        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <style>
        body {{ font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto; }}
        .header {{ background: {}; color: white; padding: 20px; border-radius: 8px 8px 0 0; }}
        .content {{ padding: 20px; background: #f8f9fa; }}
        .field {{ margin: 10px 0; }}
        .label {{ font-weight: bold; color: #666; }}
        .value {{ color: #333; }}
        .footer {{ padding: 15px; background: #e9ecef; border-radius: 0 0 8px 8px; font-size: 12px; color: #666; }}
    </style>
</head>
<body>
    <div class="header">
        <h2>🔗 Match {}</h2>
    </div>
    <div class="content">
        <div class="field">
            <span class="label">Score:</span>
            <span class="value">{:.1}%</span>
        </div>
        <div class="field">
            <span class="label">Reasoning:</span>
            <span class="value">{}</span>
        </div>
        <div class="field">
            <span class="label">Offer ID:</span>
            <span class="value">{}</span>
        </div>
        <div class="field">
            <span class="label">Request ID:</span>
            <span class="value">{}</span>
        </div>
    </div>
    <div class="footer">
        Match ID: {} | PharmaBroker
    </div>
</body>
</html>"#,
            action_color,
            action_text,
            match_entity.score * 100.0,
            match_entity.reasoning_str(),
            match_entity.offer_id,
            match_entity.request_id,
            match_entity.id,
        )
    }
}

#[async_trait]
impl MatchNotifier for EmailNotifier {
    async fn notify_new_match(&self, match_entity: &Match, action: MatchAction) -> Result<()> {
        let subject = format!(
            "[PharmaBroker] Match {} - {}",
            match action {
                MatchAction::AutoConfirm => "Auto-Confirmed",
                MatchAction::SuggestToOperator => "Suggested",
                MatchAction::QueueForReview => "For Review",
                MatchAction::Ignore => "Ignored",
            },
            match_entity.id
        );
        let html = Self::format_match_html(match_entity, &action);
        self.send_email(&subject, &html).await
    }

    async fn notify_auto_confirmed(&self, match_id: Uuid, score: f64) -> Result<()> {
        let subject = format!(
            "[PharmaBroker] Match Auto-Confirmed (Score: {:.0}%)",
            score * 100.0
        );
        let html = format!(
            "<h2>✅ Match Auto-Confirmed</h2><p>Score: {:.1}%</p><p>ID: {}</p>",
            score * 100.0,
            match_id
        );
        self.send_email(&subject, &html).await
    }

    async fn notify_suggested(&self, match_entity: &Match) -> Result<()> {
        let subject = format!("[PharmaBroker] Match Suggested - {}", match_entity.id);
        let html = Self::format_match_html(match_entity, &MatchAction::SuggestToOperator);
        self.send_email(&subject, &html).await
    }

    async fn notify_queued_for_review(&self, match_id: Uuid, reason: &str) -> Result<()> {
        let subject = "[PharmaBroker] Match Queued for Review".to_string();
        let html = format!(
            "<h2>👀 Match Queued for Review</h2><p>Reason: {}</p><p>ID: {}</p>",
            reason, match_id
        );
        self.send_email(&subject, &html).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    use crate::domain::MatchStatus;

    fn test_match() -> Match {
        Match {
            id: Uuid::new_v4(),
            offer_id: Uuid::new_v4(),
            request_id: Uuid::new_v4(),
            score: 0.85,
            reasoning: Some("High similarity".to_string()),
            matched_by: Some("system".to_string()),
            status: MatchStatus::Pending,
            created_at: Utc::now(),
            confirmed_at: None,
            notes: None,
            ai_status: None,
            ai_confidence: None,
            ai_explanation: None,
        }
    }

    #[test]
    fn test_config_from_env() {
        // Note: Can't safely remove env vars in tests without unsafe
        let config = EmailConfig::from_env();
        // Just verify it doesn't panic
        let _ = config.is_valid();
    }

    #[test]
    fn test_format_match_html() {
        let match_entity = test_match();
        let html = EmailNotifier::format_match_html(&match_entity, &MatchAction::SuggestToOperator);

        assert!(html.contains("85.0%"));
        assert!(html.contains("#17a2b8")); // Suggest color
    }

    #[test]
    fn test_config_validation() {
        let config = EmailConfig {
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 587,
            username: "user".to_string(),
            password: "pass".to_string(),
            from_address: "test@example.com".to_string(),
            from_name: "Test".to_string(),
            recipients: vec!["admin@example.com".to_string()],
            enabled: true,
            use_tls: true,
        };
        assert!(config.is_valid());

        let no_recipients = EmailConfig {
            recipients: vec![],
            ..config.clone()
        };
        assert!(!no_recipients.is_valid());
    }
}
