"""
Phase 6: Time Series Analysis
Analyzes data patterns over time.
"""
import pandas as pd
from sqlalchemy import create_engine

from config import DATABASE_URL, REPORTS_DIR


def analyze_message_volume(engine):
    """Analyze message volume over time."""
    query = '''
        SELECT DATE(timestamp) as date, COUNT(*) as messages,
               COUNT(DISTINCT group_jid) as groups,
               COUNT(DISTINCT sender_phone) as senders
        FROM raw_messages GROUP BY DATE(timestamp) ORDER BY date
    '''
    return pd.read_sql(query, engine)


def analyze_processing_rate(engine):
    """Analyze message processing success rate."""
    query = '''
        SELECT DATE(timestamp) as date, COUNT(*) as total,
               SUM(CASE WHEN processed_at IS NOT NULL THEN 1 ELSE 0 END) as processed,
               SUM(CASE WHEN error IS NOT NULL THEN 1 ELSE 0 END) as errors
        FROM raw_messages GROUP BY DATE(timestamp) ORDER BY date
    '''
    df = pd.read_sql(query, engine)
    df['success_rate'] = (df['processed'] / df['total'] * 100).round(2)
    df['error_rate'] = (df['errors'] / df['total'] * 100).round(2)
    return df


def analyze_match_creation(engine):
    """Analyze match creation over time."""
    query = '''
        SELECT DATE(created_at) as date, COUNT(*) as matches,
               AVG(score) as avg_score,
               SUM(CASE WHEN status = 'CONFIRMED' THEN 1 ELSE 0 END) as confirmed,
               SUM(CASE WHEN status = 'REJECTED' THEN 1 ELSE 0 END) as rejected
        FROM matches GROUP BY DATE(created_at) ORDER BY date
    '''
    df = pd.read_sql(query, engine)
    if not df.empty and 'avg_score' in df.columns:
        df['avg_score'] = df['avg_score'].round(3)
    return df


def main():
    print("=" * 60)
    print("PHASE 6: TIME SERIES ANALYSIS")
    print("=" * 60)

    engine = create_engine(DATABASE_URL)

    print("\n📈 MESSAGE VOLUME BY DAY")
    df_vol = analyze_message_volume(engine)
    if not df_vol.empty:
        print(df_vol.tail(10).to_string(index=False))
        avg_daily = df_vol['messages'].mean()
        print(f"\n📊 Average daily messages: {avg_daily:.0f}")

    print("\n⚙️ PROCESSING SUCCESS RATE")
    df_proc = analyze_processing_rate(engine)
    if not df_proc.empty:
        print(df_proc.tail(10).to_string(index=False))
        avg_success = df_proc['success_rate'].mean()
        print(f"\n📊 Average success rate: {avg_success:.1f}%")

    print("\n🎯 MATCH CREATION BY DAY")
    df_match = analyze_match_creation(engine)
    if not df_match.empty:
        print(df_match.tail(10).to_string(index=False))

    # Save
    output_file = f'{REPORTS_DIR}/06_time_series.xlsx'
    with pd.ExcelWriter(output_file) as writer:
        if not df_vol.empty:
            df_vol.to_excel(writer, sheet_name='Message Volume', index=False)
        if not df_proc.empty:
            df_proc.to_excel(writer, sheet_name='Processing Rate', index=False)
        if not df_match.empty:
            df_match.to_excel(writer, sheet_name='Match Creation', index=False)

    print(f"\n✅ Report saved to {output_file}")


if __name__ == '__main__':
    main()
