// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/glebarez/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/logger"
)

// DB wraps GORM database connection with SQLite optimizations
type DB struct {
	Conn *gorm.DB
	path string
}

// Config holds database configuration
type Config struct {
	Path            string
	MaxOpenConns    int
	MaxIdleConns    int
	ConnMaxLifetime time.Duration
}

// NewDB creates a new GORM database connection (CGO-free SQLite)
func NewDB(cfg *Config) (*DB, error) {
	// Ensure directory exists
	dir := filepath.Dir(cfg.Path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return nil, fmt.Errorf("create database directory: %w", err)
	}

	// Configure GORM
	gormConfig := &gorm.Config{
		Logger: logger.Default.LogMode(logger.Warn),
		NowFunc: func() time.Time {
			return time.Now().UTC()
		},
		SkipDefaultTransaction: true,
		PrepareStmt:            true,
	}

	db, err := gorm.Open(sqlite.Open(cfg.Path), gormConfig)
	if err != nil {
		return nil, fmt.Errorf("open database: %w", err)
	}

	// Get underlying sql.DB for connection pool settings
	sqlDB, err := db.DB()
	if err != nil {
		return nil, fmt.Errorf("get underlying db: %w", err)
	}

	// SQLite connection pool settings
	maxOpen := cfg.MaxOpenConns
	if maxOpen <= 0 {
		maxOpen = 1 // SQLite single writer requirement
	}
	maxIdle := cfg.MaxIdleConns
	if maxIdle <= 0 {
		maxIdle = 1
	}
	maxLifetime := cfg.ConnMaxLifetime
	if maxLifetime <= 0 {
		maxLifetime = time.Hour
	}

	sqlDB.SetMaxOpenConns(maxOpen)
	sqlDB.SetMaxIdleConns(maxIdle)
	sqlDB.SetConnMaxLifetime(maxLifetime)

	gdb := &DB{Conn: db, path: cfg.Path}

	// Set SQLite pragmas for performance
	if err := gdb.setPragmas(); err != nil {
		gdb.Close()
		return nil, fmt.Errorf("set pragmas: %w", err)
	}

	// Auto-migrate all models
	if err := gdb.Migrate(); err != nil {
		gdb.Close()
		return nil, fmt.Errorf("auto-migrate: %w", err)
	}

	return gdb, nil
}

// setPragmas configures SQLite for optimal performance
func (db *DB) setPragmas() error {
	pragmas := []string{
		"PRAGMA journal_mode=WAL",
		"PRAGMA busy_timeout = 5000",
		"PRAGMA synchronous = NORMAL",
		"PRAGMA cache_size = -64000", // 64MB cache
		"PRAGMA foreign_keys = ON",
	}

	for _, pragma := range pragmas {
		if err := db.Conn.Exec(pragma).Error; err != nil {
			return fmt.Errorf("execute pragma %q: %w", pragma, err)
		}
	}

	return nil
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
	)
	if err != nil {
		return err
	}

	// Create FTS virtual tables
	ftsSQL := `
		CREATE VIRTUAL TABLE IF NOT EXISTS requests_fts USING fts5(
			medication,
			notes,
			raw_message,
			medication_raw,
			content='requests',
			content_rowid='rowid'
		);
		CREATE TRIGGER IF NOT EXISTS requests_ai AFTER INSERT ON requests BEGIN
			INSERT INTO requests_fts(rowid, medication, notes, raw_message, medication_raw)
			VALUES (new.rowid, new.medication, new.notes, new.raw_message, new.medication_raw);
		END;
		CREATE TRIGGER IF NOT EXISTS requests_ad AFTER DELETE ON requests BEGIN
			INSERT INTO requests_fts(requests_fts, rowid, medication, notes, raw_message, medication_raw)
			VALUES('delete', old.rowid, old.medication, old.notes, old.raw_message, old.medication_raw);
		END;
		CREATE TRIGGER IF NOT EXISTS requests_au AFTER UPDATE ON requests BEGIN
			INSERT INTO requests_fts(requests_fts, rowid, medication, notes, raw_message, medication_raw)
			VALUES('delete', old.rowid, old.medication, old.notes, old.raw_message, old.medication_raw);
			INSERT INTO requests_fts(rowid, medication, notes, raw_message, medication_raw)
			VALUES (new.rowid, new.medication, new.notes, new.raw_message, new.medication_raw);
		END;

		CREATE VIRTUAL TABLE IF NOT EXISTS medication_mappings_fts USING fts5(
			arabic_name,
			english_name,
			synonyms,
			content='medication_mappings',
			content_rowid='rowid',
			tokenize='trigram'
		);
		CREATE TRIGGER IF NOT EXISTS medication_mappings_ai AFTER INSERT ON medication_mappings BEGIN
			INSERT INTO medication_mappings_fts(rowid, arabic_name, english_name, synonyms)
			VALUES (new.rowid, new.arabic_name, new.english_name, new.synonyms);
		END;
		CREATE TRIGGER IF NOT EXISTS medication_mappings_ad AFTER DELETE ON medication_mappings BEGIN
			INSERT INTO medication_mappings_fts(medication_mappings_fts, rowid, arabic_name, english_name, synonyms)
			VALUES('delete', old.rowid, old.arabic_name, old.english_name, old.synonyms);
		END;
		CREATE TRIGGER IF NOT EXISTS medication_mappings_au AFTER UPDATE ON medication_mappings BEGIN
			INSERT INTO medication_mappings_fts(medication_mappings_fts, rowid, arabic_name, english_name, synonyms)
			VALUES('delete', old.rowid, old.arabic_name, old.english_name, old.synonyms);
			INSERT INTO medication_mappings_fts(rowid, arabic_name, english_name, synonyms)
			VALUES (new.rowid, new.arabic_name, new.english_name, new.synonyms);
		END;

		CREATE VIRTUAL TABLE IF NOT EXISTS offers_fts USING fts5(
			medication,
			medication_raw,
			notes,
			raw_message,
			content='offers',
			content_rowid='rowid'
		);
		CREATE TRIGGER IF NOT EXISTS offers_ai AFTER INSERT ON offers BEGIN
			INSERT INTO offers_fts(rowid, medication, medication_raw, notes, raw_message)
			VALUES (new.rowid, new.medication, new.medication_raw, new.notes, new.raw_message);
		END;
		CREATE TRIGGER IF NOT EXISTS offers_ad AFTER DELETE ON offers BEGIN
			INSERT INTO offers_fts(offers_fts, rowid, medication, medication_raw, notes, raw_message)
			VALUES('delete', old.rowid, old.medication, old.medication_raw, old.notes, old.raw_message);
		END;
		CREATE TRIGGER IF NOT EXISTS offers_au AFTER UPDATE ON offers BEGIN
			INSERT INTO offers_fts(offers_fts, rowid, medication, medication_raw, notes, raw_message)
			VALUES('delete', old.rowid, old.medication, old.medication_raw, old.notes, old.raw_message);
			INSERT INTO offers_fts(rowid, medication, medication_raw, notes, raw_message)
			VALUES (new.rowid, new.medication, new.medication_raw, new.notes, new.raw_message);
		END;
	`
	return db.Conn.Exec(ftsSQL).Error
}
