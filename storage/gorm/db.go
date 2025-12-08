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
