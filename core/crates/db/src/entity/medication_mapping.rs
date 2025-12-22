//! MedicationMapping entity - Arabic to English medication name mappings

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "medication_mappings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub arabic_name: String,
    pub english_name: String,
    pub synonyms: Option<Vec<String>>,
    pub embedding: Option<PgVector>, // Vector(768) for semantic search
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Create a new medication mapping
    pub fn new(arabic_name: impl Into<String>, english_name: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            arabic_name: arabic_name.into(),
            english_name: english_name.into(),
            synonyms: None,
            embedding: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Set embedding
    pub fn set_embedding(&mut self, embedding: Vec<f32>) {
        self.embedding = Some(PgVector::from(embedding));
    }

    /// Format for AI prompt context
    pub fn to_prompt_context(&self) -> String {
        let synonyms = self
            .synonyms
            .as_ref()
            .map(|s| s.join(", "))
            .unwrap_or_default();
        if synonyms.is_empty() {
            format!("{} = {}", self.arabic_name, self.english_name)
        } else {
            format!(
                "{} = {} (synonyms: {})",
                self.arabic_name, self.english_name, synonyms
            )
        }
    }

    /// Get embedding as Vec<f32> if present
    pub fn get_embedding(&self) -> Option<Vec<f32>> {
        self.embedding.as_ref().map(|v| v.to_vec())
    }
}
