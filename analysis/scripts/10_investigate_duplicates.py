#!/usr/bin/env python3
"""
Phase 1: Duplicate Offers Investigation
Analyzes duplicate offers to understand root cause and prepare for cleanup.
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
    print("📊 PHASE 1: DUPLICATE OFFERS INVESTIGATION")
    print("=" * 70)
    print(f"Started: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n")

    engine = create_engine(DATABASE_URL)

    # 1. Overall duplicate count
    print("1️⃣  DUPLICATE SUMMARY")
    print("-" * 50)

    summary_query = text("""
        SELECT 
            COUNT(*) as total_active_offers,
            (SELECT COUNT(*) FROM (
                SELECT source_phone, LOWER(medication)
                FROM offers WHERE status = 'ACTIVE'
                GROUP BY source_phone, LOWER(medication)
                HAVING COUNT(*) > 1
            ) dup) as duplicate_groups,
            (SELECT COUNT(*) - COUNT(DISTINCT (source_phone, LOWER(medication)))
             FROM offers WHERE status = 'ACTIVE') as duplicate_count
        FROM offers WHERE status = 'ACTIVE'
    """)

    with engine.connect() as conn:
        result = conn.execute(summary_query).fetchone()
        print(f"  Total active offers:    {result[0]}")
        print(f"  Duplicate groups:       {result[1]}")
        print(f"  Duplicate offer count:  {result[2]}")
        print()

    # 2. Top duplicate groups
    print("2️⃣  TOP DUPLICATE GROUPS (by count)")
    print("-" * 50)

    top_dups_query = text("""
        SELECT 
            source_phone,
            medication,
            COUNT(*) as count,
            MIN(created_at) as first_seen,
            MAX(created_at) as last_seen,
            ROUND(EXTRACT(EPOCH FROM (MAX(created_at) - MIN(created_at)))/60, 1) as span_minutes
        FROM offers
        WHERE status = 'ACTIVE'
        GROUP BY source_phone, LOWER(medication), medication
        HAVING COUNT(*) > 1
        ORDER BY count DESC
        LIMIT 15
    """)

    df_top = pd.read_sql(top_dups_query, engine)
    if not df_top.empty:
        df_top["first_seen"] = pd.to_datetime(df_top["first_seen"]).dt.strftime(
            "%m-%d %H:%M"
        )
        df_top["last_seen"] = pd.to_datetime(df_top["last_seen"]).dt.strftime(
            "%m-%d %H:%M"
        )
        print(tabulate(df_top, headers="keys", tablefmt="simple", showindex=False))
    else:
        print("  ✅ No duplicates found!")
    print()

    # 3. Cross-post analysis (same message to different groups)
    print("3️⃣  CROSS-POST ANALYSIS")
    print("-" * 50)

    crosspost_query = text("""
        WITH dup_pairs AS (
            SELECT source_phone, LOWER(medication) as med_lower
            FROM offers WHERE status = 'ACTIVE'
            GROUP BY source_phone, LOWER(medication)
            HAVING COUNT(*) > 1
        )
        SELECT 
            o.source_phone,
            o.medication,
            COUNT(DISTINCT o.source_group) as distinct_groups,
            COUNT(*) as total_offers,
            STRING_AGG(DISTINCT COALESCE(o.group_name, 'Unknown'), ', ') as groups
        FROM offers o
        JOIN dup_pairs dp ON o.source_phone = dp.source_phone 
            AND LOWER(o.medication) = dp.med_lower
        WHERE o.status = 'ACTIVE'
        GROUP BY o.source_phone, o.medication
        ORDER BY distinct_groups DESC, total_offers DESC
        LIMIT 10
    """)

    df_cross = pd.read_sql(crosspost_query, engine)
    if not df_cross.empty:
        # Truncate long group names
        df_cross["groups"] = df_cross["groups"].str[:60] + "..."
        print(tabulate(df_cross, headers="keys", tablefmt="simple", showindex=False))
    print()

    # 4. Time window analysis
    print("4️⃣  TIME WINDOW ANALYSIS")
    print("-" * 50)

    time_query = text("""
        WITH dup_spans AS (
            SELECT 
                EXTRACT(EPOCH FROM (MAX(created_at) - MIN(created_at)))/60 as span_minutes
            FROM offers
            WHERE status = 'ACTIVE'
            GROUP BY source_phone, LOWER(medication)
            HAVING COUNT(*) > 1
        ),
        categorized AS (
            SELECT 
                CASE 
                    WHEN span_minutes <= 5 THEN '0-5 min'
                    WHEN span_minutes <= 10 THEN '5-10 min'
                    WHEN span_minutes <= 30 THEN '10-30 min'
                    WHEN span_minutes <= 60 THEN '30-60 min'
                    ELSE '> 1 hour'
                END as time_window,
                CASE 
                    WHEN span_minutes <= 5 THEN 1
                    WHEN span_minutes <= 10 THEN 2
                    WHEN span_minutes <= 30 THEN 3
                    WHEN span_minutes <= 60 THEN 4
                    ELSE 5
                END as sort_order
            FROM dup_spans
        )
        SELECT time_window, COUNT(*) as duplicate_groups
        FROM categorized
        GROUP BY time_window, sort_order
        ORDER BY sort_order
    """)

    df_time = pd.read_sql(time_query, engine)
    if not df_time.empty:
        print(tabulate(df_time, headers="keys", tablefmt="simple", showindex=False))
    print()

    # 5. Root cause summary
    print("5️⃣  ROOT CAUSE SUMMARY")
    print("-" * 50)

    if not df_cross.empty:
        multi_group = df_cross[df_cross["distinct_groups"] > 1]
        single_group = df_cross[df_cross["distinct_groups"] == 1]

        print(f"  Cross-posts (multiple groups): {len(multi_group)} groups")
        print(f"  Same-group duplicates:         {len(single_group)} groups")

        if len(multi_group) > len(single_group):
            print("\n  📌 PRIMARY CAUSE: Cross-posting to multiple groups")
            print("  ✅ SOLUTION: Deduplication feature already implemented!")
        else:
            print("\n  📌 PRIMARY CAUSE: Repeated posts in same group")
            print("  ⚠️ SOLUTION: May need message-level dedup")
    print()

    # 6. Export detailed report
    print("6️⃣  EXPORTING DETAILED REPORT")
    print("-" * 50)

    report_dir = Path(__file__).parent.parent / "reports"
    report_dir.mkdir(exist_ok=True)

    # Full duplicate list
    full_query = text("""
        SELECT 
            o.id,
            o.source_phone,
            o.medication,
            o.source_group,
            o.group_name,
            o.created_at,
            o.raw_message_id
        FROM offers o
        WHERE o.status = 'ACTIVE'
        AND EXISTS (
            SELECT 1 FROM offers o2 
            WHERE o2.source_phone = o.source_phone 
            AND LOWER(o2.medication) = LOWER(o.medication)
            AND o2.status = 'ACTIVE'
            AND o2.id != o.id
        )
        ORDER BY o.source_phone, o.medication, o.created_at
    """)

    df_full = pd.read_sql(full_query, engine)
    report_path = report_dir / "duplicate_offers_investigation.csv"
    df_full.to_csv(report_path, index=False)
    print(f"  Exported {len(df_full)} duplicate offers to:")
    print(f"  📁 {report_path}")
    print()

    print("=" * 70)
    print("✅ INVESTIGATION COMPLETE")
    print("=" * 70)
    print("\nNext step: Run 11_cleanup_duplicates.py to fix duplicates")


if __name__ == "__main__":
    main()
