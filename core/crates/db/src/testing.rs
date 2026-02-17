#![cfg(feature = "integration-tests")]
//! Test infrastructure for repository integration tests
//! Uses testcontainers to spin up a pgvector-enabled PostgreSQL container

use sea_orm::{Database, DatabaseConnection};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use testcontainers::{
    ContainerAsync, GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use uuid::Uuid;

use crate::migration::Migrator;
use sea_orm_migration::MigratorTrait;

/// Global counter for unique schema names
static SCHEMA_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Test database wrapper with container lifecycle management
pub struct TestDb {
    pub db: Arc<DatabaseConnection>,
    _schema_name: String,
    _container: ContainerAsync<GenericImage>,
}

impl TestDb {
    /// Creates a new test database with pgvector PostgreSQL
    pub async fn new() -> Self {
        let counter = SCHEMA_COUNTER.fetch_add(1, Ordering::SeqCst);
        let unique_id = Uuid::new_v4().simple().to_string();
        let schema_name = format!("t{}_{}", counter, &unique_id[..8]);

        let container = GenericImage::new("pgvector/pgvector", "pg18-trixie")
            .with_exposed_port(5432.tcp())
            .with_wait_for(WaitFor::message_on_stderr(
                "database system is ready to accept connections",
            ))
            .with_env_var("POSTGRES_USER", "postgres")
            .with_env_var("POSTGRES_PASSWORD", "password")
            .with_env_var("POSTGRES_DB", "pharmabroker_test")
            .start()
            .await
            .expect("Failed to start PostgreSQL container");

        let host_port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get container port");

        let connection_string = format!(
            "postgres://postgres:password@127.0.0.1:{}/pharmabroker_test?sslmode=disable&options=-c%20search_path%3D{},public",
            host_port, schema_name
        );

        // Connect to create schema first
        let base_url = format!(
            "postgres://postgres:password@127.0.0.1:{}/pharmabroker_test?sslmode=disable",
            host_port
        );
        let mut setup_opt = sea_orm::ConnectOptions::new(base_url);
        setup_opt.sqlx_logging(false);
        let setup_db = Database::connect(setup_opt)
            .await
            .expect("Failed to connect for setup");

        // Create extensions and schema
        Self::setup_schema(&setup_db, &schema_name).await;
        setup_db.close().await.ok();

        // Connect with schema in search path
        let mut opt = sea_orm::ConnectOptions::new(connection_string);
        opt.sqlx_logging(false);
        let db = Database::connect(opt)
            .await
            .expect("Failed to connect to test database");

        // Run migrations
        Migrator::up(&db, None)
            .await
            .expect("Failed to run migrations");

        TestDb {
            db: Arc::new(db),
            _schema_name: schema_name,
            _container: container,
        }
    }

    async fn setup_schema(db: &DatabaseConnection, schema_name: &str) {
        use sea_orm::{ConnectionTrait, Statement};

        db.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "CREATE EXTENSION IF NOT EXISTS vector".to_string(),
        ))
        .await
        .expect("Failed to create vector extension");

        db.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "CREATE EXTENSION IF NOT EXISTS pg_trgm".to_string(),
        ))
        .await
        .expect("Failed to create pg_trgm extension");

        db.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!("CREATE SCHEMA IF NOT EXISTS {}", schema_name),
        ))
        .await
        .expect("Failed to create test schema");
    }
}

// =============================================================================
// Test Data Factories
// =============================================================================

use chrono::Utc;
use sea_orm::ActiveValue::Set;

use crate::entity::{
    audit_log, feedback_record, group, match_, match_queue, medication_master, offer, raw_message,
    request, review_queue, weight_history,
};

/// Creates a test group ActiveModel
pub fn new_test_group(jid: &str, name: &str, monitored: bool) -> group::ActiveModel {
    group::ActiveModel {
        id: Set(Uuid::new_v4()),
        jid: Set(jid.to_string()),
        name: Set(name.to_string()),
        description: Set(Some("Test group".to_string())),
        monitored: Set(monitored),
        added_at: Set(Utc::now()),
        last_message: Set(None),
        message_count: Set(0),
    }
}

/// Creates a test participant ActiveModel
pub fn new_test_participant(jid: &str, phone: &str) -> participant::ActiveModel {
    participant::ActiveModel {
        id: Set(Uuid::new_v4()),
        jid: Set(jid.to_string()),
        phone: Set(phone.to_string()),
        push_name: Set(Some("Test User".to_string())),
        display_name: Set(None),
        label: Set(None),
        notes: Set(None),
        is_blocked: Set(false),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
}

/// Creates a test raw_message ActiveModel
pub fn new_test_raw_message(participant_id: Uuid, group_id: Uuid) -> raw_message::ActiveModel {
    let id = Uuid::new_v4();
    raw_message::ActiveModel {
        id: Set(id),
        external_id: Set(Some(Uuid::new_v4().to_string())),
        participant_id: Set(participant_id),
        group_id: Set(group_id),
        content: Set("Test message content".to_string()),
        timestamp: Set(Utc::now()),
        processed_at: Set(None),
        error: Set(None),
        reply_to_id: Set(None),
        reply_to_content: Set(None),
        reply_to_sender: Set(None),
        created_at: Set(Utc::now()),
    }
}

/// Creates a test offer ActiveModel
pub fn new_test_offer(
    raw_message_id: Uuid,
    participant_id: Uuid,
    group_id: Uuid,
) -> offer::ActiveModel {
    let now = Utc::now();
    offer::ActiveModel {
        id: Set(Uuid::new_v4()),
        raw_message_id: Set(raw_message_id),
        participant_id: Set(participant_id),
        group_id: Set(group_id),
        medication: Set("Augmentin 1g".to_string()),
        medication_raw: Set("أوجمنتين 1 جم".to_string()),
        quantity: Set(Some(rust_decimal::Decimal::new(5000, 2))), // 50.00
        unit: Set(Some("boxes".to_string())),
        price: Set(Some(rust_decimal::Decimal::new(15000, 2))), // 150.00
        currency: Set(Some("EGP".to_string())),
        expiry_date: Set(None),
        batch_number: Set(None),
        notes: Set(None),
        status: Set(offer::Status::Active),
        urgency_level: Set(offer::UrgencyLevel::Normal),
        expiry_info: Set(None),
        ai_confidence: Set(0.9),
        content_embedding: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
}

/// Creates a test request ActiveModel
pub fn new_test_request(
    raw_message_id: Uuid,
    participant_id: Uuid,
    group_id: Uuid,
) -> request::ActiveModel {
    let now = Utc::now();
    request::ActiveModel {
        id: Set(Uuid::new_v4()),
        raw_message_id: Set(raw_message_id),
        participant_id: Set(participant_id),
        group_id: Set(group_id),
        medication: Set("Augmentin 1g".to_string()),
        medication_raw: Set("أوجمنتين 1 جرام".to_string()),
        quantity: Set(Some(rust_decimal::Decimal::new(2000, 2))), // 20.00
        unit: Set(Some("boxes".to_string())),
        max_price: Set(Some(rust_decimal::Decimal::new(16000, 2))), // 160.00
        currency: Set(Some("EGP".to_string())),
        urgency_level: Set(offer::UrgencyLevel::Normal),
        expiry_requirement: Set(None),
        ai_confidence: Set(0.9),
        notes: Set(None),
        status: Set(offer::Status::Active),
        content_embedding: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
}

/// Creates a test match ActiveModel
pub fn new_test_match(offer_id: Uuid, request_id: Uuid) -> match_::ActiveModel {
    match_::ActiveModel {
        id: Set(Uuid::new_v4()),
        offer_id: Set(offer_id),
        request_id: Set(request_id),
        score: Set(0.85),
        reasoning: Set(Some("Strong medication match".to_string())),
        matched_by: Set(Some("AUTO".to_string())),
        status: Set(match_::MatchStatus::Pending),
        created_at: Set(Utc::now()),
        confirmed_at: Set(None),
        notes: Set(None),
    }
}

/// Creates a test audit_log ActiveModel
pub fn new_test_audit_log(action: &str, entity_id: &str) -> audit_log::ActiveModel {
    audit_log::ActiveModel {
        id: Set(Uuid::new_v4()),
        action: Set(action.to_string()),
        entity_type: Set("match".to_string()),
        entity_id: Set(entity_id.to_string()),
        actor: Set("test-user".to_string()),
        details: Set(None),
        ip_address: Set(None),
        user_agent: Set(None),
        created_at: Set(Utc::now()),
    }
}

/// Creates a test feedback_record ActiveModel
pub fn new_test_feedback(match_id: Uuid, confirmed: bool) -> feedback_record::ActiveModel {
    feedback_record::ActiveModel {
        id: Set(Uuid::new_v4()),
        match_id: Set(match_id),
        user_id: Set("test-user".to_string()),
        confirmed: Set(confirmed),
        medication_score: Set(0.9),
        dosage_score: Set(0.8),
        quantity_score: Set(0.7),
        price_score: Set(0.6),
        recency_score: Set(0.5),
        ai_logic_score: Set(0.0),
        total_score: Set(0.75),
        created_at: Set(Utc::now()),
    }
}

/// Creates a test weight_history ActiveModel
pub fn new_test_weight_history(source: &str) -> weight_history::ActiveModel {
    weight_history::ActiveModel {
        id: Set(Uuid::new_v4()),
        medication_weight: Set(0.40),
        recency_weight: Set(0.20),
        ai_logic_weight: Set(0.0),
        pharmaceutical_weight: Set(0.20),
        expiry_weight: Set(0.10),
        supplier_weight: Set(0.10),
        source: Set(source.to_string()),
        sample_count: Set(100),
        created_at: Set(Utc::now()),
    }
}

/// Creates a test review_queue ActiveModel
pub fn new_test_review_queue(raw_message_id: Uuid) -> review_queue::ActiveModel {
    review_queue::ActiveModel {
        id: Set(Uuid::new_v4()),
        raw_message_id: Set(raw_message_id),
        ai_result: Set(serde_json::json!({"items": []})),
        confidence: Set(0.45),
        reason: Set("low_confidence".to_string()),
        status: Set(review_queue::ReviewStatus::Pending),
        reviewed_by: Set(None),
        review_notes: Set(None),
        created_at: Set(Utc::now()),
        reviewed_at: Set(None),
    }
}

/// Creates a test match_queue ActiveModel
pub fn new_test_match_queue(request_id: Uuid) -> match_queue::ActiveModel {
    let now = Utc::now();
    match_queue::ActiveModel {
        id: Set(Uuid::new_v4()),
        request_id: Set(request_id),
        status: Set(match_queue::QueueStatus::Pending),
        priority: Set(0),
        attempts: Set(0),
        last_error: Set(None),
        next_attempt_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
    }
}

/// Creates a test medication_master ActiveModel
pub fn new_test_medication_master(
    canonical_name: &str,
    arabic_name: &str,
) -> medication_master::ActiveModel {
    let now = Utc::now();
    medication_master::ActiveModel {
        id: Set(Uuid::new_v4()),
        canonical_name: Set(canonical_name.to_string()),
        canonical_name_ar: Set(Some(arabic_name.to_string())),
        active_ingredient: Set(None),
        strength: Set(None),
        dosage_form: Set(None),
        manufacturer: Set(None),
        eda_registration: Set(None),
        therapeutic_class: Set(None),
        atc_code: Set(None),
        status: Set(medication_master::MedicationStatus::Active),
        embedding: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        created_by: Set(None),
    }
}
