-- Review Queue Migration
-- Task 3.3: Create review_queue table for AI parse results requiring human review

-- Create the review queue table
CREATE TABLE IF NOT EXISTS review_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    raw_message_id VARCHAR(255) NOT NULL,
    ai_result JSONB NOT NULL,
    confidence DOUBLE PRECISION NOT NULL,
    reason VARCHAR(500) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    reviewed_by VARCHAR(255),
    review_notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_at TIMESTAMPTZ,
    
    -- Constraints
    CONSTRAINT chk_status CHECK (status IN ('pending', 'approved', 'rejected', 'skipped')),
    CONSTRAINT chk_confidence CHECK (confidence >= 0.0 AND confidence <= 1.0)
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_review_queue_status ON review_queue(status);
CREATE INDEX IF NOT EXISTS idx_review_queue_created_at ON review_queue(created_at);
CREATE INDEX IF NOT EXISTS idx_review_queue_raw_message_id ON review_queue(raw_message_id);
CREATE INDEX IF NOT EXISTS idx_review_queue_confidence ON review_queue(confidence);

-- Composite index for pending items sorted by creation time
CREATE INDEX IF NOT EXISTS idx_review_queue_pending_created 
    ON review_queue(status, created_at) 
    WHERE status = 'pending';

-- Comments for documentation
COMMENT ON TABLE review_queue IS 'Stores AI parse results that require human review due to low confidence';
COMMENT ON COLUMN review_queue.raw_message_id IS 'Reference to the original raw message';
COMMENT ON COLUMN review_queue.ai_result IS 'The AI parse result as JSON';
COMMENT ON COLUMN review_queue.confidence IS 'Average confidence score from AI (0.0 - 1.0)';
COMMENT ON COLUMN review_queue.reason IS 'Reason for queuing (e.g., low_confidence, ambiguous_medication)';
COMMENT ON COLUMN review_queue.status IS 'Current review status: pending, approved, rejected, skipped';
COMMENT ON COLUMN review_queue.reviewed_by IS 'Identifier of the reviewer';
COMMENT ON COLUMN review_queue.review_notes IS 'Notes from the reviewer';
