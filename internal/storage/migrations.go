package storage

import (
	"fmt"
)

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

var migrations = []migration{
	{
		version: 1,
		sql: `
			-- Raw messages from WhatsApp
			CREATE TABLE raw_messages (
				id TEXT PRIMARY KEY,
				group_jid TEXT NOT NULL,
				group_name TEXT NOT NULL,
				sender_jid TEXT NOT NULL,
				sender_phone TEXT NOT NULL,
				sender_name TEXT,
				content TEXT NOT NULL,
				timestamp DATETIME NOT NULL,
				processed_at DATETIME,
				error TEXT,
				created_at DATETIME DEFAULT CURRENT_TIMESTAMP
			);
			CREATE INDEX idx_raw_messages_processed ON raw_messages(processed_at);
			CREATE INDEX idx_raw_messages_timestamp ON raw_messages(timestamp);

			-- Medication offers
			CREATE TABLE offers (
				id TEXT PRIMARY KEY,
				raw_message_id TEXT REFERENCES raw_messages(id),
				source_phone TEXT NOT NULL,
				source_name TEXT,
				source_group TEXT NOT NULL,
				group_name TEXT,
				medication TEXT NOT NULL,
				medication_raw TEXT NOT NULL,
				quantity INTEGER DEFAULT 0,
				unit TEXT,
				price REAL,
				currency TEXT DEFAULT 'EGP',
				expiry_date DATE,
				batch_number TEXT,
				notes TEXT,
				raw_message TEXT NOT NULL,
				status TEXT NOT NULL DEFAULT 'ACTIVE',
				created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
				updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
			);
			CREATE INDEX idx_offers_status ON offers(status);
			CREATE INDEX idx_offers_medication ON offers(medication);
			CREATE INDEX idx_offers_created ON offers(created_at);

			-- Medication requests
			CREATE TABLE requests (
				id TEXT PRIMARY KEY,
				raw_message_id TEXT REFERENCES raw_messages(id),
				source_phone TEXT NOT NULL,
				source_name TEXT,
				source_group TEXT NOT NULL,
				group_name TEXT,
				medication TEXT NOT NULL,
				medication_raw TEXT NOT NULL,
				quantity INTEGER DEFAULT 0,
				unit TEXT,
				max_price REAL,
				currency TEXT DEFAULT 'EGP',
				urgent BOOLEAN DEFAULT FALSE,
				notes TEXT,
				raw_message TEXT NOT NULL,
				status TEXT NOT NULL DEFAULT 'ACTIVE',
				created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
				updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
			);
			CREATE INDEX idx_requests_status ON requests(status);
			CREATE INDEX idx_requests_medication ON requests(medication);
			CREATE INDEX idx_requests_created ON requests(created_at);

			-- Matches between offers and requests
			CREATE TABLE matches (
				id TEXT PRIMARY KEY,
				offer_id TEXT NOT NULL REFERENCES offers(id),
				request_id TEXT NOT NULL REFERENCES requests(id),
				score REAL NOT NULL,
				reasoning TEXT,
				matched_by TEXT,
				status TEXT NOT NULL DEFAULT 'PENDING',
				created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
				confirmed_at DATETIME,
				notes TEXT,
				UNIQUE(offer_id, request_id)
			);
			CREATE INDEX idx_matches_status ON matches(status);
			CREATE INDEX idx_matches_offer ON matches(offer_id);
			CREATE INDEX idx_matches_request ON matches(request_id);

			-- Monitored WhatsApp groups
			CREATE TABLE groups (
				jid TEXT PRIMARY KEY,
				name TEXT NOT NULL,
				description TEXT,
				monitored BOOLEAN DEFAULT TRUE,
				added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
				last_message DATETIME,
				message_count INTEGER DEFAULT 0
			);
			CREATE INDEX idx_groups_monitored ON groups(monitored);
		`,
	},
	{
		version: 2,
		sql: `
			-- Full-text search for medications
			CREATE VIRTUAL TABLE offers_fts USING fts5(
				medication, medication_raw, notes,
				content='offers', content_rowid='rowid'
			);
			CREATE TRIGGER offers_ai AFTER INSERT ON offers BEGIN
				INSERT INTO offers_fts(rowid, medication, medication_raw, notes)
				VALUES (NEW.rowid, NEW.medication, NEW.medication_raw, NEW.notes);
			END;
			CREATE TRIGGER offers_ad AFTER DELETE ON offers BEGIN
				INSERT INTO offers_fts(offers_fts, rowid, medication, medication_raw, notes)
				VALUES('delete', OLD.rowid, OLD.medication, OLD.medication_raw, OLD.notes);
			END;
			CREATE TRIGGER offers_au AFTER UPDATE ON offers BEGIN
				INSERT INTO offers_fts(offers_fts, rowid, medication, medication_raw, notes)
				VALUES('delete', OLD.rowid, OLD.medication, OLD.medication_raw, OLD.notes);
				INSERT INTO offers_fts(rowid, medication, medication_raw, notes)
				VALUES (NEW.rowid, NEW.medication, NEW.medication_raw, NEW.notes);
			END;

			CREATE VIRTUAL TABLE requests_fts USING fts5(
				medication, medication_raw, notes,
				content='requests', content_rowid='rowid'
			);
			CREATE TRIGGER requests_ai AFTER INSERT ON requests BEGIN
				INSERT INTO requests_fts(rowid, medication, medication_raw, notes)
				VALUES (NEW.rowid, NEW.medication, NEW.medication_raw, NEW.notes);
			END;
			CREATE TRIGGER requests_ad AFTER DELETE ON requests BEGIN
				INSERT INTO requests_fts(requests_fts, rowid, medication, medication_raw, notes)
				VALUES('delete', OLD.rowid, OLD.medication, OLD.medication_raw, OLD.notes);
			END;
			CREATE TRIGGER requests_au AFTER UPDATE ON requests BEGIN
				INSERT INTO requests_fts(requests_fts, rowid, medication, medication_raw, notes)
				VALUES('delete', OLD.rowid, OLD.medication, OLD.medication_raw, OLD.notes);
				INSERT INTO requests_fts(rowid, medication, medication_raw, notes)
				VALUES (NEW.rowid, NEW.medication, NEW.medication_raw, NEW.notes);
			END;
		`,
	},
	{
		version: 3,
		sql: `
			-- Dynamic configuration storage
			CREATE TABLE config (
				key TEXT PRIMARY KEY,
				value TEXT NOT NULL,
				updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
			);
		`,
	},
	{
		version: 4,
		sql: `
			-- Composite indexes for performance optimization
			-- Optimizes GetActive queries that filter by status and order by created_at
			CREATE INDEX IF NOT EXISTS idx_offers_status_created ON offers(status, created_at DESC);
			CREATE INDEX IF NOT EXISTS idx_requests_status_created ON requests(status, created_at DESC);
			CREATE INDEX IF NOT EXISTS idx_matches_status_created ON matches(status, created_at DESC);
		`,
	},
	{
		version: 5,
		sql: `
			-- Medication name mappings (Arabic to English)
			CREATE TABLE medication_mappings (
				id TEXT PRIMARY KEY,
				arabic_name TEXT NOT NULL UNIQUE,
				english_name TEXT NOT NULL,
				created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
				updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
			);
			CREATE INDEX idx_medication_mappings_arabic ON medication_mappings(arabic_name);
		`,
	},
}
