#!/usr/bin/env python3
"""
Reprocess Failed Messages Script

This script identifies messages that failed due to temporary issues (circuit breaker, network errors)
and marks them for reprocessing by clearing their processed_at and error fields.

Usage:
    python 15_reprocess_failed_messages.py [--dry-run] [--error-type TYPE] [--limit N]

Options:
    --dry-run           Show what would be reprocessed without making changes
    --error-type TYPE   Only reprocess specific error types:
                        - circuit_breaker: Messages failed due to circuit breaker
                        - network: Messages failed due to network errors
                        - incomplete_json: Messages with incomplete JSON responses
                        - all: All retryable errors (default)
    --limit N           Maximum number of messages to reprocess (default: 1000)
    --since HOURS       Only reprocess messages from last N hours (default: 24)
"""

import sys
from pathlib import Path

# Add parent directory to path for config import
sys.path.insert(0, str(Path(__file__).parent.parent))

import argparse
from datetime import datetime, timedelta
from config import get_db_connection


def count_failed_messages(conn, error_type: str, since_hours: int):
    """Count messages that can be reprocessed"""
    cursor = conn.cursor()

    since_time = datetime.utcnow() - timedelta(hours=since_hours)

    if error_type == "circuit_breaker":
        query = """
            SELECT COUNT(*) 
            FROM raw_messages 
            WHERE error = 'Circuit breaker open'
            AND processed_at > %s
        """
    elif error_type == "network":
        query = """
            SELECT COUNT(*) 
            FROM raw_messages 
            WHERE error LIKE '%Network error%'
            AND processed_at > %s
        """
    elif error_type == "incomplete_json":
        query = """
            SELECT COUNT(*) 
            FROM raw_messages 
            WHERE error LIKE '%Incomplete JSON%'
            AND processed_at > %s
        """
    else:  # all
        query = """
            SELECT COUNT(*) 
            FROM raw_messages 
            WHERE error IS NOT NULL
            AND (
                error = 'Circuit breaker open'
                OR error LIKE '%Network error%'
                OR error LIKE '%Incomplete JSON%'
            )
            AND processed_at > %s
        """

    cursor.execute(query, (since_time,))
    count = cursor.fetchone()[0]
    cursor.close()

    return count


def get_failed_messages(conn, error_type: str, since_hours: int, limit: int):
    """Get messages that can be reprocessed"""
    cursor = conn.cursor()

    since_time = datetime.utcnow() - timedelta(hours=since_hours)

    if error_type == "circuit_breaker":
        query = """
            SELECT id, content, error, processed_at, created_at
            FROM raw_messages 
            WHERE error = 'Circuit breaker open'
            AND processed_at > %s
            ORDER BY processed_at DESC
            LIMIT %s
        """
    elif error_type == "network":
        query = """
            SELECT id, content, error, processed_at, created_at
            FROM raw_messages 
            WHERE error LIKE '%Network error%'
            AND processed_at > %s
            ORDER BY processed_at DESC
            LIMIT %s
        """
    elif error_type == "incomplete_json":
        query = """
            SELECT id, content, error, processed_at, created_at
            FROM raw_messages 
            WHERE error LIKE '%Incomplete JSON%'
            AND processed_at > %s
            ORDER BY processed_at DESC
            LIMIT %s
        """
    else:  # all
        query = """
            SELECT id, content, error, processed_at, created_at
            FROM raw_messages 
            WHERE error IS NOT NULL
            AND (
                error = 'Circuit breaker open'
                OR error LIKE '%Network error%'
                OR error LIKE '%Incomplete JSON%'
            )
            AND processed_at > %s
            ORDER BY processed_at DESC
            LIMIT %s
        """

    cursor.execute(query, (since_time, limit))
    messages = cursor.fetchall()
    cursor.close()

    return messages


def reprocess_messages(conn, message_ids: list, dry_run: bool = False):
    """Mark messages for reprocessing by clearing processed_at and error"""
    cursor = conn.cursor()

    if dry_run:
        print(f"\n[DRY RUN] Would reprocess {len(message_ids)} messages")
        return 0

    # Clear processed_at and error to trigger reprocessing
    query = """
        UPDATE raw_messages
        SET processed_at = NULL, error = NULL
        WHERE id = ANY(%s)
    """

    cursor.execute(query, (message_ids,))
    updated = cursor.rowcount
    conn.commit()
    cursor.close()

    return updated


def main():
    parser = argparse.ArgumentParser(
        description="Reprocess failed messages",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be reprocessed without making changes",
    )
    parser.add_argument(
        "--error-type",
        choices=["circuit_breaker", "network", "incomplete_json", "all"],
        default="all",
        help="Type of errors to reprocess (default: all)",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=1000,
        help="Maximum number of messages to reprocess (default: 1000)",
    )
    parser.add_argument(
        "--since",
        type=int,
        default=24,
        help="Only reprocess messages from last N hours (default: 24)",
    )

    args = parser.parse_args()

    print("=" * 80)
    print("Reprocess Failed Messages")
    print("=" * 80)
    print(f"Error Type: {args.error_type}")
    print(f"Time Window: Last {args.since} hours")
    print(f"Limit: {args.limit}")
    print(f"Mode: {'DRY RUN' if args.dry_run else 'LIVE'}")
    print("=" * 80)

    # Connect to database
    conn = get_db_connection()

    try:
        # Count eligible messages
        total_count = count_failed_messages(conn, args.error_type, args.since)
        print(f"\nTotal eligible messages: {total_count}")

        if total_count == 0:
            print("\n✅ No messages to reprocess")
            return

        # Get messages to reprocess
        messages = get_failed_messages(conn, args.error_type, args.since, args.limit)
        print(f"Messages to reprocess: {len(messages)}")

        if not messages:
            print("\n✅ No messages to reprocess")
            return

        # Show sample of messages
        print("\n" + "=" * 80)
        print("Sample Messages (first 5):")
        print("=" * 80)
        for i, msg in enumerate(messages[:5], 1):
            msg_id, content, error, processed_at, created_at = msg
            content_preview = content[:100] + "..." if len(content) > 100 else content
            print(f"\n{i}. ID: {msg_id}")
            print(f"   Created: {created_at}")
            print(f"   Processed: {processed_at}")
            print(f"   Error: {error[:100]}...")
            print(f"   Content: {content_preview}")

        if len(messages) > 5:
            print(f"\n... and {len(messages) - 5} more messages")

        # Confirm before proceeding
        if not args.dry_run:
            print("\n" + "=" * 80)
            response = input(f"Reprocess {len(messages)} messages? (yes/no): ")
            if response.lower() != "yes":
                print("❌ Cancelled")
                return

        # Reprocess messages
        message_ids = [msg[0] for msg in messages]
        updated = reprocess_messages(conn, message_ids, args.dry_run)

        if args.dry_run:
            print(
                f"\n[DRY RUN] Would mark {len(message_ids)} messages for reprocessing"
            )
        else:
            print(f"\n✅ Successfully marked {updated} messages for reprocessing")
            print(
                "\nThese messages will be automatically reprocessed by the core service."
            )
            print("Monitor the logs to see the reprocessing progress:")
            print("  docker logs -f pharma-core")

    finally:
        conn.close()


if __name__ == "__main__":
    main()
