"""
Phase 8: Matching Engine Analysis
Analyzes match quality and scoring distribution.
"""
import pandas as pd
from sqlalchemy import create_engine

from config import DATABASE_URL, REPORTS_DIR


def analyze_score_distribution(engine):
    """Analyze match score distribution by confidence band."""
    query = '''
        SELECT
            CASE
                WHEN score >= 0.9 THEN '0.9-1.0 (AUTO)'
                WHEN score >= 0.7 THEN '0.7-0.9 (SUGGEST)'
                WHEN score >= 0.5 THEN '0.5-0.7 (REVIEW)'
                ELSE '0.0-0.5 (NONE)'
            END as score_band,
            COUNT(*) as count,
            ROUND(AVG(score)::numeric, 3) as avg_score,
            SUM(CASE WHEN status = 'CONFIRMED' THEN 1 ELSE 0 END) as confirmed,
            SUM(CASE WHEN status = 'REJECTED' THEN 1 ELSE 0 END) as rejected
        FROM matches
        GROUP BY 1
        ORDER BY avg_score DESC
    '''
    df = pd.read_sql(query, engine)
    if not df.empty:
        df['confirm_rate'] = (df['confirmed'] / df['count'] * 100).round(1)
    return df


def analyze_match_outcomes(engine):
    """Analyze match confirmation/rejection rates by status."""
    query = '''
        SELECT status, COUNT(*) as count,
               ROUND(AVG(score)::numeric, 3) as avg_score,
               ROUND(MIN(score)::numeric, 3) as min_score,
               ROUND(MAX(score)::numeric, 3) as max_score
        FROM matches GROUP BY status
    '''
    return pd.read_sql(query, engine)


def analyze_feedback_correlation(engine):
    """Analyze correlation between scores and feedback."""
    query = '''
        SELECT action, COUNT(*) as count,
               ROUND(AVG(medication_score)::numeric, 3) as avg_med_score,
               ROUND(AVG(total_score)::numeric, 3) as avg_total_score
        FROM feedback_records GROUP BY action
    '''
    return pd.read_sql(query, engine)


def analyze_weight_history(engine):
    """Analyze weight changes over time."""
    query = '''
        SELECT id, source, weights, improvement,
               applied_at::date as applied_date
        FROM weight_history ORDER BY applied_at DESC LIMIT 10
    '''
    return pd.read_sql(query, engine)


def main():
    print("=" * 60)
    print("PHASE 8: MATCHING ENGINE ANALYSIS")
    print("=" * 60)

    engine = create_engine(DATABASE_URL)

    print("\n📊 SCORE DISTRIBUTION BY CONFIDENCE BAND")
    print("-" * 40)
    df_scores = analyze_score_distribution(engine)
    if not df_scores.empty:
        print(df_scores.to_string(index=False))

        # Check AUTO band performance
        auto_band = df_scores[df_scores['score_band'] == '0.9-1.0 (AUTO)']
        if not auto_band.empty:
            confirm_rate = auto_band['confirm_rate'].iloc[0]
            status = '✅' if confirm_rate > 95 else '⚠️' if confirm_rate > 80 else '❌'
            print(f"\n{status} AUTO band confirmation rate: {confirm_rate}%")

    print("\n🎯 MATCH OUTCOMES BY STATUS")
    print("-" * 40)
    df_outcomes = analyze_match_outcomes(engine)
    if not df_outcomes.empty:
        print(df_outcomes.to_string(index=False))

    print("\n📈 FEEDBACK SCORE CORRELATION")
    print("-" * 40)
    df_feedback = analyze_feedback_correlation(engine)
    if not df_feedback.empty:
        print(df_feedback.to_string(index=False))

        # Check if confirmed matches have higher scores
        confirmed = df_feedback[df_feedback['action'] == 'CONFIRMED']
        rejected = df_feedback[df_feedback['action'] == 'REJECTED']
        if not confirmed.empty and not rejected.empty:
            conf_score = confirmed['avg_total_score'].iloc[0]
            rej_score = rejected['avg_total_score'].iloc[0]
            if conf_score > rej_score:
                print(f"\n✅ Confirmed matches have higher scores ({conf_score} vs {rej_score})")
            else:
                print(f"\n⚠️ Score correlation issue: confirmed={conf_score}, rejected={rej_score}")

    print("\n⚖️ WEIGHT HISTORY")
    print("-" * 40)
    df_weights = analyze_weight_history(engine)
    if not df_weights.empty:
        print(df_weights.to_string(index=False))
    else:
        print("No weight history found.")

    # Save
    output_file = f'{REPORTS_DIR}/08_matching_analysis.xlsx'
    with pd.ExcelWriter(output_file) as writer:
        if not df_scores.empty:
            df_scores.to_excel(writer, sheet_name='Score Distribution', index=False)
        if not df_outcomes.empty:
            df_outcomes.to_excel(writer, sheet_name='Match Outcomes', index=False)
        if not df_feedback.empty:
            df_feedback.to_excel(writer, sheet_name='Feedback Correlation', index=False)
        if not df_weights.empty:
            df_weights.to_excel(writer, sheet_name='Weight History', index=False)

    print(f"\n✅ Report saved to {output_file}")


if __name__ == '__main__':
    main()
