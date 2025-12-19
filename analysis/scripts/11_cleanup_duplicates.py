#!/usr/bin/env python3
"""
Phase 2: Cleanup Duplicate Offers
Safely marks duplicate offers as EXPIRED while keeping the oldest one.
"""

import sys
from datetime import datetime
from pathlib import Path

import pandas as pd
from sqlalchemy import create_engine, text
from tabulate import tabulate

# Add parent to path for config
sys.path.insert(0, str(Path(__file__).parent.parent))
from config import DATABASE_URL


def main():
    print("=" * 70)
    print("🧹 PHASE 2: DUPLICATE OFFERS CLEANUP")
    print("=" * 70)
    print(f"Started: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n")

    engine = create_engine(DATABASE_URL)

    # 1. Pre-cleanup stats
    print("1️⃣  PRE-CLEANUP STATISTICS")
    print("-" * 50)

    with engine.connect() as conn:
        stats = conn.execute(
            text("""
            SELECT 
                (SELECT COUNT(*) FROM offers WHERE status = 'ACTIVE') as active,
                (SELECT COUNT(*) FROM offers WHERE status = 'EXPIRED') as expired,
                (SELECT COUNT(*) FROM offers WHERE status = 'MATCHED') as matched
        """)
        ).fetchone()

        print(f"  Active offers:  {stats[0]}")
        print(f"  Expired offers: {stats[1]}")
        print(f"  Matched offers: {stats[2]}")
    print()

    # 2. Identify duplicates to expire
    print("2️⃣  IDENTIFYING DUPLICATES TO EXPIRE")
    print("-" * 50)

    # Find all duplicate IDs (keeping oldest)
    dup_query = text("""
        WITH ranked AS (
            SELECT 
                id,
                source_phone,
                medication,
                created_at,
                ROW_NUMBER() OVER (
                    PARTITION BY source_phone, LOWER(medication)
                    ORDER BY created_at ASC
                ) as rn
            FROM offers
            WHERE status = 'ACTIVE'
        )
        SELECT id, source_phone, medication, created_at
        FROM ranked
        WHERE rn > 1
        ORDER BY source_phone, medication, created_at
    """)

    df_to_expire = pd.read_sql(dup_query, engine)

    print(f"  Offers to mark as EXPIRED: {len(df_to_expire)}")

    if df_to_expire.empty:
        print("\n  ✅ No duplicates to clean up!")
        return

    # Show sample
    print("\n  Sample of offers to expire:")
    sample = df_to_expire.head(10).copy()
    sample["created_at"] = pd.to_datetime(sample["created_at"]).dt.strftime(
        "%m-%d %H:%M"
    )
    print(tabulate(sample, headers="keys", tablefmt="simple", showindex=False))
    print()

    # 3. Confirm before proceeding
    print("3️⃣  CONFIRMATION")
    print("-" * 50)
    print(f"  This will mark {len(df_to_expire)} duplicate offers as EXPIRED.")
    print("  The oldest offer in each group will be preserved.\n")

    confirm = input("  Proceed with cleanup? (yes/no): ").strip().lower()

    if confirm != "yes":
        print("\n  ❌ Cleanup cancelled.")
        return
    print()

    # 4. Create backup
    print("4️⃣  CREATING BACKUP")
    print("-" * 50)

    backup_table = f"offers_backup_{datetime.now().strftime('%Y%m%d_%H%M%S')}"

    with engine.connect() as conn:
        conn.execute(text(f"CREATE TABLE {backup_table} AS SELECT * FROM offers"))
        conn.commit()

        backup_count = conn.execute(
            text(f"SELECT COUNT(*) FROM {backup_table}")
        ).fetchone()[0]
        print(f"  Created backup table: {backup_table}")
        print(f"  Backup contains {backup_count} offers")
    print()

    # 5. Execute cleanup
    print("5️⃣  EXECUTING CLEANUP")
    print("-" * 50)

    cleanup_query = text("""
        WITH to_expire AS (
            SELECT id FROM (
                SELECT 
                    id,
                    ROW_NUMBER() OVER (
                        PARTITION BY source_phone, LOWER(medication)
                        ORDER BY created_at ASC
                    ) as rn
                FROM offers
                WHERE status = 'ACTIVE'
            ) ranked
            WHERE rn > 1
        )
        UPDATE offers
        SET status = 'EXPIRED', updated_at = NOW()
        WHERE id IN (SELECT id FROM to_expire)
        RETURNING id
    """)

    with engine.connect() as conn:
        result = conn.execute(cleanup_query)
        expired_ids = result.fetchall()
        conn.commit()

        print(f"  ✅ Marked {len(expired_ids)} offers as EXPIRED")
    print()

    # 6. Post-cleanup verification
    print("6️⃣  POST-CLEANUP VERIFICATION")
    print("-" * 50)

    with engine.connect() as conn:
        # New stats
        new_stats = conn.execute(
            text("""
            SELECT 
                (SELECT COUNT(*) FROM offers WHERE status = 'ACTIVE') as active,
                (SELECT COUNT(*) FROM offers WHERE status = 'EXPIRED') as expired
        """)
        ).fetchone()

        print(f"  Active offers now:  {new_stats[0]}")
        print(f"  Expired offers now: {new_stats[1]}")

        # Check for remaining duplicates
        remaining = conn.execute(
            text("""
            SELECT COUNT(*) FROM (
                SELECT source_phone, LOWER(medication)
                FROM offers WHERE status = 'ACTIVE'
                GROUP BY source_phone, LOWER(medication)
                HAVING COUNT(*) > 1
            ) dup
        """)
        ).fetchone()[0]

        if remaining == 0:
            print("\n  ✅ No remaining duplicates!")
        else:
            print(f"\n  ⚠️ Remaining duplicate groups: {remaining}")
    print()

    # 7. Export cleanup report
    print("7️⃣  EXPORTING CLEANUP REPORT")
    print("-" * 50)

    report_dir = Path(__file__).parent.parent / "reports"
    report_dir.mkdir(exist_ok=True)

    df_to_expire["action"] = "EXPIRED"
    df_to_expire["cleanup_date"] = datetime.now().isoformat()

    report_path = (
        report_dir / f"cleanup_report_{datetime.now().strftime('%Y%m%d_%H%M%S')}.csv"
    )
    df_to_expire.to_csv(report_path, index=False)
    print("  Exported cleanup report to:")
    print(f"  📁 {report_path}")
    print()

    print("=" * 70)
    print("✅ CLEANUP COMPLETE")
    print("=" * 70)
    print(f"\n  Backup table: {backup_table}")
    print(
        "  To rollback: DELETE FROM offers; INSERT INTO offers SELECT * FROM "
        + backup_table
        + ";"
    )


if __name__ == "__main__":
    main()
