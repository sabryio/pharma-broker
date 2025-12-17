"""
Phase 3: Data Quality Assessment
Checks for duplicates, invalid values, and data consistency.
"""
import pandas as pd
from sqlalchemy import create_engine, text

from config import DATABASE_URL, REPORTS_DIR, VALID_STATUSES


def check_duplicates(engine):
    """Check for duplicate records in key tables."""
    checks = [
        ('raw_messages', 'external_id', 'Duplicate WhatsApp message IDs'),
        ('offers', 'raw_message_id, medication', 'Duplicate offers from same message'),
        ('requests', 'raw_message_id, medication', 'Duplicate requests from same message'),
        ('matches', 'offer_id, request_id', 'Duplicate matches'),
        ('medication_mappings', 'arabic_name', 'Duplicate Arabic medication names'),
    ]

    results = []
    with engine.connect() as conn:
        for table, columns, description in checks:
            query = f'''
                SELECT COUNT(*) as dup_count FROM (
                    SELECT {columns}, COUNT(*) as cnt FROM "{table}"
                    WHERE {columns.split(',')[0].strip()} IS NOT NULL
                    GROUP BY {columns} HAVING COUNT(*) > 1
                ) sub
            '''
            try:
                dup_count = conn.execute(text(query)).scalar() or 0
                results.append({
                    'table': table,
                    'check': description,
                    'duplicates': dup_count,
                    'status': '✅ OK' if dup_count == 0 else '⚠️ Has Duplicates'
                })
            except Exception as e:
                results.append({
                    'table': table, 'check': description,
                    'duplicates': 'Error', 'status': f'❌ {str(e)[:30]}'
                })
    return pd.DataFrame(results)


def check_status_values(engine):
    """Validate status field values against expected enums."""
    results = []
    with engine.connect() as conn:
        for table, valid_values in VALID_STATUSES.items():
            query = f'SELECT status, COUNT(*) as cnt FROM "{table}" GROUP BY status'
            try:
                df = pd.read_sql(query, conn)
                for _, row in df.iterrows():
                    status = row['status']
                    is_valid = status in valid_values
                    results.append({
                        'table': table, 'column': 'status',
                        'value': status, 'count': row['cnt'],
                        'valid': '✅' if is_valid else '❌ Invalid'
                    })
            except Exception:
                pass
    return pd.DataFrame(results)


def check_score_ranges(engine):
    """Validate score values are within expected ranges (0-1)."""
    checks = [
        ('matches', 'score'),
        ('feedback_records', 'medication_score'),
        ('feedback_records', 'total_score'),
        ('review_queue', 'avg_confidence'),
    ]

    results = []
    with engine.connect() as conn:
        for table, column in checks:
            query = f'''
                SELECT MIN("{column}") as min_val, MAX("{column}") as max_val,
                       AVG("{column}") as avg_val, COUNT(*) as total,
                       SUM(CASE WHEN "{column}" < 0 OR "{column}" > 1 THEN 1 ELSE 0 END) as out_of_range
                FROM "{table}"
            '''
            try:
                row = conn.execute(text(query)).fetchone()
                results.append({
                    'table': table, 'column': column,
                    'min': round(row[0], 4) if row[0] else None,
                    'max': round(row[1], 4) if row[1] else None,
                    'avg': round(row[2], 4) if row[2] else None,
                    'out_of_range': row[4],
                    'status': '✅' if row[4] == 0 else '⚠️'
                })
            except Exception as e:
                results.append({
                    'table': table, 'column': column,
                    'min': None, 'max': None, 'avg': None,
                    'out_of_range': 'Error', 'status': f'❌ {str(e)[:20]}'
                })
    return pd.DataFrame(results)


def check_empty_strings(engine):
    """Check for empty strings in required text fields."""
    checks = [
        ('raw_messages', 'content'),
        ('offers', 'medication'),
        ('offers', 'medication_raw'),
        ('requests', 'medication'),
        ('groups', 'name'),
    ]

    results = []
    with engine.connect() as conn:
        for table, column in checks:
            query = f'''
                SELECT COUNT(*) as total,
                       SUM(CASE WHEN "{column}" = '' THEN 1 ELSE 0 END) as empty_count
                FROM "{table}"
            '''
            row = conn.execute(text(query)).fetchone()
            results.append({
                'table': table, 'column': column,
                'total': row[0], 'empty_strings': row[1],
                'status': '✅' if row[1] == 0 else '⚠️ Has Empty Strings'
            })
    return pd.DataFrame(results)


def main():
    print("=" * 60)
    print("PHASE 3: DATA QUALITY ASSESSMENT")
    print("=" * 60)

    engine = create_engine(DATABASE_URL)

    print("\n🔍 DUPLICATE CHECK")
    df_dups = check_duplicates(engine)
    print(df_dups.to_string(index=False))

    print("\n📋 STATUS VALUE VALIDATION")
    df_status = check_status_values(engine)
    if not df_status.empty:
        print(df_status.to_string(index=False))

    print("\n📊 SCORE RANGE VALIDATION (0-1)")
    df_scores = check_score_ranges(engine)
    print(df_scores.to_string(index=False))

    print("\n📝 EMPTY STRING CHECK")
    df_empty = check_empty_strings(engine)
    print(df_empty.to_string(index=False))

    # Calculate quality score
    dup_issues = len(df_dups[df_dups['status'] != '✅ OK'])
    status_issues = len(df_status[df_status['valid'] != '✅']) if not df_status.empty else 0
    score_issues = len(df_scores[df_scores['status'] != '✅'])
    empty_issues = len(df_empty[df_empty['status'] != '✅'])

    total_checks = len(df_dups) + len(df_status) + len(df_scores) + len(df_empty)
    total_issues = dup_issues + status_issues + score_issues + empty_issues
    quality_score = ((total_checks - total_issues) / total_checks * 100) if total_checks > 0 else 100

    print(f"\n📈 DATA QUALITY SCORE: {quality_score:.1f}%")

    # Save
    output_file = f'{REPORTS_DIR}/03_data_quality.xlsx'
    with pd.ExcelWriter(output_file) as writer:
        df_dups.to_excel(writer, sheet_name='Duplicates', index=False)
        if not df_status.empty:
            df_status.to_excel(writer, sheet_name='Status Values', index=False)
        df_scores.to_excel(writer, sheet_name='Score Ranges', index=False)
        df_empty.to_excel(writer, sheet_name='Empty Strings', index=False)

    print(f"✅ Full report saved to {output_file}")


if __name__ == '__main__':
    main()
