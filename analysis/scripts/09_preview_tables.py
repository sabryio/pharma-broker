"""
Preview all database tables - shows first 5 rows and exports full tables to CSV.
Usage: uv run scripts/09_preview_tables.py
"""

import pandas as pd
from sqlalchemy import create_engine, text
import sys
import os
from pathlib import Path

# Add parent directory to path for config import
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from config import DATABASE_URL, REPORTS_DIR


def get_all_tables(engine):
    """Get list of all tables in public schema."""
    with engine.connect() as conn:
        result = conn.execute(
            text("""
            SELECT table_name FROM information_schema.tables
            WHERE table_schema = 'public' AND table_type = 'BASE TABLE'
            ORDER BY table_name
        """)
        )
        return [row[0] for row in result]


def preview_table(engine, table_name: str, rows: int = 5):
    """Print first N rows of a table."""
    try:
        df = pd.read_sql(f'SELECT * FROM "{table_name}" LIMIT {rows}', engine)

        print(f"\n{'=' * 80}")
        print(f"📋 {table_name.upper()} ({len(df)} rows shown)")
        print("=" * 80)

        if df.empty:
            print("  (empty table)")
            return

        # Truncate long text columns for display
        display_df = df.copy()
        for col in display_df.columns:
            if display_df[col].dtype == "object":
                display_df[col] = display_df[col].astype(str).str[:50] + display_df[
                    col
                ].astype(str).str[50:].apply(lambda x: "..." if x else "")

        pd.set_option("display.max_columns", None)
        pd.set_option("display.width", None)
        pd.set_option("display.max_colwidth", 50)

        print(display_df.to_string(index=False))

    except Exception as e:
        print(f"\n❌ Error reading {table_name}: {e}")


def export_table_to_csv(engine, table_name: str, output_dir: Path):
    """Export full table to CSV file."""
    try:
        df = pd.read_sql(f'SELECT * FROM "{table_name}"', engine)
        output_file = output_dir / f"{table_name}.csv"
        df.to_csv(output_file, index=False)
        return len(df)
    except Exception as e:
        print(f"  ❌ Error exporting {table_name}: {e}")
        return -1


def main():
    print("=" * 80)
    print("DATABASE TABLE PREVIEW & EXPORT")
    print("=" * 80)

    engine = create_engine(DATABASE_URL)
    tables = get_all_tables(engine)

    # Create output directory
    tables_dir = Path(REPORTS_DIR) / "tables"
    tables_dir.mkdir(parents=True, exist_ok=True)

    print(f"\n📊 Found {len(tables)} tables")
    print(f"📁 Exporting to: {tables_dir}")

    # Key tables to show first
    priority_tables = ["raw_messages", "offers", "requests", "matches", "groups"]

    # Show priority tables first
    for table in priority_tables:
        if table in tables:
            preview_table(engine, table)

    # Export all tables to CSV
    print("\n" + "=" * 80)
    print("💾 EXPORTING TABLES TO CSV")
    print("=" * 80)

    export_summary = []
    for table in tables:
        row_count = export_table_to_csv(engine, table, tables_dir)
        if row_count >= 0:
            export_summary.append({"table": table, "rows": row_count})
            print(f"  ✅ {table}.csv ({row_count:,} rows)")

    # Save summary
    summary_df = pd.DataFrame(export_summary)
    summary_df.to_csv(tables_dir / "_summary.csv", index=False)

    print("\n" + "=" * 80)
    print(f"✅ Exported {len(export_summary)} tables to {tables_dir}")
    print(f"📋 Summary saved to {tables_dir / '_summary.csv'}")
    print("=" * 80)


if __name__ == "__main__":
    main()
