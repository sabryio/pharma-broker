package storage

import (
	"context"
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"pharmabroker/internal/config"

	_ "modernc.org/sqlite"
)

// DB wraps the SQL database connection with read replica support
type DB struct {
	writer *sql.DB // Single writer connection (SQLite requirement)
	reader *sql.DB // Read-only connection pool for queries
	cfg    *config.DatabaseConfig
}

// New creates a new database connection with read replica support
func New(cfg *config.DatabaseConfig) (*DB, error) {
	// Ensure directory exists
	dir := filepath.Dir(cfg.Path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return nil, fmt.Errorf("create database directory: %w", err)
	}

	// Open writer connection (single connection for writes)
	writer, err := sql.Open("sqlite", cfg.Path)
	if err != nil {
		return nil, fmt.Errorf("open database writer: %w", err)
	}
	writer.SetMaxOpenConns(1) // SQLite single writer
	writer.SetMaxIdleConns(1)
	writer.SetConnMaxLifetime(time.Hour)

	// Open reader connection pool (for read-heavy operations)
	// Use read-only mode via query parameter
	readerPath := cfg.Path + "?mode=ro"
	reader, err := sql.Open("sqlite", readerPath)
	if err != nil {
		writer.Close()
		return nil, fmt.Errorf("open database reader: %w", err)
	}
	maxReadConns := cfg.MaxReadConns
	if maxReadConns <= 0 {
		maxReadConns = 5
	}
	reader.SetMaxOpenConns(maxReadConns)
	reader.SetMaxIdleConns(maxReadConns)
	reader.SetConnMaxLifetime(time.Hour)

	db := &DB{writer: writer, reader: reader, cfg: cfg}

	// Enable WAL mode if configured (on writer)
	if cfg.EnableWAL {
		if _, err := writer.Exec("PRAGMA journal_mode=WAL"); err != nil {
			db.Close()
			return nil, fmt.Errorf("enable WAL mode: %w", err)
		}
	}

	// Set pragmas on both connections
	pragmas := []string{
		"PRAGMA busy_timeout = 5000",
		"PRAGMA synchronous = NORMAL",
		"PRAGMA cache_size = -64000", // 64MB cache
		"PRAGMA foreign_keys = ON",
	}
	for _, pragma := range pragmas {
		if _, err := writer.Exec(pragma); err != nil {
			db.Close()
			return nil, fmt.Errorf("set pragma on writer: %w", err)
		}
		if _, err := reader.Exec(pragma); err != nil {
			db.Close()
			return nil, fmt.Errorf("set pragma on reader: %w", err)
		}
	}

	// Run migrations (on writer only)
	if err := db.migrate(); err != nil {
		db.Close()
		return nil, fmt.Errorf("run migrations: %w", err)
	}

	return db, nil
}

// Conn returns the writer connection (for compatibility and writes)
func (db *DB) Conn() *sql.DB {
	return db.writer
}

// Reader returns the read-only connection pool for SELECT queries
func (db *DB) Reader() *sql.DB {
	return db.reader
}

// Close closes both database connections
func (db *DB) Close() error {
	var errs []error
	if db.reader != nil {
		if err := db.reader.Close(); err != nil {
			errs = append(errs, err)
		}
	}
	if db.writer != nil {
		if err := db.writer.Close(); err != nil {
			errs = append(errs, err)
		}
	}
	if len(errs) > 0 {
		return errs[0]
	}
	return nil
}

// Transaction executes a function within a transaction (uses writer)
func (db *DB) Transaction(ctx context.Context, fn func(tx *sql.Tx) error) error {
	tx, err := db.writer.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()

	if err := fn(tx); err != nil {
		return err
	}

	return tx.Commit()
}
