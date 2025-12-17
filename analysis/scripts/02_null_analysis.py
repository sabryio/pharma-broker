"""
Phase 2: Null Value Analysis
Identifies NULL values and their distribution across all tables.
"""
import pandas as pd
from sqlalchemy import create_engine, text

from config import DATABASE_URL, REPORTS_DIR, EXPECTED_NULLS


def analyze_nulls(engine, table_name):
    """Analyze NULL values for each column in a table."""
    with engine.connect() as conn:
        result = conn.execute(text(f"""
            SELECT column_name FROM information_schema.columns
            WHERE table_name = '{table_name}' ORDER BY ordinal_position
        """))
        columns = [row[0] for row in result]

        total = conn.execute(text(f'SELECT COUNT(*) FROM "{table_name}"')).scalar()
        if total == 0:
            return pd.DataFrame()

        results = []
        for col in columns:
            null_count = conn.execute(text(f'''
                SELECT COUNT(*) FROM "{table_name}" WHERE "{col}" IS NULL
            ''')).scalar()

            results.append({
                'table': table_name,
                'column': col,
                'total_rows': total,
                'null_count': null_count,
                'null_pct': round(null_count / total * 100, 2) if total > 0 else 0,
                'has_data': total - null_count
            })

        return pd.DataFrame(results)


def categorize_nulls(df):
    """Categorize NULL values as expected or problematic."""
    def is_expected(row):
        table = row['table']
        col = row['column']
        if table in EXPECTED_NULLS and col in EXPECTED_NULLS[table]:
            return 'Expected'
        elif row['null_pct'] == 0:
            return 'No Nulls'
        elif row['null_pct'] > 50:
            return '⚠️ High Null Rate'
        else:
            return '🔍 Review'

    df['status'] = df.apply(is_expected, axis=1)
    return df


def main():
    print("=" * 60)
    print("PHASE 2: NULL VALUE ANALYSIS")
    print("=" * 60)

    engine = create_engine(DATABASE_URL)

    with engine.connect() as conn:
        result = conn.execute(text("""
            SELECT table_name FROM information_schema.tables
            WHERE table_schema = 'public' AND table_type = 'BASE TABLE'
        """))
        tables = [row[0] for row in result]

    all_results = []
    for table in tables:
        print(f"Analyzing {table}...")
        df = analyze_nulls(engine, table)
        if not df.empty:
            all_results.append(df)

    df_all = pd.concat(all_results, ignore_index=True)
    df_all = categorize_nulls(df_all)

    # Summary
    print("\n📊 NULL VALUE SUMMARY BY STATUS")
    print("-" * 40)
    summary = df_all.groupby('status').agg({
        'column': 'count',
        'null_count': 'sum'
    }).rename(columns={'column': 'columns_affected'})
    print(summary)

    # Problems
    print("\n⚠️ COLUMNS REQUIRING ATTENTION")
    print("-" * 40)
    problems = df_all[df_all['status'].isin(['⚠️ High Null Rate', '🔍 Review'])]
    problems = problems[problems['null_pct'] > 0].sort_values('null_pct', ascending=False)
    if not problems.empty:
        print(problems[['table', 'column', 'null_count', 'null_pct', 'status']].head(20).to_string(index=False))
    else:
        print("✅ No problematic columns found!")

    # Save
    output_file = f'{REPORTS_DIR}/02_null_analysis.csv'
    df_all.to_csv(output_file, index=False)
    print(f"\n✅ Full report saved to {output_file}")


if __name__ == '__main__':
    main()
