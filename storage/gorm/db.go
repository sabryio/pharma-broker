// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"fmt"
	"pharmabroker/storage/gorm/models"
	"time"

	"gorm.io/driver/postgres"
	"gorm.io/gorm"
	"gorm.io/gorm/logger"
)

// DB wraps GORM database connection with PostgreSQL optimizations
type DB struct {
	Conn *gorm.DB
	dsn  string
}

// Config holds database configuration
type Config struct {
	DSN             string
	MaxOpenConns    int
	MaxIdleConns    int
	ConnMaxLifetime time.Duration
}

// NewDB creates a new GORM database connection to PostgreSQL
func NewDB(cfg *Config) (*DB, error) {
	// Configure GORM
	gormConfig := &gorm.Config{
		Logger: logger.Default.LogMode(logger.Warn),
		NowFunc: func() time.Time {
			return time.Now().UTC()
		},
		SkipDefaultTransaction: true,
		PrepareStmt:            true,
	}

	db, err := gorm.Open(postgres.Open(cfg.DSN), gormConfig)
	if err != nil {
		return nil, fmt.Errorf("open database: %w", err)
	}

	// Get underlying sql.DB for connection pool settings
	sqlDB, err := db.DB()
	if err != nil {
		return nil, fmt.Errorf("get underlying db: %w", err)
	}

	// PostgreSQL connection pool settings
	maxOpen := cfg.MaxOpenConns
	if maxOpen <= 0 {
		maxOpen = 25 // PostgreSQL can handle many connections
	}
	maxIdle := cfg.MaxIdleConns
	if maxIdle <= 0 {
		maxIdle = 5
	}
	maxLifetime := cfg.ConnMaxLifetime
	if maxLifetime <= 0 {
		maxLifetime = 5 * time.Minute
	}

	sqlDB.SetMaxOpenConns(maxOpen)
	sqlDB.SetMaxIdleConns(maxIdle)
	sqlDB.SetConnMaxLifetime(maxLifetime)

	gdb := &DB{Conn: db, dsn: cfg.DSN}

	// Auto-migrate all models
	if err := gdb.Migrate(); err != nil {
		gdb.Close()
		return nil, fmt.Errorf("auto-migrate: %w", err)
	}

	// Setup PostgreSQL full-text search
	if err := gdb.setupFullTextSearch(); err != nil {
		gdb.Close()
		return nil, fmt.Errorf("setup full-text search: %w", err)
	}

	return gdb, nil
}

// Close closes the database connection
func (db *DB) Close() error {
	sqlDB, err := db.Conn.DB()
	if err != nil {
		return err
	}
	return sqlDB.Close()
}

// Transaction executes a function within a database transaction
func (db *DB) Transaction(fn func(tx *gorm.DB) error) error {
	return db.Conn.Transaction(fn)
}

// GORM returns the underlying GORM connection
func (db *DB) GORM() *gorm.DB {
	return db.Conn
}

// Migrate runs database migrations for all models
func (db *DB) Migrate() error {
	// Run migrations for all models
	err := db.Conn.AutoMigrate(
		&RawMessage{},
		&Offer{},
		&Request{},
		&Match{},
		&MatchQueue{},
		&AppConfig{},
		&Group{},
		&MedicationMapping{},
		&FailedMessage{},
		&MatchFeedback{},
		&DemandLeaderboard{},
		&AuditLog{},
		&UnmappedMedication{},
		&ReviewQueue{},
		&FeedbackRecord{},
		&WeightHistory{},
		&models.BotUser{},
	)
	return err
}

// setupFullTextSearch creates PostgreSQL full-text search indexes
func (db *DB) setupFullTextSearch() error {
	// Enable pg_trgm extension for trigram similarity search
	if err := db.Conn.Exec("CREATE EXTENSION IF NOT EXISTS pg_trgm").Error; err != nil {
		return fmt.Errorf("create pg_trgm extension: %w", err)
	}

	// Add tsvector columns to requests table (if not exists)
	requestsFTS := `
		DO $$ BEGIN
			IF NOT EXISTS (
				SELECT 1 FROM information_schema.columns 
				WHERE table_name = 'requests' AND column_name = 'search_vector'
			) THEN
				ALTER TABLE requests ADD COLUMN search_vector tsvector
				GENERATED ALWAYS AS (
					setweight(to_tsvector('simple', coalesce(medication, '')), 'A') ||
					setweight(to_tsvector('simple', coalesce(medication_raw, '')), 'B') ||
					setweight(to_tsvector('simple', coalesce(notes, '')), 'C') ||
					setweight(to_tsvector('simple', coalesce(raw_message, '')), 'D')
				) STORED;
			END IF;
		END $$;
		CREATE INDEX IF NOT EXISTS idx_requests_search ON requests USING GIN(search_vector);
	`
	if err := db.Conn.Exec(requestsFTS).Error; err != nil {
		return fmt.Errorf("setup requests FTS: %w", err)
	}

	// Add tsvector columns to offers table (if not exists)
	offersFTS := `
		DO $$ BEGIN
			IF NOT EXISTS (
				SELECT 1 FROM information_schema.columns 
				WHERE table_name = 'offers' AND column_name = 'search_vector'
			) THEN
				ALTER TABLE offers ADD COLUMN search_vector tsvector
				GENERATED ALWAYS AS (
					setweight(to_tsvector('simple', coalesce(medication, '')), 'A') ||
					setweight(to_tsvector('simple', coalesce(medication_raw, '')), 'B') ||
					setweight(to_tsvector('simple', coalesce(notes, '')), 'C') ||
					setweight(to_tsvector('simple', coalesce(raw_message, '')), 'D')
				) STORED;
			END IF;
		END $$;
		CREATE INDEX IF NOT EXISTS idx_offers_search ON offers USING GIN(search_vector);
	`
	if err := db.Conn.Exec(offersFTS).Error; err != nil {
		return fmt.Errorf("setup offers FTS: %w", err)
	}

	// Add trigram indexes for medication_mappings (fuzzy Arabic search)
	medicationMappingsFTS := `
		CREATE INDEX IF NOT EXISTS idx_medication_mappings_arabic_trgm 
			ON medication_mappings USING GIN(arabic_name gin_trgm_ops);
		CREATE INDEX IF NOT EXISTS idx_medication_mappings_english_trgm 
			ON medication_mappings USING GIN(english_name gin_trgm_ops);
	`
	if err := db.Conn.Exec(medicationMappingsFTS).Error; err != nil {
		return fmt.Errorf("setup medication_mappings trigram: %w", err)
	}

	return nil
}
