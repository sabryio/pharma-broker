use chrono::{DateTime, Utc};
use pgvector::Vector;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct MedicationMapping {
    pub id: String,
    pub arabic_name: String,
    pub english_name: String,
    pub synonyms: Option<Vec<String>>,
    pub embedding: Option<Vector>,
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

    /// Get embedding as Vec<f32>
    pub fn get_embedding(&self) -> Option<Vec<f32>> {
        self.embedding.as_ref().map(|v| v.to_vec())
    }

    /// Set embedding from Vec<f32>
    pub fn set_embedding(&mut self, embedding: Vec<f32>) {
        self.embedding = Some(Vector::from(embedding));
    }
}

// Custom Serialize implementation for API responses
impl Serialize for MedicationMapping {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("MedicationMapping", 7)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("arabic_name", &self.arabic_name)?;
        state.serialize_field("english_name", &self.english_name)?;
        state.serialize_field("synonyms", &self.synonyms)?;
        state.serialize_field("embedding", &self.get_embedding())?;
        state.serialize_field("created_at", &self.created_at)?;
        state.serialize_field("updated_at", &self.updated_at)?;
        state.end()
    }
}

// Custom Deserialize implementation
impl<'de> Deserialize<'de> for MedicationMapping {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            id: String,
            arabic_name: String,
            english_name: String,
            synonyms: Option<Vec<String>>,
            embedding: Option<Vec<f32>>,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
        }

        let helper = Helper::deserialize(deserializer)?;
        Ok(MedicationMapping {
            id: helper.id,
            arabic_name: helper.arabic_name,
            english_name: helper.english_name,
            synonyms: helper.synonyms,
            embedding: helper.embedding.map(Vector::from),
            created_at: helper.created_at,
            updated_at: helper.updated_at,
        })
    }
}
