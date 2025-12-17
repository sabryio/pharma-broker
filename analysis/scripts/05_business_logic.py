"""
Phase 5: Business Logic Validation
Validates data against business rules.
"""
import pandas as pd
from sqlalchemy import create_engine, text

from config import DATABASE_URL, REPORTS_DIR


def validate_business_rules(engine):
    """Run business logic validation queries."""
    validations = []

    with engine.connect() as conn:
        # BR-001: Offers must have medication name
        result = conn.execute(text('''
            SELECT COUNT(*) FROM offers WHERE medication IS NULL OR medication = ''
        ''')).scalar()
        validations.append({
            'rule_id': 'BR-001', 'rule': 'Offers must have medication name',
            'severity': 'Critical', 'violations': result,
            'status': '✅' if result == 0 else '❌'
        })

        # BR-002: Requests must have medication name
        result = conn.execute(text('''
            SELECT COUNT(*) FROM requests WHERE medication IS NULL OR medication = ''
        ''')).scalar()
        validations.append({
            'rule_id': 'BR-002', 'rule': 'Requests must have medication name',
            'severity': 'Critical', 'violations': result,
            'status': '✅' if result == 0 else '❌'
        })

        # BR-003: Match scores must be 0-1
        result = conn.execute(text('''
            SELECT COUNT(*) FROM matches WHERE score < 0 OR score > 1
        ''')).scalar()
        validations.append({
            'rule_id': 'BR-003', 'rule': 'Match scores must be 0-1',
            'severity': 'Critical', 'violations': result,
            'status': '✅' if result == 0 else '❌'
        })

        # BR-004: Confirmed matches need confirmed_at
        result = conn.execute(text('''
            SELECT COUNT(*) FROM matches WHERE status = 'CONFIRMED' AND confirmed_at IS NULL
        ''')).scalar()
        validations.append({
            'rule_id': 'BR-004', 'rule': 'Confirmed matches need confirmed_at',
            'severity': 'Warning', 'violations': result,
            'status': '✅' if result == 0 else '⚠️'
        })

        # BR-005: Raw messages must have content
        result = conn.execute(text('''
            SELECT COUNT(*) FROM raw_messages WHERE content IS NULL OR content = ''
        ''')).scalar()
        validations.append({
            'rule_id': 'BR-005', 'rule': 'Raw messages must have content',
            'severity': 'Critical', 'violations': result,
            'status': '✅' if result == 0 else '❌'
        })

        # BR-006: Processed messages should create offers/requests
        result = conn.execute(text('''
            SELECT COUNT(*) FROM raw_messages rm
            WHERE rm.processed_at IS NOT NULL AND rm.error IS NULL
            AND NOT EXISTS (SELECT 1 FROM offers o WHERE o.raw_message_id = rm.id)
            AND NOT EXISTS (SELECT 1 FROM requests r WHERE r.raw_message_id = rm.id)
        ''')).scalar()
        validations.append({
            'rule_id': 'BR-006', 'rule': 'Processed messages should create offers/requests',
            'severity': 'Warning', 'violations': result,
            'status': '✅' if result == 0 else '🔍'
        })

        # BR-007: Quantity must be non-negative
        result = conn.execute(text('''
            SELECT (SELECT COUNT(*) FROM offers WHERE quantity < 0) +
                   (SELECT COUNT(*) FROM requests WHERE quantity < 0)
        ''')).scalar()
        validations.append({
            'rule_id': 'BR-007', 'rule': 'Quantity must be non-negative',
            'severity': 'Critical', 'violations': result,
            'status': '✅' if result == 0 else '❌'
        })

        # BR-008: Price must be non-negative
        result = conn.execute(text('''
            SELECT COUNT(*) FROM offers WHERE price < 0
        ''')).scalar()
        validations.append({
            'rule_id': 'BR-008', 'rule': 'Price must be non-negative',
            'severity': 'Critical', 'violations': result,
            'status': '✅' if result == 0 else '❌'
        })

    return pd.DataFrame(validations)


def main():
    print("=" * 60)
    print("PHASE 5: BUSINESS LOGIC VALIDATION")
    print("=" * 60)

    engine = create_engine(DATABASE_URL)
    df = validate_business_rules(engine)

    print("\n📋 BUSINESS RULE VALIDATION")
    print("-" * 40)
    print(df.to_string(index=False))

    # Summary
    critical_failed = len(df[(df['severity'] == 'Critical') & (df['status'] == '❌')])
    total_rules = len(df)
    passed = len(df[df['status'] == '✅'])

    print(f"\n📊 Summary: {passed}/{total_rules} rules passed")
    if critical_failed > 0:
        print(f"❌ {critical_failed} CRITICAL rules failed - immediate attention required!")

    output_file = f'{REPORTS_DIR}/05_business_logic.csv'
    df.to_csv(output_file, index=False)
    print(f"\n✅ Report saved to {output_file}")


if __name__ == '__main__':
    main()
