//! # Identity Migration Tool
//!
//! One-time migration to populate participants and groups from legacy data.
//! This tool uses raw SQL to access fields that have been removed from the Rust models.

use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use std::env;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/pharmabroker".to_string());

    println!("\n🚀 Starting Identity Migration...");
    let db = sea_orm::Database::connect(&database_url).await?;

    // 1. Migrate Groups from raw_messages
    migrate_groups(&db).await?;

    // 2. Migrate Participants from raw_messages
    migrate_participants(&db).await?;

    // 3. Link Participants to Groups
    link_participants_to_groups(&db).await?;

    // 4. Update FKs in existing tables
    update_fks(&db).await?;

    println!("\n✅ Migration complete!");
    Ok(())
}

async fn migrate_groups(db: &DatabaseConnection) -> anyhow::Result<()> {
    println!("📦 Migrating Groups...");

    // Get unique groups from raw_messages (using raw SQL because group_jid/group_name are gone from model)
    let query = "SELECT DISTINCT group_jid, group_name FROM raw_messages WHERE group_jid IS NOT NULL AND group_jid != ''";
    let rows = db
        .query_all(Statement::from_string(DbBackend::Postgres, query))
        .await?;

    let mut count = 0;
    for row in rows {
        let jid: String = row.try_get("", "group_jid")?;
        let name: String = row.try_get("", "group_name")?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        // Insert into groups if not exists
        let insert = Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO groups (id, jid, name, monitored, added_at) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (jid) DO NOTHING",
            vec![id.into(), jid.into(), name.into(), true.into(), now.into()],
        );

        if db.execute(insert).await?.rows_affected() > 0 {
            count += 1;
        }
    }

    println!("   ✅ Inserted {} new groups", count);
    Ok(())
}

async fn migrate_participants(db: &DatabaseConnection) -> anyhow::Result<()> {
    println!("👤 Migrating Participants...");

    // Get unique participants from raw_messages
    let query = "SELECT DISTINCT sender_jid, sender_phone, sender_name FROM raw_messages WHERE sender_jid IS NOT NULL AND sender_jid != ''";
    let rows = db
        .query_all(Statement::from_string(DbBackend::Postgres, query))
        .await?;

    let mut count = 0;
    for row in rows {
        let jid: String = row.try_get("", "sender_jid")?;
        let phone: String = row.try_get("", "sender_phone")?;
        let name: Option<String> = row.try_get("", "sender_name").ok();

        let id = Uuid::new_v4();
        let now = Utc::now();

        // Insert into participants if not exists
        let insert = Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO participants (id, jid, phone, push_name, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (jid) DO NOTHING",
            vec![
                id.into(),
                jid.into(),
                phone.into(),
                name.into(),
                now.into(),
                now.into(),
            ],
        );

        if db.execute(insert).await?.rows_affected() > 0 {
            count += 1;
        }
    }

    println!("   ✅ Inserted {} new participants", count);
    Ok(())
}

async fn link_participants_to_groups(db: &DatabaseConnection) -> anyhow::Result<()> {
    println!("🔗 Linking Participants to Groups...");

    // Get unique pairs from raw_messages
    let query = "SELECT DISTINCT sender_jid, group_jid FROM raw_messages WHERE sender_jid IS NOT NULL AND group_jid IS NOT NULL";
    let rows = db
        .query_all(Statement::from_string(DbBackend::Postgres, query))
        .await?;

    let mut count = 0;
    for row in rows {
        let sender_jid: String = row.try_get("", "sender_jid")?;
        let group_jid: String = row.try_get("", "group_jid")?;

        let now = Utc::now();

        // Link them using IDs from the tables
        let insert = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"INSERT INTO participant_groups (participant_id, group_id, joined_at) 
               SELECT p.id, g.id, $3 FROM participants p, groups g 
               WHERE p.jid = $1 AND g.jid = $2 
               ON CONFLICT DO NOTHING"#,
            vec![sender_jid.into(), group_jid.into(), now.into()],
        );

        if db.execute(insert).await?.rows_affected() > 0 {
            count += 1;
        }
    }

    println!("   ✅ Created {} participant-group links", count);
    Ok(())
}

async fn update_fks(db: &DatabaseConnection) -> anyhow::Result<()> {
    println!("🛠️ Updating Foreign Keys in legacy tables...");

    // Update raw_messages
    let q1 = "UPDATE raw_messages rm SET participant_id = p.id FROM participants p WHERE rm.sender_jid = p.jid AND rm.participant_id IS NULL";
    let r1 = db
        .execute(Statement::from_string(DbBackend::Postgres, q1))
        .await?
        .rows_affected();

    let q2 = "UPDATE raw_messages rm SET group_id = g.id FROM groups g WHERE rm.group_jid = g.jid AND rm.group_id IS NULL";
    let r2 = db
        .execute(Statement::from_string(DbBackend::Postgres, q2))
        .await?
        .rows_affected();

    // Update offers
    let q3 = "UPDATE offers o SET participant_id = p.id FROM participants p WHERE o.source_phone = p.phone AND o.participant_id IS NULL";
    let r3 = db
        .execute(Statement::from_string(DbBackend::Postgres, q3))
        .await?
        .rows_affected();

    // Update requests
    let q4 = "UPDATE requests r SET participant_id = p.id FROM participants p WHERE r.source_phone = p.phone AND r.participant_id IS NULL";
    let r4 = db
        .execute(Statement::from_string(DbBackend::Postgres, q4))
        .await?
        .rows_affected();

    println!("   ✅ Updated {} raw_messages participants", r1);
    println!("   ✅ Updated {} raw_messages groups", r2);
    println!("   ✅ Updated {} offers", r3);
    println!("   ✅ Updated {} requests", r4);

    Ok(())
}
