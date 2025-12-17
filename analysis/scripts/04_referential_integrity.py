"""
Phase 4: Referential Integrity Check
Verifies foreign key relationships and identifies orphaned records.
"""
import pandas as pd
from sqlalchemy import create_engine, text

from config import DATABASE_URL, REPORTS_DIR, FK_RELATIONSHIPS


def check_foreign_keys(engine):
    """Check all foreign key relationships."""
    results = []
    with engine.connect() as conn:
        for child_table, child_col, parent_table, parent_col in FK_RELATIONSHIPS:
            desc = f'{child_table}.{child_col} → {parent_table}.{parent_col}'

            # Count orphaned records
            query = f'''
                SELECT COUNT(*) FROM "{child_table}" c
                WHERE c."{child_col}" IS NOT NULL
                AND NOT EXISTS (
                    SELECT 1 FROM "{parent_table}" p
                    WHERE p."{parent_col}" = c."{child_col}"
                )
            '''
            orphan_count = conn.execute(text(query)).scalar()

            # Count total with FK
            total_query = f'SELECT COUNT(*) FROM "{child_table}" WHERE "{child_col}" IS NOT NULL'
            total = conn.execute(text(total_query)).scalar()

            results.append({
                'relationship': desc,
                'child_table': child_table,
                'parent_table': parent_table,
                'total_refs': total,
                'orphaned': orphan_count,
                'orphan_pct': round(orphan_count / total * 100, 2) if total > 0 else 0,
                'status': '✅ OK' if orphan_count == 0 else '❌ Orphans Found'
            })

    return pd.DataFrame(results)


def main():
    print("=" * 60)
    print("PHASE 4: REFERENTIAL INTEGRITY CHECK")
    print("=" * 60)

    engine = create_engine(DATABASE_URL)

    df_fk = check_foreign_keys(engine)
    print("\n🔗 FOREIGN KEY RELATIONSHIPS")
    print("-" * 40)
    print(df_fk.to_string(index=False))

    # Summary
    total_orphans = df_fk['orphaned'].sum()
    if total_orphans > 0:
        print(f"\n⚠️ TOTAL ORPHANED RECORDS: {total_orphans}")
    else:
        print("\n✅ All foreign key relationships are valid!")

    output_file = f'{REPORTS_DIR}/04_referential_integrity.csv'
    df_fk.to_csv(output_file, index=False)
    print(f"\n✅ Report saved to {output_file}")


if __name__ == '__main__':
    main()
