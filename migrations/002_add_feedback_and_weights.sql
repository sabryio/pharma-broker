-- Migration: Add feedback_records and weight_history tables for learning system
-- These tables enable the adaptive weight learning feature

-- Feedback Records Table
-- Stores user feedback (confirm/reject) on matches for learning
CREATE TABLE IF NOT EXISTS feedback_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    match_id UUID NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    confirmed BOOLEAN NOT NULL,
    -- Factor scores at the time of feedback
    medication_score DOUBLE PRECISION NOT NULL,
    dosage_score DOUBLE PRECISION NOT NULL,
    quantity_score DOUBLE PRECISION NOT NULL,
    price_score DOUBLE PRECISION NOT NULL,
    recency_score DOUBLE PRECISION NOT NULL,
    total_score DOUBLE PRECISION NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for date range queries (learning window)
CREATE INDEX idx_feedback_records_created_at ON feedback_records(created_at);
-- Index for confirmed/rejected aggregation
CREATE INDEX idx_feedback_records_confirmed ON feedback_records(confirmed);
-- Index for match lookups
CREATE INDEX idx_feedback_records_match_id ON feedback_records(match_id);
-- Unique constraint: one feedback per match
CREATE UNIQUE INDEX idx_feedback_records_match_unique ON feedback_records(match_id);

-- Weight History Table
-- Stores historical weight configurations for auditing and rollback
CREATE TABLE IF NOT EXISTS weight_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    medication_weight DOUBLE PRECISION NOT NULL,
    dosage_weight DOUBLE PRECISION NOT NULL,
    quantity_weight DOUBLE PRECISION NOT NULL,
    price_weight DOUBLE PRECISION NOT NULL,
    recency_weight DOUBLE PRECISION NOT NULL,
    -- Source: 'initial', 'manual', 'scheduler', 'api'
    source TEXT NOT NULL,
    -- Number of feedback samples used to calculate weights
    sample_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for getting most recent weights
CREATE INDEX idx_weight_history_created_at ON weight_history(created_at DESC);

-- Insert default initial weights if table is empty
INSERT INTO weight_history (medication_weight, dosage_weight, quantity_weight, price_weight, recency_weight, source, sample_count)
SELECT 0.35, 0.20, 0.15, 0.15, 0.15, 'initial', 0
WHERE NOT EXISTS (SELECT 1 FROM weight_history);
