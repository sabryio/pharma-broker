//! Offer entity - Medication supply offers
//!
//! Represents medication supply offers from WhatsApp messages.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// Re-export common types for backward compatibility
pub use super::common::{ItemStatus as Status, UrgencyLevel};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "offers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub raw_message_id: Uuid,
    pub participant_id: Uuid,
    pub group_id: Uuid,
    pub medication: String,
    pub form: Option<String>,
    pub concentration: Option<String>,
    pub status: Status,
    pub urgency_level: UrgencyLevel,
    pub expiry_info: Option<String>,
    pub ai_confidence: f64,
    pub content_embedding: Option<PgVector>, // Vector(768) for semantic search
    pub master_medication_id: Option<Uuid>,  // FK to medication_master for deterministic matching
    pub medication_curated: bool,            // Whether medication has been curated
    pub confirmed_match_count: i32,          // Number of confirmed matches for this offer
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::raw_message::Entity",
        from = "Column::RawMessageId",
        to = "super::raw_message::Column::Id",
        on_update = "Cascade",
        on_delete = "SetNull"
    )]
    RawMessage,
    #[sea_orm(
        belongs_to = "super::participant::Entity",
        from = "Column::ParticipantId",
        to = "super::participant::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Participant,
    #[sea_orm(
        belongs_to = "super::group::Entity",
        from = "Column::GroupId",
        to = "super::group::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Group,
    #[sea_orm(has_many = "super::match_::Entity")]
    Matches,
}

impl Related<super::raw_message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RawMessage.def()
    }
}

impl Related<super::participant::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Participant.def()
    }
}

impl Related<super::group::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Group.def()
    }
}

impl Related<super::match_::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Matches.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Default for Model {
    fn default() -> Self {
        use chrono::Utc;
        Self {
            id: Uuid::new_v4(),
            raw_message_id: Uuid::new_v4(),
            participant_id: Uuid::nil(),
            group_id: Uuid::nil(),
            medication: String::new(),
            form: None,
            concentration: None,
            status: Status::Active,
            urgency_level: UrgencyLevel::Normal,
            expiry_info: None,
            ai_confidence: 0.0,
            content_embedding: None,
            master_medication_id: None,
            medication_curated: false,
            confirmed_match_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl Model {
    /// Get embedding as Vec<f32> if present
    pub fn get_embedding(&self) -> Option<Vec<f32>> {
        self.content_embedding.as_ref().map(|v| v.to_vec())
    }

    /// Check if this offer is urgent (any urgency level above Normal)
    pub fn is_urgent(&self) -> bool {
        self.urgency_level.is_urgent()
    }
}

// =============================================================================
// Builder Pattern
// =============================================================================

/// Builder for creating Offer entities with a fluent API
///
/// # Example
/// ```ignore
/// let offer = OfferBuilder::new("msg-123", "Aspirin 100mg", "+201234567890", "group@g.us")
///     .form("امبول")
///     .concentration("100mg")
///     .urgency_level(UrgencyLevel::Urgent)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct OfferBuilder {
    raw_message_id: Uuid,
    medication: String,
    participant_id: Uuid,
    group_id: Uuid,
    // Optional fields
    form: Option<String>,
    concentration: Option<String>,
    expiry_info: Option<String>,
    urgency_level: UrgencyLevel,
    ai_confidence: f64,
    content_embedding: Option<PgVector>,
}

impl OfferBuilder {
    /// Create a new builder with required fields
    pub fn new(
        raw_message_id: Uuid,
        medication: impl Into<String>,
        participant_id: Uuid,
        group_id: Uuid,
    ) -> Self {
        let medication = medication.into();
        Self {
            raw_message_id,
            medication,
            participant_id,
            group_id,
            form: None,
            concentration: None,
            expiry_info: None,
            urgency_level: UrgencyLevel::Normal,
            ai_confidence: 0.0,
            content_embedding: None,
        }
    }

    /// Set the form (physical form like امبول, فايل, etc.)
    pub fn form(mut self, form: impl Into<String>) -> Self {
        self.form = Some(form.into());
        self
    }

    /// Set the concentration (dosage/strength like 1mg, 150, etc.)
    pub fn concentration(mut self, concentration: impl Into<String>) -> Self {
        self.concentration = Some(concentration.into());
        self
    }

    /// Set expiry info text
    pub fn expiry_info(mut self, info: impl Into<String>) -> Self {
        self.expiry_info = Some(info.into());
        self
    }

    /// Set the urgency level
    pub fn urgency_level(mut self, level: UrgencyLevel) -> Self {
        self.urgency_level = level;
        self
    }

    /// Set as urgent (shorthand for urgency_level(Urgent))
    pub fn urgent(mut self) -> Self {
        self.urgency_level = UrgencyLevel::Urgent;
        self
    }

    /// Set AI confidence score
    pub fn ai_confidence(mut self, confidence: f64) -> Self {
        self.ai_confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set content embedding
    pub fn embedding(mut self, embedding: Vec<f32>) -> Self {
        self.content_embedding = Some(PgVector::from(embedding));
        self
    }

    /// Build the Offer entity
    pub fn build(self) -> Model {
        use chrono::Utc;
        let now = Utc::now();

        Model {
            id: Uuid::new_v4(),
            raw_message_id: self.raw_message_id,
            participant_id: self.participant_id,
            group_id: self.group_id,
            medication: self.medication,
            form: self.form,
            concentration: self.concentration,
            status: Status::Active,
            urgency_level: self.urgency_level,
            expiry_info: self.expiry_info,
            ai_confidence: self.ai_confidence,
            content_embedding: self.content_embedding,
            master_medication_id: None,
            medication_curated: false,
            confirmed_match_count: 0,
            created_at: now,
            updated_at: now,
        }
    }
}
