# SQLite to PostgreSQL Migration Plan

## Overview

This document outlines the comprehensive migration strategy for transitioning PharmaBroker from SQLite to PostgreSQL across production, development, and test environments.

## 1. Assessment: Current SQLite Implementation

### 1.1 Schema Analysis

#### Tables (17 GORM Models)

| Table                  | Purpose                 | Special Features                       | Migration Complexity |
| ---------------------- | ----------------------- | -------------------------------------- | -------------------- |
| `raw_messages`         | WhatsApp messages       | UUID primary key, FK relationships     | Low                  |
| `offers`               | Medication offers       | Composite indexes, nullable FKs        | Low                  |
| `requests`             | Medication requests     | FTS5 virtual table dependency          | **High**             |
| `matches`              | Offer-request matches   | Composite unique index                 | Low                  |
| `match_queue`          | Background job queue    | Auto-created timestamp                 | Low                  |
| `config`               | App key-value config    | String primary key                     | Low                  |
| `groups`               | WhatsApp groups         | Non-UUID string primary key (JID)      | Low                  |
| `medication_mappings`  | Med name translations   | FTS5 virtual table, `[]byte` embedding | **Medium**           |
| `failed_messages`      | Processing failures     | FK to `raw_messages`                   | Low                  |
| `match_feedback`       | Operator feedback       | FK to `matches`                        | Low                  |
| `demand_leaderboard`   | Medication stats        | Materialized aggregate                 | Low                  |
| `audit_logs`           | System audit trail      | Large text fields                      | Low                  |
| `unmapped_medications` | Unmapped meds           | Auto-increment ID                      | Low                  |
| `review_queue`         | Manual review items     | JSON in TEXT fields                    | **Medium**           |
| `feedback_records`     | ML training data        | Multiple FK relationships              | Low                  |
| `weight_history`       | Algorithm weights       | JSON in TEXT field                     | Low                  |
| `bot_users`            | Telegram/WhatsApp users | Multiple unique indexes                | Low                  |

#### SQLite-Specific Features in Use

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     SQLite-Specific Features                                 │
├────────────────────────────────┬────────────────────────────────────────────┤
│ Feature                        │ PostgreSQL Equivalent                       │
├────────────────────────────────┼────────────────────────────────────────────┤
│ FTS5 Virtual Tables            │ tsvector + GIN indexes                     │
│ FTS5 Triggers (AI/AD/AU)       │ tsvector_update_trigger()                  │
│ PRAGMA journal_mode=WAL        │ Not needed (PostgreSQL has WAL by default) │
│ PRAGMA busy_timeout            │ Use connection pool settings               │
│ PRAGMA synchronous=NORMAL      │ synchronous_commit = off (if needed)       │
│ PRAGMA cache_size              │ shared_buffers config                      │
│ PRAGMA foreign_keys=ON         │ Always ON in PostgreSQL                    │
│ sqlite.Open(":memory:")        │ testcontainers-go or pg.Pool               │
│ rowid (implicit)               │ BIGSERIAL or explicit ID                   │
│ TEXT for JSON                  │ JSONB type                                 │
│ []byte for embeddings          │ vector type (pgvector extension)           │
└────────────────────────────────┴────────────────────────────────────────────┘
```

### 1.2 Data Types Mapping

| SQLite Type | Current GORM Tag | PostgreSQL Type        | Notes                        |
| ----------- | ---------------- | ---------------------- | ---------------------------- |
| `TEXT` (PK) | `primaryKey`     | `TEXT` or `UUID`       | Consider UUID for new tables |
| `INTEGER`   | `autoIncrement`  | `BIGSERIAL`            | PostgreSQL auto-increment    |
| `REAL`      | `float64`        | `DOUBLE PRECISION`     | Direct mapping               |
| `TEXT`      | `type:text`      | `TEXT`                 | Direct mapping               |
| `DATETIME`  | `time.Time`      | `TIMESTAMPTZ`          | Timezone-aware               |
| `BLOB`      | `[]byte`         | `BYTEA` or `vector(N)` | Use pgvector for embeddings  |
| `BOOLEAN`   | `bool`           | `BOOLEAN`              | Direct mapping               |

### 1.3 Full-Text Search Analysis

Current FTS5 virtual tables:

1. **`requests_fts`**: Searches `medication`, `notes`, `raw_message`, `medication_raw`
2. **`medication_mappings_fts`**: Searches `arabic_name`, `english_name`, `synonyms` (trigram tokenizer)
3. **`offers_fts`**: Searches `medication`, `medication_raw`, `notes`, `raw_message`

#### PostgreSQL FTS Replacement Strategy

```sql
-- PostgreSQL equivalent: tsvector column + GIN index
ALTER TABLE requests ADD COLUMN search_vector tsvector
  GENERATED ALWAYS AS (
    to_tsvector('simple', coalesce(medication, '') || ' ' ||
                          coalesce(notes, '') || ' ' ||
                          coalesce(raw_message, '') || ' ' ||
                          coalesce(medication_raw, ''))
  ) STORED;

CREATE INDEX idx_requests_search ON requests USING GIN(search_vector);

-- For trigram search (medication_mappings):
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE INDEX idx_medication_mappings_trgm
  ON medication_mappings USING GIN(arabic_name gin_trgm_ops, english_name gin_trgm_ops);
```

### 1.4 WhatsApp Session Store

The WhatsApp client uses a **separate SQLite database** via `whatsmeow`:

```go
// messaging/whatsapp/manager.go:147
container, err := sqlstore.New(ctx, "sqlite", fmt.Sprintf("file:%s?_pragma=foreign_keys(1)", dbPath), waLog.Noop)
```

> [!IMPORTANT]
> The `whatsmeow` library supports PostgreSQL. This requires updating the driver string from `"sqlite"` to `"postgres"` and providing a PostgreSQL connection string.

---

## 2. Schema Conversion

### 2.1 GORM Driver Change

```diff
// storage/gorm/db.go

- import "github.com/glebarez/sqlite"
+ import "gorm.io/driver/postgres"

- db, err := gorm.Open(sqlite.Open(cfg.Path), gormConfig)
+ db, err := gorm.Open(postgres.Open(cfg.DSN), gormConfig)
```

### 2.2 Configuration Update

```go
// pkg/config/config.go

type DatabaseConfig struct {
-   // Path is the file path to the SQLite database.
-   Path string `mapstructure:"path"`
+   // DSN is the PostgreSQL connection string.
+   // Format: postgres://user:password@host:port/database?sslmode=disable
+   DSN string `mapstructure:"dsn"`

-   // EnableWAL enables Write-Ahead Logging mode for SQLite.
-   EnableWAL bool `mapstructure:"enable_wal"`
+   // MaxOpenConns is the maximum number of open connections.
+   MaxOpenConns int `mapstructure:"max_open_conns"`

+   // MaxIdleConns is the maximum number of idle connections.
+   MaxIdleConns int `mapstructure:"max_idle_conns"`

    // ... rest unchanged
}
```

### 2.3 Model Changes Required

#### 2.3.1 Add Generated tsvector Columns

```go
// storage/gorm/models.go

type Request struct {
    // ... existing fields ...

    // PostgreSQL full-text search vector (generated column)
    SearchVector string `gorm:"column:search_vector;type:tsvector;-:all"` // read-only
}

type Offer struct {
    // ... existing fields ...

    SearchVector string `gorm:"column:search_vector;type:tsvector;-:all"`
}

type MedicationMapping struct {
    // ... existing fields ...

-   Embedding   []byte    `gorm:"column:embedding"`
+   Embedding   []byte    `gorm:"column:embedding;type:vector(1536)"` // pgvector
}
```

#### 2.3.2 JSON Fields → JSONB

```go
// No model change needed, but add migration:
type ReviewQueue struct {
    // ... existing fields ...

-   PartialItems   string  `gorm:"column:partial_items;type:text"`
+   PartialItems   string  `gorm:"column:partial_items;type:jsonb"`

-   CorrectedItems *string `gorm:"column:corrected_items;type:text"`
+   CorrectedItems *string `gorm:"column:corrected_items;type:jsonb"`
}
```

### 2.4 Migration SQL Scripts

#### 2.4.1 Schema Creation (PostgreSQL)

```sql
-- migrations/001_init_schema.sql

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgvector";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";

-- Create tables (GORM AutoMigrate will handle this, but for reference):
-- Tables are created by GORM AutoMigrate

-- Add full-text search columns (after GORM migration)
ALTER TABLE requests ADD COLUMN IF NOT EXISTS search_vector tsvector
  GENERATED ALWAYS AS (
    setweight(to_tsvector('simple', coalesce(medication, '')), 'A') ||
    setweight(to_tsvector('simple', coalesce(medication_raw, '')), 'B') ||
    setweight(to_tsvector('simple', coalesce(notes, '')), 'C') ||
    setweight(to_tsvector('simple', coalesce(raw_message, '')), 'D')
  ) STORED;

ALTER TABLE offers ADD COLUMN IF NOT EXISTS search_vector tsvector
  GENERATED ALWAYS AS (
    setweight(to_tsvector('simple', coalesce(medication, '')), 'A') ||
    setweight(to_tsvector('simple', coalesce(medication_raw, '')), 'B') ||
    setweight(to_tsvector('simple', coalesce(notes, '')), 'C') ||
    setweight(to_tsvector('simple', coalesce(raw_message, '')), 'D')
  ) STORED;

-- Create GIN indexes
CREATE INDEX IF NOT EXISTS idx_requests_search ON requests USING GIN(search_vector);
CREATE INDEX IF NOT EXISTS idx_offers_search ON offers USING GIN(search_vector);

-- Trigram indexes for medication mappings
CREATE INDEX IF NOT EXISTS idx_medication_mappings_arabic_trgm
  ON medication_mappings USING GIN(arabic_name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_medication_mappings_english_trgm
  ON medication_mappings USING GIN(english_name gin_trgm_ops);
```

### 2.5 Search Utilities Rewrite

#### Current (SQLite FTS5):

```go
// storage/gorm/search_utils.go - FTS5 specific
func SanitizeFTSQuery(query string) string { ... }
```

#### New (PostgreSQL):

```go
// storage/gorm/search_utils_pg.go

// sanitizeTSQuery sanitizes user input for PostgreSQL full-text search.
func sanitizeTSQuery(query string) string {
    // Normalize Arabic
    query = NormalizeArabic(query)

    // Split into words
    words := strings.Fields(query)
    var terms []string
    for _, w := range words {
        // Escape special characters
        w = strings.ReplaceAll(w, "'", "''")
        terms = append(terms, w+":*") // Prefix search
    }

    return strings.Join(terms, " | ") // OR logic
}

// BuildTSQuery creates a PostgreSQL tsquery from user input.
func BuildTSQuery(query string, useOr bool) string {
    sanitized := sanitizeTSQuery(query)
    if useOr {
        return fmt.Sprintf("plainto_tsquery('simple', '%s')", sanitized)
    }
    return fmt.Sprintf("to_tsquery('simple', '%s')", sanitized)
}
```

---

## 3. Data Migration Strategy

### 3.1 Migration Tools

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Recommended Migration Toolchain                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐             │
│  │   SQLite     │───▶│   pgloader   │───▶│   PostgreSQL     │             │
│  │   (.db)      │    │   (ETL)      │    │   (target)       │             │
│  └──────────────┘    └──────────────┘    └──────────────────┘             │
│                             │                                               │
│                             ▼                                               │
│                      ┌──────────────┐                                      │
│                      │  Post-migrate│                                      │
│                      │  SQL scripts │                                      │
│                      │  (FTS, etc.) │                                      │
│                      └──────────────┘                                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 pgloader Configuration

```lisp
;; migration/pgloader.conf

LOAD DATABASE
     FROM sqlite:///data/pharmabroker.db
     INTO postgresql://postgres:password@localhost:5432/pharmabroker

WITH include no drop,
     create tables,
     create indexes,
     reset sequences,
     workers = 4,
     concurrency = 2,
     batch rows = 10000

SET work_mem to '128MB',
    maintenance_work_mem to '256MB'

CAST type TEXT to TEXT,
     type REAL to DOUBLE PRECISION,
     type INTEGER to BIGINT when (= "id" column-name),
     type BLOB to BYTEA

EXCLUDING TABLE NAMES MATCHING
     ~/.*_fts$/,           -- Skip FTS virtual tables
     ~/.*_fts_content$/,   -- Skip FTS content tables
     ~/.*_fts_idx$/,       -- Skip FTS index tables
     ~/.*_fts_docsize$/,   -- Skip FTS docsize tables
     ~/.*_fts_config$/     -- Skip FTS config tables

AFTER LOAD DO
     $$ CREATE EXTENSION IF NOT EXISTS "uuid-ossp"; $$,
     $$ CREATE EXTENSION IF NOT EXISTS "pgvector"; $$,
     $$ CREATE EXTENSION IF NOT EXISTS "pg_trgm"; $$;
```

### 3.3 Step-by-Step Data Migration

```bash
#!/bin/bash
# migration/migrate_data.sh

set -e

SQLITE_DB="${1:-./data/pharmabroker.db}"
PG_DSN="${2:-postgres://postgres:password@localhost:5432/pharmabroker}"

echo "=== Phase 1: Pre-migration validation ==="
sqlite3 "$SQLITE_DB" "SELECT COUNT(*) FROM raw_messages;"
sqlite3 "$SQLITE_DB" "SELECT COUNT(*) FROM offers;"
sqlite3 "$SQLITE_DB" "SELECT COUNT(*) FROM requests;"
sqlite3 "$SQLITE_DB" "SELECT COUNT(*) FROM matches;"

echo "=== Phase 2: Create PostgreSQL schema ==="
psql "$PG_DSN" -f migrations/001_init_schema.sql

echo "=== Phase 3: Run pgloader ==="
pgloader migration/pgloader.conf

echo "=== Phase 4: Post-migration schema updates ==="
psql "$PG_DSN" -f migrations/002_add_fts_columns.sql
psql "$PG_DSN" -f migrations/003_add_indexes.sql

echo "=== Phase 5: Validation ==="
psql "$PG_DSN" -c "SELECT COUNT(*) FROM raw_messages;"
psql "$PG_DSN" -c "SELECT COUNT(*) FROM offers;"
psql "$PG_DSN" -c "SELECT COUNT(*) FROM requests;"
psql "$PG_DSN" -c "SELECT COUNT(*) FROM matches;"

echo "=== Migration complete ==="
```

### 3.4 Downtime Estimation

| Data Volume    | Estimated Downtime | Strategy                           |
| -------------- | ------------------ | ---------------------------------- |
| < 100K rows    | ~5 minutes         | Direct migration                   |
| 100K - 1M rows | ~15-30 minutes     | Batch migration with readonly mode |
| > 1M rows      | 1-2 hours          | Blue-green deployment              |

---

## 4. Environment Configuration

### 4.1 Docker Compose (Development)

```yaml
# docker-compose.yaml (already updated by user)
services:
  postgres:
    image: pgvector/pgvector:pg18-trixie
    container_name: postgres
    environment:
      POSTGRES_DB: ${POSTGRES_DB:-pharmabroker}
      POSTGRES_USER: ${POSTGRES_USER:-postgres}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-password}
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./init-db.sh:/docker-entrypoint-initdb.d/init-db.sh
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5
```

### 4.2 Production Environment

```yaml
# Production PostgreSQL Configuration
# /etc/postgresql/17/main/postgresql.conf

# Memory
shared_buffers = 256MB            # 25% of RAM
effective_cache_size = 768MB      # 75% of RAM
work_mem = 16MB
maintenance_work_mem = 128MB

# WAL
wal_level = replica               # For replication
max_wal_senders = 3
wal_keep_size = 1GB

# Connections
max_connections = 100
idle_in_transaction_session_timeout = 60000   # 1 minute

# Logging
log_statement = 'ddl'
log_min_duration_statement = 1000  # Log slow queries > 1s

# Extensions (loaded on startup)
shared_preload_libraries = 'pg_stat_statements,vector'
```

### 4.3 Test Environment

```go
// storage/gorm/testing_pg.go

import (
    "context"
    "testing"
    "github.com/testcontainers/testcontainers-go"
    "github.com/testcontainers/testcontainers-go/modules/postgres"
)

// SetupTestDB creates a PostgreSQL container for testing
func SetupTestDB(t *testing.T) *TestDB {
    t.Helper()
    ctx := context.Background()

    // Start PostgreSQL container with pgvector
    pgContainer, err := postgres.RunContainer(ctx,
        testcontainers.WithImage("pgvector/pgvector:pg18-trixie"),
        postgres.WithDatabase("testdb"),
        postgres.WithUsername("test"),
        postgres.WithPassword("test"),
    )
    if err != nil {
        t.Fatalf("Failed to start PostgreSQL container: %v", err)
    }

    t.Cleanup(func() { pgContainer.Terminate(ctx) })

    dsn, err := pgContainer.ConnectionString(ctx, "sslmode=disable")
    if err != nil {
        t.Fatalf("Failed to get connection string: %v", err)
    }

    db, err := NewDB(&Config{DSN: dsn})
    if err != nil {
        t.Fatalf("Failed to connect: %v", err)
    }

    return &TestDB{DB: db, t: t}
}
```

### 4.4 Environment Variables

```bash
# .env.example

# PostgreSQL Connection
DATABASE_DSN=postgres://user:password@localhost:5432/pharmabroker?sslmode=disable

# Pool Settings
DATABASE_MAX_OPEN_CONNS=25
DATABASE_MAX_IDLE_CONNS=5
DATABASE_CONN_MAX_LIFETIME=5m

# WhatsApp Session Store (also PostgreSQL)
WHATSAPP_DB_DSN=postgres://user:password@localhost:5432/pharmabroker_wa?sslmode=disable
```

---

## 5. Application Layer Modifications

### 5.1 Files Requiring Changes

| File                                                                                                                 | Change Type                      | Complexity |
| -------------------------------------------------------------------------------------------------------------------- | -------------------------------- | ---------- |
| [storage/gorm/db.go](file:///e:/programming/brand-new/Golang/pharma-broker/storage/gorm/db.go)                       | Driver + Config + Remove Pragmas | High       |
| [storage/gorm/search_utils.go](file:///e:/programming/brand-new/Golang/pharma-broker/storage/gorm/search_utils.go)   | FTS5 → tsvector/tsquery          | High       |
| [storage/gorm/testing.go](file:///e:/programming/brand-new/Golang/pharma-broker/storage/gorm/testing.go)             | In-memory → testcontainers       | High       |
| [storage/gorm/models.go](file:///e:/programming/brand-new/Golang/pharma-broker/storage/gorm/models.go)               | Add tsvector columns             | Medium     |
| [pkg/config/config.go](file:///e:/programming/brand-new/Golang/pharma-broker/pkg/config/config.go)                   | DSN instead of Path              | Medium     |
| [messaging/whatsapp/manager.go](file:///e:/programming/brand-new/Golang/pharma-broker/messaging/whatsapp/manager.go) | SQLite → PostgreSQL driver       | Medium     |
| [storage/go.mod](file:///e:/programming/brand-new/Golang/pharma-broker/storage/go.mod)                               | Dependencies                     | Low        |
| [Dockerfile](file:///e:/programming/brand-new/Golang/pharma-broker/Dockerfile)                                       | Remove SQLite deps               | Low        |

### 5.2 Detailed Code Changes

#### 5.2.1 `storage/gorm/db.go`

```go
// BEFORE
import "github.com/glebarez/sqlite"

func NewDB(cfg *Config) (*DB, error) {
    db, err := gorm.Open(sqlite.Open(cfg.Path), gormConfig)
    // ...
    gdb.setPragmas() // SQLite-specific
}

// AFTER
import "gorm.io/driver/postgres"

func NewDB(cfg *Config) (*DB, error) {
    db, err := gorm.Open(postgres.Open(cfg.DSN), gormConfig)
    // ...
    // No pragmas needed for PostgreSQL
}

func (db *DB) Migrate() error {
    err := db.Conn.AutoMigrate(/* models */)
    if err != nil {
        return err
    }

    // PostgreSQL-specific: Add tsvector columns and indexes
    return db.setupFullTextSearch()
}

func (db *DB) setupFullTextSearch() error {
    // Enable extensions
    db.Conn.Exec("CREATE EXTENSION IF NOT EXISTS pg_trgm")

    // Add generated tsvector columns
    db.Conn.Exec(`
        ALTER TABLE requests ADD COLUMN IF NOT EXISTS search_vector tsvector
        GENERATED ALWAYS AS (
            to_tsvector('simple', coalesce(medication, '') || ' ' ||
                                  coalesce(medication_raw, '') || ' ' ||
                                  coalesce(notes, '') || ' ' ||
                                  coalesce(raw_message, ''))
        ) STORED
    `)

    db.Conn.Exec(`CREATE INDEX IF NOT EXISTS idx_requests_search ON requests USING GIN(search_vector)`)

    // Similar for offers, medication_mappings...
    return nil
}
```

#### 5.2.2 `messaging/whatsapp/manager.go`

```go
// BEFORE
container, err := sqlstore.New(ctx, "sqlite",
    fmt.Sprintf("file:%s?_pragma=foreign_keys(1)", dbPath), waLog.Noop)

// AFTER
container, err := sqlstore.New(ctx, "postgres",
    cfg.WhatsAppDBDSN, waLog.Noop)
```

#### 5.2.3 Dependencies Update

```bash
# storage/go.mod - Remove SQLite, add PostgreSQL
go get -u gorm.io/driver/postgres
go get github.com/testcontainers/testcontainers-go
go get github.com/testcontainers/testcontainers-go/modules/postgres

# Remove old SQLite dependencies (after migration complete)
go mod tidy
```

---

## 6. Testing and Validation

### 6.1 Pre-Migration Checklist

- [ ] All SQLite data backed up
- [ ] PostgreSQL container running with extensions enabled
- [ ] pgvector extension installed (`CREATE EXTENSION vector`)
- [ ] pg_trgm extension installed
- [ ] Connection tested from application

### 6.2 Test Plan

| Test Category         | Description                              | Success Criteria                 |
| --------------------- | ---------------------------------------- | -------------------------------- |
| **Unit Tests**        | Run existing `*_test.go` with PostgreSQL | All tests pass                   |
| **Integration Tests** | Full CRUD operations                     | No errors                        |
| **FTS Tests**         | Arabic and English text search           | Same results as SQLite FTS5      |
| **Performance Tests** | Query benchmark comparison               | Within 10% of SQLite performance |
| **Data Integrity**    | Row counts match                         | 100% match                       |
| **FK Constraints**    | Relationship integrity                   | No orphan records                |

### 6.3 Validation Queries

```sql
-- Row count validation
SELECT
    'raw_messages' as table_name, COUNT(*) FROM raw_messages
UNION ALL SELECT
    'offers', COUNT(*) FROM offers
UNION ALL SELECT
    'requests', COUNT(*) FROM requests
UNION ALL SELECT
    'matches', COUNT(*) FROM matches;

-- Full-text search validation
-- Test query that worked in SQLite
SELECT id, medication FROM requests
WHERE search_vector @@ to_tsquery('simple', 'أوجمنتين:*');

-- Trigram search for medication mappings
SELECT id, arabic_name, english_name FROM medication_mappings
WHERE arabic_name % 'اوجمنتين'
ORDER BY similarity(arabic_name, 'اوجمنتين') DESC
LIMIT 10;

-- FK integrity check
SELECT COUNT(*) FROM offers o
LEFT JOIN raw_messages rm ON o.raw_message_id = rm.id
WHERE o.raw_message_id IS NOT NULL AND rm.id IS NULL;
```

### 6.4 Performance Benchmarks

```go
// storage/gorm/benchmark_test.go

func BenchmarkRequestSearch(b *testing.B) {
    db := SetupTestDB(b)
    defer db.Close()

    repo := NewRequestRepo(db.DB)
    ctx := context.Background()

    // Seed 10K records
    seedTestData(ctx, repo, 10000)

    b.ResetTimer()
    for i := 0; i < b.N; i++ {
        _, _ = repo.Search(ctx, "أوجمنتين", 10)
    }
}
```

---

## 7. Rollback and Contingency Plans

### 7.1 Rollback Strategy

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Rollback Decision Tree                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Migration Started                                                          │
│         │                                                                   │
│         ▼                                                                   │
│  ┌──────────────┐                                                          │
│  │ Data copied? │──No──▶ Cancel migration, restore SQLite config           │
│  └──────────────┘                                                          │
│         │ Yes                                                               │
│         ▼                                                                   │
│  ┌──────────────┐                                                          │
│  │ App works?   │──No──▶ Switch back to SQLite, investigate                │
│  └──────────────┘                                                          │
│         │ Yes                                                               │
│         ▼                                                                   │
│  ┌──────────────┐                                                          │
│  │ Perf OK?     │──No──▶ Optimize PostgreSQL, consider hybrid approach     │
│  └──────────────┘                                                          │
│         │ Yes                                                               │
│         ▼                                                                   │
│  Migration Complete ✅                                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Rollback Steps

1. **Immediate (< 1 hour post-migration):**

   ```bash
   # Restore original config
   git checkout -- pkg/config/config.go
   git checkout -- storage/gorm/db.go

   # Restart with SQLite
   docker-compose down
   docker-compose up -d pharmabroker  # Uses SQLite backup
   ```

2. **After writes to PostgreSQL:**

   ```bash
   # Export new data from PostgreSQL
   pg_dump -h localhost -U postgres -d pharmabroker \
     --data-only --inserts > recovery_data.sql

   # Import to SQLite (requires conversion script)
   python scripts/pg_to_sqlite.py recovery_data.sql pharmabroker.db
   ```

### 7.3 Data Backup Requirements

| Environment | Backup Frequency | Retention | Method                 |
| ----------- | ---------------- | --------- | ---------------------- |
| Production  | Every 6 hours    | 30 days   | pg_dump + S3           |
| Development | Daily            | 7 days    | Local volume backup    |
| Test        | None             | N/A       | Recreated per test run |

### 7.4 Monitoring Alerts

```yaml
# Prometheus alerts for migration monitoring
groups:
  - name: migration_alerts
    rules:
      - alert: PostgreSQLConnectionFailed
        expr: pg_up == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "PostgreSQL connection failed"

      - alert: HighQueryLatency
        expr: histogram_quantile(0.99, rate(pg_query_duration_seconds_bucket[5m])) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Query latency P99 > 1s"
```

---

## 8. Implementation Timeline

```mermaid
gantt
    title SQLite to PostgreSQL Migration
    dateFormat  YYYY-MM-DD

    section Phase 1: Preparation
    Code changes (db.go, search_utils)     :a1, 2025-01-01, 3d
    Test infrastructure update              :a2, after a1, 2d
    Configuration updates                   :a3, after a2, 1d

    section Phase 2: Development Testing
    Unit tests with PostgreSQL              :b1, after a3, 2d
    Integration tests                       :b2, after b1, 2d
    Performance benchmarks                  :b3, after b2, 1d

    section Phase 3: Staging Migration
    Deploy PostgreSQL to staging            :c1, after b3, 1d
    Data migration (staging)                :c2, after c1, 1d
    Smoke tests                             :c3, after c2, 1d

    section Phase 4: Production Migration
    Maintenance window announcement         :d1, after c3, 1d
    Production data backup                  :d2, after d1, 1d
    Production migration                    :d3, after d2, 1d
    Validation & monitoring                 :d4, after d3, 2d

    section Phase 5: Cleanup
    Remove SQLite dependencies              :e1, after d4, 1d
    Update documentation                    :e2, after e1, 1d
```

---

## Summary

### Key Risks

| Risk                   | Mitigation                                |
| ---------------------- | ----------------------------------------- |
| FTS5 → tsvector parity | Extensive search testing with Arabic text |
| Performance regression | Benchmark before/after                    |
| Data loss              | Multiple backups, validation at each step |
| Downtime               | Plan migration during low-traffic window  |

### Success Criteria

- [ ] All 17 tables migrated with 100% data integrity
- [ ] Full-text search returns equivalent results
- [ ] Query performance within 10% of SQLite baseline
- [ ] All existing tests pass
- [ ] Zero data loss during migration
- [ ] Rollback plan tested and documented
