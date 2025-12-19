#!/usr/bin/env python3
"""
Script 13: Process Review Queue
Interactive tool to approve/reject items in the AI parsing review queue.
"""

import json
import sys
from datetime import datetime
from pathlib import Path

from sqlalchemy import create_engine, text
# from tabulate import tabulate

# Add parent to path for config
sys.path.insert(0, str(Path(__file__).parent.parent))
from config import DATABASE_URL


def get_pending_queue(engine, limit=20):
    """Get pending review queue items."""
    query = text("""
        SELECT rq.id, rq.raw_message_id, rq.parsed_items, rq.avg_confidence,
               rq.failure_reason, rq.pass_number, rq.created_at,
               rm.content, rm.group_name, rm.sender_name
        FROM review_queue rq
        JOIN raw_messages rm ON rq.raw_message_id = rm.id
        WHERE rq.status = 'PENDING'
        ORDER BY rq.created_at DESC
        LIMIT :limit
    """)
    with engine.connect() as conn:
        result = conn.execute(query, {"limit": limit})
        return [dict(row._mapping) for row in result]


def approve_item(engine, item_id, reviewed_by="admin", note=""):
    """Approve a review queue item."""
    query = text("""
        UPDATE review_queue
        SET status = 'APPROVED',
            reviewed_by = :reviewed_by,
            reviewed_at = :reviewed_at,
            review_note = :note
        WHERE id = :id
    """)
    with engine.connect() as conn:
        conn.execute(
            query,
            {
                "id": item_id,
                "reviewed_by": reviewed_by,
                "reviewed_at": datetime.now(),
                "note": note,
            },
        )
        conn.commit()


def reject_item(engine, item_id, reviewed_by="admin", reason=""):
    """Reject a review queue item."""
    query = text("""
        UPDATE review_queue
        SET status = 'REJECTED',
            reviewed_by = :reviewed_by,
            reviewed_at = :reviewed_at,
            review_note = :reason
        WHERE id = :id
    """)
    with engine.connect() as conn:
        conn.execute(
            query,
            {
                "id": item_id,
                "reviewed_by": reviewed_by,
                "reviewed_at": datetime.now(),
                "reason": reason,
            },
        )
        conn.commit()


def format_parsed_items(parsed_json):
    """Format parsed items for display."""
    if not parsed_json:
        return "  (no items parsed)"

    try:
        items = json.loads(parsed_json) if isinstance(parsed_json, str) else parsed_json
        if not items:
            return "  (empty)"

        lines = []
        for item in items:
            typ = item.get("type", "?")
            med = item.get("medication", "?")
            qty = item.get("quantity", 0)
            conf = item.get("ai_confidence", 0)
            lines.append(f"  • {typ}: {med} (qty={qty}, conf={conf:.2f})")
        return "\n".join(lines)
    except Exception as e:
        return f"  (parse error: {e})"


def main():
    print("=" * 70)
    print("📋 REVIEW QUEUE PROCESSING")
    print("=" * 70)
    print("\nCommands:")
    print("  [a]     = Approve (parse result is correct)")
    print("  [r]     = Reject (parse result is wrong)")
    print("  [s]     = Skip this item")
    print("  [v]     = View full message content")
    print("  [q]     = Quit\n")

    engine = create_engine(DATABASE_URL)

    # Get pending count
    with engine.connect() as conn:
        count = conn.execute(
            text("SELECT COUNT(*) FROM review_queue WHERE status = 'PENDING'")
        ).fetchone()[0]
    print(f"📊 {count} items pending review\n")

    if count == 0:
        print("✅ Review queue is empty!")
        return

    items = get_pending_queue(engine, limit=50)
    approved = 0
    rejected = 0
    skipped = 0

    for i, item in enumerate(items, 1):
        print("-" * 70)
        print(f"\n[{i}/{len(items)}] ID: {item['id'][:8]}...")
        print(f"    Group: {item['group_name']}")
        print(f"    Sender: {item['sender_name']}")
        print(
            f"    Pass: {item['pass_number']} | Confidence: {item['avg_confidence']:.2f}"
        )

        # Show truncated message
        content = item["content"] or ""
        if len(content) > 150:
            print(f"    Message: {content[:150]}...")
        else:
            print(f"    Message: {content}")

        if item["failure_reason"]:
            print(f"    ⚠️ Reason: {item['failure_reason']}")

        print("\n    📦 Parsed Items:")
        print(format_parsed_items(item["parsed_items"]))

        while True:
            response = input("\n    Action [a/r/s/v/q]: ").strip().lower()

            if response == "q":
                print("\n👋 Exiting...")
                break
            elif response == "s":
                skipped += 1
                print("    ⏭️  Skipped")
                break
            elif response == "v":
                print(
                    f"\n    Full message:\n    {'-' * 40}\n    {item['content']}\n    {'-' * 40}"
                )
                continue
            elif response == "a":
                approve_item(engine, item["id"])
                approved += 1
                print("    ✅ Approved")
                break
            elif response == "r":
                reason = input("    Rejection reason (optional): ").strip()
                reject_item(engine, item["id"], reason=reason)
                rejected += 1
                print("    ❌ Rejected")
                break
            else:
                print("    Invalid command. Use a/r/s/v/q")

        if response == "q":
            break

    print("\n" + "=" * 70)
    print("📊 SUMMARY")
    print("=" * 70)
    print(f"  Approved: {approved}")
    print(f"  Rejected: {rejected}")
    print(f"  Skipped:  {skipped}")
    print("\n✅ Done!")


if __name__ == "__main__":
    main()
