//! Test infrastructure for repository integration tests
//! Uses testcontainers to spin up a pgvector-enabled PostgreSQL container

use sqlx::{PgPool, postgres::PgPoolOptions};
use testcontainers::{
    ContainerAsync, GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};

/// Test database wrapper with container lifecycle management
pub struct TestDb {
    pub pool: PgPool,
    _container: ContainerAsync<GenericImage>,
}

impl TestDb {
    /// Creates a new test database with pgvector PostgreSQL
    /// Uses pgvector/pgvector:pg18-trixie for full vector extension support (same as Go tests)
    pub async fn new() -> Self {
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

        let connection_string = format!(
            "postgres://postgres:password@127.0.0.1:{}/pharmabroker_test",
            host_port
        );

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&connection_string)
            .await
            .expect("Failed to connect to test database");

        // Run migrations / setup extensions
        Self::setup_schema(&pool).await;

        let db = Self {
            pool,
            _container: container,
        };

        // Clean all tables before test (matches Go behavior)
        db.truncate_tables().await;

        db
    }

    /// Sets up the database schema and extensions
    async fn setup_schema(pool: &PgPool) {
        // Enable pgvector extension
        sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(pool)
            .await
            .expect("Failed to create vector extension");

        // Enable pg_trgm extension for fuzzy search
        sqlx::query("CREATE EXTENSION IF NOT EXISTS pg_trgm")
            .execute(pool)
            .await
            .expect("Failed to create pg_trgm extension");

        // Create medication_mappings table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS medication_mappings (
                id TEXT PRIMARY KEY,
                arabic_name TEXT NOT NULL,
                english_name TEXT NOT NULL,
                synonyms TEXT[],
                embedding vector(768),
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(pool)
        .await
        .expect("Failed to create medication_mappings table");

        // Create trigram indexes for fuzzy search
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_medication_mappings_arabic_trgm 
            ON medication_mappings USING GIN(arabic_name gin_trgm_ops)
            "#,
        )
        .execute(pool)
        .await
        .ok();

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_medication_mappings_english_trgm 
            ON medication_mappings USING GIN(english_name gin_trgm_ops)
            "#,
        )
        .execute(pool)
        .await
        .ok();
    }

    /// Truncates all tables for test isolation (matches Go behavior)
    pub async fn truncate_tables(&self) {
        let tables = [
            "feedback_records",
            "weight_history",
            "review_queue",
            "unmapped_medications",
            "audit_logs",
            "demand_leaderboard",
            "match_feedback",
            "failed_messages",
            "medication_mappings",
            "groups",
            "config",
            "match_queue",
            "matches",
            "offers",
            "requests",
            "raw_messages",
            "bot_users",
        ];

        for table in tables {
            let query = format!("TRUNCATE TABLE {} CASCADE", table);
            sqlx::query(&query).execute(&self.pool).await.ok();
        }
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
