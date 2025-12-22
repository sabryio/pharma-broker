//! Test infrastructure for repository integration tests
//!
//! Uses pharma_db's TestDb which handles container lifecycle and migrations.
//! This module provides domain-level test factories that return domain types
//! (as opposed to pharma_db::testing which returns ActiveModel types).
//!
//! Mirrors: legacy/storage/gorm/testing.go

use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

use crate::domain::{
    AuditLog, FeedbackRecord, Group, ItemStatus, Match, MatchStatus, Offer, RawMessage, Request,
    ReviewQueueItem, ReviewStatus, UrgencyLevel, WeightHistory,
};

// Re-export pharma_db's TestDb for SeaORM-based tests
pub use pharma_db::testing::TestDb as SeaOrmTestDb;

/// Test database wrapper for sqlx-based repository tests
/// Uses pharma_db migrations via SeaORM, then provides a sqlx pool
pub struct TestDb {
    pub pool: PgPool,
    _seaorm_db: SeaOrmTestDb,
}

impl TestDb {
    /// Creates a new test database with pgvector PostgreSQL
    /// Uses pharma_db's TestDb for container and migration management,
    /// then creates a sqlx pool pointing to the same database
    pub async fn new() -> Self {
        // Use pharma_db's TestDb which handles container + migrations
        let seaorm_db = SeaOrmTestDb::new().await;

        // Extract connection URL from SeaORM connection
        // SeaORM's DatabaseConnection doesn't expose the URL directly,
        // so we need to get it from the underlying sqlx pool
        let seaorm_pool = seaorm_db
            .db
            .get_postgres_connection_pool()
            .expect("Expected Postgres connection");

        // Create our own sqlx pool using the same connection options
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(seaorm_pool.connect_options().clone())
            .await
            .expect("Failed to create sqlx pool");

        TestDb {
            pool,
            _seaorm_db: seaorm_db,
        }
    }

    /// Get the underlying SeaORM database connection
    pub fn seaorm_db(&self) -> &sea_orm::DatabaseConnection {
        &self._seaorm_db.db
    }
}

// =============================================================================
// Test Data Factories (mirrors Go testing.go)
// Returns domain types for use in repository tests
// =============================================================================

/// Creates a test RawMessage with default values
/// Mirrors: NewTestRawMessage in Go
pub fn new_test_raw_message() -> RawMessage {
    RawMessage {
        id: Uuid::new_v4().to_string(),
        external_id: Some(Uuid::new_v4().to_string()),
        group_jid: "test-group@g.us".to_string(),
        group_name: "Test Group".to_string(),
        sender_jid: "sender@s.whatsapp.net".to_string(),
        sender_phone: Some("+201234567890".to_string()),
        sender_name: Some("Test Sender".to_string()),
        content: "Test message content".to_string(),
        timestamp: Utc::now(),
        processed_at: None,
        error: None,
        reply_to_id: None,
        reply_to_content: None,
        reply_to_sender: None,
        created_at: Utc::now(),
    }
}

/// Creates a test Offer with default values
/// Mirrors: NewTestOffer in Go
pub fn new_test_offer(raw_message_id: &str) -> Offer {
    let now = Utc::now();
    Offer {
        id: Uuid::new_v4().to_string(),
        raw_message_id: raw_message_id.to_string(),
        source_phone: "+201234567890".to_string(),
        source_name: Some("Test Seller".to_string()),
        source_group: "test-group@g.us".to_string(),
        medication: "Augmentin 1g".to_string(),
        medication_raw: "أوجمنتين 1 جم".to_string(),
        quantity: Some(rust_decimal::Decimal::from(50)),
        unit: Some("boxes".to_string()),
        price: Some(rust_decimal::Decimal::from(150)),
        currency: Some("EGP".to_string()),
        expiry_date: None,
        batch_number: None,
        notes: None,
        status: ItemStatus::Active,
        content_embedding: None,
        urgency_level: UrgencyLevel::Normal,
        expiry_info: None,
        ai_confidence: 0.9,
        created_at: now,
        updated_at: now,
    }
}

/// Creates a test Request with default values
/// Mirrors: NewTestRequest in Go
pub fn new_test_request(raw_message_id: &str) -> Request {
    let now = Utc::now();
    Request {
        id: Uuid::new_v4().to_string(),
        raw_message_id: raw_message_id.to_string(),
        source_phone: "+201098765432".to_string(),
        source_name: Some("Test Buyer".to_string()),
        source_group: "test-group@g.us".to_string(),
        medication: "Augmentin 1g".to_string(),
        medication_raw: "أوجمنتين 1 جرام".to_string(),
        quantity: Some(rust_decimal::Decimal::from(20)),
        unit: Some("boxes".to_string()),
        max_price: Some(rust_decimal::Decimal::from(160)),
        currency: Some("EGP".to_string()),
        urgency_level: UrgencyLevel::Normal,
        expiry_requirement: None,
        ai_confidence: 0.9,
        notes: None,
        status: ItemStatus::Active,
        content_embedding: None,
        created_at: now,
        updated_at: now,
    }
}

/// Creates a test Match with default values
/// Mirrors: NewTestMatch in Go
pub fn new_test_match(offer_id: &str, request_id: &str) -> Match {
    Match {
        id: Uuid::new_v4().to_string(),
        offer_id: offer_id.to_string(),
        request_id: request_id.to_string(),
        score: 0.85,
        reasoning: Some("Strong medication match".to_string()),
        matched_by: Some("AUTO".to_string()),
        status: MatchStatus::Pending,
        created_at: Utc::now(),
        confirmed_at: None,
        notes: None,
    }
}

/// Creates a test Group with default values
/// Mirrors: NewTestGroup in Go
pub fn new_test_group() -> Group {
    Group {
        jid: format!("{}@g.us", Uuid::new_v4()),
        name: "Test Group".to_string(),
        description: Some("Test group description".to_string()),
        monitored: true,
        added_at: Utc::now(),
        last_message: None,
        message_count: 0,
    }
}

/// Creates a test AuditLog with default values
pub fn new_test_audit_log(action: &str, entity_id: &str) -> AuditLog {
    AuditLog {
        id: Uuid::new_v4(),
        action: action.to_string(),
        entity_type: "match".to_string(),
        entity_id: entity_id.to_string(),
        actor: "test-user".to_string(),
        details: None,
        ip_address: None,
        user_agent: None,
        created_at: Utc::now(),
    }
}

/// Creates a test FeedbackRecord with default values
pub fn new_test_feedback_record(match_id: impl ToString, confirmed: bool) -> FeedbackRecord {
    FeedbackRecord {
        id: Uuid::new_v4(),
        match_id: match_id.to_string(),
        user_id: "test-user".to_string(),
        confirmed,
        medication_score: 0.9,
        dosage_score: 0.8,
        quantity_score: 0.7,
        price_score: 0.6,
        recency_score: 0.5,
        total_score: 0.75,
        created_at: Utc::now(),
    }
}

/// Creates a test WeightHistory with default values
pub fn new_test_weight_history(source: &str) -> WeightHistory {
    WeightHistory {
        id: Uuid::new_v4(),
        medication_weight: 0.35,
        dosage_weight: 0.20,
        quantity_weight: 0.15,
        price_weight: 0.15,
        recency_weight: 0.15,
        source: source.to_string(),
        sample_count: 100,
        created_at: Utc::now(),
    }
}

/// Creates a test ReviewQueueItem with default values
pub fn new_test_review_queue_item(raw_message_id: &str) -> ReviewQueueItem {
    ReviewQueueItem {
        id: Uuid::new_v4(),
        raw_message_id: raw_message_id.to_string(),
        ai_result: serde_json::json!({"items": []}),
        confidence: 0.45,
        reason: "low_confidence".to_string(),
        status: ReviewStatus::Pending,
        reviewed_by: None,
        review_notes: None,
        created_at: Utc::now(),
        reviewed_at: None,
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Helper to create embedding vector with specific values in first dimensions
pub fn make_embedding(vals: &[f32]) -> Vec<f32> {
    let mut v = vec![0.0f32; 768];
    for (i, val) in vals.iter().enumerate() {
        if i < 768 {
            v[i] = *val;
        }
    }
    v
}

/// Helper to make uniform embedding (all same value)
pub fn make_uniform_embedding(val: f32) -> Vec<f32> {
    vec![val; 768]
}

/// Helper to create a timestamp offset from now
pub fn time_ago(duration: Duration) -> DateTime<Utc> {
    Utc::now() - duration
}
