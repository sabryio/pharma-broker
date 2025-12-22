-- Add urgency level and expiry fields for AI extraction
-- Migration: 007_add_urgency_and_expiry.sql

-- Create urgency level enum type
DO $$ BEGIN
    CREATE TYPE urgency_level AS ENUM ('NORMAL', 'SOON', 'URGENT', 'CRITICAL');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Add urgency_level to requests table (complements existing urgent boolean)
ALTER TABLE requests 
ADD COLUMN IF NOT EXISTS urgency_level urgency_level DEFAULT 'NORMAL';

-- Add expiry info to requests (when requester specifies expiry requirements)
ALTER TABLE requests 
ADD COLUMN IF NOT EXISTS expiry_requirement VARCHAR(50);

-- Add urgency fields to offers table (seller may indicate urgency to sell)
ALTER TABLE offers 
ADD COLUMN IF NOT EXISTS urgent BOOLEAN DEFAULT false;

ALTER TABLE offers 
ADD COLUMN IF NOT EXISTS urgency_level urgency_level DEFAULT 'NORMAL';

-- Add expiry_info to offers (more flexible than expiry_date for AI extraction)
-- Can store "2025-06", "long expiry", "6 months", etc.
ALTER TABLE offers 
ADD COLUMN IF NOT EXISTS expiry_info VARCHAR(50);

-- Add AI confidence score to both tables
ALTER TABLE offers 
ADD COLUMN IF NOT EXISTS ai_confidence DOUBLE PRECISION DEFAULT 0;

ALTER TABLE requests 
ADD COLUMN IF NOT EXISTS ai_confidence DOUBLE PRECISION DEFAULT 0;

-- Create indexes for urgency filtering
CREATE INDEX IF NOT EXISTS idx_requests_urgency_level ON requests(urgency_level);
CREATE INDEX IF NOT EXISTS idx_requests_urgent ON requests(urgent);
CREATE INDEX IF NOT EXISTS idx_offers_urgency_level ON offers(urgency_level);
CREATE INDEX IF NOT EXISTS idx_offers_urgent ON offers(urgent);

-- Update existing records: sync urgency_level with urgent boolean
UPDATE requests 
SET urgency_level = 'URGENT' 
WHERE urgent = true AND urgency_level = 'NORMAL';

COMMENT ON COLUMN requests.urgency_level IS 'Granular urgency: NORMAL, SOON, URGENT, CRITICAL';
COMMENT ON COLUMN requests.expiry_requirement IS 'Expiry requirement from AI: YYYY-MM or description';
COMMENT ON COLUMN offers.urgency_level IS 'Seller urgency to sell: NORMAL, SOON, URGENT, CRITICAL';
COMMENT ON COLUMN offers.expiry_info IS 'Expiry info from AI: YYYY-MM or description like "long expiry"';
COMMENT ON COLUMN offers.ai_confidence IS 'AI extraction confidence score 0.00-1.00';
COMMENT ON COLUMN requests.ai_confidence IS 'AI extraction confidence score 0.00-1.00';
