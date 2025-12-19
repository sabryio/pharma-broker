#!/usr/bin/env python3
"""
Script 12: Review Unmapped Medications
Interactive tool to fix Arabic→English medication mappings.
Approved corrections are added to medication_mappings for future parsing.
"""

import sys
from datetime import datetime
from pathlib import Path
from uuid import uuid4

from sqlalchemy import create_engine, text
# from tabulate import tabulate

# Add parent to path for config
sys.path.insert(0, str(Path(__file__).parent.parent))
from config import DATABASE_URL


def get_pending_unmapped(engine, limit=20):
    """Get unmapped medications that haven't been reviewed."""
    query = text("""
        SELECT id, raw_text, ai_output, count, source_message
        FROM unmapped_medications
        WHERE reviewed = false
        ORDER BY count DESC
        LIMIT :limit
    """)
    with engine.connect() as conn:
        result = conn.execute(query, {"limit": limit})
        return [dict(row._mapping) for row in result]


def mark_reviewed(engine, unmapped_id, approved_name, reviewed_by="admin"):
    """Mark an unmapped medication as reviewed."""
    query = text("""
        UPDATE unmapped_medications
        SET reviewed = true,
            approved_name = :approved_name,
            reviewed_by = :reviewed_by,
            reviewed_at = :reviewed_at,
            updated_at = :updated_at
        WHERE id = :id
    """)
    now = datetime.now()
    with engine.connect() as conn:
        conn.execute(
            query,
            {
                "id": unmapped_id,
                "approved_name": approved_name,
                "reviewed_by": reviewed_by,
                "reviewed_at": now,
                "updated_at": now,
            },
        )
        conn.commit()


def add_to_mappings(engine, arabic_name, english_name):
    """Add a new medication mapping to the database."""
    # Check if already exists
    check_query = text("""
        SELECT id FROM medication_mappings
        WHERE arabic_name = :arabic_name
    """)
    with engine.connect() as conn:
        result = conn.execute(check_query, {"arabic_name": arabic_name}).fetchone()
        if result:
            print(f"  ⚠️  Mapping already exists for '{arabic_name}'")
            return False

    # Insert new mapping
    insert_query = text("""
        INSERT INTO medication_mappings (id, arabic_name, english_name, created_at, updated_at)
        VALUES (:id, :arabic_name, :english_name, :created_at, :updated_at)
    """)
    now = datetime.now()
    with engine.connect() as conn:
        conn.execute(
            insert_query,
            {
                "id": str(uuid4()),
                "arabic_name": arabic_name,
                "english_name": english_name,
                "created_at": now,
                "updated_at": now,
            },
        )
        conn.commit()
    return True


def main():
    print("=" * 70)
    print("📋 UNMAPPED MEDICATIONS REVIEW")
    print("=" * 70)
    print("\nCommands:")
    print("  [Enter] = Accept AI suggestion")
    print("  [text]  = Enter correct English name")
    print("  [s]     = Skip this item")
    print("  [q]     = Quit\n")

    engine = create_engine(DATABASE_URL)

    # Get pending count
    with engine.connect() as conn:
        count = conn.execute(
            text("SELECT COUNT(*) FROM unmapped_medications WHERE reviewed = false")
        ).fetchone()[0]
    print(f"📊 {count} unmapped medications pending review\n")

    if count == 0:
        print("✅ No unmapped medications to review!")
        return

    items = get_pending_unmapped(engine, limit=50)
    reviewed = 0
    skipped = 0
    added_mappings = 0

    for i, item in enumerate(items, 1):
        print("-" * 70)
        print(f"\n[{i}/{len(items)}] Raw text: \033[1m{item['raw_text']}\033[0m")
        print(f"    AI output: \033[94m{item['ai_output']}\033[0m")
        print(f"    Count: {item['count']}")
        if item["source_message"]:
            msg = (
                item["source_message"][:100] + "..."
                if len(item["source_message"]) > 100
                else item["source_message"]
            )
            print(f"    Sample: {msg}")

        response = input(f"\n    Correct name [{item['ai_output']}]: ").strip()

        if response.lower() == "q":
            print("\n👋 Exiting...")
            break
        elif response.lower() == "s":
            skipped += 1
            print("    ⏭️  Skipped")
            continue

        # Use AI output if Enter pressed, otherwise use the entered text
        approved_name = response if response else item["ai_output"]

        # Mark as reviewed
        mark_reviewed(engine, item["id"], approved_name)
        reviewed += 1
        print(f"    ✅ Approved: {approved_name}")

        # Add to medication_mappings
        if add_to_mappings(engine, item["raw_text"], approved_name):
            added_mappings += 1
            print("    ➕ Added to medication_mappings")

    print("\n" + "=" * 70)
    print("📊 SUMMARY")
    print("=" * 70)
    print(f"  Reviewed: {reviewed}")
    print(f"  Skipped:  {skipped}")
    print(f"  Added to mappings: {added_mappings}")
    print("\n✅ Done!")


if __name__ == "__main__":
    main()
