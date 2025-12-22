//! Database Diagnostics - Performance analysis tools

use crate::{Error, Result};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Database diagnostic tools for performance analysis
pub struct DbDiagnostics;

/// Query plan analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlanAnalysis {
    /// Raw EXPLAIN ANALYZE output
    pub plan: String,
    /// Whether the query uses an index
    pub uses_index: bool,
    /// Whether the query uses a sequential scan
    pub uses_seq_scan: bool,
    /// Estimated total cost
    pub total_cost: Option<f64>,
    /// Actual execution time in milliseconds
    pub execution_time_ms: Option<f64>,
    /// Number of rows returned
    pub rows_returned: Option<i64>,
    /// Warnings (e.g., seq scan on large table)
    pub warnings: Vec<String>,
}

/// Table statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStats {
    pub table_name: String,
    pub row_count: i64,
    pub dead_tuples: i64,
    pub last_vacuum: Option<String>,
    pub last_analyze: Option<String>,
    pub table_size: String,
    pub index_size: String,
    pub total_size: String,
}

/// Index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub index_name: String,
    pub table_name: String,
    pub index_size: String,
    pub index_scans: i64,
    pub tuples_read: i64,
    pub tuples_fetched: i64,
}

/// Database health summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealth {
    pub connection_count: i64,
    pub max_connections: i64,
    pub database_size: String,
    pub cache_hit_ratio: f64,
    pub tables: Vec<TableStats>,
    pub indexes: Vec<IndexStats>,
    pub slow_queries: Vec<QueryPlanAnalysis>,
}

impl DbDiagnostics {
    /// Run EXPLAIN ANALYZE on a raw SQL query and return the plan as a string.
    ///
    /// # Safety
    /// This runs the query. Do not use for non-idempotent queries on production data
    /// unless you specifically want to measure the execution time.
    pub async fn explain_analyze(db: &DatabaseConnection, sql: &str) -> Result<String> {
        let backend = db.get_database_backend();
        let explain_sql = format!("EXPLAIN ANALYZE {}", sql);
        let statement = Statement::from_string(backend, explain_sql);

        let result = db.query_all(statement).await.map_err(Error::from)?;
        let mut output = String::new();

        for row in result {
            // PostgreSQL returns EXPLAIN lines in the first column of each row
            if let Ok(line) = row.try_get_by_index::<String>(0) {
                output.push_str(&line);
                output.push('\n');
            }
        }

        Ok(output)
    }

    /// Check if a query is using an index scan (vs seq scan)
    pub async fn is_using_index(db: &DatabaseConnection, sql: &str) -> Result<bool> {
        let plan = Self::explain_analyze(db, sql).await?;
        Ok(plan.contains("Index Scan")
            || plan.contains("Index Only Scan")
            || plan.contains("Bitmap Index Scan"))
    }

    /// Analyze a query and return detailed plan analysis
    pub async fn analyze_query(db: &DatabaseConnection, sql: &str) -> Result<QueryPlanAnalysis> {
        let plan = Self::explain_analyze(db, sql).await?;

        let uses_index = plan.contains("Index Scan")
            || plan.contains("Index Only Scan")
            || plan.contains("Bitmap Index Scan");
        let uses_seq_scan = plan.contains("Seq Scan");

        // Parse execution time from plan
        let execution_time_ms = Self::parse_execution_time(&plan);
        let total_cost = Self::parse_total_cost(&plan);
        let rows_returned = Self::parse_rows(&plan);

        // Generate warnings
        let mut warnings = Vec::new();
        if uses_seq_scan && !uses_index {
            warnings.push("Query uses sequential scan - consider adding an index".to_string());
        }
        if let Some(time) = execution_time_ms
            && time > 1000.0
        {
            warnings.push(format!("Slow query: {:.2}ms execution time", time));
        }

        Ok(QueryPlanAnalysis {
            plan,
            uses_index,
            uses_seq_scan,
            total_cost,
            execution_time_ms,
            rows_returned,
            warnings,
        })
    }

    /// Parse execution time from EXPLAIN ANALYZE output
    fn parse_execution_time(plan: &str) -> Option<f64> {
        // Look for "Execution Time: X.XXX ms"
        for line in plan.lines() {
            if line.contains("Execution Time:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for (i, part) in parts.iter().enumerate() {
                    if *part == "Time:"
                        && i + 1 < parts.len()
                        && let Ok(time) = parts[i + 1].parse::<f64>()
                    {
                        return Some(time);
                    }
                }
            }
        }
        None
    }

    /// Parse total cost from EXPLAIN output
    fn parse_total_cost(plan: &str) -> Option<f64> {
        // Look for "cost=X.XX..Y.YY" and extract Y.YY
        for line in plan.lines() {
            if let Some(cost_start) = line.find("cost=") {
                let cost_str = &line[cost_start + 5..];
                if let Some(dots) = cost_str.find("..") {
                    let end_cost = &cost_str[dots + 2..];
                    if let Some(space) = end_cost.find(' ')
                        && let Ok(cost) = end_cost[..space].parse::<f64>()
                    {
                        return Some(cost);
                    }
                }
            }
        }
        None
    }

    /// Parse rows from EXPLAIN output
    fn parse_rows(plan: &str) -> Option<i64> {
        // Look for "rows=X"
        for line in plan.lines() {
            if let Some(rows_start) = line.find("rows=") {
                let rows_str = &line[rows_start + 5..];
                if let Some(space) = rows_str.find(|c: char| !c.is_ascii_digit())
                    && let Ok(rows) = rows_str[..space].parse::<i64>()
                {
                    return Some(rows);
                }
            }
        }
        None
    }

    /// Get table statistics
    pub async fn get_table_stats(db: &DatabaseConnection) -> Result<Vec<TableStats>> {
        let sql = r#"
            SELECT 
                schemaname || '.' || relname as table_name,
                n_live_tup as row_count,
                n_dead_tup as dead_tuples,
                last_vacuum::text,
                last_analyze::text,
                pg_size_pretty(pg_table_size(relid)) as table_size,
                pg_size_pretty(pg_indexes_size(relid)) as index_size,
                pg_size_pretty(pg_total_relation_size(relid)) as total_size
            FROM pg_stat_user_tables
            ORDER BY pg_total_relation_size(relid) DESC
        "#;

        let statement = Statement::from_string(DbBackend::Postgres, sql.to_string());
        let rows = db.query_all(statement).await.map_err(Error::from)?;

        let mut stats = Vec::new();
        for row in rows {
            stats.push(TableStats {
                table_name: row.try_get_by_index::<String>(0).unwrap_or_default(),
                row_count: row.try_get_by_index::<i64>(1).unwrap_or(0),
                dead_tuples: row.try_get_by_index::<i64>(2).unwrap_or(0),
                last_vacuum: row.try_get_by_index::<Option<String>>(3).ok().flatten(),
                last_analyze: row.try_get_by_index::<Option<String>>(4).ok().flatten(),
                table_size: row.try_get_by_index::<String>(5).unwrap_or_default(),
                index_size: row.try_get_by_index::<String>(6).unwrap_or_default(),
                total_size: row.try_get_by_index::<String>(7).unwrap_or_default(),
            });
        }

        Ok(stats)
    }

    /// Get index statistics
    pub async fn get_index_stats(db: &DatabaseConnection) -> Result<Vec<IndexStats>> {
        let sql = r#"
            SELECT 
                indexrelname as index_name,
                relname as table_name,
                pg_size_pretty(pg_relation_size(indexrelid)) as index_size,
                idx_scan as index_scans,
                idx_tup_read as tuples_read,
                idx_tup_fetch as tuples_fetched
            FROM pg_stat_user_indexes
            ORDER BY idx_scan DESC
        "#;

        let statement = Statement::from_string(DbBackend::Postgres, sql.to_string());
        let rows = db.query_all(statement).await.map_err(Error::from)?;

        let mut stats = Vec::new();
        for row in rows {
            stats.push(IndexStats {
                index_name: row.try_get_by_index::<String>(0).unwrap_or_default(),
                table_name: row.try_get_by_index::<String>(1).unwrap_or_default(),
                index_size: row.try_get_by_index::<String>(2).unwrap_or_default(),
                index_scans: row.try_get_by_index::<i64>(3).unwrap_or(0),
                tuples_read: row.try_get_by_index::<i64>(4).unwrap_or(0),
                tuples_fetched: row.try_get_by_index::<i64>(5).unwrap_or(0),
            });
        }

        Ok(stats)
    }

    /// Get database connection info
    pub async fn get_connection_info(db: &DatabaseConnection) -> Result<(i64, i64)> {
        let sql = r#"
            SELECT 
                (SELECT count(*) FROM pg_stat_activity) as current_connections,
                (SELECT setting::int FROM pg_settings WHERE name = 'max_connections') as max_connections
        "#;

        let statement = Statement::from_string(DbBackend::Postgres, sql.to_string());
        let row = db.query_one(statement).await.map_err(Error::from)?;

        if let Some(row) = row {
            let current = row.try_get_by_index::<i64>(0).unwrap_or(0);
            let max = row.try_get_by_index::<i64>(1).unwrap_or(100);
            Ok((current, max))
        } else {
            Ok((0, 100))
        }
    }

    /// Get database size
    pub async fn get_database_size(db: &DatabaseConnection) -> Result<String> {
        let sql = "SELECT pg_size_pretty(pg_database_size(current_database()))";
        let statement = Statement::from_string(DbBackend::Postgres, sql.to_string());
        let row = db.query_one(statement).await.map_err(Error::from)?;

        if let Some(row) = row {
            Ok(row.try_get_by_index::<String>(0).unwrap_or_default())
        } else {
            Ok("unknown".to_string())
        }
    }

    /// Get cache hit ratio
    pub async fn get_cache_hit_ratio(db: &DatabaseConnection) -> Result<f64> {
        let sql = r#"
            SELECT 
                CASE 
                    WHEN (blks_hit + blks_read) = 0 THEN 0
                    ELSE round(blks_hit::numeric / (blks_hit + blks_read) * 100, 2)
                END as cache_hit_ratio
            FROM pg_stat_database 
            WHERE datname = current_database()
        "#;

        let statement = Statement::from_string(DbBackend::Postgres, sql.to_string());
        let row = db.query_one(statement).await.map_err(Error::from)?;

        if let Some(row) = row {
            Ok(row.try_get_by_index::<f64>(0).unwrap_or(0.0))
        } else {
            Ok(0.0)
        }
    }

    /// Get unused indexes (indexes with 0 scans)
    pub async fn get_unused_indexes(db: &DatabaseConnection) -> Result<Vec<IndexStats>> {
        let sql = r#"
            SELECT 
                indexrelname as index_name,
                relname as table_name,
                pg_size_pretty(pg_relation_size(indexrelid)) as index_size,
                idx_scan as index_scans,
                idx_tup_read as tuples_read,
                idx_tup_fetch as tuples_fetched
            FROM pg_stat_user_indexes
            WHERE idx_scan = 0
            AND indexrelname NOT LIKE '%_pkey'
            ORDER BY pg_relation_size(indexrelid) DESC
        "#;

        let statement = Statement::from_string(DbBackend::Postgres, sql.to_string());
        let rows = db.query_all(statement).await.map_err(Error::from)?;

        let mut stats = Vec::new();
        for row in rows {
            stats.push(IndexStats {
                index_name: row.try_get_by_index::<String>(0).unwrap_or_default(),
                table_name: row.try_get_by_index::<String>(1).unwrap_or_default(),
                index_size: row.try_get_by_index::<String>(2).unwrap_or_default(),
                index_scans: row.try_get_by_index::<i64>(3).unwrap_or(0),
                tuples_read: row.try_get_by_index::<i64>(4).unwrap_or(0),
                tuples_fetched: row.try_get_by_index::<i64>(5).unwrap_or(0),
            });
        }

        Ok(stats)
    }

    /// Get tables that need vacuuming (high dead tuple ratio)
    pub async fn get_tables_needing_vacuum(
        db: &DatabaseConnection,
        dead_tuple_threshold: f64,
    ) -> Result<Vec<TableStats>> {
        let stats = Self::get_table_stats(db).await?;
        Ok(stats
            .into_iter()
            .filter(|t| {
                if t.row_count == 0 {
                    return false;
                }
                let ratio = t.dead_tuples as f64 / t.row_count as f64;
                ratio > dead_tuple_threshold
            })
            .collect())
    }

    /// Analyze critical queries and check for performance issues
    pub async fn analyze_critical_queries(
        db: &DatabaseConnection,
    ) -> Result<HashMap<String, QueryPlanAnalysis>> {
        let critical_queries = vec![
            (
                "active_offers",
                "SELECT * FROM offers WHERE status = 'ACTIVE' LIMIT 100",
            ),
            (
                "active_requests",
                "SELECT * FROM requests WHERE status = 'ACTIVE' LIMIT 100",
            ),
            (
                "pending_matches",
                "SELECT * FROM matches WHERE status = 'PENDING' LIMIT 100",
            ),
            (
                "recent_audit_logs",
                "SELECT * FROM audit_logs ORDER BY created_at DESC LIMIT 100",
            ),
            (
                "feedback_stats",
                "SELECT COUNT(*), AVG(total_score) FROM feedback_records WHERE created_at > NOW() - INTERVAL '30 days'",
            ),
        ];

        let mut results = HashMap::new();
        for (name, sql) in critical_queries {
            match Self::analyze_query(db, sql).await {
                Ok(analysis) => {
                    results.insert(name.to_string(), analysis);
                }
                Err(e) => {
                    tracing::warn!(query = name, error = %e, "Failed to analyze query");
                }
            }
        }

        Ok(results)
    }

    /// Get full database health summary
    pub async fn get_health_summary(db: &DatabaseConnection) -> Result<DatabaseHealth> {
        let (connection_count, max_connections) = Self::get_connection_info(db).await?;
        let database_size = Self::get_database_size(db).await?;
        let cache_hit_ratio = Self::get_cache_hit_ratio(db).await?;
        let tables = Self::get_table_stats(db).await?;
        let indexes = Self::get_index_stats(db).await?;

        // Analyze critical queries for slow query detection
        let query_analyses = Self::analyze_critical_queries(db).await?;
        let slow_queries: Vec<QueryPlanAnalysis> = query_analyses
            .into_values()
            .filter(|a| a.warnings.iter().any(|w| w.contains("Slow query")))
            .collect();

        Ok(DatabaseHealth {
            connection_count,
            max_connections,
            database_size,
            cache_hit_ratio,
            tables,
            indexes,
            slow_queries,
        })
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use crate::testing::TestDb;

    #[tokio::test]
    async fn test_explain_analyze() {
        let db = TestDb::new().await;
        let plan = DbDiagnostics::explain_analyze(&db.db, "SELECT 1")
            .await
            .unwrap();
        assert!(plan.contains("Execution Time") || plan.contains("Result"));
    }

    #[tokio::test]
    async fn test_is_using_index_false_for_constant() {
        let db = TestDb::new().await;
        let is_using = DbDiagnostics::is_using_index(&db.db, "SELECT 1")
            .await
            .unwrap();
        assert!(!is_using);
    }

    #[tokio::test]
    async fn test_analyze_query() {
        let db = TestDb::new().await;
        let analysis = DbDiagnostics::analyze_query(&db.db, "SELECT 1")
            .await
            .unwrap();
        assert!(!analysis.uses_index);
        assert!(analysis.execution_time_ms.is_some());
    }

    #[tokio::test]
    async fn test_get_table_stats() {
        let db = TestDb::new().await;
        let stats = DbDiagnostics::get_table_stats(&db.db).await.unwrap();
        // Should have some tables from migrations
        assert!(!stats.is_empty());
    }

    #[tokio::test]
    async fn test_get_database_size() {
        let db = TestDb::new().await;
        let size = DbDiagnostics::get_database_size(&db.db).await.unwrap();
        assert!(!size.is_empty());
    }

    #[tokio::test]
    async fn test_get_cache_hit_ratio() {
        let db = TestDb::new().await;
        let ratio = DbDiagnostics::get_cache_hit_ratio(&db.db).await.unwrap();
        assert!(ratio >= 0.0 && ratio <= 100.0);
    }
}
