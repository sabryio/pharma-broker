package storage

import (
	"embed"
	"fmt"
)

//go:embed schema/*.sql
var schemaFS embed.FS

// migrate runs all database migrations
func (db *DB) migrate() error {
	// Create migrations table
	if _, err := db.conn.Exec(`
		CREATE TABLE IF NOT EXISTS migrations (
			version INTEGER PRIMARY KEY,
			applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
		)
	`); err != nil {
		return fmt.Errorf("create migrations table: %w", err)
	}

	// Get current version
	var currentVersion int
	row := db.conn.QueryRow("SELECT COALESCE(MAX(version), 0) FROM migrations")
	if err := row.Scan(&currentVersion); err != nil {
		return fmt.Errorf("get current version: %w", err)
	}

	// Initialize migrations list dynamically (or statically if preferred)
	// For now we map the files to versions manually to be explicit
	var migrations = []migration{
		{
			version: 1,
			sql:     mustReadSQL("schema/v1_initial_schema.sql"),
		},
	}

	// Run pending migrations
	for _, m := range migrations {
		if m.version > currentVersion {
			if err := db.runMigration(m); err != nil {
				return fmt.Errorf("migration %d: %w", m.version, err)
			}
		}
	}

	return nil
}

func mustReadSQL(path string) string {
	content, err := schemaFS.ReadFile(path)
	if err != nil {
		panic(fmt.Sprintf("failed to read embedded migration file %s: %v", path, err))
	}
	return string(content)
}

func (db *DB) runMigration(m migration) error {
	tx, err := db.conn.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()

	if _, err := tx.Exec(m.sql); err != nil {
		return err
	}

	if _, err := tx.Exec("INSERT INTO migrations (version) VALUES (?)", m.version); err != nil {
		return err
	}

	return tx.Commit()
}

type migration struct {
	version int
	sql     string
}
