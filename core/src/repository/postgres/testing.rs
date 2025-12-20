//! Test infrastructure for repository integration tests
//! Uses testcontainers to spin up a pgvector-enabled PostgreSQL container
//!
//! Mirrors: legacy/storage/gorm/testing.go

use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::sync::atomic::{AtomicU64, Ordering};
use testcontainers::{
    ContainerAsync, GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use uuid::Uuid;

use crate::domain::{
    AuditLog, FeedbackRecord, Group, ItemStatus, Match, MatchStatus, Offer, RawMessage, Request,
    ReviewQueueItem, ReviewStatus, WeightHistory,
};

/// Global counter for unique schema names
static SCHEMA_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Test database wrapper with container lifecycle management
/// Each test gets its own schema for isolation
pub struct TestDb {
    pub pool: PgPool,
    _schema_name: String,
    _container: ContainerAsync<GenericImage>,
}

impl TestDb {
    /// Creates a new test database with pgvector PostgreSQL
    /// Uses pgvector/pgvector:pg18-trixie for full vector extension support (same as Go tests)
    /// Each test gets a unique schema for isolation when running concurrently
    pub async fn new() -> Self {
        // Generate unique schema name using counter + random UUID to avoid collisions
        // with reused containers from previous test runs
        let counter = SCHEMA_COUNTER.fetch_add(1, Ordering::SeqCst);
        let unique_id = Uuid::new_v4().simple().to_string();
        let schema_name = format!("t{}_{}", counter, &unique_id[..8]);

        // Start PostgreSQL container with pgvector support
        // Using the official pgvector image (matches Go: pgvector/pgvector:pg18-trixie)
        let container = GenericImage::new("pgvector/pgvector", "pg18-trixie")
            .with_exposed_port(5432.tcp())
            .with_wait_for(WaitFor::message_on_stderr(
                "database system is ready to accept connections",
            ))
            .with_env_var("POSTGRES_USER", "postgres")
            .with_env_var("POSTGRES_PASSWORD", "password")
            .with_env_var("POSTGRES_DB", "pharmabroker_test")
            .with_container_name("pharmabroker-test-postgres")
            .with_reuse(testcontainers::core::ReuseDirective::Always)
            .start()
            .await
            .expect("Failed to start PostgreSQL container");

        let host_port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get container port");

        // First connect without schema to create it
        let base_connection_string = format!(
            "postgres://postgres:password@127.0.0.1:{}/pharmabroker_test",
            host_port
        );

        let setup_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&base_connection_string)
            .await
            .expect("Failed to connect to test database for setup");

        // Create unique schema for this test
        Self::setup_schema(&setup_pool, &schema_name).await;

        // Close setup pool
        setup_pool.close().await;

        // Now connect with search_path set to our schema via connection options
        let connection_string = format!(
            "postgres://postgres:password@127.0.0.1:{}/pharmabroker_test?options=-c%20search_path%3D{},public",
            host_port, schema_name
        );

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&connection_string)
            .await
            .expect("Failed to connect to test database with schema");

        TestDb {
            pool,
            _schema_name: schema_name,
            _container: container,
        }
    }

    /// Sets up the database schema and extensions in a unique schema
    async fn setup_schema(pool: &PgPool, schema_name: &str) {
        // Enable pgvector extension (only needs to be done once per database)
        sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(pool)
            .await
            .expect("Failed to create vector extension");

        // Enable pg_trgm extension for fuzzy search
        sqlx::query("CREATE EXTENSION IF NOT EXISTS pg_trgm")
            .execute(pool)
            .await
            .expect("Failed to create pg_trgm extension");

        // Create unique schema for this test
        let create_schema = format!("CREATE SCHEMA IF NOT EXISTS {}", schema_name);
        sqlx::query(&create_schema)
            .execute(pool)
            .await
            .expect("Failed to create test schema");

        // Create raw_messages table
        sqlx::query(&format!(
            r#"
            CREATE TABLE {}.raw_messages (
                id TEXT PRIMARY KEY,
                external_id TEXT UNIQUE NOT NULL,
                group_jid TEXT NOT NULL,
                group_name TEXT NOT NULL,
                sender_jid TEXT NOT NULL,
                sender_phone TEXT NOT NULL,
                sender_name TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL,
                processed_at TIMESTAMPTZ,
                error TEXT,
                reply_to_id TEXT,
                reply_to_content TEXT,
                reply_to_sender TEXT
            )
            "#,
            schema_name
        ))
        .execute(pool)
        .await
        .expect("Failed to create raw_messages table");

        // Create offers table
        sqlx::query(&format!(
            r#"
            CREATE TABLE {}.offers (
                id TEXT PRIMARY KEY,
                raw_message_id TEXT REFERENCES {}.raw_messages(id),
                source_phone TEXT NOT NULL,
                source_name TEXT NOT NULL,
                source_group TEXT NOT NULL,
                group_name TEXT NOT NULL,
                medication TEXT NOT NULL,
                medication_raw TEXT NOT NULL,
                quantity DOUBLE PRECISION NOT NULL DEFAULT 0,
                unit TEXT,
                price DOUBLE PRECISION NOT NULL DEFAULT 0,
                currency TEXT,
                expiry_date TIMESTAMPTZ,
                batch_number TEXT,
                notes TEXT,
                raw_message TEXT NOT NULL,
                status VARCHAR NOT NULL DEFAULT 'ACTIVE',
                content_embedding vector(768),
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            schema_name, schema_name
        ))
        .execute(pool)
        .await
        .expect("Failed to create offers table");

        // Create requests table
        sqlx::query(&format!(
            r#"
            CREATE TABLE {}.requests (
                id TEXT PRIMARY KEY,
                raw_message_id TEXT REFERENCES {}.raw_messages(id),
                source_phone TEXT NOT NULL,
                source_name TEXT NOT NULL,
                source_group TEXT NOT NULL,
                group_name TEXT NOT NULL,
                medication TEXT NOT NULL,
                medication_raw TEXT NOT NULL,
                quantity DOUBLE PRECISION NOT NULL DEFAULT 0,
                unit TEXT,
                max_price DOUBLE PRECISION NOT NULL DEFAULT 0,
                currency TEXT,
                urgent BOOLEAN NOT NULL DEFAULT FALSE,
                notes TEXT,
                raw_message TEXT NOT NULL,
                status VARCHAR NOT NULL DEFAULT 'ACTIVE',
                content_embedding vector(768),
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            schema_name, schema_name
        ))
        .execute(pool)
        .await
        .expect("Failed to create requests table");

        // Create matches table
        sqlx::query(&format!(
            r#"
            CREATE TABLE {}.matches (
                id TEXT PRIMARY KEY,
                offer_id TEXT NOT NULL REFERENCES {}.offers(id),
                request_id TEXT NOT NULL REFERENCES {}.requests(id),
                score DOUBLE PRECISION NOT NULL,
                reasoning TEXT NOT NULL,
                matched_by TEXT,
                status VARCHAR NOT NULL DEFAULT 'PENDING',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                confirmed_at TIMESTAMPTZ,
                notes TEXT,
                UNIQUE(offer_id, request_id)
            )
            "#,
            schema_name, schema_name, schema_name
        ))
        .execute(pool)
        .await
        .expect("Failed to create matches table");

        // Create groups table
        sqlx::query(&format!(
            r#"
            CREATE TABLE {}.groups (
                jid TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                monitored BOOLEAN NOT NULL DEFAULT FALSE,
                added_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                last_message TIMESTAMPTZ,
                message_count BIGINT NOT NULL DEFAULT 0
            )
            "#,
            schema_name
        ))
        .execute(pool)
        .await
        .expect("Failed to create groups table");

        // Create medication_mappings table
        sqlx::query(&format!(
            r#"
            CREATE TABLE {}.medication_mappings (
                id TEXT PRIMARY KEY,
                arabic_name TEXT NOT NULL,
                english_name TEXT NOT NULL,
                synonyms TEXT[],
                embedding vector(768),
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            schema_name
        ))
        .execute(pool)
        .await
        .expect("Failed to create medication_mappings table");

        // Create audit_logs table
        sqlx::query(&format!(
            r#"
            CREATE TABLE {}.audit_logs (
                id UUID PRIMARY KEY,
                action TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                actor TEXT NOT NULL,
                details JSONB,
                ip_address TEXT,
                user_agent TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            schema_name
        ))
        .execute(pool)
        .await
        .expect("Failed to create audit_logs table");

        // Create feedback_records table
        sqlx::query(&format!(
            r#"
            CREATE TABLE {}.feedback_records (
                id UUID PRIMARY KEY,
                match_id UUID NOT NULL,
                user_id TEXT NOT NULL,
                confirmed BOOLEAN NOT NULL,
                medication_score DOUBLE PRECISION NOT NULL,
                dosage_score DOUBLE PRECISION NOT NULL,
                quantity_score DOUBLE PRECISION NOT NULL,
                price_score DOUBLE PRECISION NOT NULL,
                recency_score DOUBLE PRECISION NOT NULL,
                total_score DOUBLE PRECISION NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            schema_name
        ))
        .execute(pool)
        .await
        .expect("Failed to create feedback_records table");

        // Create weight_history table
        sqlx::query(&format!(
            r#"
            CREATE TABLE {}.weight_history (
                id UUID PRIMARY KEY,
                medication_weight DOUBLE PRECISION NOT NULL,
                dosage_weight DOUBLE PRECISION NOT NULL,
                quantity_weight DOUBLE PRECISION NOT NULL,
                price_weight DOUBLE PRECISION NOT NULL,
                recency_weight DOUBLE PRECISION NOT NULL,
                source TEXT NOT NULL,
                sample_count INTEGER NOT NULL DEFAULT 0,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            schema_name
        ))
        .execute(pool)
        .await
        .expect("Failed to create weight_history table");

        // Create review_queue table
        sqlx::query(&format!(
            r#"
            CREATE TABLE {}.review_queue (
                id UUID PRIMARY KEY,
                raw_message_id TEXT NOT NULL,
                ai_result JSONB NOT NULL,
                confidence DOUBLE PRECISION NOT NULL,
                reason TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                reviewed_by TEXT,
                review_notes TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                reviewed_at TIMESTAMPTZ
            )
            "#,
            schema_name
        ))
        .execute(pool)
        .await
        .expect("Failed to create review_queue table");

        // Create match_queue_items table
        sqlx::query(&format!(
            r#"
            CREATE TABLE {}.match_queue_items (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                request_id UUID NOT NULL,
                status TEXT NOT NULL DEFAULT 'PENDING',
                priority INTEGER NOT NULL DEFAULT 0,
                attempts INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            schema_name
        ))
        .execute(pool)
        .await
        .expect("Failed to create match_queue_items table");

        // Create trigram indexes for fuzzy search
        sqlx::query(&format!(
            r#"
            CREATE INDEX IF NOT EXISTS idx_medication_mappings_arabic_trgm 
            ON {}.medication_mappings USING GIN(arabic_name gin_trgm_ops)
            "#,
            schema_name
        ))
        .execute(pool)
        .await
        .ok();

        sqlx::query(&format!(
            r#"
            CREATE INDEX IF NOT EXISTS idx_medication_mappings_english_trgm 
            ON {}.medication_mappings USING GIN(english_name gin_trgm_ops)
            "#,
            schema_name
        ))
        .execute(pool)
        .await
        .ok();
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        // Schema cleanup happens asynchronously - we can't await in Drop
        // The schema will be cleaned up on next test run or container restart
        // This is acceptable for test isolation
    }
}

// =============================================================================
// Test Data Factories (mirrors Go testing.go)
// =============================================================================

/// Creates a test RawMessage with default values
/// Mirrors: NewTestRawMessage in Go
pub fn new_test_raw_message() -> RawMessage {
    RawMessage {
        id: Uuid::new_v4().to_string(),
        external_id: Uuid::new_v4().to_string(),
        group_jid: "test-group@g.us".to_string(),
        group_name: "Test Group".to_string(),
        sender_jid: "sender@s.whatsapp.net".to_string(),
        sender_phone: "+201234567890".to_string(),
        sender_name: "Test Sender".to_string(),
        content: "Test message content".to_string(),
        timestamp: Utc::now(),
        processed_at: None,
        error: None,
        reply_to_id: None,
        reply_to_content: None,
        reply_to_sender: None,
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
        source_name: "Test Seller".to_string(),
        source_group: "test-group@g.us".to_string(),
        group_name: "Test Group".to_string(),
        medication: "Augmentin 1g".to_string(),
        medication_raw: "أوجمنتين 1 جم".to_string(),
        quantity: 50.0,
        unit: Some("boxes".to_string()),
        price: 150.0,
        currency: Some("EGP".to_string()),
        expiry_date: None,
        batch_number: None,
        notes: None,
        raw_message: "للبيع: Augmentin 1g - 50 علبة".to_string(),
        status: ItemStatus::Active,
        content_embedding: None,
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
        source_name: "Test Buyer".to_string(),
        source_group: "test-group@g.us".to_string(),
        group_name: "Test Group".to_string(),
        medication: "Augmentin 1g".to_string(),
        medication_raw: "أوجمنتين 1 جرام".to_string(),
        quantity: 20.0,
        unit: Some("boxes".to_string()),
        max_price: 160.0,
        currency: Some("EGP".to_string()),
        urgent: false,
        notes: None,
        raw_message: "مطلوب: أوجمنتين 1 جرام - 20 علبة".to_string(),
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
        reasoning: "Strong medication match".to_string(),
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
pub fn new_test_feedback_record(match_id: Uuid, confirmed: bool) -> FeedbackRecord {
    FeedbackRecord {
        id: Uuid::new_v4(),
        match_id,
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
