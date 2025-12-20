-- Migration: 006_add_match_queue.sql
-- Creates the match_queue_items table for async matching

CREATE TABLE IF NOT EXISTS match_queue_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id VARCHAR(255) NOT NULL REFERENCES requests(id) ON DELETE CASCADE,
    status VARCHAR(50) NOT NULL DEFAULT 'PENDING',
    priority INT NOT NULL DEFAULT 0,
    attempts INT NOT NULL DEFAULT 0,
    last_error TEXT,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for efficient queue processing
CREATE INDEX IF NOT EXISTS idx_match_queue_status_priority 
    ON match_queue_items(status, priority DESC, next_attempt_at ASC)
    WHERE status = 'PENDING';

-- Index for finding items by request
CREATE INDEX IF NOT EXISTS idx_match_queue_request_id 
    ON match_queue_items(request_id);

-- Comment
COMMENT ON TABLE match_queue_items IS 'Queue for async request matching processing';
