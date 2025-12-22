-- PharmaBroker Initial Schema
-- Run on PostgreSQL with pgvector extension

-- Enable extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "vector";

-- Groups table (for monitoring whitelist)
CREATE TABLE IF NOT EXISTS groups (
    jid VARCHAR(50) PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    monitored BOOLEAN DEFAULT false,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_message TIMESTAMPTZ,
    message_count BIGINT DEFAULT 0
);

-- Raw messages table (incoming WhatsApp messages)
CREATE TABLE IF NOT EXISTS raw_messages (
    id VARCHAR(36) PRIMARY KEY,
    external_id VARCHAR(50),
    group_jid VARCHAR(50) NOT NULL,
    group_name VARCHAR(100),
    sender_jid VARCHAR(50) NOT NULL,
    sender_phone VARCHAR(20),
    sender_name VARCHAR(100),
    content TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    processed_at TIMESTAMPTZ,
    error TEXT,
    reply_to_id VARCHAR(50),
    reply_to_content TEXT,
    reply_to_sender VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_raw_messages_group_jid ON raw_messages(group_jid);
CREATE INDEX IF NOT EXISTS idx_raw_messages_processed_at ON raw_messages(processed_at);
CREATE INDEX IF NOT EXISTS idx_raw_messages_timestamp ON raw_messages(timestamp);

-- Offers table (medication supply offers)
CREATE TABLE IF NOT EXISTS offers (
    id VARCHAR(36) PRIMARY KEY,
    raw_message_id VARCHAR(36) REFERENCES raw_messages(id),
    source_phone VARCHAR(20) NOT NULL,
    source_name VARCHAR(100),
    source_group VARCHAR(50) NOT NULL,
    group_name VARCHAR(100),
    medication VARCHAR(200) NOT NULL,
    medication_raw VARCHAR(500),
    quantity DECIMAL(10,2) DEFAULT 0,
    unit VARCHAR(20),
    price DECIMAL(10,2) DEFAULT 0,
    currency VARCHAR(10) DEFAULT 'EGP',
    expiry_date DATE,
    batch_number VARCHAR(50),
    notes TEXT,
    raw_message TEXT,
    status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_offers_medication ON offers(medication);
CREATE INDEX IF NOT EXISTS idx_offers_status ON offers(status);
CREATE INDEX IF NOT EXISTS idx_offers_source_phone ON offers(source_phone);

-- Requests table (medication demand requests)
CREATE TABLE IF NOT EXISTS requests (
    id VARCHAR(36) PRIMARY KEY,
    raw_message_id VARCHAR(36) REFERENCES raw_messages(id),
    source_phone VARCHAR(20) NOT NULL,
    source_name VARCHAR(100),
    source_group VARCHAR(50) NOT NULL,
    group_name VARCHAR(100),
    medication VARCHAR(200) NOT NULL,
    medication_raw VARCHAR(500),
    quantity DECIMAL(10,2) DEFAULT 0,
    unit VARCHAR(20),
    max_price DECIMAL(10,2) DEFAULT 0,
    currency VARCHAR(10) DEFAULT 'EGP',
    urgent BOOLEAN DEFAULT false,
    notes TEXT,
    raw_message TEXT,
    status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_requests_medication ON requests(medication);
CREATE INDEX IF NOT EXISTS idx_requests_status ON requests(status);
CREATE INDEX IF NOT EXISTS idx_requests_source_phone ON requests(source_phone);

-- Matches table (offer-request matches)
CREATE TABLE IF NOT EXISTS matches (
    id VARCHAR(36) PRIMARY KEY,
    offer_id VARCHAR(36) NOT NULL REFERENCES offers(id),
    request_id VARCHAR(36) NOT NULL REFERENCES requests(id),
    score DECIMAL(5,2) NOT NULL,
    reasoning TEXT,
    matched_by VARCHAR(50),
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ,
    notes TEXT,
    UNIQUE(offer_id, request_id)
);

CREATE INDEX IF NOT EXISTS idx_matches_status ON matches(status);
CREATE INDEX IF NOT EXISTS idx_matches_offer_id ON matches(offer_id);
CREATE INDEX IF NOT EXISTS idx_matches_request_id ON matches(request_id);

-- Insert a test monitored group (optional)
-- INSERT INTO groups (jid, name, monitored) VALUES ('test@g.us', 'Test Group', true);
