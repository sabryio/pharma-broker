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

// DB wraps the SQL database connection
type DB struct {
	conn *sql.DB
	cfg  *config.DatabaseConfig
}

// New creates a new database connection
func New(cfg *config.DatabaseConfig) (*DB, error) {
	// Ensure directory exists
	dir := filepath.Dir(cfg.Path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return nil, fmt.Errorf("create database directory: %w", err)
	}

	// Open connection
	conn, err := sql.Open("sqlite", cfg.Path)
	if err != nil {
		return nil, fmt.Errorf("open database: %w", err)
	}

	// Configure connection pool
	conn.SetMaxOpenConns(1) // SQLite single writer
	conn.SetMaxIdleConns(1)
	conn.SetConnMaxLifetime(time.Hour)

	db := &DB{conn: conn, cfg: cfg}

	// Enable WAL mode if configured
	if cfg.EnableWAL {
		if _, err := conn.Exec("PRAGMA journal_mode=WAL"); err != nil {
			return nil, fmt.Errorf("enable WAL mode: %w", err)
		}
	}

	// Set other pragmas for performance
	pragmas := []string{
		"PRAGMA busy_timeout = 5000",
		"PRAGMA synchronous = NORMAL",
		"PRAGMA cache_size = -64000", // 64MB cache
		"PRAGMA foreign_keys = ON",
	}
	for _, pragma := range pragmas {
		if _, err := conn.Exec(pragma); err != nil {
			return nil, fmt.Errorf("set pragma: %w", err)
		}
	}

	// Run migrations
	if err := db.migrate(); err != nil {
		return nil, fmt.Errorf("run migrations: %w", err)
	}

	return db, nil
}

// Conn returns the underlying database connection
func (db *DB) Conn() *sql.DB {
	return db.conn
}

// Close closes the database connection
func (db *DB) Close() error {
	return db.conn.Close()
}

// Transaction executes a function within a transaction
func (db *DB) Transaction(ctx context.Context, fn func(tx *sql.Tx) error) error {
	tx, err := db.conn.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()

	if err := fn(tx); err != nil {
		return err
	}

	return tx.Commit()
}
