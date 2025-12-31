//! Database Health & Schema Discovery
//! Port of: 01_schema_discovery.py

use async_trait::async_trait;
use colored::*;
use pharma_db::DatabaseConnection;
use prettytable::{Table, row};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

use crate::{AnalysisPhase, fmt_num, print_header, print_subheader};

pub struct HealthAnalysis;

#[async_trait]
impl AnalysisPhase for HealthAnalysis {
    fn name(&self) -> &'static str {
        "Database Health & Schema Discovery"
    }

    async fn run(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_header("Phase 1: Database Health Check");

        self.check_connection(db).await?;
        self.get_table_overview(db).await?;
        self.get_database_size(db).await?;
        self.get_largest_tables(db).await?;

        Ok(())
    }
}

impl HealthAnalysis {
    async fn check_connection(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Connection Test");

        let result = db
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT 1".to_string(),
            ))
            .await?;

        if result.is_some() {
            println!("{}", "✅ Database connection successful".green());
        } else {
            println!("{}", "❌ Database connection failed".red());
        }

        Ok(())
    }

    async fn get_table_overview(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Table Overview");

        let sql = r#"
            SELECT 
                t.table_name,
                (SELECT COUNT(*) FROM information_schema.columns c 
                 WHERE c.table_name = t.table_name AND c.table_schema = 'public') as col_count
            FROM information_schema.tables t
            WHERE t.table_schema = 'public' AND t.table_type = 'BASE TABLE'
            ORDER BY t.table_name
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        let mut table = Table::new();
        table.add_row(row!["Table", "Columns", "Rows", "Status"]);

        let mut total_rows: i64 = 0;

        for row in rows {
            let table_name: String = row.try_get_by_index(0)?;
            let col_count: i64 = row.try_get_by_index(1)?;

            // Get row count
            let count_sql = format!("SELECT COUNT(*) FROM \"{}\"", table_name);
            let count_row = db
                .query_one(Statement::from_string(DbBackend::Postgres, count_sql))
                .await?
                .unwrap();
            let row_count: i64 = count_row.try_get_by_index(0)?;
            total_rows += row_count;

            let status = if row_count > 0 {
                "✅".green()
            } else {
                "⚠️".yellow()
            };

            table.add_row(row![table_name, col_count, fmt_num(row_count), status]);
        }

        table.printstd();
        println!("\n📊 Total rows across all tables: {}", fmt_num(total_rows));

        Ok(())
    }

    async fn get_database_size(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Database Size");

        let sql = "SELECT pg_size_pretty(pg_database_size(current_database())) as size";
        let row = db
            .query_one(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?
            .unwrap();
        let size: String = row.try_get_by_index(0)?;

        println!("💾 Database Size: {}", size.cyan());

        Ok(())
    }

    async fn get_largest_tables(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Largest Tables");

        let sql = r#"
            SELECT relname as table_name,
                   pg_size_pretty(pg_total_relation_size(relid)) as size
            FROM pg_catalog.pg_statio_user_tables
            ORDER BY pg_total_relation_size(relid) DESC
            LIMIT 5
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        let mut table = Table::new();
        table.add_row(row!["Table", "Size"]);

        for row in rows {
            let name: String = row.try_get_by_index(0)?;
            let size: String = row.try_get_by_index(1)?;
            table.add_row(row![name, size]);
        }

        table.printstd();

        Ok(())
    }
}
