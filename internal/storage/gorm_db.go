package storage

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/glebarez/sqlite" // CGO-free SQLite driver
	"gorm.io/gorm"
	"gorm.io/gorm/logger"

	"pharmabroker/internal/config"
	"pharmabroker/internal/storage/models"
)

// GormDB wraps GORM database connection with SQLite optimizations
type GormDB struct {
	DB  *gorm.DB
	cfg *config.DatabaseConfig
}

// NewGormDB creates a new GORM database connection (CGO-free)
func NewGormDB(cfg *config.DatabaseConfig) (*GormDB, error) {
	// Ensure directory exists
	dir := filepath.Dir(cfg.Path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return nil, fmt.Errorf("create database directory: %w", err)
	}

	// Build DSN with SQLite pragmas
	// Note: glebarez/sqlite uses different pragma syntax
	dsn := cfg.Path

	// Configure GORM
	gormConfig := &gorm.Config{
		Logger: logger.Default.LogMode(logger.Warn),
		NowFunc: func() time.Time {
			return time.Now().UTC()
		},
		// Disable default transaction for better performance
		SkipDefaultTransaction: true,
		// Prepare statements for better performance
		PrepareStmt: true,
	}

	db, err := gorm.Open(sqlite.Open(dsn), gormConfig)
	if err != nil {
		return nil, fmt.Errorf("open database: %w", err)
	}

	// Get underlying sql.DB for connection pool settings
	sqlDB, err := db.DB()
	if err != nil {
		return nil, fmt.Errorf("get underlying db: %w", err)
	}

	// SQLite connection pool settings (single writer requirement)
	sqlDB.SetMaxOpenConns(1)
	sqlDB.SetMaxIdleConns(1)
	sqlDB.SetConnMaxLifetime(time.Hour)

	gdb := &GormDB{DB: db, cfg: cfg}

	// Set SQLite pragmas for performance
	if err := gdb.setPragmas(); err != nil {
		gdb.Close()
		return nil, fmt.Errorf("set pragmas: %w", err)
	}

	// Run auto-migrations
	if err := gdb.migrate(); err != nil {
		gdb.Close()
		return nil, fmt.Errorf("auto migrate: %w", err)
	}

	// Create FTS tables and triggers (GORM can't auto-migrate virtual tables)
	if err := gdb.createFTSTables(); err != nil {
		gdb.Close()
		return nil, fmt.Errorf("create FTS tables: %w", err)
	}

	return gdb, nil
}

// setPragmas configures SQLite for optimal performance
func (g *GormDB) setPragmas() error {
	pragmas := []string{
		"PRAGMA journal_mode=WAL",
		"PRAGMA busy_timeout = 5000",
		"PRAGMA synchronous = NORMAL",
		"PRAGMA cache_size = -64000", // 64MB cache
		"PRAGMA foreign_keys = ON",
	}

	for _, pragma := range pragmas {
		if err := g.DB.Exec(pragma).Error; err != nil {
			return fmt.Errorf("execute pragma %q: %w", pragma, err)
		}
	}

	return nil
}

// migrate runs GORM auto-migrations for all models
func (g *GormDB) migrate() error {
	return g.DB.AutoMigrate(
		&models.RawMessage{},
		&models.Offer{},
		&models.Request{},
		&models.Match{},
		&models.MatchQueue{},
		&models.Config{},
		&models.Group{},
		&models.MedicationMapping{},
		&models.FailedMessage{},
		&models.MatchFeedback{},
		&models.DemandLeaderboard{},
		&models.AuditLog{},
		&models.UnmappedMedication{},
		&models.FeedbackRecord{},
		&models.WeightHistory{},
		&models.ReviewQueue{},
	)
}

// createFTSTables creates SQLite FTS5 virtual tables and triggers
// These cannot be auto-migrated by GORM
func (g *GormDB) createFTSTables() error {
	// Check if FTS tables already exist
	var count int64
	g.DB.Raw("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='offers_fts'").Scan(&count)
	if count > 0 {
		return nil // FTS tables already exist
	}

	ftsSQL := `
	-- FTS5 for Offers
	CREATE VIRTUAL TABLE IF NOT EXISTS offers_fts USING fts5(
		medication, medication_raw, notes,
		content='offers', content_rowid='rowid'
	);
	CREATE TRIGGER IF NOT EXISTS offers_ai AFTER INSERT ON offers BEGIN
		INSERT INTO offers_fts(rowid, medication, medication_raw, notes)
		VALUES (NEW.rowid, NEW.medication, NEW.medication_raw, NEW.notes);
	END;
	CREATE TRIGGER IF NOT EXISTS offers_ad AFTER DELETE ON offers BEGIN
		INSERT INTO offers_fts(offers_fts, rowid, medication, medication_raw, notes)
		VALUES('delete', OLD.rowid, OLD.medication, OLD.medication_raw, OLD.notes);
	END;
	CREATE TRIGGER IF NOT EXISTS offers_au AFTER UPDATE ON offers BEGIN
		INSERT INTO offers_fts(offers_fts, rowid, medication, medication_raw, notes)
		VALUES('delete', OLD.rowid, OLD.medication, OLD.medication_raw, OLD.notes);
		INSERT INTO offers_fts(rowid, medication, medication_raw, notes)
		VALUES (NEW.rowid, NEW.medication, NEW.medication_raw, NEW.notes);
	END;

	-- FTS5 for Requests
	CREATE VIRTUAL TABLE IF NOT EXISTS requests_fts USING fts5(
		medication, medication_raw, notes,
		content='requests', content_rowid='rowid'
	);
	CREATE TRIGGER IF NOT EXISTS requests_ai AFTER INSERT ON requests BEGIN
		INSERT INTO requests_fts(rowid, medication, medication_raw, notes)
		VALUES (NEW.rowid, NEW.medication, NEW.medication_raw, NEW.notes);
	END;
	CREATE TRIGGER IF NOT EXISTS requests_ad AFTER DELETE ON requests BEGIN
		INSERT INTO requests_fts(requests_fts, rowid, medication, medication_raw, notes)
		VALUES('delete', OLD.rowid, OLD.medication, OLD.medication_raw, OLD.notes);
	END;
	CREATE TRIGGER IF NOT EXISTS requests_au AFTER UPDATE ON requests BEGIN
		INSERT INTO requests_fts(requests_fts, rowid, medication, medication_raw, notes)
		VALUES('delete', OLD.rowid, OLD.medication, OLD.medication_raw, OLD.notes);
		INSERT INTO requests_fts(rowid, medication, medication_raw, notes)
		VALUES (NEW.rowid, NEW.medication, NEW.medication_raw, NEW.notes);
	END;

	-- FTS5 for Medication Mappings (with trigram tokenizer for fuzzy search)
	CREATE VIRTUAL TABLE IF NOT EXISTS medication_mappings_fts USING fts5(
		arabic_name, english_name, synonyms,
		content='medication_mappings', content_rowid='rowid',
		tokenize='trigram'
	);
	CREATE TRIGGER IF NOT EXISTS medication_mappings_ai AFTER INSERT ON medication_mappings BEGIN
		INSERT INTO medication_mappings_fts(rowid, arabic_name, english_name, synonyms)
		VALUES (NEW.rowid, NEW.arabic_name, NEW.english_name, NEW.synonyms);
	END;
	CREATE TRIGGER IF NOT EXISTS medication_mappings_ad AFTER DELETE ON medication_mappings BEGIN
		INSERT INTO medication_mappings_fts(medication_mappings_fts, rowid, arabic_name, english_name, synonyms)
		VALUES('delete', OLD.rowid, OLD.arabic_name, OLD.english_name, OLD.synonyms);
	END;
	CREATE TRIGGER IF NOT EXISTS medication_mappings_au AFTER UPDATE ON medication_mappings BEGIN
		INSERT INTO medication_mappings_fts(medication_mappings_fts, rowid, arabic_name, english_name, synonyms)
		VALUES('delete', OLD.rowid, OLD.arabic_name, OLD.english_name, OLD.synonyms);
		INSERT INTO medication_mappings_fts(rowid, arabic_name, english_name, synonyms)
		VALUES (NEW.rowid, NEW.arabic_name, NEW.english_name, NEW.synonyms);
	END;
	`

	return g.DB.Exec(ftsSQL).Error
}

// Close closes the database connection
func (g *GormDB) Close() error {
	sqlDB, err := g.DB.DB()
	if err != nil {
		return err
	}
	return sqlDB.Close()
}

// Transaction executes a function within a database transaction
func (g *GormDB) Transaction(fn func(tx *gorm.DB) error) error {
	return g.DB.Transaction(fn)
}
