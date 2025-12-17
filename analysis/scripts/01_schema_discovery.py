"""
Phase 1: Database Schema Discovery
Connects to PostgreSQL and analyzes table structure.
"""
import pandas as pd
from sqlalchemy import create_engine, inspect, text
from sqlalchemy.exc import OperationalError, ProgrammingError
from tabulate import tabulate
import sys

from config import DATABASE_URL, REPORTS_DIR


def connect_db():
    """Create database connection with error handling."""
    try:
        engine = create_engine(DATABASE_URL, pool_pre_ping=True)
        # Test connection
        with engine.connect() as conn:
            conn.execute(text("SELECT 1"))
        print("✅ Database connection successful")
        return engine
    except OperationalError as e:
        print(f"❌ Connection failed: {e}")
        sys.exit(1)


def get_table_info(engine):
    """Get all tables with row counts and column counts."""
    inspector = inspect(engine)
    tables = inspector.get_table_names()

    results = []
    with engine.connect() as conn:
        for table in tables:
            try:
                result = conn.execute(text(f'SELECT COUNT(*) FROM "{table}"'))
                row_count = result.scalar()
                columns = inspector.get_columns(table)
                col_count = len(columns)
                nullable_cols = [c['name'] for c in columns if c['nullable']]

                results.append({
                    'table': table,
                    'rows': row_count,
                    'columns': col_count,
                    'nullable_columns': len(nullable_cols),
                    'nullable_list': ', '.join(nullable_cols[:5]) + ('...' if len(nullable_cols) > 5 else '')
                })
            except ProgrammingError as e:
                print(f"⚠️ Error reading {table}: {e}")

    return pd.DataFrame(results)


def get_column_details(engine, table_name):
    """Get detailed column information for a table."""
    inspector = inspect(engine)
    columns = inspector.get_columns(table_name)

    results = []
    for col in columns:
        results.append({
            'column': col['name'],
            'type': str(col['type']),
            'nullable': '✓' if col['nullable'] else '✗',
            'default': str(col.get('default', ''))[:30] if col.get('default') else '-'
        })

    return pd.DataFrame(results)


def main():
    print("=" * 60)
    print("PHASE 1: DATABASE SCHEMA DISCOVERY")
    print("=" * 60)

    engine = connect_db()

    # 1. Table Overview
    print("\n📊 TABLE OVERVIEW")
    print("-" * 40)
    df_tables = get_table_info(engine)
    print(tabulate(df_tables, headers='keys', tablefmt='grid', showindex=False))

    # 2. Total database size
    with engine.connect() as conn:
        result = conn.execute(text("""
            SELECT pg_size_pretty(pg_database_size(current_database())) as size
        """))
        db_size = result.scalar()
        print(f"\n💾 Database Size: {db_size}")

        # Table sizes
        result = conn.execute(text("""
            SELECT relname as table,
                   pg_size_pretty(pg_total_relation_size(relid)) as size
            FROM pg_catalog.pg_statio_user_tables
            ORDER BY pg_total_relation_size(relid) DESC
            LIMIT 5
        """))
        print("\n📦 Largest Tables:")
        for row in result:
            print(f"  {row[0]}: {row[1]}")

    # 3. Key tables detailed schema
    key_tables = ['raw_messages', 'offers', 'requests', 'matches']
    for table in key_tables:
        print(f"\n📋 {table.upper()} SCHEMA")
        print("-" * 40)
        df_cols = get_column_details(engine, table)
        print(tabulate(df_cols, headers='keys', tablefmt='simple', showindex=False))

    # Save results
    output_file = f'{REPORTS_DIR}/01_table_overview.csv'
    df_tables.to_csv(output_file, index=False)
    print(f"\n✅ Results saved to {output_file}")

    # Summary
    total_rows = df_tables['rows'].sum()
    print(f"\n📈 SUMMARY: {len(df_tables)} tables, {total_rows:,} total rows")


if __name__ == '__main__':
    main()
