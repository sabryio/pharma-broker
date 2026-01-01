//! MedicationAlias entity - Maps parsed medication variations to master records
//!
//! Part of the Medication Curation System (Phase 1)

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Curation status for medication aliases
#[derive(
    Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum CurationStatus {
    #[sea_orm(string_value = "PENDING")]
    #[default]
    Pending,
    #[sea_orm(string_value = "APPROVED")]
    Approved,
    #[sea_orm(string_value = "REJECTED")]
    Rejected,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "medication_aliases")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    // The raw/parsed medication name
    pub alias_name: String,
    pub alias_name_normalized: String,

    // Link to master record (nullable until curated)
    pub master_medication_id: Option<Uuid>,

    // Curation metadata
    pub ai_suggestion_confidence: Option<f64>,
    pub curation_status: CurationStatus,
    pub curated_by: Option<String>,
    pub curated_at: Option<DateTimeUtc>,

    // Statistics
    pub occurrence_count: i32,
    pub first_seen_at: DateTimeUtc,
    pub last_seen_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::medication_master::Entity",
        from = "Column::MasterMedicationId",
        to = "super::medication_master::Column::Id"
    )]
    MasterMedication,
}

impl Related<super::medication_master::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MasterMedication.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Create a new medication alias
    pub fn new(alias_name: impl Into<String>) -> Self {
        let name = alias_name.into();
        let normalized = Self::normalize(&name);
        let now = chrono::Utc::now();

        Self {
            id: Uuid::new_v4(),
            alias_name: name,
            alias_name_normalized: normalized,
            master_medication_id: None,
            ai_suggestion_confidence: None,
            curation_status: CurationStatus::Pending,
            curated_by: None,
            curated_at: None,
            occurrence_count: 1,
            first_seen_at: now,
            last_seen_at: now,
        }
    }

    /// Normalize an alias name for comparison
    /// Converts to lowercase and normalizes Arabic-Indic numerals to Western
    pub fn normalize(name: &str) -> String {
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

    /// Check if this alias has been curated
    pub fn is_curated(&self) -> bool {
        self.curation_status != CurationStatus::Pending
    }

    /// Check if this alias is approved and linked to a master
    pub fn is_approved(&self) -> bool {
        self.curation_status == CurationStatus::Approved && self.master_medication_id.is_some()
    }

    /// Approve this alias and link to a master medication
    pub fn approve(&mut self, master_id: Uuid, curated_by: impl Into<String>) {
        self.master_medication_id = Some(master_id);
        self.curation_status = CurationStatus::Approved;
        self.curated_by = Some(curated_by.into());
        self.curated_at = Some(chrono::Utc::now());
    }

    /// Reject this alias
    pub fn reject(&mut self, curated_by: impl Into<String>) {
        self.curation_status = CurationStatus::Rejected;
        self.curated_by = Some(curated_by.into());
        self.curated_at = Some(chrono::Utc::now());
    }

    /// Increment occurrence count
    pub fn increment_count(&mut self) {
        self.occurrence_count += 1;
        self.last_seen_at = chrono::Utc::now();
    }
}
