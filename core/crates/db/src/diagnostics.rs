//! Database Diagnostics - Performance analysis tools

use crate::{Error, Result};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};

/// Database diagnostic tools for performance analysis
pub struct DbDiagnostics;

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
}
