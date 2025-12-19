"""
Phase 7: AI Parsing Quality Assessment
Analyzes the quality of AI-extracted medication data.
"""

import sys
from pathlib import Path

import pandas as pd
from sqlalchemy import create_engine

# Add parent to path for config
sys.path.insert(0, str(Path(__file__).parent.parent))
from config import DATABASE_URL, REPORTS_DIR


def analyze_extraction_completeness(engine):
    """Analyze how complete the AI extractions are."""
    with engine.connect() as conn:
        offers_query = """
            SELECT COUNT(*) as total,
                   SUM(CASE WHEN medication != '' THEN 1 ELSE 0 END) as has_medication,
                   SUM(CASE WHEN quantity > 0 THEN 1 ELSE 0 END) as has_quantity,
                   SUM(CASE WHEN price IS NOT NULL AND price > 0 THEN 1 ELSE 0 END) as has_price,
                   SUM(CASE WHEN unit IS NOT NULL THEN 1 ELSE 0 END) as has_unit
            FROM offers
        """
        offers = pd.read_sql(offers_query, conn)

        requests_query = """
            SELECT COUNT(*) as total,
                   SUM(CASE WHEN medication != '' THEN 1 ELSE 0 END) as has_medication,
                   SUM(CASE WHEN quantity > 0 THEN 1 ELSE 0 END) as has_quantity,
                   SUM(CASE WHEN max_price IS NOT NULL AND max_price > 0 THEN 1 ELSE 0 END) as has_max_price,
                   SUM(CASE WHEN unit IS NOT NULL THEN 1 ELSE 0 END) as has_unit
            FROM requests
        """
        requests = pd.read_sql(requests_query, conn)

    return offers, requests


def analyze_top_medications(engine):
    """Analyze most common medications."""
    query = """
        SELECT medication, COUNT(*) as occurrences
        FROM (SELECT medication FROM offers UNION ALL SELECT medication FROM requests) combined
        GROUP BY medication ORDER BY occurrences DESC LIMIT 30
    """
    return pd.read_sql(query, engine)


def analyze_unmapped(engine):
    """Analyze unmapped medications."""
    query = """
        SELECT raw_text, ai_output, count, reviewed, approved_name
        FROM unmapped_medications ORDER BY count DESC LIMIT 20
    """
    return pd.read_sql(query, engine)


def analyze_review_queue(engine):
    """Analyze review queue status."""
    query = """
        SELECT status, COUNT(*) as count, AVG(avg_confidence) as avg_confidence
        FROM review_queue GROUP BY status
    """
    return pd.read_sql(query, engine)


def main():
    print("=" * 60)
    print("PHASE 7: AI PARSING QUALITY ASSESSMENT")
    print("=" * 60)

    engine = create_engine(DATABASE_URL)

    print("\n📊 EXTRACTION COMPLETENESS")
    print("-" * 40)
    offers, requests = analyze_extraction_completeness(engine)

    if not offers.empty and offers["total"].iloc[0] > 0:
        total = offers["total"].iloc[0]
        print(f"\nOFFERS ({total} total):")
        for col in ["has_medication", "has_quantity", "has_price", "has_unit"]:
            val = offers[col].iloc[0]
            pct = val / total * 100
            status = "✅" if pct > 80 else "⚠️" if pct > 50 else "❌"
            print(f"  {status} {col}: {val} ({pct:.1f}%)")

    if not requests.empty and requests["total"].iloc[0] > 0:
        total = requests["total"].iloc[0]
        print(f"\nREQUESTS ({total} total):")
        for col in ["has_medication", "has_quantity", "has_max_price", "has_unit"]:
            val = requests[col].iloc[0]
            pct = val / total * 100
            status = "✅" if pct > 80 else "⚠️" if pct > 50 else "❌"
            print(f"  {status} {col}: {val} ({pct:.1f}%)")

    print("\n💊 TOP MEDICATIONS")
    df_meds = analyze_top_medications(engine)
    if not df_meds.empty:
        print(df_meds.head(10).to_string(index=False))

    print("\n❓ UNMAPPED MEDICATIONS")
    df_unmapped = analyze_unmapped(engine)
    if not df_unmapped.empty:
        print(df_unmapped.head(10).to_string(index=False))
    else:
        print("✅ No unmapped medications!")

    print("\n📋 REVIEW QUEUE STATUS")
    df_review = analyze_review_queue(engine)
    if not df_review.empty:
        print(df_review.to_string(index=False))
    else:
        print("✅ Review queue is empty!")

    # Save
    output_file = f"{REPORTS_DIR}/07_ai_parsing_quality.xlsx"
    with pd.ExcelWriter(output_file) as writer:
        df_meds.to_excel(writer, sheet_name="Top Medications", index=False)
        if not df_unmapped.empty:
            df_unmapped.to_excel(writer, sheet_name="Unmapped", index=False)
        if not df_review.empty:
            df_review.to_excel(writer, sheet_name="Review Queue", index=False)

    print(f"\n✅ Report saved to {output_file}")


if __name__ == "__main__":
    main()
