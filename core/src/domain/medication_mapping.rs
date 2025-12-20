use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MedicationMapping {
    pub id: String,
    pub arabic_name: String,
    pub english_name: String,
    pub synonyms: Option<Vec<String>>,
    pub embedding: Option<Vec<f32>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MedicationMapping {
    pub fn new(arabic_name: &str, english_name: &str) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            arabic_name: arabic_name.to_string(),
            english_name: english_name.to_string(),
            synonyms: None,
            embedding: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn to_prompt_context(&self) -> String {
        format!("{}: {}", self.arabic_name, self.english_name)
    }
}
