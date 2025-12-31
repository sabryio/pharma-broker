# PharmaBroker Data Analysis

Data quality analysis tools for PharmaBroker database.

## Tools Available

### 1. Rust Analysis Tool (Recommended)

Located in `core/crates/pharma-analysis/`, this is the primary analysis tool.

```bash
# From project root
cd core

# Run all analysis phases
cargo run --package pharma-analysis -- all

# Run specific phases
cargo run --package pharma-analysis -- health      # Database health check
cargo run --package pharma-analysis -- quality     # Data quality analysis
cargo run --package pharma-analysis -- integrity   # Referential integrity
cargo run --package pharma-analysis -- business    # Business logic validation
cargo run --package pharma-analysis -- timeseries  # Time series analysis
cargo run --package pharma-analysis -- ai-quality  # AI parsing quality
cargo run --package pharma-analysis -- matching    # Matching engine analysis
cargo run --package pharma-analysis -- stale       # Stale matches analysis

# Expire stale matches (dry-run)
cargo run --package pharma-analysis -- expire --days 14

# Expire stale matches (execute)
cargo run --package pharma-analysis -- expire --days 14 --execute
```

Or use Taskfile:

```bash
task core:analysis:all
task core:analysis:health
task core:analysis:quality
task core:analysis:matching
```

### 2. Python Scripts (Legacy)

Located in `analysis/scripts/`, these are the original Python analysis scripts.

```bash
cd analysis
uv run python scripts/07_ai_parsing_quality.py
```

## Analysis Phases

| Phase       | Rust Command | Python Script                               | Description                    |
| ----------- | ------------ | ------------------------------------------- | ------------------------------ |
| Health      | `health`     | `01_schema_discovery.py`                    | Database connectivity & schema |
| Quality     | `quality`    | `02_null_analysis.py`, `03_data_quality.py` | Nulls, duplicates, statuses    |
| Integrity   | `integrity`  | `04_referential_integrity.py`               | Foreign key validation         |
| Business    | `business`   | `05_business_logic.py`                      | Business rule validation       |
| Time Series | `timeseries` | `06_time_series.py`                         | Activity patterns over time    |
| AI Quality  | `ai-quality` | `07_ai_parsing_quality.py`                  | AI extraction completeness     |
| Matching    | `matching`   | `08_matching_analysis.py`                   | Match scores & feedback        |
| Stale       | `stale`      | `14_stale_matches.py`                       | Pending match age analysis     |

## Configuration

Set the database URL via environment variable:

```bash
# Windows
set DATABASE_URL=postgresql://postgres:password@localhost:5432/pharmabroker

# Linux/Mac
export DATABASE_URL=postgresql://postgres:password@localhost:5432/pharmabroker
```

Or use `PB_DATABASE_DSN` (both are supported).

## Reports

Analysis reports are saved to `analysis/reports/` directory.
