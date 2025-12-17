# PharmaBroker Data Analysis Roadmap

## Executive Summary

This document provides a comprehensive, step-by-step roadmap for analyzing the PharmaBroker PostgreSQL database. It covers data accuracy verification, null value identification, data integrity assessment, and complete understanding of the data flow from WhatsApp message ingestion to match generation.

**Analysis Date**: December 2025  
**Database**: PostgreSQL 15+ with pgvector extension  
**Primary Tables**: 15 core tables  
**Estimated Total Analysis Time**: 4-6 hours (all phases)

---

## Prerequisites & Environment Setup

### Required Software

| Software          | Version | Purpose                   |
| ----------------- | ------- | ------------------------- |
| Python            | 3.10+   | Analysis scripts          |
| PostgreSQL Client | 15+     | Database access           |
| Docker            | 20+     | If using containerized DB |

### Python Dependencies

```bash
uv sync
```

### Environment Configuration

```bash
# Create .env file in analysis directory
DATABASE_URL=postgresql://postgres:postgres@localhost:5432/pharmabroker

# Or export directly
export DATABASE_URL="postgresql://user:password@host:5432/pharmabroker"
```

### Directory Structure

```
pharma-broker/
├── analysis/
│   ├── 00_config.py          # Shared configuration
│   ├── 01_schema_discovery.py
│   ├── 02_null_analysis.py
│   ├── 03_data_quality.py
│   ├── 04_referential_integrity.py
│   ├── 05_business_logic.py
│   ├── 06_time_series.py
│   ├── 07_ai_parsing_quality.py
│   ├── 08_matching_analysis.py
│   ├── run_all.py
│   └── requirements.txt
├── reports/                   # Generated reports (auto-created)
```

---

## Table of Contents

1. [Database Schema Overview](#1-database-schema-overview)
2. [Data Flow Analysis](#2-data-flow-analysis)
3. [Phase 1: Connection & Schema Discovery](#phase-1-connection--schema-discovery)
4. [Phase 2: Null Value Analysis](#phase-2-null-value-analysis)
5. [Phase 3: Data Quality Assessment](#phase-3-data-quality-assessment)
6. [Phase 4: Referential Integrity](#phase-4-referential-integrity)
7. [Phase 5: Business Logic Validation](#phase-5-business-logic-validation)
8. [Phase 6: Time Series Analysis](#phase-6-time-series-analysis)
9. [Phase 7: AI Parsing Quality](#phase-7-ai-parsing-quality)
10. [Phase 8: Matching Engine Analysis](#phase-8-matching-engine-analysis)
11. [Troubleshooting Guide](#troubleshooting-guide)
12. [Glossary](#glossary)

---

## 1. Database Schema Overview

### Core Tables

| Table                  | Purpose                    | Key Columns                          | Expected Nulls                  |
| ---------------------- | -------------------------- | ------------------------------------ | ------------------------------- |
| `raw_messages`         | Incoming WhatsApp messages | id, content, group_jid, sender_phone | sender*name, reply_to*\*        |
| `offers`               | Medication supply offers   | id, medication, source_phone         | price, unit, expiry_date, notes |
| `requests`             | Medication demand requests | id, medication, source_phone         | max_price, unit, notes          |
| `matches`              | Offer-Request matches      | id, offer_id, request_id, score      | reasoning, confirmed_at, notes  |
| `groups`               | WhatsApp groups            | jid, name, monitored                 | description, last_message       |
| `medication_mappings`  | Arabic→English mappings    | id, arabic_name, english_name        | embedding                       |
| `match_feedback`       | Operator feedback          | id, match_id, decision               | operator_id, reason             |
| `review_queue`         | Manual review items        | id, raw_message_id, content          | reply_context, failure_reason   |
| `unmapped_medications` | Unknown medications        | id, raw_text, ai_output              | approved_name                   |
| `feedback_records`     | Learning feedback          | id, match_id, action, scores         | user_id                         |
| `weight_history`       | Scoring weight changes     | id, weights, source                  | improvement, notes              |
| `audit_logs`           | System audit trail         | id, action, created_at               | entity_id, old_value, new_value |
| `config`               | App configuration          | key, value                           | -                               |
| `demand_leaderboard`   | Medication demand stats    | medication, request_count            | -                               |
| `match_queue`          | Pending match jobs         | id, source_type, source_id           | -                               |

### Nullable Fields by Design

These fields are intentionally nullable based on business logic:

```
raw_messages:
  - sender_name: WhatsApp may not provide display name
  - reply_to_id/content/sender: Only set for reply messages
  - processed_at: NULL until AI processing completes
  - error: NULL if processing succeeded

offers:
  - price: Not all offers specify price
  - unit: May be implicit (e.g., "10 boxes" vs "10")
  - expiry_date: Rarely specified in messages
  - batch_number: Rarely specified
  - notes: Optional additional info

requests:
  - max_price: Budget not always specified
  - unit: May be implicit
  - notes: Optional

matches:
  - reasoning: AI-generated explanation
  - confirmed_at: NULL until confirmed
  - notes: Operator notes
```

---

## 2. Data Flow Analysis

### Message Processing Pipeline

```
┌─────────────────┐
│  WhatsApp       │
│  Message        │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  raw_messages   │  ← Step 1: Store raw message
│  (all messages) │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  AI Parser      │  ← Step 2: Extract medication info
│  (Gemini API)   │
└────────┬────────┘
         │
    ┌────┴────┐
    │         │
    ▼         ▼
┌───────┐ ┌─────────┐
│offers │ │requests │  ← Step 3: Create structured records
└───┬───┘ └────┬────┘
    │          │
    └────┬─────┘
         │
         ▼
┌─────────────────┐
│  match_queue    │  ← Step 4: Queue for matching
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Matching       │  ← Step 5: Score & create matches
│  Engine         │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  matches        │  ← Step 6: Store match results
└─────────────────┘
```

### Expected Data Relationships

```sql
-- Each offer/request should link to a raw_message
offers.raw_message_id → raw_messages.id
requests.raw_message_id → raw_messages.id

-- Each match links offer and request
matches.offer_id → offers.id
matches.request_id → requests.id

-- Feedback links to matches
match_feedback.match_id → matches.id
feedback_records.match_id → matches.id
```

---

## Phase 1: Connection & Schema Discovery

### Overview

| Attribute        | Value                               |
| ---------------- | ----------------------------------- |
| **Duration**     | 15-20 minutes                       |
| **Difficulty**   | Easy                                |
| **Dependencies** | Database access, Python environment |
| **Output Files** | `reports/01_table_overview.csv`     |

### Objectives

1. Establish secure database connection
2. Enumerate all tables and their structures
3. Capture row counts and column statistics
4. Document database size and growth potential
5. Create baseline schema documentation

### Prerequisites

- [ ] PostgreSQL database is running and accessible
- [ ] Database credentials are configured in `.env` or environment
- [ ] Python virtual environment is activated
- [ ] Required packages are installed (`sqlalchemy`, `pandas`, `tabulate`)

### Expected Deliverables

| Deliverable     | Description                         | Format         |
| --------------- | ----------------------------------- | -------------- |
| Table Overview  | All tables with row/column counts   | CSV            |
| Schema Details  | Column types, nullability, defaults | Console output |
| Database Size   | Total storage used                  | Console output |
| Connection Test | Verified connectivity               | Pass/Fail      |

### Success Criteria

- ✅ All 15 core tables are discovered
- ✅ Row counts are retrieved without errors
- ✅ No connection timeouts or authentication failures
- ✅ CSV report is generated successfully

### Best Practices

1. **Connection Pooling**: Use SQLAlchemy's connection pooling for efficiency
2. **Read-Only Access**: Use a read-only database user for analysis
3. **Timeout Configuration**: Set reasonable query timeouts (30s default)
4. **Error Handling**: Gracefully handle missing tables or permissions

### Potential Challenges

| Challenge             | Symptom                                 | Solution                        |
| --------------------- | --------------------------------------- | ------------------------------- |
| Connection refused    | `psycopg2.OperationalError`             | Check host/port, firewall rules |
| Authentication failed | `FATAL: password authentication failed` | Verify credentials in `.env`    |
| Permission denied     | `permission denied for table`           | Grant SELECT on all tables      |
| Slow queries          | Timeout on large tables                 | Add query timeout, use LIMIT    |

### Python Script: `01_schema_discovery.py`

```python
"""
Phase 1: Database Schema Discovery
Connects to PostgreSQL and analyzes table structure.
"""
import pandas as pd
from sqlalchemy import create_engine, inspect, text
from sqlalchemy.exc import OperationalError, ProgrammingError
from tabulate import tabulate
import os
import sys

# Import shared config
try:
    from config import DATABASE_URL, REPORTS_DIR
except ImportError:
    DATABASE_URL = os.getenv('DATABASE_URL', 'postgresql://postgres:postgres@localhost:5432/pharmabroker')
    REPORTS_DIR = 'reports'
    os.makedirs(REPORTS_DIR, exist_ok=True)

def connect_db():
    """Create database connection with error handling."""
    try:
        engine = create_engine(DATABASE_URL, pool_pre_ping=True)
        # Test connection
        with engine.connect() as conn:
            conn.execute(text("SELECT 1"))
        print("✅ Database connection successful")
        return engine
    except OperationalError as e:
        print(f"❌ Connection failed: {e}")
        sys.exit(1)

def get_table_info(engine):
    """Get all tables with row counts and column counts."""
    inspector = inspect(engine)
    tables = inspector.get_table_names()

    results = []
    with engine.connect() as conn:
        for table in tables:
            try:
                result = conn.execute(text(f'SELECT COUNT(*) FROM "{table}"'))
                row_count = result.scalar()
                columns = inspector.get_columns(table)
                col_count = len(columns)
                nullable_cols = [c['name'] for c in columns if c['nullable']]

                results.append({
                    'table': table,
                    'rows': row_count,
                    'columns': col_count,
                    'nullable_columns': len(nullable_cols),
                    'nullable_list': ', '.join(nullable_cols[:5]) + ('...' if len(nullable_cols) > 5 else '')
                })
            except ProgrammingError as e:
                print(f"⚠️ Error reading {table}: {e}")

    return pd.DataFrame(results)

def get_column_details(engine, table_name):
    """Get detailed column information for a table."""
    inspector = inspect(engine)
    columns = inspector.get_columns(table_name)

    results = []
    for col in columns:
        results.append({
            'column': col['name'],
            'type': str(col['type']),
            'nullable': '✓' if col['nullable'] else '✗',
            'default': str(col.get('default', ''))[:30] if col.get('default') else '-'
        })

    return pd.DataFrame(results)

def main():
    print("=" * 60)
    print("PHASE 1: DATABASE SCHEMA DISCOVERY")
    print("=" * 60)

    engine = connect_db()

    # 1. Table Overview
    print("\n📊 TABLE OVERVIEW")
    print("-" * 40)
    df_tables = get_table_info(engine)
    print(tabulate(df_tables, headers='keys', tablefmt='grid', showindex=False))

    # 2. Total database size
    with engine.connect() as conn:
        result = conn.execute(text("""
            SELECT pg_size_pretty(pg_database_size(current_database())) as size
        """))
        db_size = result.scalar()
        print(f"\n💾 Database Size: {db_size}")

        # Table sizes
        result = conn.execute(text("""
            SELECT relname as table,
                   pg_size_pretty(pg_total_relation_size(relid)) as size
            FROM pg_catalog.pg_statio_user_tables
            ORDER BY pg_total_relation_size(relid) DESC
            LIMIT 5
        """))
        print("\n📦 Largest Tables:")
        for row in result:
            print(f"  {row[0]}: {row[1]}")

    # 3. Key tables detailed schema
    key_tables = ['raw_messages', 'offers', 'requests', 'matches']
    for table in key_tables:
        print(f"\n📋 {table.upper()} SCHEMA")
        print("-" * 40)
        df_cols = get_column_details(engine, table)
        print(tabulate(df_cols, headers='keys', tablefmt='simple', showindex=False))

    # Save results
    output_file = f'{REPORTS_DIR}/01_table_overview.csv'
    df_tables.to_csv(output_file, index=False)
    print(f"\n✅ Results saved to {output_file}")

    # Summary
    total_rows = df_tables['rows'].sum()
    print(f"\n📈 SUMMARY: {len(df_tables)} tables, {total_rows:,} total rows")

if __name__ == '__main__':
    main()
```

---

## Phase 2: Null Value Analysis

### Overview

| Attribute        | Value                          |
| ---------------- | ------------------------------ |
| **Duration**     | 20-30 minutes                  |
| **Difficulty**   | Easy                           |
| **Dependencies** | Phase 1 completed              |
| **Output Files** | `reports/02_null_analysis.csv` |

### Objectives

1. Identify all NULL values across every column in every table
2. Calculate NULL percentages for each column
3. Categorize NULLs as "expected" (by design) or "problematic"
4. Flag columns with unexpectedly high NULL rates
5. Provide actionable recommendations for data quality improvement

### Prerequisites

- [ ] Phase 1 completed successfully
- [ ] Understanding of which fields are intentionally nullable (see Section 1)
- [ ] Sufficient database permissions to query all tables

### Expected Deliverables

| Deliverable          | Description                            | Format         |
| -------------------- | -------------------------------------- | -------------- |
| Null Analysis Report | Per-column null counts and percentages | CSV            |
| Status Summary       | Columns grouped by null status         | Console output |
| Problem List         | Columns requiring attention            | Console output |
| Recommendations      | Actions for high-null columns          | Console output |

### Success Criteria

- ✅ All columns analyzed across all tables
- ✅ Expected nulls correctly identified (no false positives)
- ✅ Problematic nulls flagged for review
- ✅ NULL percentage calculated accurately

### Best Practices

1. **Baseline First**: Establish expected null patterns before flagging issues
2. **Context Matters**: A 90% null rate in `expiry_date` is normal; in `medication` it's critical
3. **Trend Analysis**: Compare null rates over time if historical data exists
4. **Document Exceptions**: Record why certain high-null columns are acceptable

### Potential Challenges

| Challenge       | Symptom                   | Solution                          |
| --------------- | ------------------------- | --------------------------------- |
| False positives | Expected nulls flagged    | Update `EXPECTED_NULLS` config    |
| Slow analysis   | Timeout on large tables   | Use sampling for initial analysis |
| New columns     | Unknown null expectations | Review with domain expert         |
| Schema changes  | Missing columns           | Re-run Phase 1 first              |

### Interpretation Guide

| Status              | Meaning                   | Action                 |
| ------------------- | ------------------------- | ---------------------- |
| `No Nulls`          | Column has no NULL values | ✅ Good                |
| `Expected`          | NULLs are by design       | ✅ Good                |
| `🔍 Review`         | 1-50% nulls, not expected | Investigate cause      |
| `⚠️ High Null Rate` | >50% nulls, not expected  | Critical review needed |

### Python Script: `02_null_analysis.py`

```python
"""
Phase 2: Null Value Analysis
Identifies NULL values and their distribution across all tables.
"""
import pandas as pd
from sqlalchemy import create_engine, text
import os

try:
    from config import DATABASE_URL, REPORTS_DIR, EXPECTED_NULLS
except ImportError:
    DATABASE_URL = os.getenv('DATABASE_URL', 'postgresql://postgres:postgres@localhost:5432/pharmabroker')
    REPORTS_DIR = 'reports'
    EXPECTED_NULLS = {
        'raw_messages': ['sender_name', 'reply_to_id', 'reply_to_content', 'reply_to_sender',
                         'processed_at', 'error', 'external_id'],
        'offers': ['raw_message_id', 'source_name', 'group_name', 'unit', 'price',
                   'expiry_date', 'batch_number', 'notes'],
        'requests': ['raw_message_id', 'source_name', 'group_name', 'unit', 'max_price', 'notes'],
        'matches': ['reasoning', 'matched_by', 'confirmed_at', 'notes'],
        'groups': ['description', 'last_message'],
        'match_feedback': ['operator_id', 'reason', 'original_confidence'],
        'review_queue': ['reply_context', 'failure_reason', 'reviewed_by', 'reviewed_at',
                         'review_note', 'corrected_items'],
        'unmapped_medications': ['approved_name', 'reviewed_at'],
        'feedback_records': ['user_id'],
        'weight_history': ['improvement', 'notes', 'performance_metrics'],
        'audit_logs': ['entity_id', 'old_value', 'new_value', 'details', 'ip_address'],
        'medication_mappings': ['embedding']
    }
    os.makedirs(REPORTS_DIR, exist_ok=True)

def analyze_nulls(engine, table_name):
    """Analyze NULL values for each column in a table."""
    with engine.connect() as conn:
        result = conn.execute(text(f"""
            SELECT column_name FROM information_schema.columns
            WHERE table_name = '{table_name}' ORDER BY ordinal_position
        """))
        columns = [row[0] for row in result]

        total = conn.execute(text(f'SELECT COUNT(*) FROM "{table_name}"')).scalar()
        if total == 0:
            return pd.DataFrame()

        results = []
        for col in columns:
            null_count = conn.execute(text(f'''
                SELECT COUNT(*) FROM "{table_name}" WHERE "{col}" IS NULL
            ''')).scalar()

            results.append({
                'table': table_name,
                'column': col,
                'total_rows': total,
                'null_count': null_count,
                'null_pct': round(null_count / total * 100, 2) if total > 0 else 0,
                'has_data': total - null_count
            })

        return pd.DataFrame(results)

def categorize_nulls(df):
    """Categorize NULL values as expected or problematic."""
    def is_expected(row):
        table = row['table']
        col = row['column']
        if table in EXPECTED_NULLS and col in EXPECTED_NULLS[table]:
            return 'Expected'
        elif row['null_pct'] == 0:
            return 'No Nulls'
        elif row['null_pct'] > 50:
            return '⚠️ High Null Rate'
        else:
            return '🔍 Review'

    df['status'] = df.apply(is_expected, axis=1)
    return df

def main():
    print("=" * 60)
    print("PHASE 2: NULL VALUE ANALYSIS")
    print("=" * 60)

    engine = create_engine(DATABASE_URL)

    with engine.connect() as conn:
        result = conn.execute(text("""
            SELECT table_name FROM information_schema.tables
            WHERE table_schema = 'public' AND table_type = 'BASE TABLE'
        """))
        tables = [row[0] for row in result]

    all_results = []
    for table in tables:
        print(f"Analyzing {table}...")
        df = analyze_nulls(engine, table)
        if not df.empty:
            all_results.append(df)

    df_all = pd.concat(all_results, ignore_index=True)
    df_all = categorize_nulls(df_all)

    # Summary
    print("\n📊 NULL VALUE SUMMARY BY STATUS")
    print("-" * 40)
    summary = df_all.groupby('status').agg({
        'column': 'count',
        'null_count': 'sum'
    }).rename(columns={'column': 'columns_affected'})
    print(summary)

    # Problems
    print("\n⚠️ COLUMNS REQUIRING ATTENTION")
    print("-" * 40)
    problems = df_all[df_all['status'].isin(['⚠️ High Null Rate', '🔍 Review'])]
    problems = problems[problems['null_pct'] > 0].sort_values('null_pct', ascending=False)
    if not problems.empty:
        print(problems[['table', 'column', 'null_count', 'null_pct', 'status']].head(20).to_string(index=False))
    else:
        print("✅ No problematic columns found!")

    # Save
    output_file = f'{REPORTS_DIR}/02_null_analysis.csv'
    df_all.to_csv(output_file, index=False)
    print(f"\n✅ Full report saved to {output_file}")

if __name__ == '__main__':
    main()
```

---

## Phase 3: Data Quality Assessment

### Overview

| Attribute        | Value                                        |
| ---------------- | -------------------------------------------- |
| **Duration**     | 25-35 minutes                                |
| **Difficulty**   | Medium                                       |
| **Dependencies** | Phase 1, Phase 2 completed                   |
| **Output Files** | `reports/03_data_quality.xlsx` (multi-sheet) |

### Objectives

1. Detect duplicate records that violate uniqueness constraints
2. Validate enum/status fields contain only valid values
3. Verify numeric scores are within expected ranges (0-1)
4. Identify empty strings in required text fields
5. Assess overall data quality score

### Prerequisites

- [ ] Phases 1-2 completed
- [ ] Understanding of valid status values (ACTIVE, PENDING, etc.)
- [ ] Knowledge of score ranges (0.0 to 1.0)

### Expected Deliverables

| Deliverable       | Description                       | Format         |
| ----------------- | --------------------------------- | -------------- |
| Duplicate Report  | Tables with duplicate records     | Excel sheet    |
| Status Validation | Invalid status values found       | Excel sheet    |
| Score Ranges      | Min/max/avg for all score columns | Excel sheet    |
| Empty Strings     | Text fields with empty values     | Excel sheet    |
| Quality Score     | Overall data quality percentage   | Console output |

### Success Criteria

- ✅ Zero duplicate matches (offer_id + request_id)
- ✅ All status values are valid enums
- ✅ All scores between 0.0 and 1.0
- ✅ No empty strings in medication names
- ✅ Data quality score > 95%

### Best Practices

1. **Unique Constraints**: Verify database has proper UNIQUE indexes
2. **Enum Validation**: Consider adding CHECK constraints for status fields
3. **Score Bounds**: Add CHECK constraints for score columns (0 <= score <= 1)
4. **Empty vs NULL**: Distinguish between empty strings and NULL values

### Potential Challenges

| Challenge                | Symptom                | Solution                       |
| ------------------------ | ---------------------- | ------------------------------ |
| Duplicate detection slow | Query timeout          | Add indexes on checked columns |
| Invalid status values    | Unexpected enum values | Update application validation  |
| Scores out of range      | Values > 1 or < 0      | Fix scoring algorithm          |
| Empty medication names   | Critical data missing  | Review AI parsing logic        |

### Data Quality Scoring Formula

```
Quality Score = (
    (1 - duplicate_rate) * 0.25 +
    (1 - invalid_status_rate) * 0.25 +
    (1 - out_of_range_score_rate) * 0.25 +
    (1 - empty_string_rate) * 0.25
) * 100
```

### Python Script: `03_data_quality.py`

```python
"""
Phase 3: Data Quality Assessment
Checks for duplicates, invalid values, and data consistency.
"""
import pandas as pd
from sqlalchemy import create_engine, text
import os

try:
    from config import DATABASE_URL, REPORTS_DIR, VALID_STATUSES
except ImportError:
    DATABASE_URL = os.getenv('DATABASE_URL', 'postgresql://postgres:postgres@localhost:5432/pharmabroker')
    REPORTS_DIR = 'reports'
    VALID_STATUSES = {
        'offers': ['ACTIVE', 'MATCHED', 'EXPIRED', 'ARCHIVED'],
        'requests': ['ACTIVE', 'MATCHED', 'EXPIRED', 'ARCHIVED'],
        'matches': ['PENDING', 'CONFIRMED', 'REJECTED'],
        'review_queue': ['PENDING', 'APPROVED', 'REJECTED'],
    }
    os.makedirs(REPORTS_DIR, exist_ok=True)

def check_duplicates(engine):
    """Check for duplicate records in key tables."""
    checks = [
        ('raw_messages', 'external_id', 'Duplicate WhatsApp message IDs'),
        ('offers', 'raw_message_id, medication', 'Duplicate offers from same message'),
        ('requests', 'raw_message_id, medication', 'Duplicate requests from same message'),
        ('matches', 'offer_id, request_id', 'Duplicate matches'),
        ('medication_mappings', 'arabic_name', 'Duplicate Arabic medication names'),
    ]

    results = []
    with engine.connect() as conn:
        for table, columns, description in checks:
            query = f'''
                SELECT COUNT(*) as dup_count FROM (
                    SELECT {columns}, COUNT(*) as cnt FROM "{table}"
                    WHERE {columns.split(',')[0].strip()} IS NOT NULL
                    GROUP BY {columns} HAVING COUNT(*) > 1
                ) sub
            '''
            try:
                dup_count = conn.execute(text(query)).scalar() or 0
                results.append({
                    'table': table,
                    'check': description,
                    'duplicates': dup_count,
                    'status': '✅ OK' if dup_count == 0 else '⚠️ Has Duplicates'
                })
            except Exception as e:
                results.append({
                    'table': table, 'check': description,
                    'duplicates': 'Error', 'status': f'❌ {str(e)[:30]}'
                })
    return pd.DataFrame(results)

def check_status_values(engine):
    """Validate status field values against expected enums."""
    results = []
    with engine.connect() as conn:
        for table, valid_values in VALID_STATUSES.items():
            query = f'SELECT status, COUNT(*) as cnt FROM "{table}" GROUP BY status'
            try:
                df = pd.read_sql(query, conn)
                for _, row in df.iterrows():
                    status = row['status']
                    is_valid = status in valid_values
                    results.append({
                        'table': table, 'column': 'status',
                        'value': status, 'count': row['cnt'],
                        'valid': '✅' if is_valid else '❌ Invalid'
                    })
            except Exception:
                pass
    return pd.DataFrame(results)

def check_score_ranges(engine):
    """Validate score values are within expected ranges (0-1)."""
    checks = [
        ('matches', 'score'),
        ('feedback_records', 'medication_score'),
        ('feedback_records', 'total_score'),
        ('review_queue', 'avg_confidence'),
    ]

    results = []
    with engine.connect() as conn:
        for table, column in checks:
            query = f'''
                SELECT MIN("{column}") as min_val, MAX("{column}") as max_val,
                       AVG("{column}") as avg_val, COUNT(*) as total,
                       SUM(CASE WHEN "{column}" < 0 OR "{column}" > 1 THEN 1 ELSE 0 END) as out_of_range
                FROM "{table}"
            '''
            try:
                row = conn.execute(text(query)).fetchone()
                results.append({
                    'table': table, 'column': column,
                    'min': round(row[0], 4) if row[0] else None,
                    'max': round(row[1], 4) if row[1] else None,
                    'avg': round(row[2], 4) if row[2] else None,
                    'out_of_range': row[4],
                    'status': '✅' if row[4] == 0 else '⚠️'
                })
            except Exception as e:
                results.append({
                    'table': table, 'column': column,
                    'min': None, 'max': None, 'avg': None,
                    'out_of_range': 'Error', 'status': f'❌ {str(e)[:20]}'
                })
    return pd.DataFrame(results)

def check_empty_strings(engine):
    """Check for empty strings in required text fields."""
    checks = [
        ('raw_messages', 'content'),
        ('offers', 'medication'),
        ('offers', 'medication_raw'),
        ('requests', 'medication'),
        ('groups', 'name'),
    ]

    results = []
    with engine.connect() as conn:
        for table, column in checks:
            query = f'''
                SELECT COUNT(*) as total,
                       SUM(CASE WHEN "{column}" = '' THEN 1 ELSE 0 END) as empty_count
                FROM "{table}"
            '''
            row = conn.execute(text(query)).fetchone()
            results.append({
                'table': table, 'column': column,
                'total': row[0], 'empty_strings': row[1],
                'status': '✅' if row[1] == 0 else '⚠️ Has Empty Strings'
            })
    return pd.DataFrame(results)

def main():
    print("=" * 60)
    print("PHASE 3: DATA QUALITY ASSESSMENT")
    print("=" * 60)

    engine = create_engine(DATABASE_URL)

    print("\n🔍 DUPLICATE CHECK")
    df_dups = check_duplicates(engine)
    print(df_dups.to_string(index=False))

    print("\n📋 STATUS VALUE VALIDATION")
    df_status = check_status_values(engine)
    if not df_status.empty:
        print(df_status.to_string(index=False))

    print("\n📊 SCORE RANGE VALIDATION (0-1)")
    df_scores = check_score_ranges(engine)
    print(df_scores.to_string(index=False))

    print("\n📝 EMPTY STRING CHECK")
    df_empty = check_empty_strings(engine)
    print(df_empty.to_string(index=False))

    # Calculate quality score
    dup_issues = len(df_dups[df_dups['status'] != '✅ OK'])
    status_issues = len(df_status[df_status['valid'] != '✅']) if not df_status.empty else 0
    score_issues = len(df_scores[df_scores['status'] != '✅'])
    empty_issues = len(df_empty[df_empty['status'] != '✅'])

    total_checks = len(df_dups) + len(df_status) + len(df_scores) + len(df_empty)
    total_issues = dup_issues + status_issues + score_issues + empty_issues
    quality_score = ((total_checks - total_issues) / total_checks * 100) if total_checks > 0 else 100

    print(f"\n📈 DATA QUALITY SCORE: {quality_score:.1f}%")

    # Save
    output_file = f'{REPORTS_DIR}/03_data_quality.xlsx'
    with pd.ExcelWriter(output_file) as writer:
        df_dups.to_excel(writer, sheet_name='Duplicates', index=False)
        if not df_status.empty:
            df_status.to_excel(writer, sheet_name='Status Values', index=False)
        df_scores.to_excel(writer, sheet_name='Score Ranges', index=False)
        df_empty.to_excel(writer, sheet_name='Empty Strings', index=False)

    print(f"✅ Full report saved to {output_file}")

if __name__ == '__main__':
    main()
```

---

## Phase 4: Referential Integrity

### Overview

| Attribute        | Value                                  |
| ---------------- | -------------------------------------- |
| **Duration**     | 15-25 minutes                          |
| **Difficulty**   | Medium                                 |
| **Dependencies** | Phase 1 completed                      |
| **Output Files** | `reports/04_referential_integrity.csv` |

### Objectives

1. Verify all foreign key relationships are valid
2. Identify orphaned records (child records without parents)
3. Detect broken links in the data chain
4. Quantify data relationship health
5. Provide cleanup recommendations

### Prerequisites

- [ ] Understanding of table relationships (see Section 2)
- [ ] Knowledge of which FKs are nullable vs required

### Expected Deliverables

| Deliverable          | Description                                 | Format         |
| -------------------- | ------------------------------------------- | -------------- |
| FK Validation Report | All relationships checked                   | CSV            |
| Orphan Count         | Number of orphaned records per relationship | Console output |
| Orphan Samples       | Example orphaned records for investigation  | Console output |

### Success Criteria

- ✅ Zero orphaned matches (all offer_id and request_id exist)
- ✅ Zero orphaned feedback records
- ✅ All raw_message_id references are valid (where not NULL)

### Best Practices

1. **Cascade Deletes**: Consider ON DELETE CASCADE for dependent tables
2. **Soft Deletes**: Use status flags instead of hard deletes
3. **Regular Cleanup**: Schedule orphan detection as maintenance task
4. **FK Constraints**: Add actual FK constraints to database schema

### Potential Challenges

| Challenge       | Symptom                     | Solution                      |
| --------------- | --------------------------- | ----------------------------- |
| Many orphans    | High orphan count           | Investigate deletion patterns |
| Slow queries    | Timeout on large tables     | Add indexes on FK columns     |
| Historical data | Old records without parents | Archive or mark as legacy     |

### Python Script: `04_referential_integrity.py`

```python
"""
Phase 4: Referential Integrity Check
Verifies foreign key relationships and identifies orphaned records.
"""
import pandas as pd
from sqlalchemy import create_engine, text
import os

try:
    from config import DATABASE_URL, REPORTS_DIR, FK_RELATIONSHIPS
except ImportError:
    DATABASE_URL = os.getenv('DATABASE_URL', 'postgresql://postgres:postgres@localhost:5432/pharmabroker')
    REPORTS_DIR = 'reports'
    FK_RELATIONSHIPS = [
        ('offers', 'raw_message_id', 'raw_messages', 'id'),
        ('requests', 'raw_message_id', 'raw_messages', 'id'),
        ('matches', 'offer_id', 'offers', 'id'),
        ('matches', 'request_id', 'requests', 'id'),
        ('match_feedback', 'match_id', 'matches', 'id'),
        ('feedback_records', 'match_id', 'matches', 'id'),
        ('review_queue', 'raw_message_id', 'raw_messages', 'id'),
    ]
    os.makedirs(REPORTS_DIR, exist_ok=True)

def check_foreign_keys(engine):
    """Check all foreign key relationships."""
    results = []
    with engine.connect() as conn:
        for child_table, child_col, parent_table, parent_col in FK_RELATIONSHIPS:
            desc = f'{child_table}.{child_col} → {parent_table}.{parent_col}'

            # Count orphaned records
            query = f'''
                SELECT COUNT(*) FROM "{child_table}" c
                WHERE c."{child_col}" IS NOT NULL
                AND NOT EXISTS (
                    SELECT 1 FROM "{parent_table}" p
                    WHERE p."{parent_col}" = c."{child_col}"
                )
            '''
            orphan_count = conn.execute(text(query)).scalar()

            # Count total with FK
            total_query = f'SELECT COUNT(*) FROM "{child_table}" WHERE "{child_col}" IS NOT NULL'
            total = conn.execute(text(total_query)).scalar()

            results.append({
                'relationship': desc,
                'child_table': child_table,
                'parent_table': parent_table,
                'total_refs': total,
                'orphaned': orphan_count,
                'orphan_pct': round(orphan_count / total * 100, 2) if total > 0 else 0,
                'status': '✅ OK' if orphan_count == 0 else '❌ Orphans Found'
            })

    return pd.DataFrame(results)

def main():
    print("=" * 60)
    print("PHASE 4: REFERENTIAL INTEGRITY CHECK")
    print("=" * 60)

    engine = create_engine(DATABASE_URL)

    df_fk = check_foreign_keys(engine)
    print("\n🔗 FOREIGN KEY RELATIONSHIPS")
    print("-" * 40)
    print(df_fk.to_string(index=False))

    # Summary
    total_orphans = df_fk['orphaned'].sum()
    if total_orphans > 0:
        print(f"\n⚠️ TOTAL ORPHANED RECORDS: {total_orphans}")
    else:
        print("\n✅ All foreign key relationships are valid!")

    output_file = f'{REPORTS_DIR}/04_referential_integrity.csv'
    df_fk.to_csv(output_file, index=False)
    print(f"\n✅ Report saved to {output_file}")

if __name__ == '__main__':
    main()
```

---

## Phase 5: Business Logic Validation

### Overview

| Attribute        | Value                           |
| ---------------- | ------------------------------- |
| **Duration**     | 20-30 minutes                   |
| **Difficulty**   | Medium                          |
| **Dependencies** | Phases 1-4 completed            |
| **Output Files** | `reports/05_business_logic.csv` |

### Objectives

1. Validate data against PharmaBroker business rules
2. Ensure required fields are populated
3. Verify logical consistency (e.g., confirmed matches have timestamps)
4. Check numeric constraints (non-negative quantities/prices)
5. Validate processing pipeline completeness

### Prerequisites

- [ ] Understanding of PharmaBroker business rules
- [ ] Knowledge of required vs optional fields
- [ ] Familiarity with message processing pipeline

### Business Rules Validated

| Rule   | Description                                      | Severity |
| ------ | ------------------------------------------------ | -------- |
| BR-001 | Offers must have medication name                 | Critical |
| BR-002 | Requests must have medication name               | Critical |
| BR-003 | Match scores must be 0-1                         | Critical |
| BR-004 | Confirmed matches need confirmed_at              | Warning  |
| BR-005 | Raw messages must have content                   | Critical |
| BR-006 | Processed messages should create offers/requests | Warning  |
| BR-007 | Quantity must be non-negative                    | Critical |
| BR-008 | Price must be non-negative                       | Critical |

### Success Criteria

- ✅ All critical rules pass (0 violations)
- ✅ Warning rules have < 5% violation rate
- ✅ Business logic score > 95%

### Python Script: `05_business_logic.py`

```python
"""
Phase 5: Business Logic Validation
Validates data against business rules.
"""
import pandas as pd
from sqlalchemy import create_engine, text
import os

try:
    from config import DATABASE_URL, REPORTS_DIR
except ImportError:
    DATABASE_URL = os.getenv('DATABASE_URL', 'postgresql://postgres:postgres@localhost:5432/pharmabroker')
    REPORTS_DIR = 'reports'
    os.makedirs(REPORTS_DIR, exist_ok=True)

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
```

---

## Phase 6: Time Series Analysis

### Overview

| Attribute        | Value                         |
| ---------------- | ----------------------------- |
| **Duration**     | 25-35 minutes                 |
| **Difficulty**   | Medium                        |
| **Dependencies** | Phases 1-5 completed          |
| **Output Files** | `reports/06_time_series.xlsx` |

### Objectives

1. Analyze message volume trends over time
2. Track processing success rates daily
3. Monitor match creation patterns
4. Identify anomalies (spikes, drops, gaps)
5. Establish baseline metrics for monitoring

### Prerequisites

- [ ] Sufficient historical data (at least 7 days recommended)
- [ ] Understanding of expected daily volumes

### Key Metrics Tracked

| Metric          | Description                          | Alert Threshold  |
| --------------- | ------------------------------------ | ---------------- |
| Daily Messages  | Raw messages received per day        | < 50% of average |
| Processing Rate | % of messages successfully processed | < 90%            |
| Match Rate      | Matches created per offer/request    | < 10%            |
| Error Rate      | Messages with processing errors      | > 5%             |

### Python Script: `06_time_series.py`

```python
"""
Phase 6: Time Series Analysis
Analyzes data patterns over time.
"""
import pandas as pd
from sqlalchemy import create_engine, text
import os

try:
    from config import DATABASE_URL, REPORTS_DIR
except ImportError:
    DATABASE_URL = os.getenv('DATABASE_URL', 'postgresql://postgres:postgres@localhost:5432/pharmabroker')
    REPORTS_DIR = 'reports'
    os.makedirs(REPORTS_DIR, exist_ok=True)

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
```

---

## Phase 7: AI Parsing Quality

### Overview

| Attribute        | Value                                |
| ---------------- | ------------------------------------ |
| **Duration**     | 30-40 minutes                        |
| **Difficulty**   | Advanced                             |
| **Dependencies** | Phases 1-6 completed                 |
| **Output Files** | `reports/07_ai_parsing_quality.xlsx` |

### Objectives

1. Assess AI extraction completeness (medication, quantity, price)
2. Analyze medication name mapping accuracy
3. Review unmapped medications queue
4. Evaluate review queue status
5. Calculate AI parsing quality score

### Key Quality Indicators

| Indicator                  | Target | Critical Threshold |
| -------------------------- | ------ | ------------------ |
| Medication extraction rate | > 99%  | < 95%              |
| Quantity extraction rate   | > 80%  | < 60%              |
| Price extraction rate      | > 50%  | < 30%              |
| Unmapped medication rate   | < 5%   | > 15%              |
| Review queue backlog       | < 100  | > 500              |

### Python Script: `07_ai_parsing_quality.py`

```python
"""
Phase 7: AI Parsing Quality Assessment
Analyzes the quality of AI-extracted medication data.
"""
import pandas as pd
from sqlalchemy import create_engine, text
import os

try:
    from config import DATABASE_URL, REPORTS_DIR
except ImportError:
    DATABASE_URL = os.getenv('DATABASE_URL', 'postgresql://postgres:postgres@localhost:5432/pharmabroker')
    REPORTS_DIR = 'reports'
    os.makedirs(REPORTS_DIR, exist_ok=True)

def analyze_extraction_completeness(engine):
    """Analyze how complete the AI extractions are."""
    with engine.connect() as conn:
        offers_query = '''
            SELECT COUNT(*) as total,
                   SUM(CASE WHEN medication != '' THEN 1 ELSE 0 END) as has_medication,
                   SUM(CASE WHEN quantity > 0 THEN 1 ELSE 0 END) as has_quantity,
                   SUM(CASE WHEN price IS NOT NULL AND price > 0 THEN 1 ELSE 0 END) as has_price,
                   SUM(CASE WHEN unit IS NOT NULL THEN 1 ELSE 0 END) as has_unit
            FROM offers
        '''
        offers = pd.read_sql(offers_query, conn)

        requests_query = '''
            SELECT COUNT(*) as total,
                   SUM(CASE WHEN medication != '' THEN 1 ELSE 0 END) as has_medication,
                   SUM(CASE WHEN quantity > 0 THEN 1 ELSE 0 END) as has_quantity,
                   SUM(CASE WHEN max_price IS NOT NULL AND max_price > 0 THEN 1 ELSE 0 END) as has_max_price,
                   SUM(CASE WHEN unit IS NOT NULL THEN 1 ELSE 0 END) as has_unit
            FROM requests
        '''
        requests = pd.read_sql(requests_query, conn)

    return offers, requests

def analyze_top_medications(engine):
    """Analyze most common medications."""
    query = '''
        SELECT medication, COUNT(*) as occurrences
        FROM (SELECT medication FROM offers UNION ALL SELECT medication FROM requests) combined
        GROUP BY medication ORDER BY occurrences DESC LIMIT 30
    '''
    return pd.read_sql(query, engine)

def analyze_unmapped(engine):
    """Analyze unmapped medications."""
    query = '''
        SELECT raw_text, ai_output, count, reviewed, approved_name
        FROM unmapped_medications ORDER BY count DESC LIMIT 20
    '''
    return pd.read_sql(query, engine)

def analyze_review_queue(engine):
    """Analyze review queue status."""
    query = '''
        SELECT status, COUNT(*) as count, AVG(avg_confidence) as avg_confidence
        FROM review_queue GROUP BY status
    '''
    return pd.read_sql(query, engine)

def main():
    print("=" * 60)
    print("PHASE 7: AI PARSING QUALITY ASSESSMENT")
    print("=" * 60)

    engine = create_engine(DATABASE_URL)

    print("\n📊 EXTRACTION COMPLETENESS")
    print("-" * 40)
    offers, requests = analyze_extraction_completeness(engine)

    if not offers.empty and offers['total'].iloc[0] > 0:
        total = offers['total'].iloc[0]
        print(f"\nOFFERS ({total} total):")
        for col in ['has_medication', 'has_quantity', 'has_price', 'has_unit']:
            val = offers[col].iloc[0]
            pct = (val / total * 100)
            status = '✅' if pct > 80 else '⚠️' if pct > 50 else '❌'
            print(f"  {status} {col}: {val} ({pct:.1f}%)")

    if not requests.empty and requests['total'].iloc[0] > 0:
        total = requests['total'].iloc[0]
        print(f"\nREQUESTS ({total} total):")
        for col in ['has_medication', 'has_quantity', 'has_max_price', 'has_unit']:
            val = requests[col].iloc[0]
            pct = (val / total * 100)
            status = '✅' if pct > 80 else '⚠️' if pct > 50 else '❌'
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
    output_file = f'{REPORTS_DIR}/07_ai_parsing_quality.xlsx'
    with pd.ExcelWriter(output_file) as writer:
        df_meds.to_excel(writer, sheet_name='Top Medications', index=False)
        if not df_unmapped.empty:
            df_unmapped.to_excel(writer, sheet_name='Unmapped', index=False)
        if not df_review.empty:
            df_review.to_excel(writer, sheet_name='Review Queue', index=False)

    print(f"\n✅ Report saved to {output_file}")

if __name__ == '__main__':
    main()
```

---

## Phase 8: Matching Engine Analysis

### Overview

| Attribute        | Value                               |
| ---------------- | ----------------------------------- |
| **Duration**     | 25-35 minutes                       |
| **Difficulty**   | Advanced                            |
| **Dependencies** | All previous phases completed       |
| **Output Files** | `reports/08_matching_analysis.xlsx` |

### Objectives

1. Analyze match score distribution across confidence bands
2. Evaluate match confirmation/rejection rates
3. Correlate feedback with score components
4. Review weight history and learning effectiveness
5. Assess matching engine accuracy

### Confidence Bands

| Band    | Score Range | Action              | Target Rate     |
| ------- | ----------- | ------------------- | --------------- |
| AUTO    | 0.9 - 1.0   | Auto-confirm        | > 95% confirmed |
| SUGGEST | 0.7 - 0.9   | Suggest to operator | > 80% confirmed |
| REVIEW  | 0.5 - 0.7   | Manual review       | > 50% confirmed |
| NONE    | 0.0 - 0.5   | No match            | N/A             |

### Success Criteria

- ✅ AUTO band confirmation rate > 95%
- ✅ Average match score > 0.7
- ✅ Rejection rate in AUTO band < 2%
- ✅ Weight learning shows improvement over time

### Python Script: `08_matching_analysis.py`

```python
"""
Phase 8: Matching Engine Analysis
Analyzes match quality and scoring distribution.
"""
import pandas as pd
from sqlalchemy import create_engine, text
import os

try:
    from config import DATABASE_URL, REPORTS_DIR
except ImportError:
    DATABASE_URL = os.getenv('DATABASE_URL', 'postgresql://postgres:postgres@localhost:5432/pharmabroker')
    REPORTS_DIR = 'reports'
    os.makedirs(REPORTS_DIR, exist_ok=True)

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
```

---

## Troubleshooting Guide

### Common Issues and Solutions

| Issue              | Symptom                       | Solution                                         |
| ------------------ | ----------------------------- | ------------------------------------------------ |
| Connection refused | `psycopg2.OperationalError`   | Check DATABASE_URL, ensure PostgreSQL is running |
| Permission denied  | `permission denied for table` | Grant SELECT privileges to analysis user         |
| Module not found   | `ModuleNotFoundError`         | Run `pip install -r requirements.txt`            |
| Empty results      | All queries return 0 rows     | Verify data exists, check table names            |
| Timeout            | Query takes too long          | Add indexes, use LIMIT for initial testing       |
| Memory error       | Python crashes                | Process tables in batches                        |

### Database Connection Checklist

```bash
# 1. Test PostgreSQL connection
psql -h localhost -U postgres -d pharmabroker -c "SELECT 1"

# 2. Verify tables exist
psql -h localhost -U postgres -d pharmabroker -c "\dt"

# 3. Check row counts
psql -h localhost -U postgres -d pharmabroker -c "SELECT COUNT(*) FROM raw_messages"

# 4. Test Python connection
python -c "from sqlalchemy import create_engine; e = create_engine('$DATABASE_URL'); print(e.connect())"
```

### Performance Optimization

1. **Add Indexes**: Ensure FK columns have indexes
2. **Use LIMIT**: Test queries with LIMIT before full execution
3. **Batch Processing**: Process large tables in chunks
4. **Connection Pooling**: Use SQLAlchemy's pool for multiple queries

---

## Glossary

| Term                   | Definition                                                |
| ---------------------- | --------------------------------------------------------- |
| **Offer**              | A medication supply listing from a WhatsApp message       |
| **Request**            | A medication demand listing from a WhatsApp message       |
| **Match**              | A potential pairing between an offer and request          |
| **Score**              | A 0-1 value indicating match quality                      |
| **Confidence Band**    | Score range determining action (AUTO/SUGGEST/REVIEW/NONE) |
| **Raw Message**        | Original WhatsApp message before AI processing            |
| **Medication Mapping** | Arabic to English medication name translation             |
| **Orphan**             | A child record whose parent record doesn't exist          |
| **FK**                 | Foreign Key - a reference to another table's primary key  |

---

## Quick Reference: Running All Phases

```bash
# Setup
cd pharma-broker
python -m venv venv
venv\Scripts\activate  # Windows
pip install -r analysis/requirements.txt

# Set database URL
set DATABASE_URL=postgresql://postgres:postgres@localhost:5432/pharmabroker

# Run all phases
python analysis/run_all.py

# Or run individual phases
python analysis/01_schema_discovery.py
python analysis/02_null_analysis.py
python analysis/03_data_quality.py
python analysis/04_referential_integrity.py
python analysis/05_business_logic.py
python analysis/06_time_series.py
python analysis/07_ai_parsing_quality.py
python analysis/08_matching_analysis.py
```

---

## Action Items & Recommendations

> **Note**: This section contains recommendations identified during data analysis. Implement after reviewing all phase results.

### 🔴 High Priority

#### 1. Investigate Duplicate Offers (106 found)

**Problem**: Same raw message producing multiple offer records for the same medication.

**Investigation Query**:

```sql
-- Find duplicate offers from same message
SELECT raw_message_id, medication, COUNT(*) as duplicate_count
FROM offers
WHERE raw_message_id IS NOT NULL
GROUP BY raw_message_id, medication
HAVING COUNT(*) > 1
ORDER BY duplicate_count DESC;

-- View actual duplicates with details
SELECT o.id, o.raw_message_id, o.medication, o.medication_raw, o.created_at
FROM offers o
INNER JOIN (
    SELECT raw_message_id, medication
    FROM offers
    WHERE raw_message_id IS NOT NULL
    GROUP BY raw_message_id, medication
    HAVING COUNT(*) > 1
) dup ON o.raw_message_id = dup.raw_message_id AND o.medication = dup.medication
ORDER BY o.raw_message_id, o.medication;
```

**Root Cause Analysis**:

- [ ] AI parser extracting same medication multiple times
- [ ] Message being processed multiple times (retry logic issue)
- [ ] Chunk splitting causing duplicate extraction

**Fix Options**:

1. Add deduplication in `parsing/service.go` before saving offers
2. Add UNIQUE constraint (see below)
3. Clean existing duplicates with:
   ```sql
   -- Keep only the first occurrence (by created_at)
   DELETE FROM offers a USING offers b
   WHERE a.raw_message_id = b.raw_message_id
     AND a.medication = b.medication
     AND a.created_at > b.created_at;
   ```

---

### 🟡 Medium Priority

#### 2. Add UNIQUE Constraints

**Recommendation**: Prevent future duplicates at database level.

```sql
-- Add unique constraint for offers
ALTER TABLE offers ADD CONSTRAINT unique_offer_per_message
UNIQUE (raw_message_id, medication);

-- Add unique constraint for requests
ALTER TABLE requests ADD CONSTRAINT unique_request_per_message
UNIQUE (raw_message_id, medication);

-- Note: May fail if duplicates exist. Clean duplicates first.
```

**Go Migration** (add to `storage/gorm/db.go`):

```go
// Add after AutoMigrate
db.Conn.Exec(`
    CREATE UNIQUE INDEX IF NOT EXISTS idx_offers_unique_per_message
    ON offers (raw_message_id, medication)
    WHERE raw_message_id IS NOT NULL
`)
```

---

### 🟢 Low Priority

#### 3. Fix Empty Group Name (1 found)

**Investigation Query**:

```sql
SELECT jid, name, description, monitored, added_at
FROM groups
WHERE name = '' OR name IS NULL;
```

**Fix**:

```sql
-- Update with placeholder or fetch from WhatsApp
UPDATE groups SET name = 'Unknown Group' WHERE name = '';
```

**Prevention**: Add NOT NULL with CHECK constraint:

```sql
ALTER TABLE groups ADD CONSTRAINT groups_name_not_empty
CHECK (name <> '');
```

---

#### 4. Enable Feedback System for Adaptive Learning

**Status**: `feedback_records` table is empty - feedback system not in use.

**Problem**: The matching engine can improve over time using operator feedback, but this feature is currently disabled.

**Configuration** (in `config.yaml`):

```yaml
adaptive_learning:
  enabled: true # Change from false to true
  schedule: "0 3 * * *" # Run at 3 AM daily

  algorithm:
    learning_rate: 0.1
    min_weight: 0.05
    max_weight: 0.70
    min_samples: 100 # Need at least 100 feedback records
    analysis_window_days: 30

  auto_apply:
    enabled: false # Keep false for manual review initially
    require_improvement: true
```

**How Feedback Works**:

1. Operator confirms/rejects a match via API or WhatsApp bot
2. System records feedback in `match_feedback` table
3. Feedback is processed into `feedback_records` with score breakdown
4. Adaptive learning analyzes patterns and adjusts scoring weights
5. New weights stored in `weight_history`

**Implementation Steps**:

- [ ] Enable `adaptive_learning.enabled: true` in config
- [ ] Train operators to use confirm/reject actions
- [ ] Wait for 100+ feedback records to accumulate
- [ ] Review weight recommendations before auto-applying
- [ ] Monitor match quality improvements

**API Endpoints for Feedback**:

```bash
# Confirm a match
POST /api/matches/{id}/confirm

# Reject a match
POST /api/matches/{id}/reject

# View feedback history
GET /api/matches/{id}/feedback
```

**WhatsApp Bot Commands**:

```
/confirm <match_id>   - Confirm a match
/reject <match_id>    - Reject a match
/pending              - List pending matches
```

---

### 📊 Metrics to Monitor

| Metric                | Current | Target | Status               |
| --------------------- | ------- | ------ | -------------------- |
| Data Quality Score    | 73.7%   | >95%   | ⚠️ Needs improvement |
| Duplicate Offers      | 106     | 0      | 🔴 Fix required      |
| Duplicate Requests    | 2       | 0      | 🟡 Minor             |
| Empty Group Names     | 1       | 0      | 🟢 Minor             |
| Invalid Status Values | 0       | 0      | ✅ Good              |
| Scores Out of Range   | 0       | 0      | ✅ Good              |

---

### 📅 Implementation Timeline

| Week   | Action                           | Owner    |
| ------ | -------------------------------- | -------- |
| Week 1 | Investigate duplicate root cause | Dev Team |
| Week 1 | Clean existing duplicates        | DBA      |
| Week 2 | Add UNIQUE constraints           | Dev Team |
| Week 2 | Add deduplication in parser      | Dev Team |
| Week 3 | Monitor and verify fix           | QA       |

---

## Future Enhancements: Business Logic Validation

> Additional business rules and edge cases to implement in `05_business_logic.py`

### 🆕 Proposed New Rules

#### BR-009: Offers should have source_phone (Critical)

```python
# Offers must have a valid source phone
result = conn.execute(text('''
    SELECT COUNT(*) FROM offers WHERE source_phone IS NULL OR source_phone = ''
''')).scalar()
```

#### BR-010: Match pairs must have different sources (Warning)

```python
# Prevent self-matching (same person offering and requesting)
result = conn.execute(text('''
    SELECT COUNT(*) FROM matches m
    JOIN offers o ON m.offer_id = o.id
    JOIN requests r ON m.request_id = r.id
    WHERE o.source_phone = r.source_phone
''')).scalar()
```

#### BR-011: Expired items should not have ACTIVE status (Warning)

```python
# Items older than 30 days should be EXPIRED
result = conn.execute(text('''
    SELECT COUNT(*) FROM offers
    WHERE status = 'ACTIVE'
    AND created_at < NOW() - INTERVAL '30 days'
''')).scalar()
```

#### BR-012: Review queue items should have content (Critical)

```python
result = conn.execute(text('''
    SELECT COUNT(*) FROM review_queue WHERE content IS NULL OR content = ''
''')).scalar()
```

#### BR-013: Medication names should be normalized (Warning)

```python
# Check for potential duplicates with different casing
result = conn.execute(text('''
    SELECT COUNT(*) FROM (
        SELECT LOWER(medication), COUNT(DISTINCT medication)
        FROM offers GROUP BY LOWER(medication) HAVING COUNT(DISTINCT medication) > 1
    ) sub
''')).scalar()
```

#### BR-014: High-scoring matches should be auto-confirmed (Info)

```python
# Matches with score > 0.95 that are still PENDING
result = conn.execute(text('''
    SELECT COUNT(*) FROM matches
    WHERE score > 0.95 AND status = 'PENDING'
    AND created_at < NOW() - INTERVAL '1 hour'
''')).scalar()
```

#### BR-015: Groups should have valid JID format (Critical)

```python
# JID should end with @g.us for groups
result = conn.execute(text('''
    SELECT COUNT(*) FROM groups WHERE jid NOT LIKE '%@g.us'
''')).scalar()
```

#### BR-016: Timestamps should be reasonable (Warning)

```python
# No future dates, no dates before 2024
result = conn.execute(text('''
    SELECT COUNT(*) FROM raw_messages
    WHERE timestamp > NOW() + INTERVAL '1 day'
    OR timestamp < '2024-01-01'
''')).scalar()
```

---

### 🔄 Edge Cases to Handle

| Edge Case                    | Current Handling           | Proposed Enhancement                        |
| ---------------------------- | -------------------------- | ------------------------------------------- |
| Arabic-only medication names | Stored in `medication_raw` | Add BR to verify `medication_raw` not empty |
| Zero quantity offers         | Allowed (default 0)        | Consider flagging as warning                |
| Very high quantities (>1000) | Allowed                    | Add BR to flag potential typos              |
| Currency mismatch            | Defaults to EGP            | Add BR to validate currency codes           |
| Duplicate phone formats      | +20xxx vs 20xxx vs 0xxx    | Add normalization check                     |
| Unicode in medication names  | Allowed                    | Add BR to detect mojibake                   |
| Empty review queue reasons   | Allowed                    | Consider requiring failure_reason           |

---

### 📊 Severity Levels

| Level    | Color | Meaning                  | Action                 |
| -------- | ----- | ------------------------ | ---------------------- |
| Critical | ❌    | Data integrity violation | Immediate fix required |
| Warning  | ⚠️    | Best practice violation  | Schedule fix           |
| Info     | 🔍    | Optimization opportunity | Review when convenient |

---

### 🔧 Implementation Checklist

- [ ] Add BR-009 through BR-016 to `05_business_logic.py`
- [ ] Create separate function for each rule category
- [ ] Add configurable thresholds for numeric checks
- [ ] Create historical trend tracking for violations
- [ ] Add Slack/email alerts for critical violations
- [ ] Create dashboard widget for business rule health
- [ ] Schedule daily/weekly rule validation runs

---

### 📈 Monitoring Dashboard Metrics

```python
# Add these metrics to a monitoring dashboard
DASHBOARD_METRICS = [
    'total_violations_critical',
    'total_violations_warning',
    'rules_passing_percentage',
    'new_violations_24h',
    'resolved_violations_24h',
    'top_3_violated_rules',
]
```

---

## Future Enhancements: Analysis Infrastructure

> Strategic improvements and feature activation roadmap identified during initial data analysis.

### 📊 Empty Tables Analysis

The following tables are intentionally empty in the current deployment phase:

| Table                | Purpose                        | Activation Trigger                    | Priority |
| -------------------- | ------------------------------ | ------------------------------------- | -------- |
| `audit_logs`         | System audit trail             | Enable API audit middleware           | Medium   |
| `match_feedback`     | Operator feedback on matches   | Operators use confirm/reject actions  | High     |
| `feedback_records`   | ML training data from feedback | Auto-derived from `match_feedback`    | High     |
| `weight_history`     | Scoring algorithm adjustments  | Enable adaptive learning scheduler    | Medium   |
| `demand_leaderboard` | Popular medication tracking    | Enable leaderboard cron job           | Low      |
| `match_queue`        | Async match processing         | Queue persists only during processing | N/A      |
| `bot_users`          | WhatsApp bot preferences       | Users interact with bot               | Low      |
| `failed_messages`    | AI parsing failures            | Failures occur during processing      | N/A      |

### 🔄 Feature Activation Roadmap

#### Phase 1: Core Feedback Loop (Week 1-2)

```
┌─────────────────────────────────────────────────────────────┐
│  1. Train operators on confirm/reject workflow              │
│  2. Enable match_feedback collection via API/Bot            │
│  3. Monitor feedback_records growth                         │
│  4. Target: 100+ feedback records before adaptive learning  │
└─────────────────────────────────────────────────────────────┘
```

#### Phase 2: Adaptive Learning (Week 3-4)

```yaml
# config.yaml changes
adaptive_learning:
  enabled: true
  schedule: "0 3 * * *"
  algorithm:
    min_samples: 100
    learning_rate: 0.1
  auto_apply:
    enabled: false # Manual review first
```

#### Phase 3: Full Automation (Week 5+)

- Enable auto-apply for weight adjustments
- Activate demand leaderboard scheduler
- Enable audit logging for compliance

---

### 🛠️ Analysis Infrastructure Improvements

#### New Scripts to Develop

| Script                       | Purpose                                       | Complexity |
| ---------------------------- | --------------------------------------------- | ---------- |
| `10_data_drift.py`           | Detect changes in data distribution over time | Medium     |
| `11_medication_analytics.py` | Top medications, trends, seasonal patterns    | Medium     |
| `12_operator_performance.py` | Track operator response times and accuracy    | High       |
| `13_ai_model_comparison.py`  | Compare AI parsing across different models    | High       |
| `14_geographic_analysis.py`  | Analyze by phone prefix/region                | Low        |
| `15_export_dashboard.py`     | Generate HTML dashboard with charts           | Medium     |

#### Automated Scheduling

```python
# Proposed cron schedule for analysis scripts
ANALYSIS_SCHEDULE = {
    'daily': [
        '01_schema_discovery.py',   # Quick health check
        '02_null_analysis.py',       # Data completeness
    ],
    'weekly': [
        '03_data_quality.py',        # Full quality assessment
        '04_referential_integrity.py',
        '05_business_logic.py',
    ],
    'monthly': [
        '06_time_series.py',         # Trend analysis
        '07_ai_parsing_quality.py',  # AI performance
        '08_matching_analysis.py',   # Matching engine
    ]
}
```

#### Alerting Integration

```python
# Proposed alert thresholds
ALERTS = {
    'critical': {
        'orphaned_records': 1,           # Any orphan is critical
        'data_quality_score': 70,        # Below 70% triggers alert
        'ai_success_rate': 90,           # Below 90% triggers alert
    },
    'warning': {
        'duplicate_rate': 5,             # >5% duplicates
        'null_rate_unexpected': 20,      # >20% unexpected nulls
        'pending_matches_age_hours': 24, # Pending >24h
    }
}
```

---

### 📈 Metrics Dashboard Proposal

```
┌─────────────────────────────────────────────────────────────────┐
│                    PHARMABROKER DATA HEALTH                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ Data Quality │  │ AI Success   │  │ Match Rate   │          │
│  │    73.7%     │  │    98.6%     │  │    90.5%     │          │
│  │   ⚠️ WARN    │  │     ✅ OK    │  │     ✅ OK    │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                 │
│  Today: 364 messages │ 2,375 offers │ 2,301 matches            │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Recent Issues                                           │   │
│  │ • 106 duplicate offers (investigate AI dedup)           │   │
│  │ • 98 processed messages without output (chat noise)     │   │
│  │ • 1 empty group name (data cleanup needed)              │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

### ✅ Implementation Checklist

**Infrastructure:**

- [ ] Add CI/CD pipeline for analysis scripts
- [ ] Create scheduled GitHub Actions for daily/weekly runs
- [ ] Set up Slack/email notifications for alerts
- [ ] Create Grafana/Metabase dashboard from reports

**Data Quality:**

- [ ] Resolve 106 duplicate offers issue
- [ ] Add UNIQUE constraints to prevent future duplicates
- [ ] Implement data retention policy for old records
- [ ] Add database backup validation

**Feature Activation:**

- [ ] Enable audit_logs middleware
- [ ] Train operators on feedback workflow
- [ ] Enable adaptive_learning after 100+ feedback records
- [ ] Activate demand_leaderboard scheduler

**Documentation:**

- [ ] Create operator training guide
- [ ] Document alert response procedures
- [ ] Maintain changelog for schema changes

---

## Changelog

| Date       | Change                                                  | Author |
| ---------- | ------------------------------------------------------- | ------ |
| 2025-12-17 | Initial roadmap created                                 | System |
| 2025-12-17 | Added detailed objectives, deliverables, best practices | System |
| 2025-12-17 | Added troubleshooting guide and glossary                | System |
| 2025-12-17 | Added Action Items from Phase 3 analysis                | System |
