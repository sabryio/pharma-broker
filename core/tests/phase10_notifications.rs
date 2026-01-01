//! Phase 10: Notifications Integration Tests
//!
//! Tests for Telegram and Email notifiers.
//! See: docs/phases/10-notifications.md

use chrono::Utc;

use pharma_core::domain::{Match, MatchStatus};
use pharma_core::matching::MatchAction;
use pharma_core::notify::{
    CompositeNotifier, EmailConfig, EmailNotifier, MatchNotifier, NullNotifier, TelegramConfig,
    TelegramNotifier,
};
use uuid::Uuid;

/// Create a test match
fn test_match() -> Match {
    Match {
        id: Uuid::new_v4(),
        offer_id: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        score: 0.85,
        reasoning: Some("High medication similarity".to_string()),
        matched_by: Some("AUTO".to_string()),
        status: MatchStatus::Pending,
        created_at: Utc::now(),
        confirmed_at: None,
        notes: None,
        ai_status: None,
        ai_confidence: None,
        ai_explanation: None,
    }
}

/// Test TelegramConfig from_env (doesn't panic)
#[test]
fn test_telegram_config_from_env() {
    let config = TelegramConfig::from_env();

    // Default should be disabled when env vars not set
    assert!(!config.enabled, "Should be disabled by default");
    assert_eq!(config.parse_mode, "HTML");
}

/// Test TelegramConfig is_valid
#[test]
fn test_telegram_config_validation() {
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

/// Test EmailConfig from_env (doesn't panic)
#[test]
fn test_email_config_from_env() {
    let config = EmailConfig::from_env();

    // Default should be disabled when env vars not set
    assert!(!config.enabled, "Should be disabled by default");
    assert_eq!(config.smtp_port, 587, "Default SMTP port should be 587");
    assert!(config.use_tls, "TLS should be enabled by default");
}

/// Test EmailConfig is_valid
#[test]
fn test_email_config_validation() {
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

/// Test NullNotifier does nothing
#[tokio::test]
async fn test_null_notifier() {
    let notifier = NullNotifier;
    let match_entity = test_match();

    // Should not fail
    let result = notifier
        .notify_new_match(&match_entity, MatchAction::SuggestToOperator)
        .await;
    assert!(result.is_ok());

    let result = notifier.notify_auto_confirmed(Uuid::new_v4(), 0.95).await;
    assert!(result.is_ok());
}

/// Test CompositeNotifier chains notifiers
#[tokio::test]
async fn test_composite_notifier() {
    let notifier = CompositeNotifier::new()
        .with_notifier(NullNotifier)
        .with_notifier(NullNotifier);

    // Should call both notifiers without error
    let result = notifier
        .notify_new_match(&test_match(), MatchAction::AutoConfirm)
        .await;
    assert!(result.is_ok());
}

/// Test disabled Telegram notifier doesn't fail
#[tokio::test]
async fn test_disabled_telegram_notifier() {
    let config = TelegramConfig {
        enabled: false,
        bot_token: String::new(),
        chat_id: String::new(),
        parse_mode: "HTML".to_string(),
    };
    let notifier = TelegramNotifier::new(config);

    let result = notifier
        .notify_new_match(&test_match(), MatchAction::SuggestToOperator)
        .await;
    assert!(result.is_ok(), "Disabled notifier should succeed silently");
}

/// Test disabled Email notifier doesn't fail
#[tokio::test]
async fn test_disabled_email_notifier() {
    let config = EmailConfig {
        enabled: false,
        smtp_host: String::new(),
        smtp_port: 587,
        username: String::new(),
        password: String::new(),
        from_address: String::new(),
        from_name: String::new(),
        recipients: vec![],
        use_tls: true,
    };
    let notifier = EmailNotifier::new(config);

    let result = notifier
        .notify_new_match(&test_match(), MatchAction::AutoConfirm)
        .await;
    assert!(result.is_ok(), "Disabled notifier should succeed silently");
}
