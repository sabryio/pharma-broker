//! Medication Database Seeder
//!
//! Seeds the master_medications and medication_aliases tables with initial data
//! from the JSON seed file.
//!
//! Usage:
//!   cargo run --bin seed-medications
//!   cargo run --bin seed-medications -- --dry-run
//!   cargo run --bin seed-medications -- --clear

use std::fs;
use std::path::Path;

use chrono::Utc;
use clap::Parser;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use uuid::Uuid;

use pharma_db::entity::medication_alias::{
    ActiveModel as AliasActiveModel, CurationStatus, Entity as AliasEntity,
};
use pharma_db::entity::medication_master::{
    ActiveModel as MasterActiveModel, Entity as MasterEntity, MedicationStatus,
};

#[derive(Parser, Debug)]
#[command(name = "seed-medications")]
#[command(about = "Seed the medication database with initial data")]
struct Args {
    /// Path to the seed JSON file
    #[arg(short, long, default_value = "data/master_medications_seed.json")]
    file: String,

    /// Dry run - don't actually insert data
    #[arg(long)]
    dry_run: bool,

    /// Clear existing data before seeding
    #[arg(long)]
    clear: bool,

    /// Skip existing medications (don't update)
    #[arg(long, default_value = "true")]
    skip_existing: bool,
}

#[derive(Debug, Deserialize)]
struct SeedFile {
    version: String,
    description: String,
    medications: Vec<SeedMedication>,
}

#[derive(Debug, Deserialize)]
struct SeedMedication {
    canonical_name: String,
    canonical_name_ar: Option<String>,
    active_ingredient: Option<String>,
    strength: Option<String>,
    dosage_form: Option<String>,
    manufacturer: Option<String>,
    therapeutic_class: Option<String>,
    atc_code: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Load seed file
    let seed_path = Path::new(&args.file);
    if !seed_path.exists() {
        eprintln!("Error: Seed file not found: {}", args.file);
        std::process::exit(1);
    }

    let seed_content = fs::read_to_string(seed_path)?;
    let seed_data: SeedFile = serde_json::from_str(&seed_content)?;

    println!("📦 Medication Database Seeder");
    println!("   Version: {}", seed_data.version);
    println!("   Description: {}", seed_data.description);
    println!("   Medications: {}", seed_data.medications.len());
    println!();

    if args.dry_run {
        println!("🔍 DRY RUN MODE - No data will be inserted");
        println!();

        for med in &seed_data.medications {
            println!(
                "   {} {} ({}) - {} aliases",
                med.canonical_name,
                med.strength.as_deref().unwrap_or(""),
                med.canonical_name_ar.as_deref().unwrap_or("-"),
                med.aliases.len()
            );
        }

        println!();
        println!(
            "✅ Dry run complete. {} medications would be seeded.",
            seed_data.medications.len()
        );
        return Ok(());
    }

    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/pharma".to_string());

    println!("🔌 Connecting to database...");
    let db = sea_orm::Database::connect(&database_url).await?;
    println!("   Connected!");
    println!();

    if args.clear {
        println!("🗑️  Clearing existing data...");
        clear_data(&db).await?;
        println!("   Cleared!");
        println!();
    }

    // Seed medications
    println!("🌱 Seeding medications...");
    let mut created_count = 0;
    let mut skipped_count = 0;
    let mut alias_count = 0;

    for med in seed_data.medications {
        let result = seed_medication(&db, &med, args.skip_existing).await?;
        match result {
            SeedResult::Created(aliases) => {
                created_count += 1;
                alias_count += aliases;
                println!(
                    "   ✅ {} {} - {} aliases",
                    med.canonical_name,
                    med.strength.as_deref().unwrap_or(""),
                    aliases
                );
            }
            SeedResult::Skipped => {
                skipped_count += 1;
                println!(
                    "   ⏭️  {} {} (already exists)",
                    med.canonical_name,
                    med.strength.as_deref().unwrap_or("")
                );
            }
        }
    }

    println!();
    println!("✅ Seeding complete!");
    println!("   Created: {} medications", created_count);
    println!("   Skipped: {} medications", skipped_count);
    println!("   Aliases: {} total", alias_count);

    Ok(())
}

enum SeedResult {
    Created(usize),
    Skipped,
}

async fn seed_medication(
    db: &DatabaseConnection,
    med: &SeedMedication,
    skip_existing: bool,
) -> Result<SeedResult, Box<dyn std::error::Error>> {
    // Check if medication already exists (by name + strength)
    let mut query = MasterEntity::find().filter(
        pharma_db::entity::medication_master::Column::CanonicalName.eq(&med.canonical_name),
    );

    // Add strength filter if present
    if let Some(ref strength) = med.strength {
        query = query.filter(pharma_db::entity::medication_master::Column::Strength.eq(strength));
    } else {
        query = query.filter(pharma_db::entity::medication_master::Column::Strength.is_null());
    }

    let existing = query.one(db).await?;

    if existing.is_some() && skip_existing {
        return Ok(SeedResult::Skipped);
    }

    let now = Utc::now();
    let master_id = Uuid::new_v4();

    // Create master medication
    let master = MasterActiveModel {
        id: Set(master_id),
        canonical_name: Set(med.canonical_name.clone()),
        canonical_name_ar: Set(med.canonical_name_ar.clone()),
        active_ingredient: Set(med.active_ingredient.clone()),
        strength: Set(med.strength.clone()),
        dosage_form: Set(med.dosage_form.clone()),
        manufacturer: Set(med.manufacturer.clone()),
        therapeutic_class: Set(med.therapeutic_class.clone()),
        atc_code: Set(med.atc_code.clone()),
        status: Set(MedicationStatus::Active),
        embedding: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        created_by: Set(Some("seed_medications".to_string())),
        eda_registration: Set(None),
    };

    master.insert(db).await?;

    // Create aliases
    let mut alias_count = 0;
    for alias_name in &med.aliases {
        // Check if alias already exists
        let existing_alias = AliasEntity::find()
            .filter(pharma_db::entity::medication_alias::Column::AliasName.eq(alias_name))
            .one(db)
            .await?;

        if existing_alias.is_some() {
            continue;
        }

        let normalized = normalize_alias(alias_name);
        let alias = AliasActiveModel {
            id: Set(Uuid::new_v4()),
            alias_name: Set(alias_name.clone()),
            alias_name_normalized: Set(normalized),
            master_medication_id: Set(Some(master_id)),
            ai_suggestion_confidence: Set(Some(1.0)),
            curation_status: Set(CurationStatus::Approved),
            curated_by: Set(Some("seed_medications".to_string())),
            curated_at: Set(Some(now)),
            occurrence_count: Set(0),
            first_seen_at: Set(now),
            last_seen_at: Set(now),
        };

        alias.insert(db).await?;
        alias_count += 1;
    }

    // Also create alias for the canonical name itself
    let canonical_normalized = normalize_alias(&med.canonical_name);
    let existing_canonical = AliasEntity::find()
        .filter(
            pharma_db::entity::medication_alias::Column::AliasNameNormalized
                .eq(&canonical_normalized),
        )
        .one(db)
        .await?;

    if existing_canonical.is_none() {
        let canonical_alias = AliasActiveModel {
            id: Set(Uuid::new_v4()),
            alias_name: Set(med.canonical_name.clone()),
            alias_name_normalized: Set(canonical_normalized),
            master_medication_id: Set(Some(master_id)),
            ai_suggestion_confidence: Set(Some(1.0)),
            curation_status: Set(CurationStatus::Approved),
            curated_by: Set(Some("seed_medications".to_string())),
            curated_at: Set(Some(now)),
            occurrence_count: Set(0),
            first_seen_at: Set(now),
            last_seen_at: Set(now),
        };

        canonical_alias.insert(db).await?;
        alias_count += 1;
    }

    // Create alias for Arabic name if present
    if let Some(ref ar_name) = med.canonical_name_ar {
        let ar_normalized = normalize_alias(ar_name);
        let existing_ar = AliasEntity::find()
            .filter(
                pharma_db::entity::medication_alias::Column::AliasNameNormalized.eq(&ar_normalized),
            )
            .one(db)
            .await?;

        if existing_ar.is_none() {
            let ar_alias = AliasActiveModel {
                id: Set(Uuid::new_v4()),
                alias_name: Set(ar_name.clone()),
                alias_name_normalized: Set(ar_normalized),
                master_medication_id: Set(Some(master_id)),
                ai_suggestion_confidence: Set(Some(1.0)),
                curation_status: Set(CurationStatus::Approved),
                curated_by: Set(Some("seed_medications".to_string())),
                curated_at: Set(Some(now)),
                occurrence_count: Set(0),
                first_seen_at: Set(now),
                last_seen_at: Set(now),
            };

            ar_alias.insert(db).await?;
            alias_count += 1;
        }
    }

    Ok(SeedResult::Created(alias_count))
}

async fn clear_data(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    // Delete aliases first (foreign key constraint)
    AliasEntity::delete_many().exec(db).await?;
    // Then delete masters
    MasterEntity::delete_many().exec(db).await?;
    Ok(())
}

fn normalize_alias(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            '٠' => '0',
            '١' => '1',
            '٢' => '2',
            '٣' => '3',
            '٤' => '4',
            '٥' => '5',
            '٦' => '6',
            '٧' => '7',
            '٨' => '8',
            '٩' => '9',
            _ => c,
        })
        .collect()
}
