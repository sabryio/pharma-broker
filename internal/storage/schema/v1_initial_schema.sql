-- 1. Core Messaging Tables
-- ---------------------------------------------------------
CREATE TABLE raw_messages (
    id TEXT PRIMARY KEY,
    external_id TEXT UNIQUE, -- WhatsApp Deduplication ID
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

-- 2. Marketplace Tables (Offers & Requests)
-- ---------------------------------------------------------
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
CREATE INDEX idx_offers_status_created ON offers(status, created_at DESC);
CREATE INDEX idx_offers_medication ON offers(medication);

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
CREATE INDEX idx_requests_status_created ON requests(status, created_at DESC);
CREATE INDEX idx_requests_medication ON requests(medication);

-- 3. Matching System
-- ---------------------------------------------------------
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
CREATE INDEX idx_matches_status_created ON matches(status, created_at DESC);
CREATE INDEX idx_matches_offer ON matches(offer_id);
CREATE INDEX idx_matches_request ON matches(request_id);

CREATE TABLE match_queue (
    id TEXT PRIMARY KEY,
    source_type TEXT NOT NULL, -- 'OFFER' or 'REQUEST'
    source_id TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_match_queue_created ON match_queue(created_at);

-- 4. Configuration & Monitoring
-- ---------------------------------------------------------
CREATE TABLE config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

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

-- 5. Full-Text Search (Offers & Requests)
-- ---------------------------------------------------------
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

-- 6. Dictionaries & Knowledge Base (with Vectors)
-- ---------------------------------------------------------
CREATE TABLE medication_mappings (
    id TEXT PRIMARY KEY,
    arabic_name TEXT NOT NULL UNIQUE,
    english_name TEXT NOT NULL,
    synonyms TEXT DEFAULT '[]',
    embedding BLOB, -- Vector Embedding (768-dim float32)
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_medication_mappings_arabic ON medication_mappings(arabic_name);

-- FTS for Medication Mappings (with Trigram for Fuzzy Search)
CREATE VIRTUAL TABLE medication_mappings_fts USING fts5(
    arabic_name, english_name, synonyms,
    content='medication_mappings', content_rowid='rowid',
    tokenize='trigram'
);

CREATE TRIGGER medication_mappings_ai AFTER INSERT ON medication_mappings BEGIN
    INSERT INTO medication_mappings_fts(rowid, arabic_name, english_name, synonyms)
    VALUES (NEW.rowid, NEW.arabic_name, NEW.english_name, NEW.synonyms);
END;

CREATE TRIGGER medication_mappings_ad AFTER DELETE ON medication_mappings BEGIN
    INSERT INTO medication_mappings_fts(medication_mappings_fts, rowid, arabic_name, english_name, synonyms)
    VALUES('delete', OLD.rowid, OLD.arabic_name, OLD.english_name, OLD.synonyms);
END;

CREATE TRIGGER medication_mappings_au AFTER UPDATE ON medication_mappings BEGIN
    INSERT INTO medication_mappings_fts(medication_mappings_fts, rowid, arabic_name, english_name, synonyms)
    VALUES('delete', OLD.rowid, OLD.arabic_name, OLD.english_name, OLD.synonyms);
    INSERT INTO medication_mappings_fts(rowid, arabic_name, english_name, synonyms)
    VALUES (NEW.rowid, NEW.arabic_name, NEW.english_name, NEW.synonyms);
END;

-- 7. Dead-Letter Queue for Failed Messages
-- ---------------------------------------------------------
CREATE TABLE failed_messages (
    id TEXT PRIMARY KEY,
    raw_message_id TEXT UNIQUE REFERENCES raw_messages(id),
    failure_reason TEXT NOT NULL,
    retry_count INTEGER DEFAULT 0,
    failed_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    resolved_at DATETIME
);
CREATE INDEX idx_failed_messages_unresolved ON failed_messages(resolved_at) WHERE resolved_at IS NULL;
CREATE INDEX idx_failed_messages_retry ON failed_messages(retry_count) WHERE resolved_at IS NULL;

-- 8. Match Feedback for Operator Learning Loop
-- ---------------------------------------------------------
CREATE TABLE match_feedback (
    id TEXT PRIMARY KEY,
    match_id TEXT NOT NULL REFERENCES matches(id),
    operator_id TEXT,
    decision TEXT NOT NULL, -- 'CONFIRMED', 'REJECTED'
    reason TEXT,
    original_score REAL NOT NULL,
    original_confidence TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_feedback_match ON match_feedback(match_id);
CREATE INDEX idx_feedback_decision ON match_feedback(decision);
CREATE INDEX idx_feedback_created ON match_feedback(created_at);

-- 9. Demand Leaderboard (Materialized View Pattern)
-- ---------------------------------------------------------
CREATE TABLE demand_leaderboard (
    medication TEXT PRIMARY KEY,
    request_count INTEGER NOT NULL DEFAULT 0,
    offer_count INTEGER NOT NULL DEFAULT 0,
    demand_ratio REAL NOT NULL DEFAULT 0,
    last_updated DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_leaderboard_ratio ON demand_leaderboard(demand_ratio DESC);

-- 10. Audit Logs
-- ---------------------------------------------------------
CREATE TABLE audit_logs (
    id TEXT PRIMARY KEY,
    action TEXT NOT NULL,
    entity_id TEXT,
    old_value TEXT,
    new_value TEXT,
    details TEXT,
    ip_address TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_audit_action ON audit_logs(action);
CREATE INDEX idx_audit_entity ON audit_logs(entity_id);
CREATE INDEX idx_audit_created ON audit_logs(created_at DESC);
