-- Initialize PostgreSQL extensions for pharmabroker
-- This script runs automatically when the database is first created

-- Enable pgvector extension (for embeddings)
CREATE EXTENSION IF NOT EXISTS vector;

-- Enable pg_textsearch extension (for BM25 full-text search)
-- Note: This is a prerelease version (v1.0.0-dev)
CREATE EXTENSION IF NOT EXISTS pg_textsearch;

-- Enable pg_trgm for trigram similarity (useful for fuzzy matching)
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Verify extensions are installed
SELECT 
    extname AS extension_name,
    extversion AS version
FROM pg_extension
WHERE extname IN ('vector', 'pg_textsearch', 'pg_trgm')
ORDER BY extname;
