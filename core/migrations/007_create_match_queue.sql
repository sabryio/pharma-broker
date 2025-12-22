CREATE TABLE match_queue_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES requests(id),
    status TEXT NOT NULL DEFAULT 'PENDING', -- PENDING, PROCESSING, COMPLETED, FAILED
    priority INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_match_queue_pending ON match_queue_items(status, next_attempt_at) WHERE status IN ('PENDING', 'FAILED');
CREATE INDEX idx_match_queue_request_id ON match_queue_items(request_id);
