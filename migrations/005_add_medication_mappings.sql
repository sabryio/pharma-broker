-- Add medication_mappings table and enable trigram search
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE IF NOT EXISTS medication_mappings (
    id VARCHAR(36) PRIMARY KEY,
    arabic_name TEXT NOT NULL,
    english_name TEXT NOT NULL,
    synonyms TEXT[], -- Array of synonyms
    embedding vector(768), -- Adjust dimension if needed (e.g., 768 for small models)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create GIN index for trigram similarity search
CREATE INDEX IF NOT EXISTS idx_medication_mappings_arabic_trgm ON medication_mappings USING gin (arabic_name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_medication_mappings_english_trgm ON medication_mappings USING gin (english_name gin_trgm_ops);

-- Create HNSW index for vector similarity search (optional, if using pgvector)
CREATE INDEX IF NOT EXISTS idx_medication_mappings_embedding ON medication_mappings USING hnsw (embedding vector_cosine_ops);
