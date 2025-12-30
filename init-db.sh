#!/bin/bash
set -e

# =============================================================================
# PostgreSQL Initialization Script for PharmaBroker
# =============================================================================
# This script runs when the PostgreSQL container is first initialized.
# It creates both required databases and enables necessary extensions.
# =============================================================================

echo "Initializing PharmaBroker databases..."

# Create main application database (if not using default)
if [ "$POSTGRES_DB" != "pharmabroker" ]; then
    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" <<-EOSQL
        CREATE DATABASE pharmabroker;
EOSQL
    echo "Created database: pharmabroker"
fi

# Create WhatsApp sessions database
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" <<-EOSQL
    CREATE DATABASE whatsapp_sessions;
EOSQL
echo "Created database: whatsapp_sessions"

# Create Convex self-hosted database
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" <<-EOSQL
    CREATE DATABASE convex_self_hosted;
EOSQL
echo "Created database: convex_self_hosted"

# Enable extensions on pharmabroker database
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "pharmabroker" <<-EOSQL
    -- Enable pgvector extension for vector similarity search
    CREATE EXTENSION IF NOT EXISTS vector;
    
    -- Enable pg_trgm for fuzzy text matching
    CREATE EXTENSION IF NOT EXISTS pg_trgm;
    
    -- Enable unaccent for accent-insensitive search
    CREATE EXTENSION IF NOT EXISTS unaccent;
EOSQL
echo "Enabled extensions on pharmabroker: vector, pg_trgm, unaccent"

# Enable extensions on whatsapp_sessions database
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "whatsapp_sessions" <<-EOSQL
    -- No special extensions needed for WhatsApp sessions
    -- Just ensure the database is ready
    SELECT 1;
EOSQL
echo "Configured whatsapp_sessions database"

echo "============================================="
echo "PostgreSQL initialization complete!"
echo "  - pharmabroker:        Ready (with vector, pg_trgm, unaccent)"
echo "  - whatsapp_sessions:   Ready"
echo "  - convex_self_hosted:  Ready (for Convex backend)"
echo "============================================="
