-- Add audit_logs table
-- Task 5.3: Audit Logging

CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    actor TEXT NOT NULL,
    details JSONB,
    ip_address TEXT,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for entity-based lookups
CREATE INDEX IF NOT EXISTS idx_audit_logs_entity ON audit_logs (entity_type, entity_id);

-- Index for actor-based lookups
CREATE INDEX IF NOT EXISTS idx_audit_logs_actor ON audit_logs (actor);

-- Index for action-based lookups
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs (action);

-- Index for time-based lookups
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs (created_at DESC);

-- Comments for documentation
COMMENT ON TABLE audit_logs IS 'Stores audit trail for compliance and debugging';
COMMENT ON COLUMN audit_logs.action IS 'The action performed (e.g., match_confirmed)';
COMMENT ON COLUMN audit_logs.entity_type IS 'Type of entity affected (e.g., match, weights)';
COMMENT ON COLUMN audit_logs.entity_id IS 'ID of the affected entity';
COMMENT ON COLUMN audit_logs.actor IS 'Who performed the action (user ID or system)';
