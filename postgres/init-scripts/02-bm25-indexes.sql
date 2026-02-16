-- Create BM25 indexes for medication matching
-- This script runs after extensions are loaded but may run before migrations
-- It will create indexes only if tables exist

-- Function to create BM25 indexes if tables exist
CREATE OR REPLACE FUNCTION create_bm25_indexes_if_tables_exist()
RETURNS void AS $$
BEGIN
    -- Create BM25 index on offers.medication if table exists
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'offers') THEN
        -- Check if index already exists
        IF NOT EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'offers_medication_bm25_idx') THEN
            EXECUTE 'CREATE INDEX offers_medication_bm25_idx ON offers USING bm25(medication) WITH (text_config=''simple'')';
            RAISE NOTICE 'Created BM25 index on offers.medication';
        ELSE
            RAISE NOTICE 'BM25 index on offers.medication already exists';
        END IF;
    ELSE
        RAISE NOTICE 'Table offers does not exist yet, skipping BM25 index creation';
    END IF;

    -- Create BM25 index on requests.medication if table exists
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'requests') THEN
        -- Check if index already exists
        IF NOT EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'requests_medication_bm25_idx') THEN
            EXECUTE 'CREATE INDEX requests_medication_bm25_idx ON requests USING bm25(medication) WITH (text_config=''simple'')';
            RAISE NOTICE 'Created BM25 index on requests.medication';
        ELSE
            RAISE NOTICE 'BM25 index on requests.medication already exists';
        END IF;
    ELSE
        RAISE NOTICE 'Table requests does not exist yet, skipping BM25 index creation';
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Try to create indexes now (will succeed if tables exist)
SELECT create_bm25_indexes_if_tables_exist();

-- Note: If tables don't exist yet, they will be created by migrations
-- After migrations run, you can manually call: SELECT create_bm25_indexes_if_tables_exist();
