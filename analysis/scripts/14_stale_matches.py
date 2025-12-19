#!/usr/bin/env python3
"""
Script 14: Stale Matches Analysis & Cleanup
Analyzes pending matches and provides options to expire stale ones.
"""

import sys
from datetime import datetime, timedelta
from pathlib import Path

from sqlalchemy import create_engine, text

# Add parent to path for config
sys.path.insert(0, str(Path(__file__).parent.parent))
from config import DATABASE_URL


def get_match_stats(engine):
    """Get match statistics by status and age."""
    query = text("""
        SELECT 
            status,
            COUNT(*) as count,
            MIN(created_at) as oldest,
            MAX(created_at) as newest,
            AVG(score) as avg_score
        FROM matches
        GROUP BY status
        ORDER BY count DESC
    """)
    with engine.connect() as conn:
        result = conn.execute(query)
        return [dict(row._mapping) for row in result]


def get_stale_matches(engine, days=7):
    """Get pending matches older than N days."""
    cutoff = datetime.now() - timedelta(days=days)
    query = text("""
        SELECT 
            m.id, m.score, m.created_at,
            o.medication as offer_med,
            r.medication as request_med
        FROM matches m
        JOIN offers o ON m.offer_id = o.id
        JOIN requests r ON m.request_id = r.id
        WHERE m.status = 'PENDING'
          AND m.created_at < :cutoff
        ORDER BY m.created_at ASC
        LIMIT 50
    """)
    with engine.connect() as conn:
        result = conn.execute(query, {"cutoff": cutoff})
        return [dict(row._mapping) for row in result]


def get_age_distribution(engine):
    """Get distribution of pending match ages."""
    query = text("""
        SELECT 
            age_bucket,
            COUNT(*) as count,
            ROUND(AVG(score)::numeric, 2) as avg_score
        FROM (
            SELECT 
                CASE 
                    WHEN created_at > NOW() - INTERVAL '1 day' THEN '< 1 day'
                    WHEN created_at > NOW() - INTERVAL '3 days' THEN '1-3 days'
                    WHEN created_at > NOW() - INTERVAL '7 days' THEN '3-7 days'
                    WHEN created_at > NOW() - INTERVAL '14 days' THEN '1-2 weeks'
                    WHEN created_at > NOW() - INTERVAL '30 days' THEN '2-4 weeks'
                    ELSE '> 1 month'
                END as age_bucket,
                CASE 
                    WHEN created_at > NOW() - INTERVAL '1 day' THEN 1
                    WHEN created_at > NOW() - INTERVAL '3 days' THEN 2
                    WHEN created_at > NOW() - INTERVAL '7 days' THEN 3
                    WHEN created_at > NOW() - INTERVAL '14 days' THEN 4
                    WHEN created_at > NOW() - INTERVAL '30 days' THEN 5
                    ELSE 6
                END as sort_order,
                score
            FROM matches
            WHERE status = 'PENDING'
        ) sub
        GROUP BY age_bucket, sort_order
        ORDER BY sort_order
    """)
    with engine.connect() as conn:
        result = conn.execute(query)
        return [dict(row._mapping) for row in result]


def expire_old_matches(engine, days=14, dry_run=True):
    """Expire matches older than N days. Returns count of affected rows."""
    cutoff = datetime.now() - timedelta(days=days)

    if dry_run:
        query = text("""
            SELECT COUNT(*) FROM matches
            WHERE status = 'PENDING' AND created_at < :cutoff
        """)
        with engine.connect() as conn:
            result = conn.execute(query, {"cutoff": cutoff}).fetchone()
            return result[0]
    else:
        # First get IDs of matches to expire
        query = text("""
            UPDATE matches
            SET status = 'EXPIRED', notes = 'Auto-expired: stale match'
            WHERE status = 'PENDING' AND created_at < :cutoff
            RETURNING id
        """)
        with engine.connect() as conn:
            result = conn.execute(query, {"cutoff": cutoff})
            count = len(result.fetchall())
            conn.commit()
            return count


def main():
    print("=" * 70)
    print("📊 STALE MATCHES ANALYSIS")
    print("=" * 70)

    engine = create_engine(DATABASE_URL)

    # 1. Overall stats
    print("\n1️⃣  MATCH STATUS OVERVIEW")
    print("-" * 50)
    stats = get_match_stats(engine)
    for s in stats:
        oldest = s["oldest"].strftime("%Y-%m-%d") if s["oldest"] else "N/A"
        _newest = s["newest"].strftime("%Y-%m-%d") if s["newest"] else "N/A"
        print(
            f"  {s['status']}: {s['count']} (oldest: {oldest}, avg_score: {s['avg_score']:.2f})"
        )

    # 2. Age distribution
    print("\n2️⃣  PENDING MATCHES BY AGE")
    print("-" * 50)
    age_dist = get_age_distribution(engine)
    for a in age_dist:
        print(
            f"  {a['age_bucket']:12} : {a['count']:5} matches (avg score: {a['avg_score']})"
        )

    # 3. Sample stale matches
    print("\n3️⃣  SAMPLE STALE MATCHES (>7 days)")
    print("-" * 50)
    stale = get_stale_matches(engine, days=7)
    if stale:
        for m in stale[:10]:
            age = (datetime.now() - m["created_at"]).days
            print(
                f"  {m['id'][:8]}... | {m['offer_med'][:20]:20} ↔ {m['request_med'][:20]:20} | score={m['score']:.2f} | {age}d old"
            )
    else:
        print("  ✅ No stale matches!")

    # 4. Cleanup options
    print("\n4️⃣  CLEANUP OPTIONS")
    print("-" * 50)

    for days in [7, 14, 30]:
        count = expire_old_matches(engine, days=days, dry_run=True)
        print(f"  Expire matches > {days} days: {count} would be affected")

    # 5. Interactive cleanup
    print("\n" + "=" * 70)
    response = input(
        "Expire matches older than how many days? (7/14/30/n to skip): "
    ).strip()

    if response.lower() == "n":
        print("👋 Skipped cleanup")
        return

    try:
        days = int(response)
        if days not in [7, 14, 30]:
            print("❌ Invalid option")
            return
    except ValueError:
        print("❌ Invalid input")
        return

    count = expire_old_matches(engine, days=days, dry_run=True)
    confirm = input(
        f"\n⚠️  This will expire {count} matches. Continue? (yes/no): "
    ).strip()

    if confirm.lower() == "yes":
        expired = expire_old_matches(engine, days=days, dry_run=False)
        print(f"\n✅ Expired {expired} stale matches")
    else:
        print("❌ Cancelled")


if __name__ == "__main__":
    main()
