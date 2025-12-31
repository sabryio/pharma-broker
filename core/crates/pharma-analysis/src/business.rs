//! Business Logic Validation
//! Port of: 05_business_logic.py

use async_trait::async_trait;
use colored::*;
use pharma_db::DatabaseConnection;
use prettytable::{Table, row};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

use crate::{AnalysisPhase, print_header};

/// Business rule definition
struct BusinessRule {
    id: &'static str,
    description: &'static str,
    severity: Severity,
    query: &'static str,
}

#[derive(Clone, Copy)]
enum Severity {
    Critical,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Critical => write!(f, "Critical"),
            Severity::Warning => write!(f, "Warning"),
            Severity::Info => write!(f, "Info"),
        }
    }
}

pub struct BusinessLogicAnalysis;

#[async_trait]
impl AnalysisPhase for BusinessLogicAnalysis {
    fn name(&self) -> &'static str {
        "Business Logic Validation"
    }

    async fn run(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_header("Business Logic Validation");

        let rules = get_business_rules();
        let mut table = Table::new();
        table.add_row(row![
            "Rule ID",
            "Description",
            "Severity",
            "Violations",
            "Status"
        ]);

        let mut critical_failed = 0;
        let mut total_passed = 0;

        for rule in &rules {
            let violations = db
                .query_one(Statement::from_string(
                    DbBackend::Postgres,
                    rule.query.to_string(),
                ))
                .await?
                .map(|r| r.try_get_by_index::<i64>(0).unwrap_or(0))
                .unwrap_or(0);

            let status = if violations == 0 {
                total_passed += 1;
                "✅".green().to_string()
            } else {
                match rule.severity {
                    Severity::Critical => {
                        critical_failed += 1;
                        "❌".red().to_string()
                    }
                    Severity::Warning => "⚠️".yellow().to_string(),
                    Severity::Info => "🔍".blue().to_string(),
                }
            };

            table.add_row(row![
                rule.id,
                rule.description,
                rule.severity.to_string(),
                violations,
                status
            ]);
        }

        table.printstd();

        // Summary
        println!(
            "\n📊 Summary: {}/{} rules passed",
            total_passed,
            rules.len()
        );
        if critical_failed > 0 {
            println!(
                "{}",
                format!(
                    "❌ {} CRITICAL rules failed - immediate attention required!",
                    critical_failed
                )
                .red()
                .bold()
            );
        } else {
            println!("{}", "✅ All critical rules passed!".green());
        }

        Ok(())
    }
}

fn get_business_rules() -> Vec<BusinessRule> {
    vec![
        BusinessRule {
            id: "BR-001",
            description: "Offers must have medication name",
            severity: Severity::Critical,
            query: "SELECT COUNT(*) FROM offers WHERE medication IS NULL OR medication = ''",
        },
        BusinessRule {
            id: "BR-002",
            description: "Requests must have medication name",
            severity: Severity::Critical,
            query: "SELECT COUNT(*) FROM requests WHERE medication IS NULL OR medication = ''",
        },
        BusinessRule {
            id: "BR-003",
            description: "Match scores must be 0-1",
            severity: Severity::Critical,
            query: "SELECT COUNT(*) FROM matches WHERE score < 0 OR score > 1",
        },
        BusinessRule {
            id: "BR-004",
            description: "Confirmed matches need confirmed_at",
            severity: Severity::Warning,
            query: "SELECT COUNT(*) FROM matches WHERE status = 'CONFIRMED' AND confirmed_at IS NULL",
        },
        BusinessRule {
            id: "BR-005",
            description: "Raw messages must have content",
            severity: Severity::Critical,
            query: "SELECT COUNT(*) FROM raw_messages WHERE content IS NULL OR content = ''",
        },
        BusinessRule {
            id: "BR-006",
            description: "Processed messages should create offers/requests",
            severity: Severity::Info,
            query: r#"
                SELECT COUNT(*) FROM raw_messages rm
                WHERE rm.processed_at IS NOT NULL AND rm.error IS NULL
                AND NOT EXISTS (SELECT 1 FROM offers o WHERE o.raw_message_id = rm.id)
                AND NOT EXISTS (SELECT 1 FROM requests r WHERE r.raw_message_id = rm.id)
            "#,
        },
        BusinessRule {
            id: "BR-007",
            description: "Quantity must be non-negative",
            severity: Severity::Critical,
            query: r#"
                SELECT (SELECT COUNT(*) FROM offers WHERE quantity < 0) +
                       (SELECT COUNT(*) FROM requests WHERE quantity < 0)
            "#,
        },
        BusinessRule {
            id: "BR-008",
            description: "Price must be non-negative",
            severity: Severity::Critical,
            query: "SELECT COUNT(*) FROM offers WHERE price < 0",
        },
        BusinessRule {
            id: "BR-009",
            description: "Groups must have JID",
            severity: Severity::Critical,
            query: "SELECT COUNT(*) FROM groups WHERE jid IS NULL OR jid = ''",
        },
        BusinessRule {
            id: "BR-010",
            description: "Feedback records must reference valid match",
            severity: Severity::Warning,
            query: r#"
                SELECT COUNT(*) FROM feedback_records f
                WHERE NOT EXISTS (SELECT 1 FROM matches m WHERE m.id = f.match_id)
            "#,
        },
    ]
}
