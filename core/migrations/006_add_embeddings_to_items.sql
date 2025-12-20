-- Add embedding columns to offers and requests for semantic deduplication
-- vector(768) assumes models like all-mpnet-base-v2 or similar. 
-- If using text-embedding-3-small, use 1536. We'll default to 768 to match medication_mappings.

ALTER TABLE offers ADD COLUMN IF NOT EXISTS content_embedding vector(768);
CREATE INDEX IF NOT EXISTS idx_offers_content_embedding ON offers USING hnsw (content_embedding vector_cosine_ops);

ALTER TABLE requests ADD COLUMN IF NOT EXISTS content_embedding vector(768);
CREATE INDEX IF NOT EXISTS idx_requests_content_embedding ON requests USING hnsw (content_embedding vector_cosine_ops);
