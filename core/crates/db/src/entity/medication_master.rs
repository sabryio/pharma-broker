//! MedicationMaster entity - Authoritative master medication records
//!
//! Part of the Medication Curation System (Phase 1)

use pgvector::Vector as PgVector;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Master medication status
#[derive(
    Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum MedicationStatus {
    #[sea_orm(string_value = "ACTIVE")]
    #[default]
    Active,
    #[sea_orm(string_value = "DISCONTINUED")]
    Discontinued,
    #[sea_orm(string_value = "RECALLED")]
    Recalled,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "medication_master")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    // Core identification
    pub canonical_name: String,
    pub canonical_name_ar: Option<String>,

    // Pharmaceutical details
    pub active_ingredient: Option<String>,
    pub strength: Option<String>,
    pub dosage_form: Option<String>,
    pub manufacturer: Option<String>,

    // Regulatory
    pub eda_registration: Option<String>,

    // Classification
    pub therapeutic_class: Option<String>,
    pub atc_code: Option<String>,

    // Status
    pub status: MedicationStatus,

    // AI Semantic Support
    pub embedding: Option<PgVector>,

    // Metadata
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    pub created_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::medication_alias::Entity")]
    Aliases,
}

impl Related<super::medication_alias::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Aliases.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Create a new medication master record
    pub fn new(canonical_name: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            canonical_name: canonical_name.into(),
            canonical_name_ar: None,
            active_ingredient: None,
            strength: None,
            dosage_form: None,
            manufacturer: None,
            eda_registration: None,
            therapeutic_class: None,
            atc_code: None,
            status: MedicationStatus::Active,
            embedding: None,
            created_at: now,
            updated_at: now,
            created_by: None,
        }
    }

    /// Create with Arabic name
    pub fn with_arabic_name(mut self, name: impl Into<String>) -> Self {
        self.canonical_name_ar = Some(name.into());
        self
    }

    /// Create with strength
    pub fn with_strength(mut self, strength: impl Into<String>) -> Self {
        self.strength = Some(strength.into());
        self
    }

    /// Create with active ingredient
    pub fn with_active_ingredient(mut self, ingredient: impl Into<String>) -> Self {
        self.active_ingredient = Some(ingredient.into());
        self
    }

    /// Create with manufacturer
    pub fn with_manufacturer(mut self, manufacturer: impl Into<String>) -> Self {
        self.manufacturer = Some(manufacturer.into());
        self
    }

    /// Get display name (English with Arabic fallback)
    pub fn display_name(&self) -> &str {
        &self.canonical_name
    }

    /// Get full display with strength
    pub fn full_display(&self) -> String {
        match &self.strength {
            Some(s) => format!("{} {}", self.canonical_name, s),
            None => self.canonical_name.clone(),
        }
    }

    /// Get embedding as Vec<f32>
    pub fn get_embedding(&self) -> Option<Vec<f32>> {
        self.embedding.as_ref().map(|e| e.as_slice().to_vec())
    }

    /// Set embedding from Vec<f32>
    pub fn set_embedding(&mut self, embedding: Vec<f32>) {
        self.embedding = Some(pgvector::Vector::from(embedding));
    }

    /// Convert to prompt context string for AI parsing
    pub fn to_prompt_context(&self) -> String {
        let mut parts = vec![self.canonical_name.clone()];

        if let Some(ar) = &self.canonical_name_ar {
            parts.push(format!("({})", ar));
        }

        if let Some(strength) = &self.strength {
            parts.push(format!("[{}]", strength));
        }

        if let Some(ingredient) = &self.active_ingredient {
            parts.push(format!("- {}", ingredient));
        }

        parts.join(" ")
    }
}
