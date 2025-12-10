#!/bin/bash
set -e

# This script runs when the PostgreSQL container is first initialized
# It only enables the pgvector extension

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    -- Enable pgvector extension (required for vector operations)
    CREATE EXTENSION IF NOT EXISTS vector;
EOSQL

echo "PostgreSQL initialized with pgvector extension!"
